//! Connexion par le navigateur : vrai mini-serveur sur 127.0.0.1 avec un client TCP local, puis flux complet contre
//! le serveur de fixtures. Aucun accès à Microsoft ni au trousseau (`remember = false`).
use opaline_core::auth::{self, AuthManager, Endpoints};
use opaline_core::auth_browser as b;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

mod support;
use support::Server;

fn http(port: u16, raw: &str) -> String {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.write_all(raw.as_bytes()).unwrap();
    let mut out = String::new();
    let _ = s.read_to_string(&mut out);
    out
}

fn port_of(uri: &str) -> u16 {
    uri.rsplit(':').next().unwrap().parse().unwrap()
}

fn serve(state: &'static str, timeout: Duration) -> (u16, Arc<AtomicBool>, std::thread::JoinHandle<anyhow::Result<b::Redirect>>) {
    let (l, uri) = b::bind_loopback().unwrap();
    assert!(uri.starts_with("http://localhost:"));
    assert!(l.local_addr().unwrap().ip().is_loopback());
    let cancel = Arc::new(AtomicBool::new(false));
    let c2 = cancel.clone();
    (port_of(&uri), cancel, std::thread::spawn(move || b::wait_for_redirect(l, state, &c2, timeout)))
}

#[test]
fn server_rejects_noise_then_accepts_one_valid_request_and_stops() {
    let (port, _c, h) = serve("ST", Duration::from_secs(10));
    assert!(http(port, "POST /?code=x&state=ST HTTP/1.1\r\n\r\n").starts_with("HTTP/1.1 405"));
    assert!(http(port, "GET /favicon.ico HTTP/1.1\r\n\r\n").starts_with("HTTP/1.1 404"));
    assert!(http(port, "garbage\r\n\r\n").starts_with("HTTP/1.1 400"));
    assert!(http(port, "GET /?code=x&state=WRONG HTTP/1.1\r\n\r\n").starts_with("HTTP/1.1 400"));
    let ok = http(port, "GET /?code=SECRET-CODE&state=ST&x=<script>alert(1)</script> HTTP/1.1\r\nHost: localhost\r\n\r\n");
    assert!(ok.starts_with("HTTP/1.1 200") && ok.contains("Connexion réussie") && ok.contains("Tu peux revenir sur Opaline"));
    assert!(!ok.contains("SECRET-CODE") && !ok.contains("alert") && !ok.to_lowercase().contains("<script"));
    assert!(ok.contains("Content-Security-Policy: default-src 'none'"));
    assert!(matches!(h.join().unwrap().unwrap(), b::Redirect::Code(c) if c == "SECRET-CODE"));
    // le serveur est arrêté : seconde requête impossible
    std::thread::sleep(Duration::from_millis(100));
    assert!(TcpStream::connect(("127.0.0.1", port)).is_err());
}

#[test]
fn denied_page_and_error_code() {
    let (port, _c, h) = serve("ST", Duration::from_secs(10));
    let r = http(port, "GET /?error=access_denied&error_description=secret%20detail&state=ST HTTP/1.1\r\n\r\n");
    assert!(r.contains("Connexion refusée") && !r.contains("secret"));
    assert!(matches!(h.join().unwrap().unwrap(), b::Redirect::Denied(e) if e == "access_denied"));
}

#[test]
fn timeout_and_cancel() {
    let (_p, _c, h) = serve("ST", Duration::from_millis(200));
    assert!(h.join().unwrap().err().unwrap().to_string().contains("délai dépassé"));
    let (_p, cancel, h) = serve("ST", Duration::from_secs(30));
    cancel.store(true, Ordering::SeqCst);
    assert!(h.join().unwrap().err().unwrap().to_string().contains("annulée"));
}

fn fixtures() -> (Server, Endpoints) {
    let srv = Server::start();
    let ep = Endpoints::single_base(&srv.base);
    srv.put("/ms/token", r#"{"access_token":"MS-TOKEN-SECRET","refresh_token":"REFRESH-SECRET"}"#);
    srv.put("/xbl/user/authenticate", r#"{"Token":"XBL","DisplayClaims":{"xui":[{"uhs":"UHS"}]}}"#);
    srv.put("/xsts/xsts/authorize", r#"{"Token":"XSTS","DisplayClaims":{"xui":[{"uhs":"UHS"}]}}"#);
    srv.put("/mc/authentication/login_with_xbox", r#"{"access_token":"MC-TOKEN-SECRET"}"#);
    srv.put("/mc/entitlements/mcstore", r#"{"items":[{"name":"game_minecraft"}]}"#);
    srv.put("/mc/minecraft/profile", r#"{"id":"0123456789abcdef0123456789abcdef","name":"Steve"}"#);
    (srv, ep)
}

/// Joue le rôle du navigateur : lit l'URL d'autorisation et appelle la redirection avec `query` (`{state}` remplacé).
fn browser(url: &str, query: &str) {
    let u = reqwest::Url::parse(url).unwrap();
    let q: std::collections::HashMap<_, _> = u.query_pairs().into_owned().collect();
    assert_eq!(q["code_challenge_method"], "S256");
    assert_eq!(q["prompt"], "select_account");
    let port = port_of(&q["redirect_uri"]);
    let req = format!("GET /?{} HTTP/1.1\r\n\r\n", query.replace("{state}", &q["state"]));
    std::thread::spawn(move || {
        http(port, &req);
    });
}

#[test]
fn full_browser_flow_sends_verifier_matching_challenge() {
    let (srv, ep) = fixtures();
    let c = auth::http_client().unwrap();
    let mut challenge = String::new();
    let s = AuthManager::default()
        .login_browser(
            &c,
            &ep,
            "cid",
            false,
            |u| {
                challenge = reqwest::Url::parse(u).unwrap().query_pairs().find(|(k, _)| k == "code_challenge").unwrap().1.into_owned();
                browser(u, "code=AUTH-CODE&state={state}");
            },
            &AtomicBool::new(false),
            Duration::from_secs(10),
        )
        .unwrap();
    assert_eq!(s.username, "Steve");
    let req = srv.requests().into_iter().find(|r| r.path == "/ms/token").unwrap().body_text();
    assert!(req.contains("grant_type=authorization_code") && req.contains("code=AUTH-CODE") && req.contains("client_id=cid"));
    let verifier = req.split('&').find_map(|p| p.strip_prefix("code_verifier=")).unwrap();
    assert_eq!(b::pkce_challenge(verifier), challenge);
}

#[test]
fn denied_and_failed_flows_leak_nothing() {
    let (srv, ep) = fixtures();
    let c = auth::http_client().unwrap();
    let m = AuthManager::default();
    let err = |q: &'static str| {
        m.login_browser(&c, &ep, "cid", false, |u| browser(u, q), &AtomicBool::new(false), Duration::from_secs(10)).err().unwrap().to_string()
    };
    assert_eq!(err("error=access_denied&error_description=PRIVATE&state={state}"), "connexion refusée sur la page Microsoft");
    assert!(!err("error=server_error&error_description=PRIVATE&state={state}").contains("PRIVATE"));
    srv.put("/ms/token", r#"{"error":"invalid_grant","error_description":"PRIVATE"}"#);
    srv.queue("/ms/token", &[(400, r#"{"error":"invalid_grant"}"#)]);
    let e = err("code=LEAKY-CODE&state={state}");
    assert!(e.contains("invalid_grant") && !e.contains("LEAKY-CODE") && !e.contains("PRIVATE"));
}
