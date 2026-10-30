//! Serveur HTTP de fixtures local (std uniquement) : Range honoré/ignoré, coupure, 404/500.
use std::collections::HashMap;
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
}

struct Route {
    body: Vec<u8>,
    mode: Mode,
}

#[derive(Clone, Debug)]
pub struct Req {
    pub path: String,
    pub range: Option<String>,
}

type Routes = Arc<Mutex<HashMap<String, Route>>>;

pub struct Server {
    pub base: String,
    routes: Routes,
    log: Arc<Mutex<Vec<Req>>>,
}

impl Server {
    pub fn start() -> Server {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", l.local_addr().unwrap());
        let routes: Routes = Arc::default();
        let log: Arc<Mutex<Vec<Req>>> = Arc::default();
        let (r, g) = (routes.clone(), log.clone());
        std::thread::spawn(move || {
            for s in l.incoming().flatten() {
                let (r, g) = (r.clone(), g.clone());
                std::thread::spawn(move || handle(s, &r, &g));
            }
        });
        Server { base, routes, log }
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    pub fn put(&self, path: &str, body: impl Into<Vec<u8>>) {
        self.put_mode(path, body, Mode::Normal);
    }

    pub fn put_mode(&self, path: &str, body: impl Into<Vec<u8>>, mode: Mode) {
        self.routes.lock().unwrap().insert(path.to_string(), Route { body: body.into(), mode });
    }

    pub fn requests(&self) -> Vec<Req> {
        self.log.lock().unwrap().clone()
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

fn handle(mut s: TcpStream, routes: &Routes, log: &Arc<Mutex<Vec<Req>>>) {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = s.read(&mut tmp).unwrap_or(0);
        if n == 0 {
            return;
        }
        buf.extend_from_slice(&tmp[..n]);
    }
    let text = String::from_utf8_lossy(&buf).into_owned();
    let mut lines = text.lines();
    let target = lines.next().unwrap_or("").split(' ').nth(1).unwrap_or("/").to_string();
    let path = target.split('?').next().unwrap_or("/").to_string();
    let range = lines.find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.eq_ignore_ascii_case("range").then(|| v.trim().to_string())
    });
    log.lock().unwrap().push(Req { path: path.clone(), range: range.clone() });

    let found = {
        let mut m = routes.lock().unwrap();
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
    let start = range
        .filter(|_| !matches!(mode, Mode::IgnoreRange))
        .and_then(|r| r.strip_prefix("bytes=")?.strip_suffix('-')?.parse::<usize>().ok());
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
