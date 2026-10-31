//! Cas 4 : vérification de `launcher.json` (mise à jour proposée ou silence) et garde https de `open_url`.
#![allow(dead_code)]

#[path = "../src/download.rs"]
mod download;
#[path = "../src/safe.rs"]
mod safe;
#[path = "../src/update.rs"]
mod update;
mod support;

use support::{Mode, Server, dead_base};

fn client() -> reqwest::blocking::Client {
    download::http_client().unwrap()
}

fn expected_url(w: &str, l: &str) -> String {
    if cfg!(windows) { w } else { l }.to_string()
}

#[test]
fn newer_version_proposes_update_for_this_os() {
    let srv = Server::start();
    srv.put("/launcher.json", r#"{"version":"0.3.0","windows":"https://w.example/w.exe","linux":"https://l.example/l","pack":"1.2.0"}"#);
    let u = update::check(&client(), &srv.url("/launcher.json"), "0.1.0").expect("mise à jour attendue");
    assert_eq!(u.version, "0.3.0");
    assert_eq!(u.url, expected_url("https://w.example/w.exe", "https://l.example/l"));
}

#[test]
fn newer_patch_and_v_prefix_are_detected() {
    let srv = Server::start();
    srv.put("/launcher.json", r#"{"version":"v0.1.10","windows":"https://w","linux":"https://l"}"#);
    assert!(update::check(&client(), &srv.url("/launcher.json"), "0.1.9").is_some());
}

#[test]
fn equal_or_older_version_is_silent() {
    let srv = Server::start();
    srv.put("/launcher.json", r#"{"version":"0.1.0","windows":"https://w","linux":"https://l"}"#);
    assert!(update::check(&client(), &srv.url("/launcher.json"), "0.1.0").is_none());
    srv.put("/launcher.json", r#"{"version":"0.0.9","windows":"https://w","linux":"https://l"}"#);
    assert!(update::check(&client(), &srv.url("/launcher.json"), "0.1.0").is_none());
}

#[test]
fn newer_without_url_for_this_os_is_silent() {
    let srv = Server::start();
    let other = if cfg!(windows) { "linux" } else { "windows" };
    srv.put("/launcher.json", format!(r#"{{"version":"0.3.0","{other}":"https://x"}}"#));
    assert!(update::check(&client(), &srv.url("/launcher.json"), "0.1.0").is_none());
}

#[test]
fn invalid_json_is_silent() {
    let srv = Server::start();
    srv.put("/launcher.json", "{ceci n'est pas du json");
    assert!(update::check(&client(), &srv.url("/launcher.json"), "0.1.0").is_none());
}

#[test]
fn json_without_version_is_silent() {
    let srv = Server::start();
    srv.put("/launcher.json", r#"{"windows":"https://w"}"#);
    assert!(update::check(&client(), &srv.url("/launcher.json"), "0.1.0").is_none());
}

#[test]
fn http_error_is_silent() {
    let srv = Server::start();
    srv.put_mode("/launcher.json", "", Mode::Status(500));
    assert!(update::check(&client(), &srv.url("/launcher.json"), "0.1.0").is_none());
    srv.put_mode("/launcher.json", "", Mode::Status(404));
    assert!(update::check(&client(), &srv.url("/launcher.json"), "0.1.0").is_none());
}

#[test]
fn network_down_is_silent_without_panic() {
    let base = dead_base();
    assert!(update::check(&client(), &format!("{base}/launcher.json"), "0.1.0").is_none());
}

#[test]
fn unparseable_remote_version_is_silent() {
    let srv = Server::start();
    srv.put("/launcher.json", r#"{"version":"bientot","windows":"https://w","linux":"https://l"}"#);
    assert!(update::check(&client(), &srv.url("/launcher.json"), "0.1.0").is_none());
}

#[test]
fn open_url_refuses_non_https() {
    // Aucun appel avec une URL https : `open::that` lancerait un navigateur réel.
    for u in ["http://w.example/w.exe", "file:///C:/x.exe", "javascript:alert(1)", "HTTPS://w.example", "w.example/w.exe", ""] {
        let e = update::open_url(u).err().unwrap_or_else(|| panic!("URL acceptée : {u}"));
        assert_eq!(e.to_string(), "URL refusée (https uniquement)", "URL {u}");
    }
}
