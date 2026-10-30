//! Tests d'intégration du launcher sur serveur de fixtures local (aucun réseau réel).
//! Le crate est un binaire : les modules sans UI sont recompilés ici via `#[path]`.
#![allow(dead_code)]

#[path = "../src/config.rs"]
mod config;
#[path = "../src/download.rs"]
mod download;
#[path = "../src/fabric.rs"]
mod fabric;
#[path = "../src/install.rs"]
mod install;
#[path = "../src/java.rs"]
mod java;
#[path = "../src/mojang.rs"]
mod mojang;
#[path = "../src/packwiz.rs"]
mod packwiz;
#[path = "../src/safe.rs"]
mod safe;
#[path = "../src/stats.rs"]
mod stats;
mod support;

use config::Dirs;
use download::{Job, Progress};
use install::{Endpoints, InstallParams};
use safe::Hash;
use sha1::{Digest, Sha1};
use sha2::Sha256;
use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use support::{Mode, Server, dead_base};

const MC: &str = "26.3";
const LOADER: &str = "0.20.1";

fn sha1_hex(b: &[u8]) -> String {
    hex::encode(Sha1::digest(b))
}
fn sha256_hex(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

/// Dossier temporaire supprimé en fin de test.
struct Tmp(PathBuf);
impl Tmp {
    fn new(tag: &str) -> Tmp {
        static N: AtomicUsize = AtomicUsize::new(0);
        let d = std::env::temp_dir().join(format!("crl-it-{tag}-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        Tmp(d)
    }
}
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn client() -> reqwest::blocking::Client {
    download::http_client().unwrap()
}

fn find_file(dir: &Path, name: &str) -> bool {
    fs::read_dir(dir).into_iter().flatten().flatten().any(|e| {
        let p = e.path();
        p.file_name().is_some_and(|n| n == name) || (p.is_dir() && find_file(&p, name))
    })
}

/// Tous les fichiers sous `dir` qui ne sont pas sous `except`.
fn files_outside(dir: &Path, except: &Path, acc: &mut Vec<PathBuf>) {
    for e in fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.starts_with(except) {
            continue;
        }
        if p.is_dir() {
            files_outside(&p, except, acc);
        } else {
            acc.push(p);
        }
    }
}

fn make_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (n, b) in entries {
        w.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
        w.write_all(b).unwrap();
    }
    w.finish().unwrap().into_inner()
}

fn java_exe() -> &'static str {
    if cfg!(windows) { "java.exe" } else { "java" }
}

fn part_of(dest: &Path) -> PathBuf {
    PathBuf::from(format!("{}.part", dest.display()))
}

fn pack_toml(index: &str) -> String {
    format!(
        "[index]\nfile = \"index.toml\"\nhash-format = \"sha256\"\nhash = \"{}\"\n[versions]\nfabric = \"{LOADER}\"\nminecraft = \"{MC}\"\n",
        sha256_hex(index.as_bytes())
    )
}

/// Écrit pack.toml, index.toml et les métafichiers (seulement ceux dont le chemin reste dans `dir`).
fn write_pack(dir: &Path, metas: &[(&str, String)]) {
    let mut index = String::from("hash-format = \"sha256\"\n");
    for (file, content) in metas {
        index += &format!("[[files]]\nfile = \"{file}\"\nhash = \"{}\"\nmetafile = true\n", sha256_hex(content.as_bytes()));
        if !file.contains("..") && !file.starts_with('/') && !file.contains(':') {
            let p = dir.join(file);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, content).unwrap();
        }
    }
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join("index.toml"), &index).unwrap();
    fs::write(dir.join("pack.toml"), pack_toml(&index)).unwrap();
}

fn meta(srv: &Server, name: &str, filename: &str, side: &str, body: &[u8]) -> String {
    format!(
        "name = \"{name}\"\nfilename = \"{}\"\nside = \"{side}\"\n[download]\nurl = \"{}\"\nhash-format = \"sha256\"\nhash = \"{}\"\n",
        filename.replace('\\', "\\\\"), // échappement TOML : le chemin brut arrive tel quel au launcher
        srv.url(&format!("/mods/{name}.jar")),
        sha256_hex(body)
    )
}

/// Monde de fixtures complet : serveur, pack packwiz, jar classroomrun, endpoints.
struct Fx {
    srv: Server,
    tmp: Tmp,
    dirs: Dirs,
    pack: PathBuf,
    jar: PathBuf,
    ep: Endpoints,
    asset_hash: String,
}

impl Fx {
    fn new(tag: &str) -> Fx {
        Fx::with_java_zip(tag, make_zip(&[(&format!("jdk-25-jre/bin/{}", java_exe()), b"fake-java")]))
    }

    fn with_java_zip(tag: &str, java_zip: Vec<u8>) -> Fx {
        let srv = Server::start();
        let tmp = Tmp::new(tag);
        let b = srv.base.clone();

        // Mojang
        let client_jar = b"client-jar".to_vec();
        let lib = b"lib-a".to_vec();
        let asset = b"asset-icon".to_vec();
        let asset_hash = sha1_hex(&asset);
        let index = format!(r#"{{"objects":{{"icons/a.png":{{"hash":"{asset_hash}","size":10}}}}}}"#);
        let version = format!(
            r#"{{"id":"{MC}","mainClass":"net.minecraft.client.main.Main","arguments":{{"game":[],"jvm":[]}},
            "assetIndex":{{"id":"26","sha1":"{}","url":"{b}/mc/assets.json"}},
            "downloads":{{"client":{{"sha1":"{}","url":"{b}/mc/client.jar"}}}},
            "javaVersion":{{"majorVersion":25}},
            "libraries":[{{"name":"org.x:a:1","downloads":{{"artifact":{{"path":"org/x/a/1/a-1.jar","sha1":"{}","url":"{b}/lib/a-1.jar"}}}}}}]}}"#,
            sha1_hex(index.as_bytes()),
            sha1_hex(&client_jar),
            sha1_hex(&lib)
        );
        srv.put("/manifest.json", format!(r#"{{"versions":[{{"id":"{MC}","url":"{b}/mc/{MC}.json","sha1":"{}"}}]}}"#, sha1_hex(version.as_bytes())));
        srv.put(&format!("/mc/{MC}.json"), version);
        srv.put("/mc/client.jar", client_jar);
        srv.put("/mc/assets.json", index);
        srv.put("/lib/a-1.jar", lib);
        srv.put(&format!("/assets/{}/{asset_hash}", &asset_hash[..2]), asset);

        // Fabric meta + Maven (une lib avec sha1 dans le profil, une avec .sha1 publié à côté)
        let loader_jar = b"fabric-loader-jar".to_vec();
        let inter_jar = b"intermediary-jar".to_vec();
        let profile = format!(
            r#"{{"id":"x","mainClass":"net.fabricmc.loader.impl.launch.knot.KnotClient","arguments":{{"game":[],"jvm":[]}},
            "libraries":[{{"name":"net.fabricmc:fabric-loader:{LOADER}","url":"{b}/maven/","sha1":"{}"}},
            {{"name":"net.fabricmc:intermediary:{MC}","url":"{b}/maven/"}}]}}"#,
            sha1_hex(&loader_jar)
        );
        srv.put(&format!("/fabric/v2/versions/loader/{MC}/{LOADER}/profile/json"), profile);
        srv.put(&format!("/maven/net/fabricmc/fabric-loader/{LOADER}/fabric-loader-{LOADER}.jar"), loader_jar);
        srv.put(&format!("/maven/net/fabricmc/intermediary/{MC}/intermediary-{MC}.jar"), inter_jar.clone());
        srv.put(&format!("/maven/net/fabricmc/intermediary/{MC}/intermediary-{MC}.jar.sha1"), sha1_hex(&inter_jar));

        // Adoptium
        srv.put(
            "/adoptium/v3/assets/latest/25/hotspot",
            format!(
                r#"[{{"binary":{{"package":{{"link":"{b}/adoptium/jre.zip","name":"jre.zip","checksum":"{}"}}}}}}]"#,
                sha256_hex(&java_zip)
            ),
        );
        srv.put("/adoptium/jre.zip", java_zip);

        // Pack packwiz local : a (both), cli (client), srv (server)
        let pack = tmp.0.join("pack");
        let mods: [(&str, &str); 3] = [("a", "both"), ("cli", "client"), ("srv", "server")];
        let metas: Vec<(String, String)> = mods
            .iter()
            .map(|(n, side)| {
                let body = format!("mod-{n}").into_bytes();
                srv.put(&format!("/mods/{n}.jar"), body.clone());
                (format!("mods/{n}.pw.toml"), meta(&srv, n, &format!("{n}.jar"), side, &body))
            })
            .collect();
        let metas: Vec<(&str, String)> = metas.iter().map(|(f, c)| (f.as_str(), c.clone())).collect();
        write_pack(&pack, &metas);

        let jar = tmp.0.join("build/libs/classroomrun-0.1.jar");
        fs::create_dir_all(jar.parent().unwrap()).unwrap();
        fs::write(&jar, b"classroomrun").unwrap();

        let ep = Endpoints {
            mojang_manifest: format!("{b}/manifest.json"),
            fabric_meta: format!("{b}/fabric"),
            adoptium: format!("{b}/adoptium"),
            assets: format!("{b}/assets"),
        };
        let dirs = Dirs::new(tmp.0.join("data"));
        Fx { srv, tmp, dirs, pack, jar, ep, asset_hash }
    }

    fn params(&self) -> InstallParams {
        InstallParams { dirs: self.dirs.clone(), pack_dir: self.pack.clone(), mod_jar: Some(self.jar.clone()), endpoints: self.ep.clone() }
    }

    /// (chemin de java, id de version) ; `Installed` n'est pas `Debug`.
    fn install(&self) -> anyhow::Result<(PathBuf, String)> {
        let i = install::install(&client(), &self.params(), &Arc::new(Progress::default()))?;
        Ok((i.java, i.version.id))
    }

    fn mods_dir(&self) -> PathBuf {
        self.dirs.instance().join("mods")
    }
}

fn job(url: String, dest: PathBuf, body: &[u8]) -> Job {
    Job { url, dest, hash: Some(Hash::new("sha256", &sha256_hex(body)).unwrap()) }
}

fn pattern(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i * 31 % 251) as u8).collect()
}

// ---------- Cas 1 : installation complète puis réinstallation ----------

#[test]
fn case1_full_install_then_second_install_skips_everything() {
    let fx = Fx::new("full");
    let (java, id) = fx.install().expect("installation sur fixtures");
    assert!(java.is_file() && java.ends_with(format!("bin/{}", java_exe())));
    assert_eq!(id, MC);

    let d = &fx.dirs;
    let check = |p: PathBuf, sha1: &str| {
        assert!(Hash::new("sha1", sha1).unwrap().matches_file(&p).unwrap(), "hash KO : {}", p.display());
    };
    check(d.client_jar(MC), &sha1_hex(b"client-jar"));
    check(d.libraries().join("org/x/a/1/a-1.jar"), &sha1_hex(b"lib-a"));
    check(d.assets().join("objects").join(&fx.asset_hash[..2]).join(&fx.asset_hash), &fx.asset_hash);
    assert!(d.assets().join("indexes/26.json").is_file());
    check(d.libraries().join(format!("net/fabricmc/fabric-loader/{LOADER}/fabric-loader-{LOADER}.jar")), &sha1_hex(b"fabric-loader-jar"));
    check(d.libraries().join(format!("net/fabricmc/intermediary/{MC}/intermediary-{MC}.jar")), &sha1_hex(b"intermediary-jar"));
    assert!(d.versions().join(format!("fabric-loader-{LOADER}-{MC}/fabric-loader-{LOADER}-{MC}.json")).is_file());
    for (f, c) in [("a.jar", "mod-a"), ("cli.jar", "mod-cli"), ("classroomrun-0.1.jar", "classroomrun")] {
        assert_eq!(fs::read(fx.mods_dir().join(f)).unwrap(), c.as_bytes(), "{f}");
    }
    assert!(!d.downloads().join("jre.zip.part").exists());

    let before = fx.srv.requests().len();
    fx.install().expect("seconde installation");
    let again: Vec<String> = fx.srv.requests()[before..].iter().map(|r| r.path.clone()).collect();
    for heavy in ["/mc/client.jar", "/lib/", "/assets/", "/mods/", "/adoptium/jre.zip", "/maven/net/fabricmc/fabric-loader/"] {
        assert!(!again.iter().any(|p| p.starts_with(heavy)), "retéléchargé à la 2e installation : {heavy} ({again:?})");
    }
}

// ---------- Cas 2 : reprise Range ----------

#[test]
fn case2_interrupted_download_resumes_with_range() {
    let srv = Server::start();
    let tmp = Tmp::new("resume");
    let body = pattern(100_000);
    srv.put_mode("/f.bin", body.clone(), Mode::TruncateOnce(30_000));
    let dest = tmp.0.join("out/f.bin");
    let j = job(srv.url("/f.bin"), dest.clone(), &body);

    let err = download::fetch(&client(), &j).expect_err("la coupure doit échouer");
    let part = part_of(&dest);
    let have = fs::metadata(&part).expect(".part laissé après coupure").len();
    assert!(have > 0 && have < body.len() as u64, "part = {have} ({err:#})");
    assert!(!dest.exists());

    assert!(download::fetch(&client(), &j).unwrap());
    let last = srv.requests().pop().unwrap();
    assert_eq!(last.range.as_deref(), Some(format!("bytes={have}-").as_str()));
    assert_eq!(fs::read(&dest).unwrap(), body);
    assert!(!part.exists());
}

#[test]
fn case2_fetch_all_retries_by_itself_after_truncation() {
    let srv = Server::start();
    let tmp = Tmp::new("retry");
    let body = pattern(50_000);
    srv.put_mode("/f.bin", body.clone(), Mode::TruncateOnce(10_000));
    let j = job(srv.url("/f.bin"), tmp.0.join("f.bin"), &body);
    download::fetch_all(&client(), vec![j], 4, &Arc::new(Progress::default())).unwrap();
    assert_eq!(fs::read(tmp.0.join("f.bin")).unwrap(), body);
}

#[test]
fn case2_server_ignoring_range_still_gives_correct_file() {
    let srv = Server::start();
    let tmp = Tmp::new("norange");
    let body = pattern(40_000);
    srv.put_mode("/f.bin", body.clone(), Mode::IgnoreRange);
    let dest = tmp.0.join("f.bin");
    fs::write(part_of(&dest), &body[..1234]).unwrap();
    assert!(download::fetch(&client(), &job(srv.url("/f.bin"), dest.clone(), &body)).unwrap());
    assert_eq!(srv.requests()[0].range.as_deref(), Some("bytes=1234-"));
    assert_eq!(fs::read(&dest).unwrap(), body);
}

#[test]
fn case2_complete_part_with_416_is_accepted() {
    let srv = Server::start();
    let tmp = Tmp::new("416");
    let body = pattern(5_000);
    srv.put("/f.bin", body.clone());
    let dest = tmp.0.join("f.bin");
    fs::write(part_of(&dest), &body).unwrap();
    assert!(download::fetch(&client(), &job(srv.url("/f.bin"), dest.clone(), &body)).unwrap());
    assert_eq!(fs::read(&dest).unwrap(), body);
}

// ---------- Cas 3 : hash invalide ----------

#[test]
fn case3_invalid_hash_errors_and_leaves_no_file() {
    let srv = Server::start();
    let tmp = Tmp::new("badhash");
    srv.put("/f.bin", b"contenu corrompu".to_vec());
    let dest = tmp.0.join("f.bin");
    // un ancien fichier invalide est aussi remplacé puis supprimé
    fs::write(&dest, b"vieux").unwrap();
    let j = job(srv.url("/f.bin"), dest.clone(), b"contenu attendu");
    let e = download::fetch(&client(), &j).unwrap_err().to_string();
    assert!(e.contains("hash invalide") && e.contains("/f.bin"), "{e}");
    assert!(!dest.exists() && !part_of(&dest).exists());

    let e = download::fetch_all(&client(), vec![j], 2, &Arc::new(Progress::default())).unwrap_err().to_string();
    assert!(e.contains("hash invalide"), "{e}");
    assert!(!dest.exists());
}

#[test]
fn case3_install_with_wrong_library_content_fails_without_final_file() {
    let fx = Fx::new("badlib");
    fx.srv.put("/lib/a-1.jar", b"pas le bon contenu".to_vec());
    let e = fx.install().unwrap_err().to_string();
    assert!(e.contains("hash invalide"), "{e}");
    assert!(!fx.dirs.libraries().join("org/x/a/1/a-1.jar").exists());
}

#[test]
fn case3_http_errors_are_reported() {
    let srv = Server::start();
    let tmp = Tmp::new("http");
    srv.put_mode("/boom", b"x".to_vec(), Mode::Status(500));
    for p in ["/boom", "/absent"] {
        let e = download::fetch(&client(), &job(srv.url(p), tmp.0.join("x.bin"), b"x")).unwrap_err().to_string();
        assert!(e.contains("HTTP 500") || e.contains("HTTP 404"), "{e}");
        assert!(!tmp.0.join("x.bin").exists());
    }
}

// ---------- Cas 4 : écriture impossible ----------

#[test]
fn case4_parent_is_a_file_gives_clean_error() {
    let srv = Server::start();
    let tmp = Tmp::new("parentfile");
    srv.put("/f.bin", b"x".to_vec());
    let blocker = tmp.0.join("blocker");
    fs::write(&blocker, b"je suis un fichier").unwrap();
    let r = download::fetch(&client(), &job(srv.url("/f.bin"), blocker.join("sous/f.bin"), b"x"));
    assert!(r.is_err());
    assert!(blocker.is_file());
}

#[test]
fn case4_readonly_part_file_gives_clean_error() {
    let srv = Server::start();
    let tmp = Tmp::new("readonly");
    srv.put("/f.bin", b"abcdef".to_vec());
    let dest = tmp.0.join("f.bin");
    let part = part_of(&dest);
    fs::write(&part, b"ab").unwrap();
    let mut perm = fs::metadata(&part).unwrap().permissions();
    perm.set_readonly(true);
    fs::set_permissions(&part, perm.clone()).unwrap();
    let r = download::fetch(&client(), &job(srv.url("/f.bin"), dest.clone(), b"abcdef"));
    #[allow(clippy::permissions_set_readonly_false)]
    perm.set_readonly(false);
    fs::set_permissions(&part, perm).unwrap();
    assert!(r.is_err(), "écriture dans un .part en lecture seule acceptée");
    assert!(!dest.exists());
}

#[test]
fn case4_install_root_is_a_file_gives_clean_error() {
    let mut fx = Fx::new("rootfile");
    let root = fx.tmp.0.join("rootfile");
    fs::write(&root, b"x").unwrap();
    fx.dirs = Dirs::new(&root);
    assert!(fx.install().is_err());
    assert!(root.is_file());
}

#[test]
fn case4_mods_dir_is_a_file_gives_clean_error() {
    let fx = Fx::new("modsfile");
    fs::create_dir_all(fx.dirs.instance()).unwrap();
    fs::write(fx.mods_dir(), b"x").unwrap();
    assert!(fx.install().is_err());
}

// ---------- Cas 5 : pas de réseau, stats ----------

#[test]
fn case5_no_network_install_and_download_fail_cleanly() {
    let mut fx = Fx::new("offline");
    let dead = dead_base();
    fx.ep = Endpoints { mojang_manifest: format!("{dead}/m.json"), fabric_meta: dead.clone(), adoptium: dead.clone(), assets: dead.clone() };
    let e = format!("{:#}", fx.install().unwrap_err());
    assert!(e.contains("requête"), "{e}");
    assert!(!fx.dirs.client_jar(MC).exists());

    let tmp = Tmp::new("offline2");
    let r = download::fetch_all(&client(), vec![job(format!("{dead}/x"), tmp.0.join("x"), b"x")], 2, &Arc::new(Progress::default()));
    assert!(r.is_err());
    assert!(!tmp.0.join("x").exists());
}

#[test]
fn case5_stats_unreachable_or_invalid_gives_displayable_error() {
    let c = client();
    let e = format!("{:#}", stats::fetch(&c, &format!("{}/stats", dead_base())).unwrap_err());
    assert!(e.contains("injoignable"), "{e}");

    let srv = Server::start();
    srv.put("/ok", br#"{"runNumber":3,"running":true,"players":["A"]}"#.to_vec());
    srv.put("/garbage", b"<html>pas du json</html>".to_vec());
    srv.put("/cut", br#"{"runNumber":3,"play"#.to_vec());
    srv.put("/wrongtype", br#"{"runNumber":"trois"}"#.to_vec());
    srv.put_mode("/err", b"x".to_vec(), Mode::Status(500));
    let s = stats::fetch(&c, &srv.url("/ok")).unwrap();
    assert_eq!((s.run_number, s.running, s.players.len()), (3, true, 1));
    for p in ["/garbage", "/cut", "/wrongtype"] {
        let e = format!("{:#}", stats::fetch(&c, &srv.url(p)).unwrap_err());
        assert!(e.contains("invalide"), "{p}: {e}");
    }
    for p in ["/err", "/missing"] {
        let e = stats::fetch(&c, &srv.url(p)).unwrap_err().to_string();
        assert!(e.starts_with("HTTP 5") || e.starts_with("HTTP 4"), "{p}: {e}");
    }
}

#[test]
fn case5_invalid_server_json_gives_clean_install_error() {
    let fx = Fx::new("badjson");
    fx.srv.put("/manifest.json", b"pas du json".to_vec());
    assert!(format!("{:#}", fx.install().unwrap_err()).contains("JSON invalide"));
    let fx = Fx::new("nojava");
    fx.srv.put("/adoptium/v3/assets/latest/25/hotspot", b"[]".to_vec());
    assert!(fx.install().unwrap_err().to_string().contains("aucun JRE"));
}

// ---------- Cas 6 : manifeste malveillant, zip-slip ----------

fn evil_pack_attempt(tag: &str, rel_file: &str, filename: &str) {
    let fx = Fx::new(tag);
    fx.srv.put("/mods/evil.jar", b"evil".to_vec());
    let m = meta(&fx.srv, "evil", filename, "both", b"evil");
    write_pack(&fx.pack, &[(rel_file, m)]);
    let e = fx.install().expect_err(&format!("doit refuser {rel_file:?} / {filename:?}"));
    assert!(!format!("{e:#}").is_empty());
    for n in ["evil.jar", "abs-evil.jar"] {
        assert!(!find_file(&fx.tmp.0, n), "{n} écrit pour {rel_file:?}/{filename:?}");
    }
    assert_eq!(fx.srv.count("/mods/"), 0, "téléchargement tenté pour {rel_file:?}/{filename:?} : {e:#}");
}

#[test]
fn case6_malicious_manifest_filename_is_refused() {
    for f in ["../evil.jar", "../../evil.jar", "/abs-evil.jar", "C:\\abs-evil.jar", "a/../../evil.jar", "..\\evil.jar"] {
        evil_pack_attempt("evilname", "mods/evil.pw.toml", f);
    }
}

#[test]
fn case6_malicious_manifest_file_path_is_refused() {
    for f in ["../evil.pw.toml", "/abs-evil.pw.toml", "C:\\abs-evil.pw.toml", "mods/../../evil.pw.toml"] {
        evil_pack_attempt("evilpath", f, "evil.jar");
    }
}

#[test]
fn case6_malicious_pack_index_file_is_refused() {
    let tmp = Tmp::new("evilindex");
    fs::write(tmp.0.join("outside.toml"), "hash-format = \"sha256\"\n").unwrap();
    let pack = tmp.0.join("pack");
    fs::create_dir_all(&pack).unwrap();
    for idx in ["../outside.toml", "/outside.toml", "C:\\outside.toml"] {
        let t = format!(
            "[index]\nfile = \"{}\"\nhash-format = \"sha256\"\nhash = \"{}\"\n[versions]\nminecraft = \"{MC}\"\nfabric = \"{LOADER}\"\n",
            idx.replace('\\', "\\\\"),
            sha256_hex(b"hash-format = \"sha256\"\n")
        );
        fs::write(pack.join("pack.toml"), t).unwrap();
        assert!(packwiz::load_pack(&pack).is_err(), "{idx}");
    }
}

#[test]
fn case6_zip_slip_in_java_archive_is_refused() {
    for slip in ["../evil.txt", "a/../../evil.txt", "/abs-evil.txt", "..\\evil.txt", "C:\\abs-evil.txt"] {
        let tmp = Tmp::new("slip");
        let zp = tmp.0.join("e.zip");
        fs::write(&zp, make_zip(&[("ok/bin/x", b"x"), (slip, b"evil")])).unwrap();
        let out = tmp.0.join("deep/out");
        let r = java::extract(&zp, &out);
        let mut outside = Vec::new();
        files_outside(&tmp.0, &out, &mut outside);
        assert_eq!(outside, std::slice::from_ref(&zp), "fichier écrit hors de la destination pour {slip:?} (result {r:?})");
    }
    let tmp = Tmp::new("slip2");
    let zp = tmp.0.join("e.zip");
    fs::write(&zp, make_zip(&[("../evil.txt", b"x")])).unwrap();
    assert!(java::extract(&zp, &tmp.0.join("deep/out")).is_err());
}

#[test]
fn case6_zip_slip_aborts_whole_install() {
    let fx = Fx::with_java_zip("slipinstall", make_zip(&[("../../evil.txt", b"evil"), ("jdk/bin/java", b"x")]));
    assert!(fx.install().is_err());
    assert!(!find_file(&fx.tmp.0, "evil.txt"));
    assert!(java::find_java(&fx.dirs.java(25)).is_none());
}

// ---------- Cas 7 : side=server, ménage de instance/mods ----------

#[test]
fn case7_server_mods_skipped_and_stale_jars_removed_only_in_instance_mods() {
    let fx = Fx::new("cleanup");
    let mods = fx.mods_dir();
    fs::create_dir_all(&mods).unwrap();
    let inst = fx.dirs.instance();
    let keep = [
        inst.join("other.jar"),
        fx.dirs.libraries().join("keep.jar"),
        fx.dirs.root.join("keep.jar"),
        fx.tmp.0.join("pack/keep.jar"),
        mods.join("config.json"),
        mods.join("notes.txt"),
    ];
    for p in &keep {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, b"garder").unwrap();
    }
    let stale = [mods.join("old.jar"), mods.join("srv.jar"), mods.join("classroomrun-0.0.9.jar")];
    for p in &stale {
        fs::write(p, b"vieux").unwrap();
    }
    fx.install().unwrap();
    assert!(!fx.srv.requests().iter().any(|r| r.path == "/mods/srv.jar"), "mod server téléchargé");
    for p in &stale {
        assert!(!p.exists(), "devrait être supprimé : {}", p.display());
    }
    for p in &keep {
        assert!(p.exists(), "ne devait pas être touché : {}", p.display());
    }
    for f in ["a.jar", "cli.jar", "classroomrun-0.1.jar"] {
        assert!(mods.join(f).is_file(), "{f}");
    }
}

// ---------- Bug connu (rapporté, non corrigé) ----------

/// Deux jobs vers la même destination dans un même `fetch_all` écrivent le même `.part` en parallèle.
/// Lancer avec `cargo test -- --ignored` pour le reproduire (échec intermittent).
#[test]
#[ignore = "bug download.rs:fetch_all : jobs à destination identique concurrents (.part partagé)"]
fn known_bug_duplicate_destinations_race_on_same_part_file() {
    for _ in 0..10 {
        let srv = Server::start();
        let tmp = Tmp::new("dup");
        let body = pattern(3_000_000);
        srv.put("/f.bin", body.clone());
        let jobs: Vec<Job> = (0..8).map(|_| job(srv.url("/f.bin"), tmp.0.join("f.bin"), &body)).collect();
        download::fetch_all(&client(), jobs, 8, &Arc::new(Progress::default())).unwrap();
        assert_eq!(fs::read(tmp.0.join("f.bin")).unwrap(), body);
    }
}
