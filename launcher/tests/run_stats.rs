//! `stats::fetch_value` contre un serveur de fixtures : JSON, 404 -> None, erreur serveur, serveur absent.
mod support;

use opaline_core::stats;
use support::{Mode, Server, dead_base};

fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::new()
}

#[test]
fn stats_history_and_404() {
    let srv = Server::start();
    srv.put("/stats", r#"{"runNumber":3,"running":true}"#);
    srv.put("/history", r#"[{"run":1}]"#);
    srv.put_mode("/history/9", "", Mode::Status(404));
    srv.put_mode("/boom", "", Mode::Status(500));
    let v = stats::fetch_value(&client(), &srv.url("/stats")).unwrap().unwrap();
    assert_eq!(v["runNumber"], 3);
    assert!(stats::fetch_value(&client(), &srv.url("/history")).unwrap().unwrap().is_array());
    assert!(stats::fetch_value(&client(), &srv.url("/history/9")).unwrap().is_none());
    assert!(stats::fetch_value(&client(), &srv.url("/boom")).is_err());
    let e = stats::fetch_value(&client(), &format!("{}/stats", dead_base())).unwrap_err().to_string();
    assert!(!e.contains("127.0.0.1"), "l'adresse ne doit pas fuiter : {e}");
}
