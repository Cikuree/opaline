//! Second mode de lancement : Opaline prépare une version et un profil dans le dossier `.minecraft` du Launcher Minecraft
//! officiel, qui connecte le compte et lance le jeu. Aucune action destructive : `launcher_profiles.json` n'est modifié que
//! par fusion (sauvegarde unique, écriture atomique) et une version existante d'un autre outil n'est jamais écrasée.
use crate::config::{Dirs, dev_env};
use crate::fabric::FabricLib;
use crate::safe::{Hash, safe_join};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

/// Famille du paquet Microsoft Store du Launcher Minecraft.
pub const STORE_FAMILY: &str = "Microsoft.4297127D64EC6_8wekyb3d8bbwe";
const MANAGED: &str = "opaline.managed";
/// Fichiers de `shared/libraries` à copier pour Forge / NeoForge (écrit à l'installation du loader).
pub const LOADER_FILES: &str = "opaline-loader-files.json";
/// Arguments JVM par défaut du Launcher officiel (champ « Arguments JVM » d'un nouveau profil) ; seul `-Xmx` est remplacé.
pub const DEFAULT_JAVA_ARGS: &str =
    "-Xmx2G -XX:+UnlockExperimentalVMOptions -XX:+UseG1GC -XX:G1NewSizePercent=20 -XX:G1ReservePercent=20 -XX:MaxGCPauseMillis=50 -XX:G1HeapRegionSize=32M";
const NEED_LAUNCHER: &str = "Installe et ouvre une fois le Launcher Minecraft";

pub fn profile_key(pack_id: &str) -> String {
    format!("opaline-{pack_id}")
}

/// Identifiant de version propre à Opaline dans `.minecraft/versions` : jamais de collision avec une installation manuelle.
pub fn version_id(pack_id: &str) -> String {
    format!("opaline-{pack_id}")
}

/// `DEFAULT_JAVA_ARGS` avec `-Xmx<mémoire>M`.
pub fn java_args(memory_mb: u32) -> String {
    DEFAULT_JAVA_ARGS.split(' ').map(|a| if a.starts_with("-Xmx") { format!("-Xmx{memory_mb}M") } else { a.to_string() }).collect::<Vec<_>>().join(" ")
}

/// Sortie de `tasklist` : vrai si l'un des noms d'image y figure (insensible à la casse).
pub fn tasklist_has(output: &str, names: &[&str]) -> bool {
    let low = output.to_ascii_lowercase();
    names.iter().any(|n| low.lines().any(|l| l.split_whitespace().next() == Some(&n.to_ascii_lowercase())))
}

/// Le Launcher Minecraft tourne déjà : il réécrirait `launcher_profiles.json` en quittant. Windows : `MinecraftLauncher.exe` et
/// `Minecraft.exe` (version Store) ; Linux : `minecraft-launcher`. En cas de doute (commande indisponible) : faux.
pub fn launcher_running() -> bool {
    use std::process::Command;
    if cfg!(windows) {
        let mut c = Command::new("tasklist");
        c.args(["/NH", "/FO", "CSV"]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            c.creation_flags(0x0800_0000);
        }
        // sortie CSV : "image","pid",... -> on retire les guillemets pour ne garder que le nom en tête de ligne
        return c
            .output()
            .is_ok_and(|o| tasklist_has(&String::from_utf8_lossy(&o.stdout).replace(['"', ','], " "), &["MinecraftLauncher.exe", "Minecraft.exe"]));
    }
    cfg!(target_os = "linux") && Command::new("pgrep").args(["-x", "minecraft-launcher"]).output().is_ok_and(|o| o.status.success())
}

/// Que faire quand on veut préparer le profil officiel ?
#[derive(Debug, PartialEq, Eq)]
pub enum LauncherDecision {
    /// Le Launcher ne tourne pas : on prépare le profil.
    Free,
    /// Le Launcher tourne seul : proposer de le fermer.
    OfferClose,
    /// Une partie lancée par le Launcher tourne : ne rien fermer.
    GameRunning,
}

pub fn decide(launcher_running: bool, game_running: bool) -> LauncherDecision {
    match (launcher_running, game_running) {
        (false, _) => LauncherDecision::Free,
        (true, false) => LauncherDecision::OfferClose,
        (true, true) => LauncherDecision::GameRunning,
    }
}

/// Une ligne de commande est celle d'un Minecraft lancé depuis `.minecraft` ou depuis une instance d'Opaline (`markers`).
pub fn is_launcher_game(cmdline: &str, markers: &[String]) -> bool {
    let norm = |s: &str| s.to_ascii_lowercase().replace('\\', "/");
    let c = norm(cmdline);
    let minecraft = ["net.minecraft", "--gamedir", "bootstraplauncher", "knotclient", "fabricloader", "--assetsdir"].iter().any(|k| c.contains(k));
    // Le marqueur doit être un nom de dossier complet (`.minecraft` ne doit pas correspondre à `net.minecraft.client`).
    let in_dir = |m: &str| c.match_indices(&norm(m)).any(|(i, x)| c[i + x.len()..].chars().next().is_none_or(|n| matches!(n, '/' | ' ' | '"' | '\'')));
    minecraft && markers.iter().any(|m| in_dir(m))
}

/// Lignes de commande des processus `java` / `javaw` en cours (liste vide si indisponible).
#[allow(unreachable_code)]
fn java_cmdlines() -> Vec<String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let out = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", "Get-CimInstance Win32_Process -Filter \"Name='javaw.exe' OR Name='java.exe'\" | ForEach-Object { $_.CommandLine }"])
            .creation_flags(0x0800_0000)
            .output();
        return out.map(|o| String::from_utf8_lossy(&o.stdout).lines().map(str::to_string).collect()).unwrap_or_default();
    }
    #[cfg(target_os = "linux")]
    {
        return fs::read_dir("/proc")
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| fs::read(e.path().join("cmdline")).ok())
            .map(|b| String::from_utf8_lossy(&b).replace('\0', " "))
            .filter(|c| c.split_whitespace().next().is_some_and(|x| x.ends_with("java")))
            .collect();
    }
    Vec::new()
}

/// Une partie lancée par le Launcher officiel tourne : processus java de `.minecraft` / d'une instance d'Opaline, ou monde ouvert
/// (`session.lock`) dans le `.minecraft` ou une instance d'Opaline.
pub fn game_running(root: &crate::config::Dirs, mc: &Path) -> bool {
    let instances = root.root.join("instances");
    let markers = vec![".minecraft".to_string(), instances.to_string_lossy().into_owned(), mc.to_string_lossy().into_owned()];
    if java_cmdlines().iter().any(|c| is_launcher_game(c, &markers)) {
        return true;
    }
    crate::backup::world_open_in(&mc.join("saves")) || fs::read_dir(&instances).into_iter().flatten().flatten().any(|i| crate::backup::world_open_in(&i.path().join("saves")))
}

/// Ferme le Launcher Minecraft seulement (jamais `java` / `javaw`) : fermeture douce, attente de 5 s au plus, puis forcée.
pub fn close_launcher() -> Result<()> {
    use std::process::Command;
    use std::time::{Duration, Instant};
    let kill = |force: bool| {
        if cfg!(windows) {
            for img in ["MinecraftLauncher.exe", "Minecraft.exe"] {
                let mut c = Command::new("taskkill");
                c.args(["/IM", img]);
                if force {
                    c.arg("/F");
                }
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    c.creation_flags(0x0800_0000);
                }
                let _ = c.output();
            }
        } else {
            let _ = Command::new("pkill").args([if force { "-KILL" } else { "-TERM" }, "-x", "minecraft-launcher"]).output();
        }
    };
    kill(false);
    let t = Instant::now();
    while launcher_running() && t.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(250));
    }
    if launcher_running() {
        kill(true);
        std::thread::sleep(Duration::from_millis(500));
    }
    if launcher_running() {
        bail!("le Launcher Minecraft ne se ferme pas : ferme-le toi-même");
    }
    Ok(())
}

/// Taille d'un fichier ; `None` s'il n'existe pas. Toute autre erreur est propagée.
fn file_len(p: &Path) -> Result<Option<u64>> {
    match fs::metadata(p) {
        Ok(m) => Ok(Some(m.len())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("lecture de {}", p.display())),
    }
}

/// `.minecraft` selon l'OS : `data_dir` = `%APPDATA%` (Windows) ou `~/Library/Application Support` (macOS).
pub fn minecraft_dir_for(os: &str, data_dir: &Path, home: &Path) -> PathBuf {
    match os {
        "windows" => data_dir.join(".minecraft"),
        "macos" => data_dir.join("minecraft"),
        _ => home.join(".minecraft"),
    }
}

/// Dossier `.minecraft` de l'utilisateur (surcharge de dev : `OPALINE_MINECRAFT_DIR`).
pub fn minecraft_dir() -> Option<PathBuf> {
    if let Some(p) = dev_env("OPALINE_MINECRAFT_DIR") {
        return Some(PathBuf::from(p));
    }
    let b = directories::BaseDirs::new()?;
    Some(minecraft_dir_for(std::env::consts::OS, b.data_dir(), b.home_dir()))
}

/// Le dossier doit déjà contenir `launcher_profiles.json` (créé par le Launcher au premier démarrage).
pub fn profiles_path(mc: &Path) -> Result<PathBuf> {
    let p = mc.join("launcher_profiles.json");
    if !p.is_file() {
        bail!("{NEED_LAUNCHER} (launcher_profiles.json introuvable)");
    }
    Ok(p)
}

/// Date ISO 8601 UTC (`2026-10-31T12:00:00.000Z`) à partir de secondes Unix.
pub fn iso_utc(secs: u64) -> String {
    let (days, rem) = ((secs / 86400) as i64, secs % 86400);
    // jours depuis 1970 -> date civile (algorithme de H. Hinnant)
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.000Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

pub struct ProfileSpec<'a> {
    pub name: &'a str,
    pub version_id: &'a str,
    pub game_dir: &'a Path,
    pub java_args: &'a str,
    /// Data URL PNG ou nom d'icône du Launcher.
    pub icon: &'a str,
    /// Horodatage ISO (`created` si le profil est nouveau, `lastUsed` toujours).
    pub now: &'a str,
}

/// Ajoute ou met à jour le profil `key` ; tout le reste du document (autres profils, réglages, clés inconnues) est conservé.
/// Mise à jour idempotente : `created` et `icon` d'un profil existant sont gardés.
pub fn merge_profile(doc: &mut Value, key: &str, p: &ProfileSpec) -> Result<()> {
    let root = doc.as_object_mut().context("launcher_profiles.json n'est pas un objet JSON")?;
    let profiles =
        root.entry("profiles").or_insert_with(|| json!({})).as_object_mut().context("« profiles » n'est pas un objet dans launcher_profiles.json")?;
    let o = profiles
        .entry(key)
        .or_insert_with(|| json!({ "created": p.now, "icon": p.icon }))
        .as_object_mut()
        .context("le profil Opaline existant n'est pas un objet")?;
    for (k, v) in [
        ("name", p.name),
        ("type", "custom"),
        ("lastVersionId", p.version_id),
        ("gameDir", &p.game_dir.to_string_lossy()),
        ("javaArgs", p.java_args),
        ("lastUsed", p.now),
    ] {
        o.insert(k.into(), json!(v));
    }
    for (k, v) in [("created", p.now), ("icon", p.icon)] {
        o.entry(k).or_insert_with(|| json!(v));
    }
    Ok(())
}

pub fn read_profiles(path: &Path) -> Result<Value> {
    let bytes = fs::read(path).with_context(|| format!("lecture de {}", path.display()))?;
    serde_json::from_slice(&bytes).context("launcher_profiles.json illisible : Opaline n'y touche pas")
}

/// Sauvegarde `launcher_profiles.json.opaline.bak` (une seule fois), puis écriture atomique (`.tmp` + renommage).
pub fn write_profiles(path: &Path, doc: &Value) -> Result<()> {
    let bak = path.with_file_name("launcher_profiles.json.opaline.bak");
    if !bak.exists() {
        fs::copy(path, &bak).context("sauvegarde de launcher_profiles.json")?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(doc)?)?;
    fs::rename(&tmp, path).context("écriture de launcher_profiles.json")
}

fn atomic_copy(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst.parent().context("chemin sans dossier parent")?)?;
    let tmp = dst.with_extension("opaline-tmp");
    fs::copy(src, &tmp)?;
    fs::rename(&tmp, dst)?;
    Ok(())
}

fn sha1_ok(p: &Path, sha1: &str) -> bool {
    Hash::new("sha1", sha1).and_then(|h| h.matches_file(p)).unwrap_or(false)
}

/// Copie `src` vers `dst` si `dst` manque, ou (sha1 connu) ne correspond pas alors que `src` correspond.
/// Sans sha1, un fichier déjà présent est laissé tel quel. Renvoie vrai si une copie a eu lieu.
pub fn sync_library(src: &Path, dst: &Path, sha1: Option<&str>) -> Result<bool> {
    if !src.is_file() {
        return Ok(false);
    }
    let need = match sha1.filter(|s| !s.is_empty()) {
        Some(h) => !sha1_ok(dst, h) && sha1_ok(src, h),
        None => !dst.exists(),
    };
    if need {
        atomic_copy(src, dst)?;
    }
    Ok(need)
}

/// Copie les libraries du loader déjà présentes dans `shared/libraries` vers `.minecraft/libraries` (celles issues des
/// processors Forge / NeoForge n'ont pas d'URL : le Launcher ne peut pas les télécharger).
pub fn sync_libraries(src_libs: &Path, dst_libs: &Path, libs: &[FabricLib]) -> Result<usize> {
    let mut n = 0;
    for l in libs {
        let rel = l.rel_path()?;
        n += usize::from(sync_library(&safe_join(src_libs, &rel)?, &safe_join(dst_libs, &rel)?, l.sha1.as_deref())?);
    }
    Ok(n)
}

/// Copie les fichiers listés par `opaline-loader-files.json` (sorties des processors Forge / NeoForge, libraries sans URL).
fn sync_loader_files(dirs: &Dirs, src_id: &str, mc: &Path) -> Result<()> {
    let list = dirs.versions().join(src_id).join(LOADER_FILES);
    let bytes = fs::read(&list).with_context(|| format!("liste des fichiers du loader absente pour {src_id} : lance « Vérifier et réparer » sur le pack"))?;
    let entries: Vec<Value> = serde_json::from_slice(&bytes).context("liste des fichiers du loader illisible : répare le pack")?;
    for e in &entries {
        let rel = e.get("path").and_then(Value::as_str).context("entrée sans chemin dans la liste des fichiers du loader")?;
        sync_library(&safe_join(&dirs.libraries(), rel)?, &safe_join(&mc.join("libraries"), rel)?, e.get("sha1").and_then(Value::as_str))?;
    }
    Ok(())
}

/// Installe la version du loader `src_id` (`shared/versions`) sous l'identifiant `vid` (`opaline-<pack>`) dans `.minecraft`, avec
/// le champ `id` réécrit, le jar du loader renommé s'il existe et les libraries copiées. `installer_loader` : Forge / NeoForge
/// (la liste des fichiers produits par les processors est alors exigée). Le dossier de `vid` appartient à Opaline (marqueur
/// `opaline.managed`) : un dossier de ce nom sans marqueur n'est jamais touché.
pub fn install_version(dirs: &Dirs, mc: &Path, src_id: &str, vid: &str, libs: &[FabricLib], installer_loader: bool) -> Result<()> {
    crate::safe::safe_file_name(src_id)?;
    crate::safe::safe_file_name(vid)?;
    let raw = fs::read(dirs.version_json(src_id)).with_context(|| format!("version {src_id} absente : lance d'abord l'installation du pack"))?;
    let mut v: Value = serde_json::from_slice(&raw).context("JSON de version du loader illisible")?;
    v.as_object_mut().context("JSON de version invalide")?.insert("id".into(), json!(vid));
    let out = serde_json::to_vec_pretty(&v)?;
    let vdir = mc.join("versions").join(vid);
    let managed = vdir.join(MANAGED);
    if vdir.exists() && !managed.exists() {
        bail!("le dossier versions/{vid} existe dans .minecraft sans venir d'Opaline : Opaline n'y touche pas");
    }
    fs::create_dir_all(&vdir)?;
    let json_dst = vdir.join(format!("{vid}.json"));
    if fs::read(&json_dst).ok().as_deref() != Some(out.as_slice()) {
        let tmp = json_dst.with_extension("opaline-tmp");
        fs::write(&tmp, &out)?;
        fs::rename(&tmp, &json_dst)?;
    }
    if !managed.exists() {
        fs::write(&managed, "Version écrite par Opaline.\n")?;
    }
    // Forge / NeoForge : jar client sous le nom du loader (`${version_name}.jar`), renommé comme la version.
    let (jar_src, jar_dst) = (dirs.client_jar(src_id), vdir.join(format!("{vid}.jar")));
    if let Some(len) = file_len(&jar_src)?
        && file_len(&jar_dst)? != Some(len)
    {
        atomic_copy(&jar_src, &jar_dst)?;
    }
    sync_libraries(&dirs.libraries(), &mc.join("libraries"), libs)?;
    if installer_loader {
        sync_loader_files(dirs, src_id, mc)?;
    }
    Ok(())
}

/// Ouvre le Launcher Minecraft. Faux si introuvable (le profil est écrit quand même).
pub fn open_launcher() -> bool {
    use std::process::{Command, Stdio};
    let spawn = |mut c: Command| c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().is_ok();
    if cfg!(windows) {
        let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
        if local.is_some_and(|l| l.join("Packages").join(STORE_FAMILY).is_dir()) {
            let mut c = Command::new("explorer.exe");
            c.arg(format!("shell:AppsFolder\\{STORE_FAMILY}!Minecraft"));
            if spawn(c) {
                return true;
            }
        }
        // shortcut: seuls les deux emplacements usuels de l'installeur classique, pas de lecture du registre.
        return ["ProgramFiles(x86)", "ProgramFiles"]
            .iter()
            .filter_map(std::env::var_os)
            .map(|p| PathBuf::from(p).join("Minecraft Launcher").join("MinecraftLauncher.exe"))
            .find(|e| e.is_file())
            .is_some_and(|e| spawn(Command::new(e)));
    }
    // shortcut: macOS sans ouverture automatique (le profil est écrit, l'utilisateur ouvre le Launcher).
    cfg!(target_os = "linux") && spawn(Command::new("minecraft-launcher"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("opaline-off-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn spec<'a>(now: &'a str, dir: &'a Path) -> ProfileSpec<'a> {
        ProfileSpec { name: "Opaline · Classroom Run", version_id: "fabric-loader-0.20.1-26.3", game_dir: dir, java_args: "-Xmx4096M", icon: "Furnace", now }
    }

    #[test]
    fn launcher_decision_and_game_detection() {
        assert_eq!(decide(false, false), LauncherDecision::Free);
        assert_eq!(decide(false, true), LauncherDecision::Free);
        assert_eq!(decide(true, false), LauncherDecision::OfferClose);
        assert_eq!(decide(true, true), LauncherDecision::GameRunning);
        let m = vec![".minecraft".to_string(), r"C:\Users\u\AppData\Roaming\Opaline\instances".to_string()];
        assert!(is_launcher_game(r"javaw.exe -Xmx4G net.minecraft.client.main.Main --gameDir C:\Users\u\AppData\Roaming\.minecraft", &m));
        assert!(is_launcher_game(r"javaw.exe -cp x cpw.mods.bootstraplauncher.BootstrapLauncher --gameDir C:\Users\u\AppData\Roaming\Opaline\instances\classroom-run", &m));
        assert!(!is_launcher_game("java -jar server.jar nogui", &m));
        assert!(!is_launcher_game(r"javaw.exe net.minecraft.client.main.Main --gameDir D:\autre\dossier", &m));
    }

    #[test]
    fn minecraft_dir_per_os() {
        let (d, h) = (Path::new("/r"), Path::new("/h"));
        assert_eq!(minecraft_dir_for("windows", d, h), d.join(".minecraft"));
        assert_eq!(minecraft_dir_for("macos", d, h), d.join("minecraft"));
        assert_eq!(minecraft_dir_for("linux", d, h), h.join(".minecraft"));
    }

    #[test]
    fn missing_profiles_file_is_a_readable_error() {
        let d = tmp("missing");
        assert!(profiles_path(&d).unwrap_err().to_string().contains("Installe et ouvre une fois le Launcher Minecraft"));
        fs::write(d.join("launcher_profiles.json"), "{}").unwrap();
        assert!(profiles_path(&d).is_ok());
    }

    #[test]
    fn iso_dates() {
        assert_eq!(iso_utc(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(iso_utc(1_782_864_000 + 3661), "2026-07-01T01:01:01.000Z");
        assert_eq!(iso_utc(951_782_400), "2000-02-29T00:00:00.000Z");
    }

    #[test]
    fn merge_keeps_everything_else_and_is_idempotent() {
        let mut doc: Value = serde_json::from_str(r#"{"profiles":{"abc":{"name":"x","custom":1}},"settings":{"a":true},"version":6,"inconnu":[1]}"#).unwrap();
        let before = doc.clone();
        let dir = Path::new("/inst");
        merge_profile(&mut doc, "opaline-classroom-run", &spec("2026-01-01T00:00:00.000Z", dir)).unwrap();
        let p = &doc["profiles"]["opaline-classroom-run"];
        assert_eq!(
            (p["type"].as_str(), p["lastVersionId"].as_str(), p["javaArgs"].as_str()),
            (Some("custom"), Some("fabric-loader-0.20.1-26.3"), Some("-Xmx4096M"))
        );
        assert_eq!(p["created"], "2026-01-01T00:00:00.000Z");
        for k in ["settings", "version", "inconnu"] {
            assert_eq!(doc[k], before[k]);
        }
        assert_eq!(doc["profiles"]["abc"], before["profiles"]["abc"]);
        // deuxième passe plus tardive : created conservé, lastUsed mis à jour, un seul profil Opaline
        merge_profile(&mut doc, "opaline-classroom-run", &spec("2026-02-02T00:00:00.000Z", dir)).unwrap();
        let p = &doc["profiles"]["opaline-classroom-run"];
        assert_eq!((p["created"].as_str(), p["lastUsed"].as_str()), (Some("2026-01-01T00:00:00.000Z"), Some("2026-02-02T00:00:00.000Z")));
        assert_eq!(doc["profiles"].as_object().unwrap().len(), 2);
    }

    #[test]
    fn merge_rejects_non_objects() {
        let mut doc = json!([1]);
        assert!(merge_profile(&mut doc, "k", &spec("n", Path::new("/i"))).is_err());
        let mut doc = json!({"profiles": 3});
        assert!(merge_profile(&mut doc, "k", &spec("n", Path::new("/i"))).is_err());
    }

    #[test]
    fn unreadable_json_is_an_error_and_file_untouched() {
        let d = tmp("bad");
        let f = d.join("launcher_profiles.json");
        fs::write(&f, "{ pas du json").unwrap();
        assert!(read_profiles(&f).is_err());
        assert_eq!(fs::read_to_string(&f).unwrap(), "{ pas du json");
    }

    #[test]
    fn write_backs_up_once_and_replaces_atomically() {
        let d = tmp("write");
        let f = d.join("launcher_profiles.json");
        fs::write(&f, r#"{"a":1}"#).unwrap();
        write_profiles(&f, &json!({"a": 2})).unwrap();
        write_profiles(&f, &json!({"a": 3})).unwrap();
        assert_eq!(fs::read_to_string(d.join("launcher_profiles.json.opaline.bak")).unwrap(), r#"{"a":1}"#);
        assert_eq!(read_profiles(&f).unwrap()["a"], 3);
        assert!(!d.join("launcher_profiles.json.tmp").exists());
    }

    #[test]
    fn library_copy_uses_sha1() {
        let d = tmp("libs");
        let (src, dst) = (d.join("s.jar"), d.join("o/d.jar"));
        fs::write(&src, b"abc").unwrap();
        let sha = "a9993e364706816aba3e25717850c26c9cd0d89d"; // sha1("abc")
        assert!(sync_library(&src, &dst, Some(sha)).unwrap(), "absent : copié");
        assert!(!sync_library(&src, &dst, Some(sha)).unwrap(), "déjà bon : rien");
        fs::write(&dst, b"zzz").unwrap();
        assert!(sync_library(&src, &dst, Some(sha)).unwrap(), "sha1 différent : remplacé");
        assert_eq!(fs::read(&dst).unwrap(), b"abc");
        fs::write(&dst, b"zzz").unwrap();
        assert!(!sync_library(&src, &dst, None).unwrap(), "sans sha1, existant conservé");
        assert!(!sync_library(&src, &dst, Some("0000000000000000000000000000000000000000")).unwrap(), "source elle-même invalide : jamais propagée");
        assert!(!sync_library(&d.join("absent"), &d.join("x"), None).unwrap());
    }

    #[test]
    fn version_is_written_under_the_opaline_id() {
        let d = tmp("ver");
        let (dirs, mc) = (Dirs::new(d.join("opaline")).for_pack("p").unwrap(), d.join("mc"));
        let src = "fabric-loader-0.20.1-26.3";
        let vid = version_id("p");
        fs::create_dir_all(dirs.versions().join(src)).unwrap();
        fs::write(dirs.version_json(src), r#"{"id":"fabric-loader-0.20.1-26.3","inheritsFrom":"26.3","x":1}"#).unwrap();
        // une installation manuelle du même loader n'est jamais touchée
        let manual = mc.join("versions").join(src);
        fs::create_dir_all(&manual).unwrap();
        fs::write(manual.join(format!("{src}.json")), "manuel").unwrap();
        install_version(&dirs, &mc, src, &vid, &[], false).unwrap();
        let got: Value = serde_json::from_slice(&fs::read(mc.join("versions").join(&vid).join(format!("{vid}.json"))).unwrap()).unwrap();
        assert_eq!((got["id"].as_str(), got["inheritsFrom"].as_str(), got["x"].as_i64()), (Some("opaline-p"), Some("26.3"), Some(1)));
        assert_eq!(fs::read_to_string(manual.join(format!("{src}.json"))).unwrap(), "manuel");
        // mise à jour du contenu : acceptée (dossier à Opaline) ; idempotent
        fs::write(dirs.version_json(src), r#"{"id":"a","inheritsFrom":"26.3","x":2}"#).unwrap();
        install_version(&dirs, &mc, src, &vid, &[], false).unwrap();
        install_version(&dirs, &mc, src, &vid, &[], false).unwrap();
        assert!(fs::read_to_string(mc.join("versions").join(&vid).join(format!("{vid}.json"))).unwrap().contains("\"x\": 2"));
    }

    #[test]
    fn foreign_opaline_named_folder_is_refused() {
        let d = tmp("foreign");
        let (dirs, mc) = (Dirs::new(d.join("opaline")).for_pack("p").unwrap(), d.join("mc"));
        fs::create_dir_all(dirs.versions().join("l")).unwrap();
        fs::write(dirs.version_json("l"), r#"{"id":"l"}"#).unwrap();
        fs::create_dir_all(mc.join("versions/opaline-p")).unwrap();
        assert!(install_version(&dirs, &mc, "l", "opaline-p", &[], false).is_err());
    }

    #[test]
    fn installer_loaders_copy_listed_files_and_jar() {
        let d = tmp("forge");
        let (dirs, mc) = (Dirs::new(d.join("opaline")).for_pack("p").unwrap(), d.join("mc"));
        let src = "forge-1.20.1-47.4.28";
        fs::create_dir_all(dirs.versions().join(src)).unwrap();
        fs::write(dirs.version_json(src), format!(r#"{{"id":"{src}"}}"#)).unwrap();
        fs::write(dirs.client_jar(src), b"jar").unwrap();
        fs::create_dir_all(dirs.libraries().join("net/x")).unwrap();
        fs::write(dirs.libraries().join("net/x/out.jar"), b"abc").unwrap();
        let list = dirs.versions().join(src).join(LOADER_FILES);
        assert!(install_version(&dirs, &mc, src, "opaline-p", &[], true).is_err(), "liste absente : réparation demandée");
        fs::write(&list, r#"[{"path":"net/x/out.jar","sha1":"a9993e364706816aba3e25717850c26c9cd0d89d"},{"path":"net/x/absent.jar","sha1":null}]"#).unwrap();
        install_version(&dirs, &mc, src, "opaline-p", &[], true).unwrap();
        assert_eq!(fs::read(mc.join("libraries/net/x/out.jar")).unwrap(), b"abc");
        assert_eq!(fs::read(mc.join("versions/opaline-p/opaline-p.jar")).unwrap(), b"jar");
        fs::write(&list, r#"[{"path":"../evil.jar","sha1":null}]"#).unwrap();
        assert!(install_version(&dirs, &mc, src, "opaline-p", &[], true).is_err(), "chemin hors libraries refusé");
    }

    #[test]
    fn java_args_replace_only_xmx() {
        let a = java_args(6144);
        assert!(a.starts_with("-Xmx6144M ") && a.contains("-XX:+UseG1GC") && !a.contains("-Xmx2G"));
        assert_eq!(a.matches("-Xmx").count(), 1);
    }

    #[test]
    fn detects_launcher_processes() {
        let out = "MinecraftLauncher.exe   1234 Console\nsvchost.exe 4 Services\n";
        assert!(tasklist_has(out, &["MinecraftLauncher.exe", "Minecraft.exe"]));
        assert!(tasklist_has("minecraft.exe  9", &["Minecraft.exe"]));
        assert!(!tasklist_has("MinecraftBedrock.exe 1\nnotminecraft.exe 2", &["Minecraft.exe"]));
    }
}
