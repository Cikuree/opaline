//! Cas 3 : pack distant (pack.zip servi par le serveur de fixtures) : extraction, zip-slip, hashes.
#![allow(dead_code)]

use opaline_core::{config, download, packwiz, safe};

mod support;

use config::Dirs;
use download::Job;
use safe::Hash;
use sha2::{Digest, Sha256};
use std::io::{Cursor, Write};
use std::path::PathBuf;
use support::{Mode, Server};

const JAR: &[u8] = b"JAR-FIXTURE-CONTENT";
const CFG: &[u8] = b"{\"k\":1}\n";

fn sha256(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

fn meta(jar_url: &str) -> String {
    format!("name = \"a\"\nfilename = \"a.jar\"\nside = \"client\"\n[download]\nurl = \"{jar_url}\"\nhash-format = \"sha256\"\nhash = \"{}\"\n", sha256(JAR))
}

fn index(meta_text: &str) -> String {
    format!(
        "hash-format = \"sha256\"\n[[files]]\nfile = \"mods/a.pw.toml\"\nhash = \"{}\"\nmetafile = true\n[[files]]\nfile = \"config/x.json\"\nhash = \"{}\"\n",
        sha256(meta_text.as_bytes()),
        sha256(CFG)
    )
}

fn pack_toml(index_text: &str) -> String {
    format!(
        "name = \"Classroom\"\nversion = \"1.2.3\"\n[index]\nfile = \"index.toml\"\nhash-format = \"sha256\"\nhash = \"{}\"\n[versions]\nminecraft = \"26.3\"\nfabric = \"0.20.1\"\n",
        sha256(index_text.as_bytes())
    )
}

/// Contenu cohérent d'un pack : (pack.toml, index.toml, mods/a.pw.toml).
fn good_pack(srv: &Server) -> (String, String, String) {
    let m = meta(&srv.url("/files/a.jar"));
    let i = index(&m);
    (pack_toml(&i), i, m)
}

fn make_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (n, b) in entries {
        w.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
        w.write_all(b).unwrap();
    }
    w.finish().unwrap().into_inner()
}

fn pack_entries<'a>(prefix: &str, p: &'a str, i: &'a str, m: &'a str) -> Vec<(String, Vec<u8>)> {
    vec![
        (format!("{prefix}pack.toml"), p.as_bytes().to_vec()),
        (format!("{prefix}index.toml"), i.as_bytes().to_vec()),
        (format!("{prefix}mods/a.pw.toml"), m.as_bytes().to_vec()),
        (format!("{prefix}config/x.json"), CFG.to_vec()),
    ]
}

fn zip_of(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
    let refs: Vec<(&str, &[u8])> = entries.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect();
    make_zip(&refs)
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("crl-it-pack-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn client() -> reqwest::blocking::Client {
    download::http_client().unwrap()
}

#[test]
fn remote_zip_with_pack_dir_is_extracted_and_hashes_check() {
    let srv = Server::start();
    let (p, i, m) = good_pack(&srv);
    srv.put("/pack.zip", zip_of(&pack_entries("pack/", &p, &i, &m)));
    srv.put("/files/a.jar", JAR);
    let base = tmp("ok");
    let dirs = Dirs::new(base.join("root"));

    let dir = packwiz::fetch_remote_pack(&client(), &dirs, &srv.url("/pack.zip")).expect("extraction");
    assert!(dir.ends_with("pack") && dir.join("pack.toml").is_file(), "dossier : {}", dir.display());
    assert_eq!(srv.count("/pack.zip"), 1);

    let pack = packwiz::load_pack(&dir).expect("pack vérifié");
    assert_eq!(pack.version, "1.2.3");
    assert_eq!(pack.mods.len(), 1);
    assert_eq!(pack.mods[0].url, srv.url("/files/a.jar"));
    assert_eq!(pack.mods[0].hash, Hash::new("sha256", &sha256(JAR)).unwrap());
    assert_eq!(pack.files[0].path, "config/x.json");

    // hash du jar vérifié au téléchargement : le bon fichier passe
    let dest = base.join("mods").join("a.jar");
    let job = Job { url: pack.mods[0].url.clone(), dest: dest.clone(), hash: pack.mods[0].hash.clone(), size: None };
    assert!(download::fetch(&client(), &job).unwrap());
    assert_eq!(std::fs::read(&dest).unwrap(), JAR);
}

#[test]
fn zip_with_pack_files_at_root_is_accepted() {
    let srv = Server::start();
    let (p, i, m) = good_pack(&srv);
    srv.put("/pack.zip", zip_of(&pack_entries("", &p, &i, &m)));
    let dirs = Dirs::new(tmp("root").join("root"));
    let dir = packwiz::fetch_remote_pack(&client(), &dirs, &srv.url("/pack.zip")).expect("extraction");
    assert!(packwiz::load_pack(&dir).is_ok());
}

#[test]
fn jar_with_wrong_hash_is_refused_and_leaves_no_file() {
    let srv = Server::start();
    let (p, i, m) = good_pack(&srv);
    srv.put("/pack.zip", zip_of(&pack_entries("pack/", &p, &i, &m)));
    // le serveur sert un autre contenu que celui annoncé par le métafichier
    srv.put("/files/a.jar", b"TAMPERED".to_vec());
    let base = tmp("badjar");
    let dirs = Dirs::new(base.join("root"));
    let dir = packwiz::fetch_remote_pack(&client(), &dirs, &srv.url("/pack.zip")).unwrap();
    let pack = packwiz::load_pack(&dir).unwrap();
    let dest = base.join("mods").join("a.jar");
    let job = Job { url: pack.mods[0].url.clone(), dest: dest.clone(), hash: pack.mods[0].hash.clone(), size: None };
    let e = download::fetch(&client(), &job).expect_err("hash invalide");
    assert!(format!("{e:#}").contains("hash"), "message : {e:#}");
    assert!(!dest.exists(), "fichier final écrit malgré hash faux");
}

#[test]
fn zip_slip_entries_are_refused() {
    let evil: [&str; 4] = ["../evil.txt", "pack/../../evil.txt", "..\\evil.txt", "pack/../../../evil.txt"];
    for name in evil {
        let srv = Server::start();
        let (p, i, m) = good_pack(&srv);
        let mut entries = pack_entries("pack/", &p, &i, &m);
        entries.push((name.to_string(), b"PWNED".to_vec()));
        srv.put("/pack.zip", zip_of(&entries));
        let base = tmp("slip");
        let dirs = Dirs::new(base.join("root"));
        let r = packwiz::fetch_remote_pack(&client(), &dirs, &srv.url("/pack.zip"));
        let e = r.err().unwrap_or_else(|| panic!("zip-slip accepté : {name}"));
        assert!(e.to_string().contains("entrée zip refusée"), "{name} : message {e:#}");
        // rien n'a été écrit hors du dossier de travail
        assert!(!base.join("evil.txt").exists() && !base.join("root").join("evil.txt").exists(), "{name} : fichier écrit");
        assert!(!base.join("root").join("downloads").join("evil.txt").exists());
    }
}

#[test]
fn absolute_zip_entry_never_reaches_filesystem_root() {
    // `enclosed_name` retire la racine : l'entrée reste dans le dossier de travail, ou est refusée.
    let srv = Server::start();
    let (p, i, m) = good_pack(&srv);
    let mut entries = pack_entries("pack/", &p, &i, &m);
    entries.push(("/abs-evil.txt".to_string(), b"PWNED".to_vec()));
    srv.put("/pack.zip", zip_of(&entries));
    let dirs = Dirs::new(tmp("abs").join("root"));
    let _ = packwiz::fetch_remote_pack(&client(), &dirs, &srv.url("/pack.zip"));
    assert!(!std::path::Path::new("/abs-evil.txt").exists(), "chemin absolu écrit à la racine");
}

#[test]
fn index_changed_without_updating_pack_toml_is_refused() {
    let srv = Server::start();
    let (p, i, m) = good_pack(&srv);
    // index modifié (commentaire ajouté) mais pack.toml garde l'ancien hash
    let tampered = format!("{i}# ajouté\n");
    srv.put("/pack.zip", zip_of(&pack_entries("pack/", &p, &tampered, &m)));
    let dirs = Dirs::new(tmp("idx").join("root"));
    let dir = packwiz::fetch_remote_pack(&client(), &dirs, &srv.url("/pack.zip")).unwrap();
    let e = packwiz::load_pack(&dir).expect_err("index modifié accepté");
    assert_eq!(e.to_string(), "hash de index.toml différent de celui de pack.toml");
}

#[test]
fn pw_toml_modified_is_refused() {
    let srv = Server::start();
    let (p, i, _m) = good_pack(&srv);
    // métafichier modifié (URL détournée) : son hash dans index.toml ne correspond plus
    let evil = meta("https://attacker.invalid/a.jar");
    srv.put("/pack.zip", zip_of(&pack_entries("pack/", &p, &i, &evil)));
    let dirs = Dirs::new(tmp("meta").join("root"));
    let dir = packwiz::fetch_remote_pack(&client(), &dirs, &srv.url("/pack.zip")).unwrap();
    let e = packwiz::load_pack(&dir).expect_err("métafichier modifié accepté");
    assert_eq!(e.to_string(), "hash de l'index invalide pour mods/a.pw.toml");
}

#[test]
fn coherent_rehash_of_whole_zip_is_not_detected() {
    // Limite connue : les hashes ne sont pas signés. Un zip entièrement recalculé par un attaquant
    // passe ; la confiance repose sur le canal HTTPS de la release. Test documentant ce comportement.
    let srv = Server::start();
    let m = meta("https://attacker.invalid/a.jar");
    let i = index(&m);
    let p = pack_toml(&i);
    srv.put("/pack.zip", zip_of(&pack_entries("pack/", &p, &i, &m)));
    let dirs = Dirs::new(tmp("rehash").join("root"));
    let dir = packwiz::fetch_remote_pack(&client(), &dirs, &srv.url("/pack.zip")).unwrap();
    assert!(packwiz::load_pack(&dir).is_ok());
}

#[test]
fn http_error_on_pack_zip_is_reported() {
    let srv = Server::start();
    srv.put_mode("/pack.zip", "", Mode::Status(404));
    let dirs = Dirs::new(tmp("404").join("root"));
    let e = packwiz::fetch_remote_pack(&client(), &dirs, &srv.url("/pack.zip")).expect_err("404");
    assert!(e.to_string().starts_with("HTTP 404"), "message : {e}");
}

#[test]
fn zip_without_pack_toml_is_reported() {
    let srv = Server::start();
    srv.put("/pack.zip", make_zip(&[("autre.txt", b"x")]));
    let dirs = Dirs::new(tmp("nopack").join("root"));
    let e = packwiz::fetch_remote_pack(&client(), &dirs, &srv.url("/pack.zip")).expect_err("pas de pack.toml");
    assert_eq!(e.to_string(), "pack.zip : pack.toml introuvable");
}
