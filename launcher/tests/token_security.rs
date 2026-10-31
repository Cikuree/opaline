//! Cas 2 : aucun jeton dans les erreurs, Debug, ligne de commande loggée ou config.json.
//! Jetons fictifs reconnaissables : tout ce qui contient `TOKEN-SECRET-123` est une fuite.
#![allow(dead_code)]

use opaline_core::{auth, config, fabric, launch, mojang};

mod support;

use auth::{Endpoints, Session};
use config::{Config, Dirs};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use support::{Mode, Server, dead_base};

const SECRET: &str = "TOKEN-SECRET-123";
const CID: &str = "client-id-fixture";

fn assert_no_secret(what: &str, s: &str) {
    assert!(!s.contains(SECRET), "fuite de jeton dans {what} : {s}");
}

fn fixtures() -> (Server, Endpoints) {
    let srv = Server::start();
    let ep = Endpoints::single_base(&srv.base);
    srv.put("/ms/devicecode", r#"{"user_code":"A","verification_uri":"https://x","device_code":"DEV-CODE-123","expires_in":900,"interval":1}"#);
    srv.put("/ms/token", r#"{"access_token":"MS-TOKEN-SECRET-123","refresh_token":"REFRESH-TOKEN-SECRET-123"}"#);
    srv.put("/xbl/user/authenticate", r#"{"Token":"XBL-TOKEN-SECRET-123","DisplayClaims":{"xui":[{"uhs":"UHS"}]}}"#);
    srv.put("/xsts/xsts/authorize", r#"{"Token":"XSTS-TOKEN-SECRET-123","DisplayClaims":{"xui":[{"uhs":"UHS"}]}}"#);
    srv.put("/mc/authentication/login_with_xbox", r#"{"access_token":"MC-TOKEN-SECRET-123"}"#);
    srv.put("/mc/entitlements/mcstore", r#"{"items":[{"name":"game_minecraft"}]}"#);
    srv.put("/mc/minecraft/profile", r#"{"id":"0123456789abcdef0123456789abcdef","name":"Steve"}"#);
    (srv, ep)
}

fn full_flow(ep: &Endpoints) -> anyhow::Result<Session> {
    let c = auth::http_client()?;
    let dc = auth::start_device_code(&c, ep, CID)?;
    let t = auth::poll_device_code(&c, ep, CID, &dc, &AtomicBool::new(false))?;
    auth::minecraft_login(&c, ep, &t.access_token)
}

/// Vérifie le message dans toutes ses formes (Display, Debug anyhow, chaîne complète).
fn check_error(what: &str, r: anyhow::Result<Session>) -> String {
    let e = match r {
        Ok(_) => panic!("{what} : succès inattendu"),
        Err(e) => e,
    };
    assert_no_secret(&format!("{what} (Display)"), &e.to_string());
    assert_no_secret(&format!("{what} (Debug)"), &format!("{e:?}"));
    assert_no_secret(&format!("{what} (alternate)"), &format!("{e:#}"));
    e.to_string()
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("crl-it-sec-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

#[test]
fn session_debug_hides_access_token() {
    let s = Session::microsoft("Steve".into(), "uuid".into(), "MC-TOKEN-SECRET-123".into());
    assert_no_secret("Session Debug", &format!("{s:?}"));
    assert!(format!("{s:?}").contains("masqué"));
    assert_no_secret("Session clone Debug", &format!("{:?}", s.clone()));
}

#[test]
fn error_bodies_with_tokens_are_not_echoed() {
    // Chaque cas renvoie des jetons dans le corps de l'erreur : le message ne doit en reprendre aucun.
    type Setup = Box<dyn Fn(&Server)>;
    let cases: Vec<(&str, Setup)> = vec![
        ("devicecode 400", Box::new(|s| s.put_mode("/ms/devicecode", r#"{"error":"invalid_client","device_code":"DEV-TOKEN-SECRET-123"}"#, Mode::Reply(400)))),
        ("XBL 500", Box::new(|s| s.put_mode("/xbl/user/authenticate", r#"{"Token":"XBL-TOKEN-SECRET-123"}"#, Mode::Reply(500)))),
        ("XSTS 401 sans XErr", Box::new(|s| s.put_mode("/xsts/xsts/authorize", r#"{"Token":"XSTS-TOKEN-SECRET-123"}"#, Mode::Reply(401)))),
        ("XSTS 401 XErr inconnu", Box::new(|s| s.put_mode("/xsts/xsts/authorize", r#"{"XErr":9999,"Token":"XSTS-TOKEN-SECRET-123"}"#, Mode::Reply(401)))),
        ("XSTS 401 XErr connu", Box::new(|s| s.put_mode("/xsts/xsts/authorize", r#"{"XErr":2148916233,"Token":"XSTS-TOKEN-SECRET-123"}"#, Mode::Reply(401)))),
        ("login_with_xbox 500", Box::new(|s| s.put_mode("/mc/authentication/login_with_xbox", r#"{"access_token":"MC-TOKEN-SECRET-123"}"#, Mode::Reply(500)))),
        ("login_with_xbox 403", Box::new(|s| s.put_mode("/mc/authentication/login_with_xbox", r#"{"access_token":"MC-TOKEN-SECRET-123"}"#, Mode::Reply(403)))),
        ("entitlements 500", Box::new(|s| s.put_mode("/mc/entitlements/mcstore", r#"{"access_token":"MC-TOKEN-SECRET-123"}"#, Mode::Reply(500)))),
        ("profil 500", Box::new(|s| s.put_mode("/mc/minecraft/profile", r#"{"access_token":"MC-TOKEN-SECRET-123"}"#, Mode::Reply(500)))),
        ("login sans access_token", Box::new(|s| s.put("/mc/authentication/login_with_xbox", r#"{"refresh_token":"REFRESH-TOKEN-SECRET-123"}"#))),
        ("profil sans nom", Box::new(|s| s.put("/mc/minecraft/profile", r#"{"id":"x","access_token":"MC-TOKEN-SECRET-123"}"#))),
        ("XBL sans uhs", Box::new(|s| s.put("/xbl/user/authenticate", r#"{"Token":"XBL-TOKEN-SECRET-123"}"#))),
        ("token 200 sans access_token", Box::new(|s| s.put("/ms/token", r#"{"refresh_token":"REFRESH-TOKEN-SECRET-123"}"#))),
        ("token JSON tronqué", Box::new(|s| s.put("/ms/token", r#"{"access_token":"MS-TOKEN-SECRET-123", "#))),
        ("token non JSON", Box::new(|s| s.put("/ms/token", "MS-TOKEN-SECRET-123"))),
        (
            "token refusé avec jetons",
            Box::new(|s| s.put_mode("/ms/token", r#"{"error":"authorization_declined","access_token":"MS-TOKEN-SECRET-123"}"#, Mode::Reply(400))),
        ),
    ];
    for (what, setup) in cases {
        let (srv, ep) = fixtures();
        setup(&srv);
        let msg = check_error(what, full_flow(&ep));
        assert!(!msg.is_empty(), "{what} : message vide");
    }
}

#[test]
fn refresh_errors_do_not_echo_refresh_token() {
    let (srv, ep) = fixtures();
    srv.put_mode("/ms/token", r#"{"error":"invalid_request","refresh_token":"REFRESH-TOKEN-SECRET-123"}"#, Mode::Reply(400));
    let c = auth::http_client().unwrap();
    let e = auth::refresh(&c, &ep, CID, "REFRESH-TOKEN-SECRET-123").err().expect("erreur");
    assert_no_secret("refresh Display", &e.to_string());
    assert_no_secret("refresh Debug", &format!("{e:?}"));
}

#[test]
fn network_error_message_has_no_token_or_url() {
    let ep = Endpoints::single_base(&dead_base());
    let c = auth::http_client().unwrap();
    let e = auth::refresh(&c, &ep, CID, "REFRESH-TOKEN-SECRET-123").err().expect("erreur");
    assert_no_secret("erreur réseau", &format!("{e:?}"));
    assert!(!format!("{e:#}").contains("127.0.0.1"), "URL présente dans l'erreur réseau : {e:#}");
}

#[test]
fn successful_login_then_config_json_has_no_secret() {
    let (_srv, ep) = fixtures();
    let s = full_flow(&ep).expect("flux complet");
    assert_no_secret("Session après login", &format!("{s:?}"));
    let dirs = Dirs::new(tmp("config"));
    Config::default().save(&dirs).unwrap();
    let text = std::fs::read_to_string(dirs.config_file()).unwrap();
    assert_no_secret("config.json", &text);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    let mut keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(keys, ["mc_name", "mc_uuid", "memory_mb", "nickname", "no_remember", "perf_mode"]);
    let _ = std::fs::remove_dir_all(&dirs.root);
}

fn version_json() -> mojang::VersionJson {
    serde_json::from_str(
        r#"{"id":"26.3","mainClass":"net.minecraft.client.main.Main",
        "arguments":{"jvm":["-cp","${classpath}"],
          "game":["--username","${auth_player_name}","--uuid","${auth_uuid}","--accessToken","${auth_access_token}",
            "--assetIndex","${assets_index_name}",
            {"rules":[{"action":"allow","features":{"is_quick_play_multiplayer":true}}],"value":["--quickPlayMultiplayer","${quickPlayMultiplayer}"]}]},
        "assetIndex":{"id":"34","sha1":"a","url":"u"},
        "downloads":{"client":{"sha1":"a","url":"u"}},"javaVersion":{"majorVersion":25},"libraries":[]}"#,
    )
    .unwrap()
}

fn fabric_profile() -> fabric::FabricProfile {
    serde_json::from_str(r#"{"mainClass":"net.fabricmc.loader.impl.launch.knot.KnotClient","arguments":{"game":[],"jvm":[]},"libraries":[]}"#).unwrap()
}

fn command_for(session: &Session, extra_jvm: Option<&str>) -> anyhow::Result<Vec<String>> {
    let (v, mut f) = (version_json(), fabric_profile());
    if let Some(x) = extra_jvm {
        f.arguments.jvm.push(mojang::Arg::Plain(x.into()));
    }
    let dirs = Dirs::new(tmp("cmd"));
    let env = mojang::Env::current(&[]);
    launch::build_command(&launch::LaunchInput { java: Path::new("java"), version: &v, fabric: &f, dirs: &dirs, session, memory_mb: 2048, env: &env })
}

#[test]
fn command_line_is_redacted_for_logs() {
    let s = Session::microsoft("Steve".into(), "uuid".into(), "MC-TOKEN-SECRET-123".into());
    let cmd = command_for(&s, Some("-Dtoken=MC-TOKEN-SECRET-123")).unwrap();
    // la commande réelle contient bien le jeton (sinon le test ne prouve rien)
    assert!(cmd.iter().any(|a| a.contains(SECRET)));
    let logged = launch::redact(&cmd, s.access_token(), Some("home-server:25565"));
    assert_no_secret("ligne de commande loggée", &logged);
    assert!(logged.contains("--accessToken <masqué>"));
    assert!(!cmd.iter().any(|a| a.contains("quickPlay")), "aucune connexion automatique");
    let game_line = launch::redact_text("Connecting to play.exemple.fr, 25565", s.access_token(), Some("play.exemple.fr:25565"));
    assert!(game_line.contains("<serveur>") && !game_line.contains("play.exemple.fr"));
}

#[test]
fn command_line_redaction_keeps_offline_token_readable_but_masks_flag_value() {
    // Hors ligne : jeton factice "0" (un caractère) ; la valeur suivant --accessToken reste masquée.
    let s = Session::offline("Bob");
    let cmd = command_for(&s, None).unwrap();
    let logged = launch::redact(&cmd, s.access_token(), None);
    assert!(logged.contains("--accessToken <masqué>"));
}

#[test]
fn bug_leftover_variable_error_echoes_token() {
    let s = Session::microsoft("Steve".into(), "uuid".into(), "MC-TOKEN-SECRET-123".into());
    let mut v = version_json();
    v.arguments.game.push(mojang::Arg::Plain("--session=${auth_access_token}${inconnue}".into()));
    let f = fabric_profile();
    let dirs = Dirs::new(tmp("leftover"));
    let env = mojang::Env::current(&[]);
    let e = launch::build_command(&launch::LaunchInput {
        java: Path::new("java"),
        version: &v,
        fabric: &f,
        dirs: &dirs,
        session: &s,
        memory_mb: 2048,
        env: &env,
    })
    .expect_err("variable non substituée doit échouer");
    assert_no_secret("erreur build_command", &format!("{e:#}"));
}
