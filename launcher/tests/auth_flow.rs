//! Cas 1 : flux Microsoft complet (device code -> XBL -> XSTS -> login_with_xbox -> licence -> profil)
//! contre le serveur de fixtures. Aucun réseau réel. Le trousseau n'est jamais appelé : `login_interactive`
//! et `silent_login` écrivent/lisent le keyring système, donc les tests composent les étapes pures et réseau.
#![allow(dead_code)]

use opaline_core::auth;

mod support;

use auth::{Endpoints, Session};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use support::{Mode, Server, dead_base};

const CID: &str = "client-id-fixture";
const DEVICE: &str =
    r#"{"user_code":"ABCD-1234","verification_uri":"https://www.microsoft.com/link","device_code":"DEV-CODE-123","expires_in":900,"interval":1}"#;
const TOKEN_OK: &str = r#"{"token_type":"Bearer","access_token":"MS-TOKEN-SECRET-123","refresh_token":"REFRESH-TOKEN-SECRET-123"}"#;
const XBL_OK: &str = r#"{"Token":"XBL-TOKEN-SECRET-123","DisplayClaims":{"xui":[{"uhs":"UHS123"}]}}"#;
const XSTS_OK: &str = r#"{"Token":"XSTS-TOKEN-SECRET-123","DisplayClaims":{"xui":[{"uhs":"UHS123"}]}}"#;
const MC_OK: &str = r#"{"username":"steve","access_token":"MC-TOKEN-SECRET-123","token_type":"Bearer"}"#;
const ENT_OK: &str = r#"{"items":[{"name":"product_minecraft","signature":"x"},{"name":"game_minecraft","signature":"y"}]}"#;
const PROFILE_OK: &str = r#"{"id":"0123456789abcdef0123456789abcdef","name":"Steve"}"#;
const NO_LICENSE: &str = "Ce compte Microsoft ne possède pas Minecraft : Java Edition.";
const NOT_APPROVED: &str = "Application Azure non encore approuvée par Mojang (403 sur login_with_xbox).";

fn client() -> reqwest::blocking::Client {
    auth::http_client().unwrap()
}

/// Serveur avec le chemin nominal complet ; les tests remplacent ce qu'ils veulent tester.
fn fixtures() -> (Server, Endpoints) {
    let srv = Server::start();
    let ep = Endpoints::single_base(&srv.base);
    srv.put("/ms/devicecode", DEVICE);
    srv.put("/ms/token", TOKEN_OK);
    srv.put("/xbl/user/authenticate", XBL_OK);
    srv.put("/xsts/xsts/authorize", XSTS_OK);
    srv.put("/mc/authentication/login_with_xbox", MC_OK);
    srv.put("/mc/entitlements/mcstore", ENT_OK);
    srv.put("/mc/minecraft/profile", PROFILE_OK);
    (srv, ep)
}

/// Enchaîne les étapes réseau (sans trousseau) comme `login_interactive`.
fn full_flow(ep: &Endpoints) -> anyhow::Result<Session> {
    let c = client();
    let dc = auth::start_device_code(&c, ep, CID)?;
    let t = auth::poll_device_code(&c, ep, CID, &dc, &AtomicBool::new(false))?;
    auth::minecraft_login(&c, ep, &t.access_token)
}

fn err_of(r: anyhow::Result<Session>) -> String {
    match r {
        Ok(s) => panic!("succès inattendu pour {:?}", s),
        Err(e) => e.to_string(),
    }
}

#[test]
fn full_flow_success_after_authorization_pending() {
    let (srv, ep) = fixtures();
    srv.queue("/ms/token", &[(400, r#"{"error":"authorization_pending"}"#)]);
    let s = full_flow(&ep).expect("flux complet");
    assert_eq!((s.username.as_str(), s.uuid.as_str(), s.user_type), ("Steve", "0123456789abcdef0123456789abcdef", "msa"));
    assert_eq!(s.access_token(), "MC-TOKEN-SECRET-123");
    // deux interrogations du jeton : pending puis succès
    assert_eq!(srv.count("/ms/token"), 2);
    let polls = srv.requests().into_iter().filter(|r| r.path == "/ms/token").map(|r| r.body_text()).collect::<Vec<_>>();
    assert!(
        polls.iter().all(|b| { b.contains("grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code") && b.contains("device_code=DEV-CODE-123") })
    );
    // la chaîne transmet bien chaque jeton au service suivant
    let xbl = srv.requests().into_iter().find(|r| r.path == "/xbl/user/authenticate").unwrap();
    assert!(xbl.body_text().contains("d=MS-TOKEN-SECRET-123"));
    let xsts = srv.requests().into_iter().find(|r| r.path == "/xsts/xsts/authorize").unwrap();
    assert!(xsts.body_text().contains("XBL-TOKEN-SECRET-123"));
    let ent = srv.requests().into_iter().find(|r| r.path == "/mc/entitlements/mcstore").unwrap();
    assert_eq!(ent.auth.as_deref(), Some("Bearer MC-TOKEN-SECRET-123"));
}

#[test]
fn device_code_request_carries_client_and_scope() {
    let (srv, ep) = fixtures();
    full_flow(&ep).unwrap();
    let dc = srv.requests().into_iter().find(|r| r.path == "/ms/devicecode").unwrap();
    assert_eq!(dc.method, "POST");
    assert!(dc.body_text().contains("client_id=client-id-fixture") && dc.body_text().contains("scope=XboxLive.signin+offline_access"));
}

#[test]
fn slow_down_raises_interval_then_succeeds() {
    let (srv, ep) = fixtures();
    srv.queue("/ms/token", &[(400, r#"{"error":"slow_down"}"#)]);
    let t0 = Instant::now();
    full_flow(&ep).expect("flux complet après slow_down");
    // intervalle 1 s, puis +5 s après slow_down : la 2e interrogation attend au moins 6 s
    assert!(t0.elapsed() >= Duration::from_secs(6), "intervalle non augmenté : {:?}", t0.elapsed());
    assert_eq!(srv.count("/ms/token"), 2);
}

#[test]
fn expired_token_from_server_asks_to_restart() {
    let (srv, ep) = fixtures();
    srv.queue("/ms/token", &[(400, r#"{"error":"expired_token"}"#)]);
    assert_eq!(err_of(full_flow(&ep)), "code expiré : relancez la connexion");
}

#[test]
fn deadline_without_answer_expires_locally() {
    let (srv, ep) = fixtures();
    srv.put_mode("/ms/token", r#"{"error":"authorization_pending"}"#, Mode::Reply(400));
    let c = client();
    let dc =
        auth::parse_device_code(&serde_json::json!({"user_code": "A", "verification_uri": "https://x", "device_code": "D", "expires_in": 1, "interval": 1}))
            .unwrap();
    let t0 = Instant::now();
    let e = auth::poll_device_code(&c, &ep, CID, &dc, &AtomicBool::new(false)).err().expect("doit expirer").to_string();
    assert_eq!(e, "code expiré : relancez la connexion");
    assert!(t0.elapsed() < Duration::from_secs(4));
}

#[test]
fn declined_and_cancelled_are_reported() {
    let (srv, ep) = fixtures();
    srv.queue("/ms/token", &[(400, r#"{"error":"authorization_declined"}"#)]);
    let c = client();
    let dc = auth::start_device_code(&c, &ep, CID).unwrap();
    let e = auth::poll_device_code(&c, &ep, CID, &dc, &AtomicBool::new(false)).err().unwrap().to_string();
    assert_eq!(e, "connexion refusée sur la page Microsoft");
    // annulation : le drapeau est pris en compte pendant l'attente, sans requête
    let (srv2, ep2) = fixtures();
    let cancel = AtomicBool::new(true);
    let e = auth::poll_device_code(&c, &ep2, CID, &dc, &cancel).err().unwrap().to_string();
    assert_eq!(e, "connexion annulée");
    assert_eq!(srv2.count("/ms/token"), 0);
    drop(srv);
}

#[test]
fn device_code_refused_mentions_client_id() {
    let (srv, ep) = fixtures();
    srv.put_mode("/ms/devicecode", r#"{"error":"invalid_client"}"#, Mode::Reply(400));
    assert_eq!(err_of(full_flow(&ep)), "demande de code refusée (HTTP 400, invalid_client) : vérifiez l'ID client de l'app Azure");
}

#[test]
fn xsts_2148916233_no_xbox_account() {
    let (srv, ep) = fixtures();
    srv.put_mode(
        "/xsts/xsts/authorize",
        r#"{"XErr":2148916233,"Identity":"0","Message":"","Redirect":"https://start.ui.xboxlive.com/AddAccount"}"#,
        Mode::Reply(401),
    );
    assert_eq!(err_of(full_flow(&ep)), "Ce compte Microsoft n'a pas de compte Xbox : créez-en un sur xbox.com puis réessayez.");
    assert_eq!(srv.count("/mc/authentication/login_with_xbox"), 0, "pas d'appel login_with_xbox après échec XSTS");
}

#[test]
fn xsts_2148916238_child_account() {
    let (srv, ep) = fixtures();
    srv.put_mode("/xsts/xsts/authorize", r#"{"XErr":2148916238,"Identity":"0"}"#, Mode::Reply(401));
    assert_eq!(err_of(full_flow(&ep)), "Compte enfant : il doit être rattaché à une famille Microsoft (autorisation d'un parent) pour jouer.");
}

#[test]
fn empty_entitlements_means_no_license() {
    let (srv, ep) = fixtures();
    srv.put("/mc/entitlements/mcstore", r#"{"items":[]}"#);
    assert_eq!(err_of(full_flow(&ep)), NO_LICENSE);
    assert_eq!(srv.count("/mc/minecraft/profile"), 0, "pas de profil sans licence");
}

#[test]
fn bedrock_only_entitlement_is_not_java_license() {
    let (srv, ep) = fixtures();
    srv.put("/mc/entitlements/mcstore", r#"{"items":[{"name":"product_minecraft_bedrock"}]}"#);
    assert_eq!(err_of(full_flow(&ep)), NO_LICENSE);
}

#[test]
fn profile_404_means_no_license() {
    let (srv, ep) = fixtures();
    srv.put_mode("/mc/minecraft/profile", r#"{"error":"NOT_FOUND"}"#, Mode::Reply(404));
    assert_eq!(err_of(full_flow(&ep)), NO_LICENSE);
}

#[test]
fn login_with_xbox_403_means_app_not_approved() {
    let (srv, ep) = fixtures();
    srv.put_mode("/mc/authentication/login_with_xbox", r#"{"path":"/authentication/login_with_xbox","errorMessage":"Forbidden"}"#, Mode::Reply(403));
    assert_eq!(err_of(full_flow(&ep)), NOT_APPROVED);
    assert_eq!(srv.count("/mc/entitlements/mcstore"), 0, "pas de licence vérifiée si login refusé");
}

#[test]
fn other_http_errors_name_the_step() {
    let (srv, ep) = fixtures();
    srv.put_mode("/xbl/user/authenticate", "{}", Mode::Reply(500));
    assert_eq!(err_of(full_flow(&ep)), "Xbox Live (authentification) : HTTP 500");

    let (srv, ep) = fixtures();
    srv.put_mode("/mc/authentication/login_with_xbox", "{}", Mode::Reply(500));
    assert_eq!(err_of(full_flow(&ep)), "login_with_xbox : HTTP 500");

    let (srv, ep) = fixtures();
    srv.put_mode("/mc/entitlements/mcstore", "{}", Mode::Reply(503));
    assert_eq!(err_of(full_flow(&ep)), "vérification de la licence : HTTP 503");
    drop(srv);
}

#[test]
fn network_down_is_a_clean_error() {
    let ep = Endpoints::single_base(&dead_base());
    let e = err_of(full_flow(&ep));
    assert!(e.starts_with("erreur réseau"), "message : {e}");
}

#[test]
fn refresh_invalid_grant_asks_to_reconnect() {
    let (srv, ep) = fixtures();
    srv.put_mode("/ms/token", r#"{"error":"invalid_grant","error_description":"AADSTS70008"}"#, Mode::Reply(400));
    let r = auth::refresh(&client(), &ep, CID, "REFRESH-TOKEN-SECRET-123").unwrap();
    assert!(r.is_none(), "invalid_grant doit donner None (reconnexion)");
}

#[test]
fn refresh_interaction_required_asks_to_reconnect() {
    let (srv, ep) = fixtures();
    srv.put_mode("/ms/token", r#"{"error":"interaction_required"}"#, Mode::Reply(400));
    assert!(auth::refresh(&client(), &ep, CID, "r").unwrap().is_none());
}

#[test]
fn refresh_success_returns_new_tokens() {
    let (_srv, ep) = fixtures();
    let t = auth::refresh(&client(), &ep, CID, "old").unwrap().expect("jetons");
    assert_eq!(t.access_token, "MS-TOKEN-SECRET-123");
    assert_eq!(t.refresh_token.as_deref(), Some("REFRESH-TOKEN-SECRET-123"));
}

#[test]
fn refresh_other_error_is_an_error_not_a_reconnect() {
    let (srv, ep) = fixtures();
    srv.put_mode("/ms/token", r#"{"error":"invalid_request"}"#, Mode::Reply(400));
    let e = auth::refresh(&client(), &ep, CID, "r").err().expect("erreur").to_string();
    assert_eq!(e, "rafraîchissement refusé : invalid_request");
}
