//! Téléchargements reprenables (Range + `.part`), vérifiés par hash, en parallèle limité.
use crate::safe::Hash;
use anyhow::{Context, Result, anyhow, bail};
use reqwest::StatusCode;
use reqwest::blocking::Client;
use reqwest::header::RANGE;
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Job {
    pub url: String,
    pub dest: PathBuf,
    pub hash: Hash,
    /// Taille attendue si connue (Mojang) : un flux plus long est coupé, un flux plus court garde son .part.
    pub size: Option<u64>,
}

/// Délai sans aucune donnée reçue avant abandon (connexion, en-têtes ou corps).
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(60);

pub fn http_client() -> Result<Client> {
    // Pas de timeout total (gros fichiers) : l'inactivité est surveillée dans `download_part`.
    Ok(Client::builder()
        .user_agent(concat!("Opaline/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .timeout(None::<Duration>)
        .build()?)
}

fn part_path(dest: &Path) -> PathBuf {
    let mut s = dest.as_os_str().to_owned();
    s.push(".part");
    PathBuf::from(s)
}

/// Vrai si `dest` existe déjà et son hash est valide.
fn already_valid(job: &Job) -> bool {
    job.dest.is_file() && job.hash.matches_file(&job.dest).unwrap_or(false)
}

/// Télécharge un fichier. Retourne `Ok(false)` s'il était déjà présent et valide.
pub fn fetch(client: &Client, job: &Job) -> Result<bool> {
    if already_valid(job) {
        return Ok(false);
    }
    if job.dest.exists() {
        fs::remove_file(&job.dest).with_context(|| format!("suppression de {}", job.dest.display()))?;
    }
    if let Some(parent) = job.dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let part = part_path(&job.dest);
    download_part(client, &job.url, &part, job.size)?;
    if !job.hash.matches_file(&part)? {
        let _ = fs::remove_file(&part);
        bail!("hash invalide pour {} (attendu {}), fichier supprimé", job.url, job.hash.expected());
    }
    fs::rename(&part, &job.dest).with_context(|| format!("renommage vers {}", job.dest.display()))?;
    Ok(true)
}

enum Msg {
    Head(StatusCode),
    Chunk(Vec<u8>),
    Err(String),
}

/// La lecture réseau (bloquante) tourne dans un fil ; ici on écrit et on surveille l'inactivité.
fn download_part(client: &Client, url: &str, part: &Path, expected: Option<u64>) -> Result<()> {
    let mut have = fs::metadata(part).map(|m| m.len()).unwrap_or(0);
    if expected.is_some_and(|e| have > e) {
        fs::remove_file(part)?;
        have = 0;
    }
    let (tx, rx) = mpsc::sync_channel::<Msg>(8);
    let (c, u) = (client.clone(), url.to_string());
    std::thread::spawn(move || {
        let mut req = c.get(&u);
        if have > 0 {
            req = req.header(RANGE, format!("bytes={have}-"));
        }
        let mut resp = match req.send() {
            Ok(r) => r,
            Err(e) => {
                let _ = tx.send(Msg::Err(format!("requête {u} : {e}")));
                return;
            }
        };
        if tx.send(Msg::Head(resp.status())).is_err() {
            return;
        }
        let mut buf = [0u8; 64 * 1024];
        loop {
            match resp.read(&mut buf) {
                Ok(0) => return,
                Ok(n) => {
                    if tx.send(Msg::Chunk(buf[..n].to_vec())).is_err() {
                        return;
                    }
                }
                Err(e) => {
                    let _ = tx.send(Msg::Err(format!("lecture de {u} : {e}")));
                    return;
                }
            }
        }
    });
    let mut file: Option<File> = None;
    let mut written = 0u64;
    loop {
        match rx.recv_timeout(IDLE_TIMEOUT) {
            Ok(Msg::Head(status)) => {
                if status == StatusCode::RANGE_NOT_SATISFIABLE && have > 0 {
                    // Le .part est probablement complet : la vérification du hash tranchera.
                    return Ok(());
                }
                if !status.is_success() {
                    bail!("HTTP {status} pour {url}");
                }
                if status == StatusCode::PARTIAL_CONTENT {
                    file = Some(OpenOptions::new().append(true).open(part)?);
                    written = have;
                } else {
                    file = Some(File::create(part)?); // le serveur ignore Range : on réécrit tout
                }
            }
            Ok(Msg::Chunk(data)) => {
                let Some(f) = file.as_mut() else { bail!("réponse sans en-tête : {url}") };
                f.write_all(&data)?;
                written += data.len() as u64;
                if expected.is_some_and(|e| written > e) {
                    drop(file);
                    let _ = fs::remove_file(part);
                    bail!("{url} dépasse la taille attendue ({} octets), fichier supprimé", expected.unwrap_or(0));
                }
            }
            Ok(Msg::Err(e)) => bail!("{e}"),
            Err(RecvTimeoutError::Timeout) => bail!("aucune donnée reçue depuis {} s : {url}", IDLE_TIMEOUT.as_secs()),
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    let Some(mut f) = file else { bail!("connexion fermée sans réponse : {url}") };
    f.flush()?;
    if let Some(e) = expected
        && written < e
    {
        // On garde le .part : le prochain essai reprend avec Range.
        bail!("flux interrompu pour {url} ({written}/{e} octets)");
    }
    Ok(())
}

/// Compteurs partagés avec l'interface.
#[derive(Default)]
pub struct Progress {
    pub done: AtomicUsize,
    pub total: AtomicUsize,
    pub stage: Mutex<String>,
}

impl Progress {
    pub fn set_stage(&self, s: &str) {
        *self.stage.lock().unwrap() = s.to_string();
        self.done.store(0, Ordering::Relaxed);
        self.total.store(0, Ordering::Relaxed);
    }
}

/// Exécute tous les jobs avec au plus `concurrency` fils. S'arrête à la première erreur.
pub fn fetch_all(client: &Client, jobs: Vec<Job>, concurrency: usize, progress: &Arc<Progress>) -> Result<()> {
    let jobs = dedupe(jobs)?;
    progress.total.fetch_add(jobs.len(), Ordering::Relaxed);
    let next = AtomicUsize::new(0);
    let failed = AtomicBool::new(false);
    let first_err: Mutex<Option<anyhow::Error>> = Mutex::new(None);
    std::thread::scope(|s| {
        for _ in 0..concurrency.clamp(1, jobs.len().max(1)) {
            s.spawn(|| {
                while !failed.load(Ordering::Relaxed) {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(job) = jobs.get(i) else { break };
                    // 2 tentatives : une coupure réseau laisse un .part qui sera repris.
                    let r = fetch(client, job).or_else(|_| fetch(client, job));
                    match r {
                        Ok(_) => {
                            progress.done.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(e) => {
                            failed.store(true, Ordering::Relaxed);
                            first_err.lock().unwrap().get_or_insert(e);
                        }
                    }
                }
            });
        }
    });
    match first_err.into_inner().unwrap() {
        Some(e) => Err(anyhow!(e)),
        None => Ok(()),
    }
}

/// Un seul job par destination (sinon deux fils écriraient le même .part). Même destination avec
/// un hash différent : erreur.
fn dedupe(jobs: Vec<Job>) -> Result<Vec<Job>> {
    let mut by_dest: HashMap<PathBuf, usize> = HashMap::new();
    let mut out: Vec<Job> = Vec::new();
    for j in jobs {
        match by_dest.get(&j.dest) {
            Some(&i) if out[i].hash == j.hash => {}
            Some(_) => bail!("deux fichiers différents visent la même destination : {}", j.dest.display()),
            None => {
                by_dest.insert(j.dest.clone(), out.len());
                out.push(j);
            }
        }
    }
    Ok(out)
}

pub fn get_text(client: &Client, url: &str) -> Result<String> {
    let r = client.get(url).send().with_context(|| format!("requête {url}"))?;
    if !r.status().is_success() {
        bail!("HTTP {} pour {url}", r.status());
    }
    Ok(r.text()?)
}

pub fn get_json<T: serde::de::DeserializeOwned>(client: &Client, url: &str) -> Result<T> {
    let text = get_text(client, url)?;
    serde_json::from_str(&text).with_context(|| format!("JSON invalide depuis {url}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn existing_valid_file_is_skipped_and_bad_one_is_not() {
        let dir = std::env::temp_dir().join(format!("crl-dl-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join("a.bin");
        fs::write(&p, b"abc").unwrap();
        let good = Hash::new("sha1", "a9993e364706816aba3e25717850c26c9cd0d89d").unwrap();
        let bad = Hash::new("sha1", &"0".repeat(40)).unwrap();
        let job = |h| Job { url: "http://x".into(), dest: p.clone(), hash: h, size: None };
        assert!(already_valid(&job(good)));
        assert!(!already_valid(&job(bad)));
        assert_eq!(part_path(&p).file_name().unwrap(), "a.bin.part");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn dedupe_by_destination() {
        let h = |c: &str| Hash::new("sha1", &c.repeat(40)).unwrap();
        let j = |d: &str, hh| Job { url: "u".into(), dest: PathBuf::from(d), hash: hh, size: None };
        let out = dedupe(vec![j("a", h("1")), j("a", h("1")), j("b", h("2"))]).unwrap();
        assert_eq!(out.len(), 2);
        assert!(dedupe(vec![j("a", h("1")), j("a", h("2"))]).is_err());
    }
}
