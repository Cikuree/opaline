//! Helpers partagés des tests Opaline : fixtures locales (aucun réseau), dossiers temporaires, packs packwiz.
#![allow(dead_code)]
use crate::support::Server;
use opaline_core::config::Dirs;
use opaline_core::download::{self, Progress};
use opaline_core::install::{self, Endpoints, InstallParams};
use sha1::{Digest, Sha1};
use sha2::Sha256;
use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

pub const MC: &str = "26.3";
pub const LOADER: &str = "0.20.1";

/// Sérialise les tests qui modifient des variables d'environnement (process partagé par les threads de test).
pub fn env_lock() -> MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    L.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn set_env(k: &str, v: &str) {
    // SAFETY: appelé uniquement sous `env_lock()`, avant toute lecture concurrente de la variable.
    unsafe { std::env::set_var(k, v) }
}

pub fn unset_env(k: &str) {
    // SAFETY: idem `set_env`.
    unsafe { std::env::remove_var(k) }
}

/// Dossier temporaire unique, supprimé au `Drop`.
pub struct Tmp(pub PathBuf);

impl Tmp {
    pub fn new(tag: &str) -> Tmp {
        static N: AtomicUsize = AtomicUsize::new(0);
        let d = std::env::temp_dir().join(format!("opl-{tag}-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
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

pub fn sha256_hex(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

pub fn sha1_hex(b: &[u8]) -> String {
    hex::encode(Sha1::digest(b))
}

pub fn make_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (n, b) in entries {
        w.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
        w.write_all(b).unwrap();
    }
    w.finish().unwrap().into_inner()
}

pub fn java_exe() -> &'static str {
    if cfg!(windows) { "java.exe" } else { "java" }
}

pub fn http() -> reqwest::blocking::Client {
    download::http_client().unwrap()
}

pub struct ModSpec<'a> {
    pub name: &'a str,
    pub filename: &'a str,
    pub body: &'a [u8],
}

/// Écrit un dossier packwiz (pack.toml, index.toml, mods/<name>.pw.toml) dont les jars sont servis par `srv`
/// sous `route` (ex. `/packs/alpha`). Renvoie le hash de l'index, c'est-à-dire le `packHash` du catalogue.
pub fn write_packwiz(dir: &Path, srv: &Server, route: &str, version: &str, mods: &[ModSpec]) -> String {
    fs::create_dir_all(dir.join("mods")).unwrap();
    let mut index = String::from("hash-format = \"sha256\"\n");
    for m in mods {
        let url = srv.url(&format!("{route}/{}", m.filename));
        let meta = format!(
            "name = \"{}\"\nfilename = \"{}\"\nside = \"both\"\n[download]\nurl = \"{url}\"\nhash-format = \"sha256\"\nhash = \"{}\"\n",
            m.name,
            m.filename,
            sha256_hex(m.body)
        );
        let rel = format!("mods/{}.pw.toml", m.name);
        index += &format!("[[files]]\nfile = \"{rel}\"\nhash = \"{}\"\nmetafile = true\n", sha256_hex(meta.as_bytes()));
        fs::write(dir.join(&rel), &meta).unwrap();
        srv.put(&format!("{route}/{}", m.filename), m.body.to_vec());
    }
    let pack = format!(
        "name = \"Pack {version}\"\nversion = \"{version}\"\n[index]\nfile = \"index.toml\"\nhash-format = \"sha256\"\nhash = \"{}\"\n[versions]\nminecraft = \"{MC}\"\nfabric = \"{LOADER}\"\n",
        sha256_hex(index.as_bytes())
    );
    fs::write(dir.join("index.toml"), &index).unwrap();
    fs::write(dir.join("pack.toml"), pack).unwrap();
    sha256_hex(index.as_bytes())
}

/// Zip d'un pack packwiz (fichiers à la racine du dossier `pack/`, comme la release).
pub fn pack_zip(dir: &Path) -> Vec<u8> {
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    for rel in ["pack.toml", "index.toml", "mods"] {
        let p = dir.join(rel);
        if p.is_file() {
            entries.push((format!("pack/{rel}"), fs::read(&p).unwrap()));
        } else {
            for e in fs::read_dir(&p).unwrap().flatten() {
                entries.push((format!("pack/{rel}/{}", e.file_name().to_string_lossy()), fs::read(e.path()).unwrap()));
            }
        }
    }
    let refs: Vec<(&str, &[u8])> = entries.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect();
    make_zip(&refs)
}

/// Fixtures Mojang, Fabric et Adoptium pour `install::install` : tout est servi par `srv`.
pub fn install_endpoints(srv: &Server) -> Endpoints {
    let b = srv.base.clone();
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

    let loader_jar = b"fabric-loader-jar".to_vec();
    let profile = format!(
        r#"{{"id":"x","mainClass":"net.fabricmc.loader.impl.launch.knot.KnotClient","arguments":{{"game":[],"jvm":[]}},
        "libraries":[{{"name":"net.fabricmc:fabric-loader:{LOADER}","url":"{b}/maven/","sha1":"{}"}}]}}"#,
        sha1_hex(&loader_jar)
    );
    srv.put(&format!("/fabric/v2/versions/loader/{MC}/{LOADER}/profile/json"), profile);
    srv.put(&format!("/maven/net/fabricmc/fabric-loader/{LOADER}/fabric-loader-{LOADER}.jar"), loader_jar);

    let java_zip = make_zip(&[(&format!("jdk-25-jre/bin/{}", java_exe()), b"fake-java")]);
    srv.put(
        "/adoptium/v3/assets/latest/25/hotspot",
        format!(r#"[{{"binary":{{"package":{{"link":"{b}/adoptium/jre.zip","name":"jre.zip","checksum":"{}"}}}}}}]"#, sha256_hex(&java_zip)),
    );
    srv.put("/adoptium/jre.zip", java_zip);

    Endpoints {
        mojang_manifest: format!("{b}/manifest.json"),
        fabric_meta: format!("{b}/fabric"),
        quilt_meta: format!("{b}/quilt"),
        neoforge_maven: format!("{b}/neoforge"),
        forge_maven: format!("{b}/forge"),
        adoptium: format!("{b}/adoptium"),
        assets: format!("{b}/assets"),
    }
}

/// Installe `pack_dir` dans `dirs` avec les fixtures `ep`.
pub fn install_pack_dir(ep: &Endpoints, dirs: &Dirs, pack_dir: &Path) -> anyhow::Result<()> {
    let p = InstallParams { dirs: dirs.clone(), pack_dir: pack_dir.to_path_buf(), mod_jar: None, endpoints: ep.clone(), overrides: None };
    install::install(&http(), &p, &Arc::new(Progress::default())).map(|_| ())
}

/// Entrée de catalogue `packs.json` (une release).
pub fn catalog_entry(id: &str, loader: &str, pack_url: &str, pack_hash: &str) -> String {
    format!(
        r#"{{"id":"{id}","name":"Pack {id}","description":"d","mc":"{MC}","loader":"{loader}","loaderVersion":"{LOADER}","modsCount":1,"thumbnail":"ender_eye","widgets":["server"],"pack":"{pack_url}","packHash":"{pack_hash}","server":{{"host":"classroom","port":25565,"motd":"Classroom Run","statsPort":0}}}}"#
    )
}

pub fn catalog_json(entries: &[String]) -> String {
    format!(r#"{{"packs":[{}]}}"#, entries.join(","))
}
