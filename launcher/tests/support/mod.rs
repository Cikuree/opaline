//! Serveur HTTP de fixtures local (std uniquement) : Range honoré/ignoré, coupure, 404/500,
//! réponses scriptées par chemin (file d'attente) et requêtes journalisées avec corps et `Authorization`.
#![allow(dead_code)]
use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub enum Mode {
    Normal,
    /// Répond 200 avec le contenu complet même si Range est demandé.
    IgnoreRange,
    /// La prochaine réponse annonce la taille complète mais coupe la connexion après N octets.
    TruncateOnce(usize),
    /// Répond ce code HTTP sans contenu utile.
    Status(u16),
    /// Répond ce code HTTP avec le corps de la route (JSON pour les API).
    Reply(u16),
}

struct Route {
    body: Vec<u8>,
    mode: Mode,
}

#[derive(Clone, Debug)]
pub struct Req {
    pub method: String,
    pub path: String,
    pub range: Option<String>,
    /// En-tête `Authorization` brut (ex. `Bearer ...`).
    pub auth: Option<String>,
    /// Corps de la requête (formulaire ou JSON pour les POST).
    pub body: Vec<u8>,
}

impl Req {
    pub fn body_text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

type Queue = VecDeque<(u16, Vec<u8>)>;

#[derive(Default)]
struct Shared {
    routes: Mutex<HashMap<String, Route>>,
    queues: Mutex<HashMap<String, Queue>>,
    log: Mutex<Vec<Req>>,
}

pub struct Server {
    pub base: String,
    shared: Arc<Shared>,
}

impl Server {
    pub fn start() -> Server {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", l.local_addr().unwrap());
        let shared = Arc::new(Shared::default());
        let s2 = shared.clone();
        std::thread::spawn(move || {
            for s in l.incoming().flatten() {
                let sh = s2.clone();
                std::thread::spawn(move || handle(s, &sh));
            }
        });
        Server { base, shared }
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    pub fn put(&self, path: &str, body: impl Into<Vec<u8>>) {
        self.put_mode(path, body, Mode::Normal);
    }

    pub fn put_mode(&self, path: &str, body: impl Into<Vec<u8>>, mode: Mode) {
        self.shared.routes.lock().unwrap().insert(path.to_string(), Route { body: body.into(), mode });
    }

    /// Réponses successives pour `path` (code, corps) ; une fois la file vide, la route reprend.
    pub fn queue(&self, path: &str, replies: &[(u16, &str)]) {
        let q = replies.iter().map(|(c, b)| (*c, b.as_bytes().to_vec())).collect();
        self.shared.queues.lock().unwrap().insert(path.to_string(), q);
    }

    pub fn requests(&self) -> Vec<Req> {
        self.shared.log.lock().unwrap().clone()
    }

    pub fn count(&self, prefix: &str) -> usize {
        self.requests().iter().filter(|r| r.path.starts_with(prefix)).count()
    }
}

/// Port sur lequel plus rien n'écoute (connexion refusée).
pub fn dead_base() -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    format!("http://{}", l.local_addr().unwrap())
}

fn handle(mut s: TcpStream, sh: &Shared) {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];
    let head_end = loop {
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        let n = s.read(&mut tmp).unwrap_or(0);
        if n == 0 {
            return;
        }
        buf.extend_from_slice(&tmp[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
    let mut lines = head.lines();
    let mut first = lines.next().unwrap_or("").split(' ');
    let method = first.next().unwrap_or("").to_string();
    let target = first.next().unwrap_or("/").to_string();
    let path = target.split('?').next().unwrap_or("/").to_string();
    let headers: Vec<(String, String)> = lines.filter_map(|l| l.split_once(':').map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))).collect();
    let get = |k: &str| headers.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
    let range = get("range");
    let len: usize = get("content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut body = buf[head_end..].to_vec();
    while body.len() < len {
        let n = s.read(&mut tmp).unwrap_or(0);
        if n == 0 {
            break;
        }
        body.extend_from_slice(&tmp[..n]);
    }
    body.truncate(len);
    sh.log.lock().unwrap().push(Req { method, path: path.clone(), range: range.clone(), auth: get("authorization"), body });

    let queued = sh.queues.lock().unwrap().get_mut(&path).and_then(|q| q.pop_front());
    if let Some((c, b)) = queued {
        return reply(&mut s, &format!("{c} Fixture"), &[], &b, None);
    }
    let found = {
        let mut m = sh.routes.lock().unwrap();
        m.get_mut(&path).map(|r| {
            let snapshot = (r.body.clone(), r.mode.clone());
            if matches!(r.mode, Mode::TruncateOnce(_)) {
                r.mode = Mode::Normal;
            }
            snapshot
        })
    };
    let Some((body, mode)) = found else {
        return reply(&mut s, "404 Not Found", &[], b"not found", None);
    };
    if let Mode::Status(c) = mode {
        return reply(&mut s, &format!("{c} Fixture"), &[], b"status", None);
    }
    if let Mode::Reply(c) = mode {
        return reply(&mut s, &format!("{c} Fixture"), &[], &body, None);
    }
    let start = range.filter(|_| !matches!(mode, Mode::IgnoreRange)).and_then(|r| r.strip_prefix("bytes=")?.strip_suffix('-')?.parse::<usize>().ok());
    let cut = if let Mode::TruncateOnce(n) = mode { Some(n) } else { None };
    match start {
        Some(n) if n >= body.len() => {
            let h = format!("Content-Range: bytes */{}", body.len());
            reply(&mut s, "416 Range Not Satisfiable", &[&h], b"", None)
        }
        Some(n) => {
            let h = format!("Content-Range: bytes {n}-{}/{}", body.len() - 1, body.len());
            reply(&mut s, "206 Partial Content", &[&h], &body[n..], cut)
        }
        None => reply(&mut s, "200 OK", &[], &body, cut),
    }
}

fn reply(s: &mut TcpStream, status: &str, headers: &[&str], body: &[u8], cut: Option<usize>) {
    let mut head = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n", body.len());
    for h in headers {
        head += &format!("{h}\r\n");
    }
    head += "\r\n";
    let _ = s.write_all(head.as_bytes());
    let n = cut.map_or(body.len(), |c| c.min(body.len()));
    let _ = s.write_all(&body[..n]);
    let _ = s.flush();
    let _ = s.shutdown(Shutdown::Both);
}
