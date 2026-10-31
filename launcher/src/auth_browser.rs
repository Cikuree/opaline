//! Connexion Microsoft par le navigateur : code d'autorisation + PKCE (S256), client public, redirection loopback.
//!
//! Microsoft identity platform (apps de bureau, RFC 8252 §7.3) : l'URI enregistrée `http://localhost` accepte n'importe
//! quel port à l'exécution (le port est ignoré pour la comparaison des URI loopback `localhost`). On écoute donc sur un
//! port éphémère, uniquement sur 127.0.0.1. Aucun code, jeton ni paramètre reçu n'apparaît dans les erreurs ou pages.
use anyhow::{Context, Result, anyhow, bail};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use reqwest::Url;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Délai maximal accordé à l'utilisateur pour finir la connexion dans le navigateur.
pub const BROWSER_TIMEOUT: Duration = Duration::from_secs(300);

fn random_b64(bytes: usize) -> Result<String> {
    let mut b = vec![0u8; bytes];
    getrandom::fill(&mut b).map_err(|_| anyhow!("générateur aléatoire du système indisponible"))?;
    Ok(URL_SAFE_NO_PAD.encode(b))
}

/// Verifier PKCE : 32 octets aléatoires, 43 caractères (RFC 7636 §4.1).
pub fn new_verifier() -> Result<String> {
    random_b64(32)
}

pub fn new_state() -> Result<String> {
    random_b64(16)
}

/// `code_challenge` S256 = base64url(SHA-256(verifier)), sans padding.
pub fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

pub fn authorize_url(authorize_endpoint: &str, client_id: &str, redirect_uri: &str, state: &str, challenge: &str) -> Result<String> {
    let params = [
        ("client_id", client_id),
        ("response_type", "code"),
        ("redirect_uri", redirect_uri),
        ("response_mode", "query"),
        ("scope", crate::auth::SCOPE),
        ("state", state),
        ("code_challenge", challenge),
        ("code_challenge_method", "S256"),
        ("prompt", "select_account"),
    ];
    Ok(Url::parse_with_params(authorize_endpoint, params).context("URL d'autorisation invalide")?.into())
}

pub fn code_form(client_id: &str, code: &str, redirect_uri: &str, verifier: &str) -> Vec<(&'static str, String)> {
    vec![
        ("grant_type", "authorization_code".into()),
        ("client_id", client_id.into()),
        ("code", code.into()),
        ("redirect_uri", redirect_uri.into()),
        ("code_verifier", verifier.into()),
        ("scope", crate::auth::SCOPE.into()),
    ]
}

/// Contenu utile d'une redirection valide. Pas de `Debug` : `Code` est un secret.
#[derive(PartialEq, Eq)]
pub enum Redirect {
    Code(String),
    /// Code `error` OAuth (borné aux caractères sûrs ; `error_description` est ignoré).
    Denied(String),
}

/// Pourquoi une requête est rejetée (la boucle continue d'écouter).
#[derive(Debug, PartialEq, Eq)]
pub enum Reject {
    Malformed,
    Method,
    NotFound,
    BadState,
}

fn safe_error_code(s: &str) -> String {
    if !s.is_empty() && s.len() <= 40 && s.bytes().all(|b| b.is_ascii_lowercase() || b == b'_') { s.to_string() } else { "unknown_error".into() }
}

/// Analyse l'en-tête d'une requête HTTP reçue sur la redirection. Valide : `GET /?...` avec le bon `state`
/// et soit `code`, soit `error`.
pub fn parse_redirect_request(head: &str, expected_state: &str) -> Result<Redirect, Reject> {
    let line = head.lines().next().ok_or(Reject::Malformed)?;
    let mut parts = line.split(' ');
    let (Some(method), Some(target), Some(version), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else {
        return Err(Reject::Malformed);
    };
    if !version.starts_with("HTTP/1.") || !target.starts_with('/') {
        return Err(Reject::Malformed);
    }
    if method != "GET" {
        return Err(Reject::Method);
    }
    let url = Url::parse(&format!("http://localhost{target}")).map_err(|_| Reject::Malformed)?;
    if url.path() != "/" {
        return Err(Reject::NotFound);
    }
    let get = |k: &str| url.query_pairs().find(|(n, _)| n == k).map(|(_, v)| v.into_owned());
    if get("state").as_deref() != Some(expected_state) {
        return Err(Reject::BadState);
    }
    if let Some(e) = get("error") {
        return Ok(Redirect::Denied(safe_error_code(&e)));
    }
    match get("code") {
        Some(c) if !c.is_empty() => Ok(Redirect::Code(c)),
        _ => Err(Reject::Malformed),
    }
}

const PAGE_STYLE: &str = "body{margin:0;min-height:100vh;display:grid;place-items:center;background:#17131f;color:#f3eefb;font-family:system-ui,sans-serif;text-align:center}\
main{max-width:30em;padding:2em}h1{font-size:1.6em;margin:.2em 0}p{color:#b9aecb}.o{color:#c9a7ff;font-size:2.4em}";

fn page(title: &str, text: &str) -> String {
    format!(
        "<!doctype html><html lang=\"fr\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\">\
         <title>Opaline</title><style>{PAGE_STYLE}</style></head><body><main><div class=\"o\">&#9675;</div><h1>{title}</h1><p>{text}</p></main></body></html>"
    )
}

fn respond(mut s: TcpStream, status: &str, body: &str) {
    let msg = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\n\
         X-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'none'; style-src 'unsafe-inline'\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = s.write_all(msg.as_bytes());
    let _ = s.flush();
}

/// Lit l'en-tête de requête (8 Kio max, 2 s max). Pas de corps lu.
fn read_head(s: &mut TcpStream) -> Option<String> {
    s.set_nonblocking(false).ok()?;
    s.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    while buf.len() < 8192 && !buf.windows(4).any(|w| w == b"\r\n\r\n") {
        match s.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    }
    String::from_utf8(buf).ok()
}

/// Écoute sur 127.0.0.1, port éphémère. Retourne l'écouteur et l'URI de redirection `http://localhost:<port>`.
pub fn bind_loopback() -> Result<(TcpListener, String)> {
    let l = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).map_err(|_| anyhow!("impossible d'ouvrir le port local de redirection"))?;
    l.set_nonblocking(true)?;
    let port = l.local_addr()?.port();
    Ok((l, format!("http://localhost:{port}")))
}

/// Attend UNE requête valide ; les autres reçoivent une erreur HTTP sans corps et l'attente continue.
/// Le serveur s'arrête (l'écouteur est consommé) après la réponse.
pub fn wait_for_redirect(listener: TcpListener, state: &str, cancel: &AtomicBool, timeout: Duration) -> Result<Redirect> {
    let deadline = Instant::now() + timeout;
    loop {
        if cancel.load(Ordering::Relaxed) {
            bail!("connexion annulée");
        }
        if Instant::now() > deadline {
            bail!("délai dépassé : relancez la connexion");
        }
        let mut s = match listener.accept() {
            Ok((s, _)) => s,
            Err(_) => {
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }
        };
        let parsed = match read_head(&mut s) {
            Some(h) => parse_redirect_request(&h, state),
            None => Err(Reject::Malformed),
        };
        match parsed {
            Ok(r) => {
                let body = match &r {
                    Redirect::Code(_) => page("Connexion réussie", "Tu peux revenir sur Opaline."),
                    Redirect::Denied(_) => page("Connexion refusée", "Tu peux fermer cet onglet et réessayer depuis Opaline."),
                };
                respond(s, "200 OK", &body);
                return Ok(r);
            }
            Err(Reject::Method) => respond(s, "405 Method Not Allowed", ""),
            Err(Reject::NotFound) => respond(s, "404 Not Found", ""),
            Err(_) => respond(s, "400 Bad Request", ""),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_matches_rfc7636_vector() {
        assert_eq!(pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
        let v = new_verifier().unwrap();
        assert_eq!(v.len(), 43);
        assert_ne!(v, new_verifier().unwrap());
    }

    #[test]
    fn authorize_url_has_all_parameters() {
        let u = authorize_url("https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize", "cid", "http://localhost:5555", "st", "ch").unwrap();
        let u = Url::parse(&u).unwrap();
        assert_eq!(u.host_str(), Some("login.microsoftonline.com"));
        let q: std::collections::HashMap<_, _> = u.query_pairs().into_owned().collect();
        for (k, v) in [
            ("client_id", "cid"),
            ("response_type", "code"),
            ("redirect_uri", "http://localhost:5555"),
            ("scope", "XboxLive.signin offline_access"),
            ("state", "st"),
            ("code_challenge", "ch"),
            ("code_challenge_method", "S256"),
            ("prompt", "select_account"),
        ] {
            assert_eq!(q.get(k).map(String::as_str), Some(v), "{k}");
        }
        assert!(!q.contains_key("code_verifier"));
    }

    fn code(c: &str) -> Result<Redirect, Reject> {
        Ok(Redirect::Code(c.into()))
    }

    #[test]
    fn parses_redirects() {
        let p = |h: &str| parse_redirect_request(h, "S");
        assert!(p("GET /?code=abc%2Fdef&state=S HTTP/1.1\r\nHost: x\r\n\r\n") == code("abc/def"));
        assert!(p("GET /?error=access_denied&error_description=x%20y&state=S HTTP/1.1\r\n\r\n") == Ok(Redirect::Denied("access_denied".into())));
        assert!(p("GET /?error=<script>&state=S HTTP/1.1\r\n\r\n") == Ok(Redirect::Denied("unknown_error".into())));
        assert_eq!(p("GET /?code=abc&state=BAD HTTP/1.1\r\n\r\n").err(), Some(Reject::BadState));
        assert_eq!(p("GET /?code=abc HTTP/1.1\r\n\r\n").err(), Some(Reject::BadState));
        assert_eq!(p("GET /?state=S HTTP/1.1\r\n\r\n").err(), Some(Reject::Malformed));
        assert_eq!(p("GET /?code=&state=S HTTP/1.1\r\n\r\n").err(), Some(Reject::Malformed));
        assert_eq!(p("GET /other?code=a&state=S HTTP/1.1\r\n\r\n").err(), Some(Reject::NotFound));
        assert_eq!(p("POST /?code=a&state=S HTTP/1.1\r\n\r\n").err(), Some(Reject::Method));
        for bad in ["", "GET", "GET / HTTP/1.1 extra", "GET /?code=a&state=S", "GET http://x/ HTTP/1.1", "\r\n"] {
            assert_eq!(p(bad).err(), Some(Reject::Malformed), "{bad:?}");
        }
    }
}
