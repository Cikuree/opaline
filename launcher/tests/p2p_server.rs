//! Serveur de présence P2P : requêtes réelles sur 127.0.0.1 (le test seul peut choisir l'adresse ; l'application n'écoute que sur l'IP Tailscale).
use opaline_core::p2p::{self, Handler, Invite, Presence, Server};
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const FRIEND: &str = "00000000-0000-0000-0000-000000000001";
const STRANGER: &str = "00000000-0000-0000-0000-000000000002";

#[derive(Default)]
struct Fake {
    invites: Mutex<Vec<Invite>>,
    /// Simule un ami dont l'IP connue diffère de l'IP source.
    wrong_source: AtomicBool,
}

impl Handler for Fake {
    fn presence(&self) -> Option<Presence> {
        Some(Presence { app: "opaline".into(), v: 1, uuid: "00000000-0000-0000-0000-0000000000aa".into(), name: "Moi".into(), in_game: true, pack_id: Some("survie".into()), pack_version: Some("a1b2".into()), app_version: "0.3.0".into() })
    }
    fn may_invite(&self, uuid: &str, from: IpAddr) -> bool {
        uuid == FRIEND && from.is_loopback() && !self.wrong_source.load(Ordering::SeqCst)
    }
    fn on_invite(&self, invite: Invite) {
        self.invites.lock().unwrap().push(invite);
    }
}

fn start() -> (Server, Arc<Fake>) {
    let h = Arc::new(Fake::default());
    (Server::start("127.0.0.1:0".parse().unwrap(), h.clone()).unwrap(), h)
}

fn raw(addr: SocketAddr, req: &[u8]) -> String {
    let mut s = TcpStream::connect(addr).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let _ = s.write_all(req);
    let mut out = Vec::new();
    let _ = s.read_to_end(&mut out);
    String::from_utf8_lossy(&out).into_owned()
}

fn post(addr: SocketAddr, body: &str) -> String {
    raw(addr, format!("POST /opaline/v1/invite HTTP/1.1\r\nContent-Length: {}\r\n\r\n{body}", body.len()).as_bytes())
}

fn invite(uuid: &str) -> String {
    format!(r#"{{"fromUuid":"{uuid}","fromName":"Luna","packId":"survie"}}"#)
}

#[test]
fn presence_answers_with_the_documented_fields_only() {
    let (srv, _) = start();
    let r = raw(srv.local_addr(), b"GET /opaline/v1/presence HTTP/1.1\r\nHost: x\r\n\r\n");
    assert!(r.starts_with("HTTP/1.1 200 OK") && r.contains("Connection: close"), "{r}");
    let body: serde_json::Value = serde_json::from_str(r.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    let mut keys: Vec<_> = body.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, ["app", "appVersion", "inGame", "name", "packId", "packVersion", "uuid", "v"]);
    let p = p2p::query_presence(srv.local_addr(), Duration::from_millis(800)).expect("client");
    assert_eq!((p.name.as_str(), p.in_game, p.pack_id.as_deref()), ("Moi", true, Some("survie")));
}

#[test]
fn invites_only_from_friends_and_rate_limited() {
    let (srv, h) = start();
    let a = srv.local_addr();
    assert!(post(a, &invite(STRANGER)).starts_with("HTTP/1.1 403"));
    assert!(post(a, &invite(FRIEND)).starts_with("HTTP/1.1 200"));
    assert!(post(a, &invite(FRIEND)).starts_with("HTTP/1.1 429"), "1 invitation / 30 s");
    assert!(post(a, "pas du json").starts_with("HTTP/1.1 400"));
    assert!(post(a, r#"{"fromUuid":"x","fromName":"Luna","packId":"survie"}"#).starts_with("HTTP/1.1 400"));
    let inv = |u: &str| Invite { from_uuid: u.into(), from_name: "Luna".into(), pack_id: "survie".into() };
    let t = Duration::from_millis(800);
    assert_eq!(p2p::send_invite(a, &inv(STRANGER), t), Err("Cet ami ne t’a pas dans sa liste d’amis"));
    assert_eq!(p2p::send_invite(a, &inv(FRIEND), t), Err("Invitation déjà envoyée : patiente 30 secondes"));
    let got = h.invites.lock().unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!((got[0].from_name.as_str(), got[0].pack_id.as_str()), ("Luna", "survie"));
}

#[test]
fn unknown_routes_big_bodies_and_big_headers_are_refused() {
    let (srv, h) = start();
    let a = srv.local_addr();
    assert!(raw(a, b"GET /autre HTTP/1.1\r\n\r\n").starts_with("HTTP/1.1 404"));
    assert!(raw(a, b"DELETE /opaline/v1/presence HTTP/1.1\r\n\r\n").starts_with("HTTP/1.1 404"));
    assert!(raw(a, b"GET /opaline/v1/invite HTTP/1.1\r\n\r\n").starts_with("HTTP/1.1 404"));
    let big = "x".repeat(2000);
    assert!(post(a, &big).starts_with("HTTP/1.1 413"));
    assert!(raw(a, b"POST /opaline/v1/invite HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n").starts_with("HTTP/1.1 411"));
    let huge = format!("GET /opaline/v1/presence HTTP/1.1\r\nX-Pad: {}\r\n\r\n", "a".repeat(9000));
    assert!(raw(a, huge.as_bytes()).starts_with("HTTP/1.1 431"));
    assert!(h.invites.lock().unwrap().is_empty());
}

#[test]
fn slow_clients_time_out_and_connections_are_capped() {
    let (srv, _) = start();
    let a = srv.local_addr();
    let started = std::time::Instant::now();
    let r = raw(a, b"GET /opaline/v1/pres"); // requête jamais terminée
    assert!(r.starts_with("HTTP/1.1 408"), "{r}");
    assert!(started.elapsed() < Duration::from_secs(4));
    let idle: Vec<TcpStream> = (0..8).map(|_| TcpStream::connect(a).unwrap()).collect();
    std::thread::sleep(Duration::from_millis(300)); // laisse le serveur les compter
    let mut ninth = TcpStream::connect(a).unwrap();
    ninth.set_read_timeout(Some(Duration::from_millis(1500))).unwrap();
    let _ = ninth.write_all(b"GET /opaline/v1/presence HTTP/1.1\r\n\r\n");
    let mut out = Vec::new();
    let _ = ninth.read_to_end(&mut out);
    assert!(out.is_empty(), "la 9e connexion simultanée est fermée sans réponse");
    drop(idle);
}

#[test]
fn discovery_ignores_non_opaline_and_closed_ports() {
    let junk = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = junk.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for s in junk.incoming().flatten().take(1) {
            let mut s = s;
            let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 15\r\n\r\n{\"app\":\"autre\"}");
        }
    });
    let lo = Ipv4Addr::LOCALHOST;
    assert!(p2p::discover(&[lo], port, Duration::from_millis(800)).is_empty());
    let closed = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    assert!(p2p::discover(&[lo], closed, Duration::from_millis(800)).is_empty());
}

#[test]
fn only_tailnet_addresses_can_be_hosted() {
    let h: Arc<dyn Handler> = Arc::new(Fake::default());
    for ip in [Ipv4Addr::LOCALHOST, Ipv4Addr::UNSPECIFIED, Ipv4Addr::new(192, 168, 1, 10)] {
        assert!(Server::start_tailnet(ip, h.clone()).is_err(), "{ip}");
    }
    let mut host = p2p::Hosting::default();
    assert_eq!(host.sync(None, &h), p2p::HostState::Off);
}

#[test]
fn invite_from_another_source_address_is_refused() {
    let (srv, h) = start();
    h.wrong_source.store(true, Ordering::SeqCst);
    assert!(post(srv.local_addr(), &invite(FRIEND)).starts_with("HTTP/1.1 403"));
    assert!(h.invites.lock().unwrap().is_empty());
}

#[test]
fn conflicting_content_lengths_are_refused() {
    let (srv, _) = start();
    let r = raw(srv.local_addr(), b"POST /opaline/v1/invite HTTP/1.1\r\nContent-Length: 2\r\nContent-Length: 3\r\n\r\n{}");
    assert!(r.starts_with("HTTP/1.1 400"), "{r}");
    assert!(srv.is_alive());
}

#[test]
fn trickling_peer_is_dropped_at_the_total_deadline() {
    let slow = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = slow.local_addr().unwrap();
    std::thread::spawn(move || {
        if let Ok((mut s, _)) = slow.accept() {
            for _ in 0..20 {
                if s.write_all(b"x").is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(300));
            }
        }
    });
    let t = std::time::Instant::now();
    assert!(p2p::query_presence(addr, Duration::from_millis(800)).is_none());
    assert!(t.elapsed() < Duration::from_millis(1500), "{:?}", t.elapsed());
}
