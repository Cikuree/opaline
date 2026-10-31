//! Loaders (Quilt, NeoForge, Forge) sans réseau : variables et coordonnées Maven, processors client, classpath,
//! fusion `inheritsFrom`, sorties sha1, liste blanche d'hôtes, clés packwiz, extraction de l'installeur.
use opaline_core::auth::Session;
use opaline_core::config::Dirs;
use opaline_core::fabric::FabricProfile;
use opaline_core::install::{Endpoints, installer_url};
use opaline_core::launch::{LaunchInput, build_command};
use opaline_core::loader::*;
use opaline_core::mojang::{Env, VersionJson};
use opaline_core::packwiz;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

fn fixture(name: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)).unwrap()
}

fn profile(kind: &str) -> InstallProfile {
    serde_json::from_str(&fixture(&format!("{kind}_install_profile.json"))).unwrap()
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("opaline-loaders-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn ctx(kind: &str) -> Ctx {
    let p = profile(kind);
    Ctx::new(Path::new("/shared"), Path::new("/shared/libraries"), Path::new("/shared/versions/26.3/26.3.jar"), Path::new("/dl/installer.jar"), Path::new("/work/installer"), "26.3", &p)
        .unwrap()
}

fn norm(s: &str) -> String {
    s.replace('\\', "/")
}

#[test]
fn variables_and_maven_coordinates_resolve() {
    let c = ctx("forge");
    assert_eq!(norm(&c.resolve("{MINECRAFT_JAR}").unwrap()), "/shared/versions/26.3/26.3.jar");
    assert_eq!(c.resolve("{SIDE}").unwrap(), "client");
    assert_eq!(norm(&c.resolve("{ROOT}/libraries/x").unwrap()), "/shared/libraries/x");
    assert_eq!(norm(&c.resolve("{LIBRARY_DIR}").unwrap()), "/shared/libraries");
    // données côté client : coordonnées entre crochets, littéral entre apostrophes, fichier extrait
    assert_eq!(norm(&c.resolve("{PATCHED}").unwrap()), "/shared/libraries/net/minecraftforge/forge/26.3-66.1.0/forge-26.3-66.1.0-client.jar");
    assert_eq!(c.resolve("{PATCHED_SHA}").unwrap(), "9581d4977f841f7ce33bde86954d586981a30c5b");
    assert_eq!(norm(&c.resolve("{BINPATCH}").unwrap()), "/work/installer/data/client.lzma");
    assert_eq!(norm(&c.resolve("[net.minecraft:client:26.3:mappings@txt]").unwrap()), "/shared/libraries/net/minecraft/client/26.3/client-26.3-mappings.txt");
    assert_eq!(c.resolve("'littéral {X}'").unwrap(), "littéral {X}");
    assert_eq!(norm(&c.resolve("/data/run.sh").unwrap()), "/work/installer/data/run.sh");
    // erreurs : variable inconnue, accolade ouverte, chemin qui sort du dossier
    assert!(c.resolve("{NOPE}").unwrap_err().to_string().contains("NOPE"));
    assert!(c.resolve("{ROOT").is_err());
    assert!(c.resolve("/data/../../evil").is_err());
    assert!(c.resolve("[a:..:1]").is_err());
    // NeoForge : MCP_VERSION littéral, PATCHED sans classifier
    let n = ctx("neoforge");
    assert_eq!(n.resolve("{MCP_VERSION}").unwrap(), "26.3-1");
    assert!(norm(&n.resolve("{PATCHED}").unwrap()).ends_with("net/neoforged/minecraft-client-patched/26.3.0.100/minecraft-client-patched-26.3.0.100.jar"));
}

#[test]
fn only_client_processors_are_selected() {
    let f = profile("forge");
    let sel = client_processors(&f);
    assert!(!sel.is_empty() && sel.iter().all(|p| p.sides.as_ref().is_none_or(|s| s.iter().any(|x| x == "client"))));
    assert!(sel.iter().any(|p| p.jar.starts_with("net.minecraftforge:binarypatcher")));
    assert!(sel.iter().all(|p| !p.args.contains(&"EXTRACT_FILES".to_string())), "EXTRACT_FILES est côté serveur");
    // NeoForge : EXTRACT_FILES (serveur) écarté, PROCESS_MINECRAFT_JAR (sans sides) gardé
    let n = profile("neoforge");
    let sel = client_processors(&n);
    assert_eq!(sel.len(), 1);
    assert!(sel[0].args.contains(&"PROCESS_MINECRAFT_JAR".to_string()) && sel[0].sides.is_none());
    // sides absent ou vide de « client »
    let mut p = n.clone();
    p.processors[1].sides = Some(vec!["server".into()]);
    assert!(client_processors(&p).is_empty());
    p.processors[1].sides = Some(vec!["client".into(), "server".into()]);
    assert_eq!(client_processors(&p).len(), 1);
}

#[test]
fn processor_classpath_and_arguments() {
    let c = ctx("forge");
    let f = profile("forge");
    let bp = client_processors(&f).into_iter().find(|p| p.jar.contains("binarypatcher")).unwrap().clone();
    let cp = c.processor_classpath(&bp).unwrap();
    assert!(norm(&cp[0].to_string_lossy()).ends_with("net/minecraftforge/binarypatcher/1.3.4/binarypatcher-1.3.4.jar"));
    assert_eq!(cp.len(), 1 + bp.classpath.len());
    assert_eq!(join_classpath(&cp[..2], ";").matches(';').count(), 1);
    assert_eq!(join_classpath(&cp[..2], ":").matches(';').count(), 0);
    let args: Vec<String> = bp.args.iter().map(|a| norm(&c.resolve(a).unwrap())).collect();
    assert!(args.windows(2).any(|w| w[0] == "--clean" && w[1] == "/shared/versions/26.3/26.3.jar"));
    assert!(args.windows(2).any(|w| w[0] == "--apply" && w[1] == "/work/installer/data/client.lzma"));
    assert!(args.iter().all(|a| !a.contains('{')));
}

#[test]
fn manifest_main_class_is_read_from_the_jar() {
    assert_eq!(manifest_main_class("Manifest-Version: 1.0\r\nMain-Class: a.b.C\r\n").as_deref(), Some("a.b.C"));
    assert_eq!(manifest_main_class("Main-Class: a.b.Very\r\n LongName\r\n").as_deref(), Some("a.b.VeryLongName"));
    assert_eq!(manifest_main_class("Manifest-Version: 1.0\n"), None);
    let d = tmp("jar");
    let jar = d.join("p.jar");
    let mut z = zip::ZipWriter::new(fs::File::create(&jar).unwrap());
    z.start_file("META-INF/MANIFEST.MF", zip::write::SimpleFileOptions::default()).unwrap();
    z.write_all(b"Manifest-Version: 1.0\r\nMain-Class: net.example.Tool\r\n\r\n").unwrap();
    z.finish().unwrap();
    assert_eq!(jar_main_class(&jar).unwrap(), "net.example.Tool");
    assert!(jar_main_class(&d.join("absent.jar")).is_err());
    fs::remove_dir_all(&d).unwrap();
}

fn sha1_hex(b: &[u8]) -> String {
    use sha1::{Digest, Sha1};
    hex::encode(Sha1::digest(b))
}

#[test]
fn outputs_are_verified_by_sha1() {
    let d = tmp("outputs");
    let libs = d.join("libraries");
    let mut p = profile("forge");
    let patched = libs.join("net/minecraftforge/forge/26.3-66.1.0/forge-26.3-66.1.0-client.jar");
    let mk = |shared: &Path| Ctx::new(shared, &libs, Path::new("mc.jar"), Path::new("i.jar"), &d.join("data"), "26.3", &profile("forge")).unwrap();
    let bp = client_processors(&p).into_iter().find(|x| !x.outputs.is_empty()).unwrap().clone();
    // absent
    assert_eq!(mk(&d).outputs_valid(&bp).unwrap(), Some(false));
    // sha1 incorrect
    fs::create_dir_all(patched.parent().unwrap()).unwrap();
    fs::write(&patched, b"pas le bon contenu").unwrap();
    assert_eq!(mk(&d).outputs_valid(&bp).unwrap(), Some(false));
    // sha1 correct : on fait annoncer au profil le hash du fichier présent
    let good = sha1_hex(b"pas le bon contenu");
    for dd in p.data.values_mut() {
        if dd.client.starts_with('\'') && dd.client.trim_matches('\'').len() == 40 && dd.client.contains("9581d49") {
            dd.client = format!("'{good}'");
        }
    }
    let c = Ctx::new(&d, &libs, Path::new("mc.jar"), Path::new("i.jar"), &d.join("data"), "26.3", &p).unwrap();
    assert_eq!(c.outputs_valid(&bp).unwrap(), Some(true));
    // un processor sans sortie déclarée : indéterminé
    let n = profile("neoforge");
    let nc = Ctx::new(&d, &libs, Path::new("mc.jar"), Path::new("i.jar"), &d.join("data"), "26.3", &n).unwrap();
    assert_eq!(nc.outputs_valid(client_processors(&n)[0]).unwrap(), None);
    fs::remove_dir_all(&d).unwrap();
}

fn vanilla() -> VersionJson {
    serde_json::from_str(
        r#"{"id":"26.3","mainClass":"net.minecraft.client.main.Main",
        "arguments":{"jvm":["-Djava.library.path=${natives_directory}","-cp","${classpath}"],
          "game":["--username","${auth_player_name}","--version","${version_name}","--assetIndex","${assets_index_name}"]},
        "assetIndex":{"id":"34","sha1":"a","url":"u"},"downloads":{"client":{"sha1":"a","url":"u"}},"javaVersion":{"majorVersion":25},
        "libraries":[{"name":"org.ow2.asm:asm:9.0","downloads":{"artifact":{"path":"org/ow2/asm/asm/9.0/asm-9.0.jar","sha1":"a","url":"u"}}}]}"#,
    )
    .unwrap()
}

#[test]
fn inherited_version_is_merged_with_vanilla_for_launch() {
    for (kind, main, version_name) in [
        ("neoforge", "net.neoforged.fml.startup.Client", "neoforge-26.3.0.100"),
        ("forge", "net.minecraftforge.bootstrap.ForgeBootstrap", "26.3-forge-66.1.0"),
    ] {
        let lv: LoaderVersion = serde_json::from_str(&fixture(&format!("{kind}_version.json"))).unwrap();
        assert_eq!(lv.inherits_from.as_deref(), Some("26.3"));
        let prof: FabricProfile = lv.clone().into_profile("26.3").unwrap();
        assert!(lv.clone().into_profile("26.4").is_err(), "inheritsFrom doit correspondre à la version du jeu");
        let (v, dirs, session) = (vanilla(), Dirs::new("/root"), Session::offline("Bob"));
        let env = Env { os: "windows", arch: "x86_64", features: HashMap::new() };
        let cmd = build_command(&LaunchInput { java: Path::new("/j/java"), version: &v, fabric: &prof, dirs: &dirs, session: &session, memory_mb: 2048, env: &env }).unwrap();
        let main_at = cmd.iter().position(|a| a == main).unwrap();
        // jvm vanilla puis jvm du loader, avant la classe principale ; game vanilla puis game du loader, après
        let cp_at = cmd.iter().position(|a| a == "-cp").unwrap();
        assert!(cp_at < main_at && cmd.iter().any(|a| a.starts_with("-DlibraryDirectory=") || a.starts_with("-XX:")));
        assert!(cmd[..main_at].iter().all(|a| !a.contains("${")));
        let game = &cmd[main_at + 1..];
        assert_eq!(&game[..2], ["--username", "Bob"]);
        assert!(game.windows(2).any(|w| w == ["--version", version_name]), "version_name = id du loader : {game:?}");
        assert!(game.iter().any(|a| a == "--fml.mcVersion" || a == "--launchTarget"));
        // le classpath garde les libraries du loader par leur chemin de manifeste, le client.jar vanilla en dernier
        let cp = &cmd[cp_at + 1];
        assert!(cp.rsplit(';').next().unwrap().ends_with(&format!("{version_name}.jar")), "jar client sous le nom du loader : {cp}");
        for l in &prof.libraries {
            if l.url.is_empty() && l.name.contains(":client") {
                assert!(cp.contains(&l.rel_path().unwrap().replace('/', "\\")) || cp.contains(&l.rel_path().unwrap()));
            }
        }
    }
}

#[test]
fn host_allow_list() {
    for ok in [
        "https://maven.neoforged.net/releases/net/neoforged/neoforge/1/x.jar",
        "https://maven.minecraftforge.net/net/minecraftforge/forge/a/b.jar",
        "https://libraries.minecraft.net/a.jar",
        "https://maven.fabricmc.net/a.jar",
        "https://maven.quiltmc.org/repository/release/a.jar",
        "https://MAVEN.FabricMC.net/a.jar",
        "https://maven.fabricmc.net:443/a.jar", // port par défaut, normalisé par l'URL
    ] {
        assert!(check_host_in(ok, false).is_ok(), "{ok}");
    }
    for bad in [
        "https://evil.example/a.jar",
        "http://maven.neoforged.net/a.jar",
        "https://maven.neoforged.net.evil.example/a.jar",
        "https://evil.maven.neoforged.net/a.jar",
        "https://maven.neoforged.net@evil.example/a.jar",
        "https://user:pw@maven.neoforged.net/a.jar",
        "http://127.0.0.1:80@evil.example/x.jar",
        "http://127.0.0.1@evil.example/x.jar",
        "https://evil.example/maven.neoforged.net/a.jar",
        "https://maven.neoforged.net:8443/a.jar",
        "https://repo1.maven.org/maven2/a.jar",
        "ftp://maven.fabricmc.net/a.jar",
        "http://127.0.0.1.evil.example/a.jar",
        "pas une url",
        "",
    ] {
        assert!(check_host_in(bad, false).is_err(), "{bad}");
        assert!(check_host_in(bad, true).is_err(), "{bad}");
    }
    // boucle locale : debug seulement, jamais en release, et jamais avec identifiants
    for lo in ["http://127.0.0.1:8080/a.jar", "http://localhost/a.jar", "http://[::1]:9/a.jar"] {
        assert!(check_host_in(lo, true).is_ok(), "{lo}");
        assert!(check_host_in(lo, false).is_err(), "{lo} refusé en release");
    }
    assert!(check_host_in("https://127.0.0.1/a.jar", true).is_err());
}

#[test]
fn version_strings_are_plain() {
    for ok in ["26.3", "0.31.0-beta.5", "26.3-66.1.0", "1.21.1", "0.20.0+build.1"] {
        assert!(valid_version(ok), "{ok}");
    }
    for bad in ["", "..", "1..2", "a/b", "a\\b", "a b", "../x", "1;2", "é"] {
        assert!(!valid_version(bad), "{bad}");
    }
}

#[test]
fn inherits_from_and_unsupported_libraries_are_refused_before_any_download() {
    let p = profile("forge");
    let mut lv: LoaderVersion = serde_json::from_str(&fixture("forge_version.json")).unwrap();
    assert!(lv.check("26.3", &p).is_ok());
    assert!(lv.check("26.4", &p).unwrap_err().to_string().contains("26.3"));
    let mut other = p.clone();
    other.minecraft = Some("1.20.1".into());
    assert!(lv.check("26.3", &other).is_err());
    lv.inherits_from = None;
    assert!(lv.check("26.3", &p).is_err());
    lv.inherits_from = Some("26.3".into());
    let mut lib: ProfileLib = serde_json::from_str(r#"{"name":"a:b:1","rules":[{"action":"allow"}]}"#).unwrap();
    lv.libraries.push(lib.clone());
    assert!(lv.check("26.3", &p).unwrap_err().to_string().contains("a:b:1"));
    lv.libraries.pop();
    lib = serde_json::from_str(r#"{"name":"a:b:1","natives":{"windows":"x"}}"#).unwrap();
    assert!(lib.check_supported().is_err());
}

#[test]
fn install_lock_is_exclusive_and_stale_locks_are_cleaned() {
    let d = tmp("lock");
    let l = InstallLock::acquire(&d).unwrap();
    assert!(InstallLock::acquire(&d).unwrap_err().to_string().contains("déjà en cours"));
    drop(l);
    assert!(!d.join(".lock").exists());
    // verrou périmé : repris
    fs::write(d.join(".lock"), "1").unwrap();
    let old = std::time::SystemTime::now() - Duration::from_secs(7200);
    fs::File::options().write(true).open(d.join(".lock")).unwrap().set_modified(old).unwrap();
    assert!(InstallLock::acquire(&d).is_ok());
    fs::remove_dir_all(&d).unwrap();
}

#[test]
fn installer_urls_and_forge_version_forms() {
    let ep = Endpoints::default();
    assert_eq!(
        installer_url(&ep, LoaderKind::NeoForge, "26.3", "26.3.0.100").unwrap(),
        "https://maven.neoforged.net/releases/net/neoforged/neoforge/26.3.0.100/neoforge-26.3.0.100-installer.jar"
    );
    let forge = "https://maven.minecraftforge.net/net/minecraftforge/forge/26.3-66.1.0/forge-26.3-66.1.0-installer.jar";
    assert_eq!(installer_url(&ep, LoaderKind::Forge, "26.3", "66.1.0").unwrap(), forge);
    assert_eq!(installer_url(&ep, LoaderKind::Forge, "26.3", "26.3-66.1.0").unwrap(), forge);
    assert!(installer_url(&ep, LoaderKind::Forge, "26.3", "../x").is_err());
    assert!(installer_url(&ep, LoaderKind::Fabric, "26.3", "0.1").is_err());
    for u in [installer_url(&ep, LoaderKind::NeoForge, "26.3", "1").unwrap(), forge.to_string()] {
        assert!(check_host(&u).is_ok());
    }
}

#[test]
fn packwiz_loader_keys() {
    let pack = |versions: &str| format!("[index]\nfile=\"index.toml\"\nhash-format=\"sha256\"\nhash=\"AB\"\n[versions]\nminecraft=\"26.3\"\n{versions}");
    for (line, kind, v) in [
        ("fabric=\"0.20.1\"", LoaderKind::Fabric, "0.20.1"),
        ("quilt=\"0.31.0-beta.5\"", LoaderKind::Quilt, "0.31.0-beta.5"),
        ("neoforge=\"26.3.0.100\"", LoaderKind::NeoForge, "26.3.0.100"),
        ("forge=\"66.1.0\"", LoaderKind::Forge, "66.1.0"),
    ] {
        let (versions, _, _) = packwiz::parse_pack(&pack(line)).unwrap();
        assert_eq!(versions.loader().unwrap(), (kind, v.to_string()));
    }
    assert!(packwiz::parse_pack(&pack("")).unwrap().0.loader().is_err());
    assert!(packwiz::parse_pack(&pack("fabric=\"1\"\nquilt=\"2\"")).unwrap().0.loader().is_err());
    assert_eq!(LoaderKind::parse("NeoForge"), Some(LoaderKind::NeoForge));
    assert_eq!(LoaderKind::parse("liteloader"), None);
    assert_eq!(forge_version("26.3", "26.3-66.1.0"), "66.1.0");
    assert_eq!(forge_version("26.3", "66.1.0"), "66.1.0");
}

#[test]
fn installer_data_is_extracted_inside_the_target_only() {
    let d = tmp("zip");
    let jar = d.join("installer.jar");
    let mut z = zip::ZipWriter::new(fs::File::create(&jar).unwrap());
    let o = zip::write::SimpleFileOptions::default();
    for (n, c) in [("install_profile.json", "{}"), ("data/client.lzma", "binpatch"), ("data/sub/x.txt", "x"), ("maven/a/b.jar", "m")] {
        z.start_file(n, o).unwrap();
        z.write_all(c.as_bytes()).unwrap();
    }
    z.finish().unwrap();
    let dest = d.join("out");
    extract_data(&jar, &dest).unwrap();
    assert_eq!(fs::read(dest.join("data/client.lzma")).unwrap(), b"binpatch");
    assert!(dest.join("data/sub/x.txt").is_file() && !dest.join("install_profile.json").exists() && !dest.join("maven").exists());
    assert_eq!(read_entry(&jar, "/install_profile.json").unwrap().unwrap(), b"{}");
    assert!(read_entry(&jar, "version.json").unwrap().is_none());
    fs::remove_dir_all(&d).unwrap();
}

#[test]
fn processor_timeout_kills_the_process() {
    let d = tmp("timeout");
    let log = d.join("loader.log");
    #[cfg(windows)]
    let mut cmd = std::process::Command::new("ping");
    #[cfg(windows)]
    cmd.args(["-n", "30", "127.0.0.1"]);
    #[cfg(not(windows))]
    let mut cmd = std::process::Command::new("sleep");
    #[cfg(not(windows))]
    cmd.arg("30");
    let t = std::time::Instant::now();
    let e = run_logged(cmd, &log, Duration::from_millis(600)).unwrap_err().to_string();
    assert!(e.contains("dépasse") && t.elapsed() < Duration::from_secs(10), "{e}");
    fs::remove_dir_all(&d).unwrap();
}
