//! Cas 5 : Server List Ping contre un faux serveur SLP local (127.0.0.1, aucun réseau réel).
#![allow(dead_code)]

#[path = "../src/ping.rs"]
mod ping;

use ping::{PROTOCOL, ping};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Lit un paquet Minecraft : (id, charge utile).
fn read_pkt(c: &mut TcpStream) -> std::io::Result<(i32, Vec<u8>)> {
    let len = ping::read_varint(c).map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut buf = vec![0u8; len as usize];
    c.read_exact(&mut buf)?;
    let mut s = &buf[..];
    let id = ping::read_varint(&mut s).map_err(|e| std::io::Error::other(e.to_string()))?;
    Ok((id, s.to_vec()))
}

fn pkt(id: i32, body: &[u8]) -> Vec<u8> {
    let mut inner = Vec::new();
    ping::write_varint(&mut inner, id);
    inner.extend_from_slice(body);
    let mut out = Vec::new();
    ping::write_varint(&mut out, inner.len() as i32);
    out.extend(inner);
    out
}

fn status_body(json: &str) -> Vec<u8> {
    let mut b = Vec::new();
    ping::write_varint(&mut b, json.len() as i32);
    b.extend_from_slice(json.as_bytes());
    b
}

/// Serveur SLP : `script` reçoit la connexion après le handshake (et la requête de statut lue).
fn fake_server(script: impl FnOnce(TcpStream) + Send + 'static) -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        if let Ok((c, _)) = l.accept() {
            script(c);
        }
    });
    port
}

const ONLINE: &str = r#"{"version":{"name":"26.3","protocol":777},"players":{"max":20,"online":2,"sample":[{"name":"Alice","id":"a"},{"name":"Bob","id":"b"}]},"description":{"text":"Classroom","extra":[{"text":" Run"}]}}"#;

#[test]
fn online_with_players_is_parsed_and_handshake_is_correct() {
    let (tx, rx) = mpsc::channel();
    let port = fake_server(move |mut c| {
        let (id, hs) = read_pkt(&mut c).unwrap();
        // handshake : id 0, protocole 777, hôte, port, intention 1 (statut)
        let mut s = &hs[..];
        let proto = ping::read_varint(&mut s).unwrap();
        let hlen = ping::read_varint(&mut s).unwrap() as usize;
        let host = String::from_utf8(s[..hlen].to_vec()).unwrap();
        s = &s[hlen..];
        let port = u16::from_be_bytes([s[0], s[1]]);
        s = &s[2..];
        let next = ping::read_varint(&mut s).unwrap();
        tx.send((id, proto, host, port, next)).unwrap();
        let (sid, _) = read_pkt(&mut c).unwrap();
        assert_eq!(sid, 0, "requête de statut attendue");
        c.write_all(&pkt(0, &status_body(ONLINE))).unwrap();
        let (pid, payload) = read_pkt(&mut c).unwrap();
        assert_eq!(pid, 1, "ping attendu");
        c.write_all(&pkt(1, &payload)).unwrap();
    });
    let st = ping(&format!("127.0.0.1:{port}"), Duration::from_secs(3)).expect("statut");
    let (id, proto, host, hport, next) = rx.recv().unwrap();
    assert_eq!((id, proto, host.as_str(), next), (0, PROTOCOL, "127.0.0.1", 1));
    assert_eq!(hport, port);
    assert_eq!((st.online, st.max, st.protocol, st.version.as_str()), (2, 20, 777, "26.3"));
    assert_eq!(st.sample, ["Alice", "Bob"]);
    assert_eq!(st.motd, "Classroom Run");
    assert!(st.latency_ms.is_some());
}

#[test]
fn truncated_status_string_is_an_error() {
    // La longueur annoncée de la chaîne dépasse ce qui suit dans le paquet.
    let port = fake_server(|mut c| {
        let _ = read_pkt(&mut c);
        let _ = read_pkt(&mut c);
        let mut body = Vec::new();
        ping::write_varint(&mut body, 200);
        body.extend_from_slice(br#"{"versi"#);
        c.write_all(&pkt(0, &body)).unwrap();
    });
    let e = ping(&format!("127.0.0.1:{port}"), Duration::from_secs(3)).expect_err("tronqué");
    assert_eq!(e.to_string(), "chaîne de statut tronquée");
}

#[test]
fn truncated_packet_is_an_error_not_a_panic() {
    // Le paquet annonce 500 octets, la connexion se coupe après 10.
    let port = fake_server(|mut c| {
        let _ = read_pkt(&mut c);
        let _ = read_pkt(&mut c);
        let mut out = Vec::new();
        ping::write_varint(&mut out, 500);
        out.extend_from_slice(&[0u8; 10]);
        c.write_all(&out).unwrap();
    });
    assert!(ping(&format!("127.0.0.1:{port}"), Duration::from_secs(3)).is_err());
}

#[test]
fn invalid_status_json_is_an_error() {
    let port = fake_server(|mut c| {
        let _ = read_pkt(&mut c);
        let _ = read_pkt(&mut c);
        c.write_all(&pkt(0, &status_body("pas du json"))).unwrap();
    });
    let e = ping(&format!("127.0.0.1:{port}"), Duration::from_secs(3)).expect_err("json invalide");
    assert_eq!(e.to_string(), "réponse de statut invalide");
}

#[test]
fn oversized_packet_is_refused() {
    let port = fake_server(|mut c| {
        let _ = read_pkt(&mut c);
        let _ = read_pkt(&mut c);
        let mut out = Vec::new();
        ping::write_varint(&mut out, 3 * 1024 * 1024);
        c.write_all(&out).unwrap();
    });
    let e = ping(&format!("127.0.0.1:{port}"), Duration::from_secs(3)).expect_err("paquet trop grand");
    assert!(e.to_string().starts_with("taille de paquet invalide"), "message : {e}");
}

#[test]
fn closed_port_is_a_connection_error() {
    // Port alloué puis libéré : plus rien n'écoute.
    let port = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let t0 = Instant::now();
    let e = ping(&format!("127.0.0.1:{port}"), Duration::from_secs(3)).expect_err("port fermé");
    assert!(e.to_string().starts_with("connexion à 127.0.0.1:"), "message : {e}");
    assert!(t0.elapsed() < Duration::from_secs(3));
}

#[test]
fn silent_server_times_out() {
    // Accepte la connexion mais n'envoie jamais rien.
    let (done_tx, done_rx) = mpsc::channel::<()>();
    let port = fake_server(move |c| {
        let _ = done_rx.recv_timeout(Duration::from_secs(5));
        drop(c);
    });
    let t0 = Instant::now();
    let r = ping(&format!("127.0.0.1:{port}"), Duration::from_millis(500));
    let elapsed = t0.elapsed();
    let _ = done_tx.send(());
    assert!(r.is_err(), "pas de timeout");
    assert!(elapsed < Duration::from_secs(4), "délai dépassé : {elapsed:?}");
}

#[test]
fn split_host_handles_default_and_explicit_port() {
    assert_eq!(ping::split_host("classroom"), ("classroom".to_string(), 25565));
    assert_eq!(ping::split_host("127.0.0.1:25566"), ("127.0.0.1".to_string(), 25566));
    assert_eq!(ping::split_host("classroom:"), ("classroom".to_string(), 25565));
}

#[test]
#[ignore = "bug ping.rs:155 (split_host) : une adresse IPv6 entre crochets avec port garde le port par défaut et le crochet dans l'hôte"]
fn bug_split_host_ipv6_with_port() {
    assert_eq!(ping::split_host("[2001:db8::1]:25566"), ("2001:db8::1".to_string(), 25566));
}
