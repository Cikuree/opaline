//! Server List Ping (handshake + status request sur TCP). Protocole 777 = Minecraft 26.3
//! (vérifié : `version.json` du jar 26.3, `SharedConstants.RELEASE_NETWORK_PROTOCOL_VERSION`).
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::Value;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

pub const PROTOCOL: i32 = 777;
pub const DEFAULT_PORT: u16 = 25565;
const MAX_PACKET: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub struct ServerStatus {
    pub version: String,
    pub protocol: i32,
    pub online: u32,
    pub max: u32,
    pub sample: Vec<String>,
    /// UUID des joueurs de `sample` (même ordre ; chaîne vide si le serveur n'en donne pas).
    pub sample_ids: Vec<String>,
    pub motd: String,
    pub latency_ms: Option<u64>,
}

pub fn write_varint(out: &mut Vec<u8>, v: i32) {
    let mut v = v as u32;
    loop {
        let b = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            return;
        }
        out.push(b | 0x80);
    }
}

pub fn read_varint(r: &mut impl Read) -> Result<i32> {
    let mut v: u32 = 0;
    for i in 0..5 {
        let mut b = [0u8];
        r.read_exact(&mut b)?;
        v |= u32::from(b[0] & 0x7f) << (7 * i);
        if b[0] & 0x80 == 0 {
            return Ok(v as i32);
        }
    }
    bail!("VarInt trop long")
}

fn packet(id: i32, body: &[u8]) -> Vec<u8> {
    let mut inner = Vec::new();
    write_varint(&mut inner, id);
    inner.extend_from_slice(body);
    let mut out = Vec::new();
    write_varint(&mut out, inner.len() as i32);
    out.extend(inner);
    out
}

/// Texte que le MOTD du serveur doit contenir (`motd=Classroom Run` dans server.properties).
pub const EXPECTED_MOTD: &str = "Classroom Run";

/// Le MOTD (codes de format `§x` retirés) contient `EXPECTED_MOTD`.
pub fn motd_matches(st: &ServerStatus) -> bool {
    motd_contains(st, EXPECTED_MOTD)
}

/// Le MOTD (codes de format retirés) contient `expected`.
pub fn motd_contains(st: &ServerStatus, expected: &str) -> bool {
    let mut out = String::new();
    let mut it = st.motd.chars();
    while let Some(c) = it.next() {
        if c == '\u{a7}' {
            it.next();
        } else {
            out.push(c);
        }
    }
    out.contains(expected)
}

pub fn handshake(host: &str, port: u16) -> Vec<u8> {
    let mut b = Vec::new();
    write_varint(&mut b, PROTOCOL);
    write_varint(&mut b, host.len() as i32);
    b.extend_from_slice(host.as_bytes());
    b.extend_from_slice(&port.to_be_bytes());
    write_varint(&mut b, 1); // intention : STATUS
    packet(0, &b)
}

fn read_packet(r: &mut impl Read) -> Result<(i32, Vec<u8>)> {
    let len = read_varint(r)?;
    if len <= 0 || len as usize > MAX_PACKET {
        bail!("taille de paquet invalide : {len}");
    }
    let mut buf = vec![0u8; len as usize];
    r.read_exact(&mut buf)?;
    let mut c = &buf[..];
    let id = read_varint(&mut c)?;
    Ok((id, c.to_vec()))
}

/// Texte brut d'un composant de chat (chaîne, ou objet `text` + `extra`).
fn flatten_component(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(a) => a.iter().map(flatten_component).collect(),
        Value::Object(o) => {
            let mut s = o.get("text").and_then(Value::as_str).unwrap_or("").to_string();
            if let Some(e) = o.get("extra") {
                s += &flatten_component(e);
            }
            s
        }
        _ => String::new(),
    }
}

pub fn parse_status(json: &str) -> Result<ServerStatus> {
    #[derive(Deserialize)]
    struct S {
        #[serde(default)]
        description: Value,
        players: Option<P>,
        version: Option<V>,
    }
    #[derive(Deserialize)]
    struct P {
        max: u32,
        online: u32,
        #[serde(default)]
        sample: Vec<Named>,
    }
    #[derive(Deserialize)]
    struct Named {
        name: String,
        #[serde(default)]
        id: String,
    }
    #[derive(Deserialize)]
    struct V {
        name: String,
        protocol: i32,
    }
    let s: S = serde_json::from_str(json).context("réponse de statut invalide")?;
    let (online, max, named) = s.players.map_or((0, 0, vec![]), |p| (p.online, p.max, p.sample));
    let sample_ids = named.iter().map(|n| n.id.clone()).collect();
    let sample = named.into_iter().map(|n| n.name).collect();
    let (version, protocol) = s.version.map_or((String::new(), 0), |v| (v.name, v.protocol));
    Ok(ServerStatus { version, protocol, online, max, sample, sample_ids, motd: flatten_component(&s.description), latency_ms: None })
}

/// Déroule le dialogue sur un flux déjà connecté (testable sans réseau).
pub fn ping_stream(s: &mut (impl Read + Write), host: &str, port: u16) -> Result<ServerStatus> {
    s.write_all(&handshake(host, port))?;
    s.write_all(&packet(0, &[]))?;
    let (id, body) = read_packet(s)?;
    if id != 0 {
        bail!("paquet de statut inattendu (id {id})");
    }
    let mut c = &body[..];
    let n = read_varint(&mut c)?;
    if n < 0 || n as usize > c.len() {
        bail!("chaîne de statut tronquée");
    }
    let mut st = parse_status(std::str::from_utf8(&c[..n as usize]).context("statut non UTF-8")?)?;
    let t = Instant::now();
    s.write_all(&packet(1, &0x1234_5678_i64.to_be_bytes()))?;
    if let Ok((1, _)) = read_packet(s) {
        st.latency_ms = Some(t.elapsed().as_millis() as u64);
    }
    Ok(st)
}

/// `hôte[:port]` -> (hôte, port).
pub fn split_host(addr: &str) -> (String, u16) {
    let a = addr.trim();
    if let Some(rest) = a.strip_prefix('[') {
        // IPv6 : [adresse] ou [adresse]:port
        if let Some((h, tail)) = rest.split_once(']') {
            let port = tail.strip_prefix(':').and_then(|p| p.parse().ok()).unwrap_or(DEFAULT_PORT);
            return (h.to_string(), port);
        }
    }
    match a.rsplit_once(':') {
        Some((h, p)) if !h.is_empty() && !h.contains(':') => (h.to_string(), p.parse().unwrap_or(DEFAULT_PORT)),
        _ => (a.to_string(), DEFAULT_PORT),
    }
}

pub fn ping(addr: &str, timeout: Duration) -> Result<ServerStatus> {
    let (host, port) = split_host(addr);
    let sa = (host.as_str(), port).to_socket_addrs()?.next().with_context(|| format!("adresse introuvable : {addr}"))?;
    let mut s = TcpStream::connect_timeout(&sa, timeout).with_context(|| format!("connexion à {addr}"))?;
    s.set_read_timeout(Some(timeout))?;
    s.set_write_timeout(Some(timeout))?;
    ping_stream(&mut s, &host, port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_roundtrip() {
        for v in [0, 1, 127, 128, 777, 25565, i32::MAX, -1] {
            let mut b = Vec::new();
            write_varint(&mut b, v);
            assert_eq!(read_varint(&mut &b[..]).unwrap(), v);
        }
        assert_eq!(handshake("a", 25565)[..3], [8, 0, 0x89]); // longueur, id 0, 777 = 0x89 0x06
    }

    #[test]
    fn parses_status_with_object_motd() {
        let j = r#"{"version":{"name":"26.3","protocol":777},"players":{"max":20,"online":2,"sample":[{"name":"A","id":"x"},{"name":"B","id":"y"}]},
            "description":{"text":"Class","extra":[{"text":"room"}]}}"#;
        let s = parse_status(j).unwrap();
        assert_eq!((s.online, s.max, s.protocol, s.motd.as_str()), (2, 20, 777, "Classroom"));
        assert_eq!(s.sample, ["A", "B"]);
        assert_eq!(parse_status(r#"{"description":"x"}"#).unwrap().motd, "x");
        assert!(parse_status("nope").is_err());
    }

    #[test]
    fn motd_check_ignores_format_codes() {
        let mk = |m: &str| ServerStatus {
            version: String::new(),
            protocol: 0,
            online: 0,
            max: 0,
            sample: vec![],
            sample_ids: vec![],
            motd: m.into(),
            latency_ms: None,
        };
        assert!(motd_matches(&mk("\u{a7}aClassroom \u{a7}lRun")));
        assert!(!motd_matches(&mk("A Minecraft Server")));
    }

    #[test]
    fn splits_ipv6() {
        assert_eq!(split_host("[2001:db8::1]:25566"), ("2001:db8::1".into(), 25566));
        assert_eq!(split_host("[::1]"), ("::1".into(), 25565));
        assert_eq!(split_host("::1"), ("::1".into(), 25565));
    }

    #[test]
    fn splits_host() {
        assert_eq!(split_host("a.fr"), ("a.fr".into(), 25565));
        assert_eq!(split_host(" a.fr:1234 "), ("a.fr".into(), 1234));
    }

    #[test]
    fn pings_a_local_fake_server() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let t = std::thread::spawn(move || {
            let (mut c, _) = l.accept().unwrap();
            let (id, _) = read_packet(&mut c).unwrap();
            assert_eq!(id, 0); // handshake
            let (id, _) = read_packet(&mut c).unwrap();
            assert_eq!(id, 0); // status request
            let json = r#"{"version":{"name":"26.3","protocol":777},"players":{"max":5,"online":1},"description":"hi"}"#;
            let mut body = Vec::new();
            write_varint(&mut body, json.len() as i32);
            body.extend_from_slice(json.as_bytes());
            c.write_all(&packet(0, &body)).unwrap();
            let (id, payload) = read_packet(&mut c).unwrap();
            assert_eq!(id, 1);
            c.write_all(&packet(1, &payload)).unwrap();
        });
        let st = ping(&format!("127.0.0.1:{port}"), Duration::from_secs(3)).unwrap();
        t.join().unwrap();
        assert_eq!((st.online, st.max, st.motd.as_str()), (1, 5, "hi"));
        assert!(st.latency_ms.is_some());
    }
}
