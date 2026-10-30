//! Téléchargements reprenables (Range + `.part`), vérifiés par hash, en parallèle limité.
use crate::safe::Hash;
use anyhow::{Context, Result, anyhow, bail};
use reqwest::StatusCode;
use reqwest::blocking::Client;
use reqwest::header::RANGE;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Job {
    pub url: String,
    pub dest: PathBuf,
    pub hash: Option<Hash>,
}

pub fn http_client() -> Result<Client> {
    Ok(Client::builder()
        .user_agent(concat!("ClassroomRunLauncher/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(600)) // par fichier ; une coupure laisse un .part repris au prochain essai
        .build()?)
}

fn part_path(dest: &Path) -> PathBuf {
    let mut s = dest.as_os_str().to_owned();
    s.push(".part");
    PathBuf::from(s)
}

/// Vrai si `dest` existe déjà et (si un hash est connu) est valide.
fn already_valid(job: &Job) -> bool {
    match (&job.hash, job.dest.is_file()) {
        (_, false) => false,
        (None, true) => true,
        (Some(h), true) => h.matches_file(&job.dest).unwrap_or(false),
    }
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
    download_part(client, &job.url, &part)?;
    if let Some(h) = &job.hash
        && !h.matches_file(&part)?
    {
        let _ = fs::remove_file(&part);
        bail!("hash invalide pour {} (attendu {}), fichier supprimé", job.url, h.expected());
    }
    fs::rename(&part, &job.dest).with_context(|| format!("renommage vers {}", job.dest.display()))?;
    Ok(true)
}

fn download_part(client: &Client, url: &str, part: &Path) -> Result<()> {
    let have = fs::metadata(part).map(|m| m.len()).unwrap_or(0);
    let mut req = client.get(url);
    if have > 0 {
        req = req.header(RANGE, format!("bytes={have}-"));
    }
    let mut resp = req.send().with_context(|| format!("requête {url}"))?;
    let status = resp.status();
    if status == StatusCode::RANGE_NOT_SATISFIABLE && have > 0 {
        // Le .part est probablement complet : la vérification du hash tranchera ; sinon on repart de zéro.
        return Ok(());
    }
    if !status.is_success() {
        bail!("HTTP {status} pour {url}");
    }
    let resumed = status == StatusCode::PARTIAL_CONTENT;
    let mut f = if resumed {
        OpenOptions::new().append(true).open(part)?
    } else {
        File::create(part)? // le serveur ignore Range : on réécrit tout
    };
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = resp.read(&mut buf).with_context(|| format!("lecture de {url}"))?;
        if n == 0 {
            break;
        }
        f.write_all(&buf[..n])?;
    }
    f.flush()?;
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
        let job = |h| Job { url: "http://x".into(), dest: p.clone(), hash: h };
        assert!(already_valid(&job(Some(good))));
        assert!(!already_valid(&job(Some(bad))));
        assert!(already_valid(&job(None)));
        assert_eq!(part_path(&p).file_name().unwrap(), "a.bin.part");
        fs::remove_dir_all(&dir).unwrap();
    }
}
