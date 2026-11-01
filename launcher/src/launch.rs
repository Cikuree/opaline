//! Construction de la ligne de commande Java et lancement du jeu.
use crate::auth::Session;
use crate::config::Dirs;
use crate::fabric::FabricProfile;
use crate::mojang::{Env, VersionJson, applicable_libraries, expand_args};
use crate::safe::safe_join;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

pub const LAUNCHER_NAME: &str = "Opaline";

#[derive(Clone, Copy)]
pub struct LaunchInput<'a> {
    pub java: &'a Path,
    pub version: &'a VersionJson,
    pub fabric: &'a FabricProfile,
    pub dirs: &'a Dirs,
    pub session: &'a Session,
    pub memory_mb: u32,
    pub env: &'a Env,
}

/// Clé de dédoublonnage : `groupe:artefact[:classifieur]` (sans version).
fn lib_key(name: &str) -> String {
    let p: Vec<&str> = name.split(':').collect();
    match p.as_slice() {
        [g, a, _v, rest @ ..] => format!("{g}:{a}:{}", rest.join(":")),
        _ => name.to_string(),
    }
}

/// Classpath : libraries Mojang applicables, puis Fabric (une version Fabric remplace la Mojang du même
/// artefact), puis le client.jar.
pub fn classpath(i: &LaunchInput) -> Result<Vec<PathBuf>> {
    let libs = i.dirs.libraries();
    let mut entries: Vec<(String, PathBuf)> = Vec::new();
    let mut push = |key: String, path: PathBuf| {
        entries.retain(|(k, _)| *k != key);
        entries.push((key, path));
    };
    for (l, a) in applicable_libraries(&i.version.libraries, i.env) {
        push(lib_key(&l.name), safe_join(&libs, &a.path)?);
    }
    for l in &i.fabric.libraries {
        push(lib_key(&l.name), safe_join(&libs, &l.rel_path()?)?);
    }
    let mut cp: Vec<PathBuf> = entries.into_iter().map(|(_, p)| p).collect();
    // Forge / NeoForge : le jar client porte le nom du loader (`${version_name}.jar` de leur ignoreList), copié à l'installation.
    cp.push(i.dirs.client_jar(i.fabric.version_name.as_deref().unwrap_or(&i.version.id)));
    Ok(cp)
}

/// Retourne `[java, args...]`. Fonction pure (aucun accès disque ni réseau). Arrêt au menu principal.
pub fn build_command(i: &LaunchInput) -> Result<Vec<String>> {
    build_command_with(i, None)
}

/// Comme `build_command` ; `quick_play` : adresse du serveur à rejoindre (`--quickPlayMultiplayer`, Minecraft 1.20+ : la règle
/// `is_quick_play_multiplayer` du version.json ; sans cette règle, l'argument n'est simplement pas ajouté).
pub fn build_command_with(i: &LaunchInput, quick_play: Option<&str>) -> Result<Vec<String>> {
    let mut env = i.env.clone();
    if quick_play.is_some() {
        env.features.insert("is_quick_play_multiplayer".into(), true);
    }
    let i = &LaunchInput { env: &env, ..*i };
    let sep = if i.env.os == "windows" { ";" } else { ":" };
    let libs = i.dirs.libraries();
    let cp = classpath(i)?.iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>().join(sep);
    let vars: HashMap<&str, String> = HashMap::from([
        ("auth_player_name", i.session.username.clone()),
        ("auth_uuid", i.session.uuid.clone()),
        ("auth_access_token", i.session.access_token().to_string()),
        ("auth_xuid", i.session.xuid.clone()),
        ("user_type", i.session.user_type.to_string()),
        ("clientid", i.session.client_id.clone()),
        ("version_name", i.fabric.version_name.clone().unwrap_or_else(|| i.version.id.clone())),
        ("library_directory", libs.to_string_lossy().into_owned()),
        ("classpath_separator", sep.into()),
        ("version_type", "release".into()),
        ("game_directory", i.dirs.instance().to_string_lossy().into_owned()),
        ("assets_root", i.dirs.assets().to_string_lossy().into_owned()),
        ("assets_index_name", i.version.asset_index.id.clone()),
        ("natives_directory", i.dirs.natives().to_string_lossy().into_owned()),
        ("launcher_name", LAUNCHER_NAME.into()),
        ("launcher_version", env!("CARGO_PKG_VERSION").into()),
        ("classpath", cp),
        ("quickPlayMultiplayer", quick_play.unwrap_or_default().to_string()),
    ]);
    let mut cmd = vec![i.java.to_string_lossy().into_owned(), "-Xms512M".into(), format!("-Xmx{}M", i.memory_mb)];
    cmd.extend(expand_args(&i.version.arguments.jvm, i.env, &vars));
    cmd.extend(expand_args(&i.fabric.arguments.jvm, i.env, &vars));
    cmd.push(i.fabric.main_class.clone());
    cmd.extend(expand_args(&i.version.arguments.game, i.env, &vars));
    cmd.extend(expand_args(&i.fabric.arguments.game, i.env, &vars));
    if let Some(a) = cmd.iter().find(|a| a.contains("${")) {
        // Seulement le nom de la variable : l'argument contient peut-être déjà des valeurs substituées (jeton).
        let name = a.find("${").map(|i| a[i..].split('}').next().unwrap_or("").to_string() + "}").unwrap_or_default();
        anyhow::bail!("variable non substituée dans la ligne de commande : {name}");
    }
    Ok(cmd)
}

/// Pour les logs : masque la valeur qui suit `--accessToken`, le token où qu'il apparaisse
/// (ignoré s'il ne fait qu'un caractère, comme le token factice hors ligne) et l'adresse du serveur.
pub fn redact_text(text: &str, token: &str, server: Option<&str>) -> String {
    let mut s = text.to_string();
    if token.len() > 1 {
        s = s.replace(token, "<masqué>");
    }
    if let Some(sv) = server.filter(|s| !s.is_empty()) {
        s = s.replace(sv, "<serveur>");
        // Hôte seul aussi (log du jeu), mais seulement s'il est assez distinctif (un nom court comme `home-server` est aussi un mot courant).
        if let Some((h, _)) = sv.rsplit_once(':').filter(|(h, _)| h.contains('.')) {
            s = s.replace(h, "<serveur>");
        }
    }
    s
}

pub fn redact(cmd: &[String], token: &str, server: Option<&str>) -> String {
    let mut out = Vec::with_capacity(cmd.len());
    let mut hide = false;
    for a in cmd {
        let shown = if hide { "<masqué>".to_string() } else { redact_text(a, token, server) };
        out.push(shown);
        hide = a == "--accessToken";
    }
    out.join(" ")
}

/// Lance le jeu dans l'instance isolée ; stdout/stderr vont dans `instance/logs/launcher-game.log`.
pub fn spawn(cmd: &[String], dirs: &Dirs) -> Result<Child> {
    let game_dir = dirs.instance();
    std::fs::create_dir_all(game_dir.join("logs"))?;
    std::fs::create_dir_all(dirs.natives())?;
    let log = std::fs::File::create(game_dir.join("logs").join("launcher-game.log"))?;
    let mut c = Command::new(&cmd[0]);
    c.args(&cmd[1..]).current_dir(&game_dir).stdin(Stdio::null()).stdout(log.try_clone()?).stderr(log);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW : pas de console noire
    }
    c.spawn().with_context(|| format!("lancement de {}", cmd[0]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version() -> VersionJson {
        serde_json::from_str(
            r#"{"id":"26.3","mainClass":"net.minecraft.client.main.Main",
            "arguments":{"jvm":["-Djava.library.path=${natives_directory}/java","-cp","${classpath}",
                {"rules":[{"action":"allow","os":{"name":"osx"}}],"value":"-XstartOnFirstThread"}],
              "game":["--username","${auth_player_name}","--uuid","${auth_uuid}","--accessToken","${auth_access_token}",
                "--assetIndex","${assets_index_name}",
                {"rules":[{"action":"allow","features":{"is_quick_play_multiplayer":true}}],"value":["--quickPlayMultiplayer","${quickPlayMultiplayer}"]}]},
            "assetIndex":{"id":"34","sha1":"a","url":"u"},
            "downloads":{"client":{"sha1":"a","url":"u"}},"javaVersion":{"majorVersion":25},
            "libraries":[
              {"name":"org.ow2.asm:asm:9.0","downloads":{"artifact":{"path":"org/ow2/asm/asm/9.0/asm-9.0.jar","sha1":"a","url":"u"}}},
              {"name":"l:n:1:natives-linux","rules":[{"action":"allow","os":{"name":"linux"}}],"downloads":{"artifact":{"path":"l/n-linux.jar","sha1":"a","url":"u"}}},
              {"name":"l:n:1:natives-windows","rules":[{"action":"allow","os":{"name":"windows"}}],"downloads":{"artifact":{"path":"l/n-win.jar","sha1":"a","url":"u"}}}]}"#,
        )
        .unwrap()
    }

    fn fabric() -> FabricProfile {
        serde_json::from_str(
            r#"{"mainClass":"net.fabricmc.loader.impl.launch.knot.KnotClient",
            "arguments":{"game":[],"jvm":["-DFabricMcEmu= net.minecraft.client.main.Main "]},
            "libraries":[{"name":"org.ow2.asm:asm:9.10.1","url":"https://m/","sha1":"b"},
                         {"name":"net.fabricmc:fabric-loader:0.20.1","url":"https://m/","sha1":"c"}]}"#,
        )
        .unwrap()
    }

    fn cmd(os: &'static str) -> Vec<String> {
        let (v, f) = (version(), fabric());
        let dirs = Dirs::new("/root");
        let session = Session::offline("Bob");
        let env = Env { os, arch: "x86_64", features: HashMap::new() };
        build_command(&LaunchInput { java: Path::new("/j/java"), version: &v, fabric: &f, dirs: &dirs, session: &session, memory_mb: 2048, env: &env })
            .unwrap()
    }

    #[test]
    fn command_layout_windows() {
        let c = cmd("windows");
        assert_eq!(&c[..3], ["/j/java", "-Xms512M", "-Xmx2048M"]);
        let main = c.iter().position(|a| a.ends_with("KnotClient")).unwrap();
        // jvm avant mainClass, game après
        assert!(c[..main].iter().any(|a| a == "-cp") && c[..main].contains(&"-DFabricMcEmu= net.minecraft.client.main.Main ".to_string()));
        let game = &c[main + 1..];
        assert_eq!(&game[..2], ["--username", "Bob"]);
        assert!(!c.iter().any(|a| a.contains("quickPlay")), "JOUER ne se connecte jamais seul");
        assert!(game.windows(2).any(|w| w == ["--assetIndex", "34"]));
        assert!(!c.iter().any(|a| a == "-XstartOnFirstThread"));
        // classpath : séparateur ';', asm Mojang remplacé par celui de Fabric, natives windows, client.jar en dernier
        let cp = &c[c.iter().position(|a| a == "-cp").unwrap() + 1];
        let parts: Vec<&str> = cp.split(';').collect();
        assert!(cp.contains("asm-9.10.1.jar") && !cp.contains("asm-9.0.jar"));
        assert!(cp.contains("n-win.jar") && !cp.contains("n-linux.jar"));
        assert!(parts.last().unwrap().ends_with("26.3.jar"));
        assert!(parts.iter().all(|p| !p.contains("${")));
        assert!(c.iter().all(|a| !a.contains("${")), "variable non substituée");
    }

    #[test]
    fn quick_play_adds_server_argument() {
        let (v, f) = (version(), fabric());
        let dirs = Dirs::new("/root");
        let session = Session::offline("Bob");
        let env = Env { os: "linux", arch: "x86_64", features: HashMap::new() };
        let input = LaunchInput { java: Path::new("/j/java"), version: &v, fabric: &f, dirs: &dirs, session: &session, memory_mb: 2048, env: &env };
        let c = build_command_with(&input, Some("home-server.x.ts.net")).unwrap();
        assert!(c.windows(2).any(|w| w == ["--quickPlayMultiplayer", "home-server.x.ts.net"]));
        assert!(env.features.is_empty(), "l'environnement de l'appelant n'est pas modifié");
        let c = build_command_with(&input, None).unwrap();
        assert!(!c.iter().any(|a| a.contains("quickPlay")));
        // Minecraft < 1.20 : pas de règle quick play dans le version.json, rien n'est ajouté
        let mut old = version();
        old.arguments.game.pop();
        let input = LaunchInput { version: &old, ..input };
        assert!(!build_command_with(&input, Some("a.b")).unwrap().iter().any(|a| a.contains("quickPlay")));
    }

    #[test]
    fn linux_separator() {
        let c = cmd("linux");
        let cp = &c[c.iter().position(|a| a == "-cp").unwrap() + 1];
        assert!(!cp.contains(';') && cp.contains("n-linux.jar"));
    }

    #[test]
    fn redacts_token() {
        let c = cmd("windows");
        assert!(c.iter().any(|a| a == "0")); // le token factice est bien dans la commande
        let r = redact(&c, "0", None);
        assert!(r.contains("--accessToken <masqué> --assetIndex"));
        // token long présent ailleurs (ex. propriété -D) : masqué par valeur
        let mut c2 = c.clone();
        c2.push("-Dx=secrettoken123".into());
        assert!(!redact(&c2, "secrettoken123", None).contains("secrettoken123"));
    }

    #[test]
    fn redacts_server_address() {
        let c = cmd("windows");
        assert!(!redact_text("Connecting to secret.host, 25565", "0", Some("secret.host:25565")).contains("secret.host"));
        assert!(!redact(&c, "0", Some("secret.host:25565")).contains("secret.host"));
    }

    #[test]
    fn leftover_variable_is_an_error() {
        let mut v = version();
        v.arguments.game.push(crate::mojang::Arg::Plain("${nope}".into()));
        let f = fabric();
        let dirs = Dirs::new("/root");
        let session = Session::offline("Bob");
        let env = Env { os: "linux", arch: "x86_64", features: HashMap::new() };
        let r = build_command(&LaunchInput {
            java: Path::new("/j/java"),
            version: &v,
            fabric: &f,
            dirs: &dirs,
            session: &session,
            memory_mb: 1,
            env: &env,
        });
        assert!(r.is_err());
    }
}
