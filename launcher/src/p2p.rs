//! Présence et invitations entre amis, pair à pair sur le tailnet (protocole v1, docs/OPALINE_CONTRACT.md).
//! Le tailnet est la frontière de confiance : l'UUID annoncé n'est pas authentifié. Ni IP ni nom de machine ne sont loggés.
use crate::friends::{dashed, is_mc_name};
use crate::tailscale::is_tailnet_ip;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{ErrorKind, Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const PORT: u16 = 25590;
const MAX_HEADERS: usize = 8 * 1024;
const MAX_BODY: usize = 1024;
const READ_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_CONNECTIONS: usize = 8;
const INVITE_GAP: Duration = Duration::from_secs(30);
const INVITE_MAX: usize = 20;
pub const DISCOVERY_TIMEOUT: Duration = Duration::from_millis(800);

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Presence {
    pub app: String,
    pub v: u32,
    pub uuid: String,
    pub name: String,
    pub in_game: bool,
    pub pack_id: Option<String>,
    pub pack_version: Option<String>,
    pub app_version: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Invite {
    pub from_uuid: String,
    pub from_name: String,
    pub pack_id: String,
}

/// Identifiant de pack : 1 à 64 caractères `[a-z0-9_-]`.
pub fn valid_pack_id(s: &str) -> bool {
    (1..=64).contains(&s.len()) && s.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-')
}

fn valid_tag(s: &str, max: usize) -> bool {
    (1..=max).contains(&s.len()) && s.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
}

/// Présence reçue : refusée si ce n'est pas Opaline v1 ou si un champ est invalide ; UUID normalisé.
pub fn validate_presence(p: Presence) -> Option<Presence> {
    let ok = p.app == "opaline"
        && p.v == 1
        && is_mc_name(&p.name)
        && p.pack_id.as_deref().is_none_or(valid_pack_id)
        && p.pack_version.as_deref().is_none_or(|v| valid_tag(v, 64))
        && valid_tag(&p.app_version, 32);
    ok.then(|| dashed(&p.uuid).map(|uuid| Presence { uuid, ..p })).flatten()
}

/// Corps d'invitation : JSON strict, pseudo / UUID / pack validés, UUID normalisé.
pub fn parse_invite(body: &[u8]) -> Option<Invite> {
    let i: Invite = serde_json::from_slice(body).ok()?;
    (is_mc_name(&i.from_name) && valid_pack_id(&i.pack_id)).then(|| dashed(&i.from_uuid).map(|from_uuid| Invite { from_uuid, ..i })).flatten()
}

/// Anti-spam : 1 invitation / 30 s par expéditeur, et au plus 20 expéditeurs distincts par fenêtre de 30 s (file bornée).
#[derive(Default)]
pub struct Gate {
    last: HashMap<String, Instant>,
}

impl Gate {
    pub fn allow(&mut self, uuid: &str, now: Instant) -> bool {
        self.last.retain(|_, t| now.duration_since(*t) < INVITE_GAP);
        if self.last.contains_key(uuid) || self.last.len() >= INVITE_MAX {
            return false;
        }
        self.last.insert(uuid.to_string(), now);
        true
    }
}

/// Ce que le serveur demande à l'application.
pub trait Handler: Send + Sync {
    /// `None` : pas d'identité (présence désactivée).
    fn presence(&self) -> Option<Presence>;
    /// Invitation acceptable : `uuid` est dans la liste d'amis ET `from` (IP source de la connexion) est l'adresse connue de cet ami.
    fn may_invite(&self, uuid: &str, from: IpAddr) -> bool;
    fn on_invite(&self, invite: Invite);
}

struct Request {
    method: String,
    path: String,
    content_length: Option<usize>,
    chunked: bool,
}

fn parse_head(head: &str) -> Option<Request> {
    let mut lines = head.split("\r\n");
    let mut first = lines.next()?.split(' ');
    let (method, path, version) = (first.next()?, first.next()?, first.next()?);
    if !version.starts_with("HTTP/1.") || first.next().is_some() {
        return None;
    }
    let (mut content_length, mut chunked) = (None, false);
    for l in lines {
        let (k, v) = l.split_once(':')?;
        match k.trim().to_ascii_lowercase().as_str() {
            "content-length" => {
                let n = v.trim().parse().ok()?;
                if content_length.is_some_and(|c| c != n) {
                    return None; // deux longueurs différentes : requête ambiguë
                }
                content_length = Some(n);
            }
            "transfer-encoding" => chunked = true,
            _ => {}
        }
    }
    Some(Request { method: method.into(), path: path.into(), content_length, chunked })
}

fn err(code: u16) -> (u16, String) {
    (code, format!(r#"{{"error":{code}}}"#))
}

/// Route une requête déjà lue. Méthodes et chemins inconnus : 404.
fn route(req: &Request, body: &[u8], from: IpAddr, h: &dyn Handler, gate: &Mutex<Gate>) -> (u16, String) {
    match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/opaline/v1/presence") => match h.presence() {
            Some(p) => (200, serde_json::to_string(&p).unwrap_or_default()),
            None => err(503),
        },
        ("POST", "/opaline/v1/invite") => {
            let Some(inv) = parse_invite(body) else { return err(400) };
            if !h.may_invite(&inv.from_uuid, from) {
                return err(403);
            }
            if !gate.lock().unwrap().allow(&inv.from_uuid, Instant::now()) {
                return err(429);
            }
            h.on_invite(inv);
            (200, r#"{"ok":true}"#.into())
        }
        _ => err(404),
    }
}

fn read_some(s: &mut TcpStream, buf: &mut [u8], deadline: Instant) -> Result<usize, u16> {
    let left = deadline.saturating_duration_since(Instant::now());
    if left.is_zero() {
        return Err(408);
    }
    s.set_read_timeout(Some(left)).map_err(|_| 400u16)?;
    s.read(buf).map_err(|_| 408u16)
}

/// Lit en-têtes (≤ 8 Ko) puis corps (≤ 1 Ko) sous un délai total de 2 s.
fn read_request(s: &mut TcpStream) -> Result<(Request, Vec<u8>), u16> {
    let deadline = Instant::now() + READ_TIMEOUT;
    let (mut buf, mut tmp) = (Vec::new(), [0u8; 1024]);
    let end = loop {
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i;
        }
        if buf.len() > MAX_HEADERS {
            return Err(431);
        }
        match read_some(s, &mut tmp, deadline)? {
            0 => return Err(400),
            n => buf.extend_from_slice(&tmp[..n]),
        }
    };
    if end + 4 > MAX_HEADERS {
        return Err(431);
    }
    let req = std::str::from_utf8(&buf[..end]).ok().and_then(parse_head).ok_or(400u16)?;
    let mut body = buf[end + 4..].to_vec();
    if req.method == "POST" {
        let len = match (req.content_length, req.chunked) {
            (Some(n), false) => n,
            _ => return Err(411),
        };
        if len > MAX_BODY {
            return Err(413);
        }
        while body.len() < len {
            match read_some(s, &mut tmp, deadline)? {
                0 => return Err(400),
                n => body.extend_from_slice(&tmp[..n]),
            }
        }
        body.truncate(len);
    }
    Ok((req, body))
}

fn reason(code: u16) -> &'static str {
    match code {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        408 => "Request Timeout",
        411 => "Length Required",
        413 => "Payload Too Large",
        429 => "Too Many Requests",
        431 => "Request Header Fields Too Large",
        _ => "Service Unavailable",
    }
}

fn serve(mut s: TcpStream, h: &dyn Handler, gate: &Mutex<Gate>) {
    let Ok(peer) = s.peer_addr() else { return };
    let _ = s.set_write_timeout(Some(READ_TIMEOUT));
    let (code, body) = match read_request(&mut s) {
        Ok((req, body)) => route(&req, &body, peer.ip(), h, gate),
        Err(c) => err(c),
    };
    let _ = write!(s, "HTTP/1.1 {code} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", reason(code), body.len());
}

/// Serveur d'écoute ; s'arrête (et libère le port) à la destruction.
pub struct Server {
    stop: Arc<AtomicBool>,
    dead: Arc<AtomicBool>,
    addr: SocketAddr,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Server {
    /// Écoute sur l'IPv4 Tailscale seulement, port 25590 : toute autre adresse est refusée.
    pub fn start_tailnet(ip: Ipv4Addr, h: Arc<dyn Handler>) -> std::io::Result<Server> {
        if !is_tailnet_ip(ip) {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "adresse hors du tailnet"));
        }
        Server::start(SocketAddr::new(ip.into(), PORT), h)
    }

    /// Adresse libre (tests d'intégration sur 127.0.0.1) ; l'application n'appelle que `start_tailnet`.
    pub fn start(addr: SocketAddr, h: Arc<dyn Handler>) -> std::io::Result<Server> {
        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;
        let addr = listener.local_addr()?;
        let (stop, active, gate) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicUsize::new(0)), Arc::new(Mutex::new(Gate::default())));
        let (stop2, dead) = (stop.clone(), Arc::new(AtomicBool::new(false)));
        let dead2 = dead.clone();
        let thread = std::thread::spawn(move || {
            while !stop2.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((s, _)) => {
                        if active.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
                            active.fetch_sub(1, Ordering::SeqCst);
                            continue; // au-delà de 8 connexions simultanées : fermée sans réponse
                        }
                        let (h, gate, active) = (h.clone(), gate.clone(), active.clone());
                        std::thread::spawn(move || {
                            if s.set_nonblocking(false).is_ok() {
                                serve(s, h.as_ref(), &gate);
                            }
                            active.fetch_sub(1, Ordering::SeqCst);
                        });
                    }
                    Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted | ErrorKind::ConnectionAborted | ErrorKind::ConnectionReset) => {
                        std::thread::sleep(Duration::from_millis(25));
                    }
                    Err(_) => {
                        dead2.store(true, Ordering::SeqCst); // écoute tombée : `Hosting::sync` relie le port
                        break;
                    }
                }
            }
        });
        Ok(Server { stop, dead, addr, thread: Some(thread) })
    }

    /// Faux si la boucle d'écoute s'est arrêtée sur une erreur.
    pub fn is_alive(&self) -> bool {
        !self.dead.load(Ordering::SeqCst)
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostState {
    Off,
    Listening,
    Failed,
}

/// Cycle de vie de l'écoute : suit l'IP Tailscale (`None` = pas connecté).
#[derive(Default)]
pub struct Hosting {
    cur: Option<(Ipv4Addr, Server)>,
}

impl Hosting {
    pub fn sync(&mut self, ip: Option<Ipv4Addr>, h: &Arc<dyn Handler>) -> HostState {
        let Some(ip) = ip else {
            self.cur = None;
            return HostState::Off;
        };
        if self.cur.as_ref().is_some_and(|(i, s)| *i == ip && s.is_alive()) {
            return HostState::Listening;
        }
        self.cur = None; // libère le port avant de relier
        match Server::start_tailnet(ip, h.clone()) {
            Ok(s) => {
                self.cur = Some((ip, s));
                HostState::Listening
            }
            Err(_) => HostState::Failed,
        }
    }
}

/// Un échange HTTP/1.1 minimal (réponse ≤ 8 Ko) ; `None` si injoignable, illisible ou plus long que `timeout` au total.
fn exchange(addr: SocketAddr, req: &str, timeout: Duration) -> Option<(u16, Vec<u8>)> {
    let deadline = Instant::now() + timeout;
    let mut s = TcpStream::connect_timeout(&addr, timeout).ok()?;
    s.set_write_timeout(Some(timeout)).ok()?;
    s.write_all(req.as_bytes()).ok()?;
    let (mut buf, mut tmp) = (Vec::new(), [0u8; 1024]);
    loop {
        // Échéance totale : un pair qui goutte-à-goutte ne retient pas la découverte.
        let left = deadline.checked_duration_since(Instant::now()).filter(|d| !d.is_zero())?;
        s.set_read_timeout(Some(left)).ok()?;
        match s.read(&mut tmp).ok()? {
            0 => break,
            n => buf.extend_from_slice(&tmp[..n]),
        }
        if buf.len() > 8192 {
            return None;
        }
    }
    let end = buf.windows(4).position(|w| w == b"\r\n\r\n")?;
    let code = std::str::from_utf8(&buf[..end]).ok()?.split(' ').nth(1)?.parse().ok()?;
    Some((code, buf[end + 4..].to_vec()))
}

pub fn query_presence(addr: SocketAddr, timeout: Duration) -> Option<Presence> {
    let (code, body) = exchange(addr, "GET /opaline/v1/presence HTTP/1.1\r\nHost: opaline\r\nConnection: close\r\n\r\n", timeout)?;
    (code == 200).then(|| serde_json::from_slice(&body).ok()).flatten().and_then(validate_presence)
}

/// Interroge les pairs, 8 à la fois ; seuls les pairs Opaline répondent correctement.
pub fn discover(ips: &[Ipv4Addr], port: u16, timeout: Duration) -> Vec<(Ipv4Addr, Presence)> {
    let mut out = Vec::new();
    for chunk in ips.chunks(MAX_CONNECTIONS) {
        std::thread::scope(|sc| {
            let hs: Vec<_> = chunk.iter().map(|ip| sc.spawn(move || query_presence(SocketAddr::new((*ip).into(), port), timeout).map(|p| (*ip, p)))).collect();
            out.extend(hs.into_iter().filter_map(|h| h.join().ok().flatten()));
        });
    }
    out
}

/// Envoie une invitation ; erreur lisible, sans adresse.
pub fn send_invite(addr: SocketAddr, inv: &Invite, timeout: Duration) -> Result<(), &'static str> {
    let body = serde_json::to_string(inv).map_err(|_| "invitation invalide")?;
    let req = format!("POST /opaline/v1/invite HTTP/1.1\r\nHost: opaline\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
    match exchange(addr, &req, timeout) {
        Some((200, _)) => Ok(()),
        Some((403, _)) => Err("Cet ami ne t’a pas dans sa liste d’amis"),
        Some((429, _)) => Err("Invitation déjà envoyée : patiente 30 secondes"),
        Some(_) => Err("Cet ami n’a pas pu recevoir l’invitation"),
        None => Err("Ami injoignable sur Tailscale"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const U: &str = "3f9a21c4-7be0-4a11-9c3d-0b6e5f51e51d";

    #[test]
    fn invite_validation() {
        let ok = format!(r#"{{"fromUuid":"{}","fromName":"Luna_Fox","packId":"classroom-run"}}"#, U.to_uppercase().replace('-', ""));
        assert_eq!(parse_invite(ok.as_bytes()).unwrap().from_uuid, U);
        for bad in [
            r#"{"fromUuid":"x","fromName":"Luna","packId":"a"}"#.to_string(),
            format!(r#"{{"fromUuid":"{U}","fromName":"Lu na","packId":"a"}}"#),
            format!(r#"{{"fromUuid":"{U}","fromName":"Luna","packId":"../x"}}"#),
            format!(r#"{{"fromUuid":"{U}","fromName":"Luna","packId":"a","extra":1}}"#),
            format!(r#"{{"fromUuid":"{U}","fromName":"Luna"}}"#),
            "nope".into(),
        ] {
            assert!(parse_invite(bad.as_bytes()).is_none(), "{bad}");
        }
    }

    #[test]
    fn presence_validation() {
        let p = Presence { app: "opaline".into(), v: 1, uuid: U.replace('-', ""), name: "Luna".into(), in_game: true, pack_id: Some("survie".into()), pack_version: Some("a1b2".into()), app_version: "0.3.0".into() };
        assert_eq!(validate_presence(p.clone()).unwrap().uuid, U);
        for bad in [Presence { app: "autre".into(), ..p.clone() }, Presence { v: 2, ..p.clone() }, Presence { name: "a b".into(), ..p.clone() }, Presence { pack_id: Some("A/B".into()), ..p.clone() }, Presence { uuid: "zz".into(), ..p }] {
            assert!(validate_presence(bad).is_none());
        }
    }

    #[test]
    fn gate_limits_per_sender_and_queue() {
        let (mut g, t0) = (Gate::default(), Instant::now());
        assert!(g.allow("a", t0));
        assert!(!g.allow("a", t0 + Duration::from_secs(29)));
        assert!(g.allow("a", t0 + Duration::from_secs(31)));
        let mut g = Gate::default();
        for i in 0..20 {
            assert!(g.allow(&format!("u{i}"), t0));
        }
        assert!(!g.allow("u20", t0 + Duration::from_secs(1)), "file pleine");
        assert!(g.allow("u20", t0 + Duration::from_secs(31)), "file vidée après la fenêtre");
    }

    #[test]
    fn head_parsing() {
        let r = parse_head("POST /x HTTP/1.1\r\nHost: a\r\ncontent-LENGTH: 12").unwrap();
        assert_eq!((r.method.as_str(), r.path.as_str(), r.content_length, r.chunked), ("POST", "/x", Some(12), false));
        assert!(parse_head("POST /x HTTP/1.1\r\nTransfer-Encoding: chunked").unwrap().chunked);
        assert_eq!(parse_head("POST /x HTTP/1.1\r\nContent-Length: 5\r\nContent-Length: 5").unwrap().content_length, Some(5));
        for bad in ["POST /x HTTP/1.1\r\nContent-Length: 5\r\nContent-Length: 6", "GET /x", "GET /x HTTP/2", "GET /x HTTP/1.1\r\nsans-deux-points", "POST /x HTTP/1.1\r\nContent-Length: -1", "GET /a b HTTP/1.1"] {
            assert!(parse_head(bad).is_none(), "{bad}");
        }
    }
}
