//! Client de l'endpoint `GET /stats` du serveur Classroom Run.
use anyhow::{Context, Result, bail};
use reqwest::blocking::Client;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default)]
pub struct Stats {
    #[serde(rename = "runNumber")]
    pub run_number: u32,
    #[serde(rename = "totalTimeMs")]
    pub total_time_ms: u64,
    pub running: bool,
    pub deaths: u32,
    #[serde(rename = "currentSplit")]
    pub current_split: i64,
    #[serde(rename = "currentSplitName")]
    pub current_split_name: Option<String>,
    #[serde(rename = "reachedSplits")]
    pub reached_splits: Vec<String>,
    #[serde(rename = "bestSplit")]
    pub best_split: Option<String>,
    pub players: Vec<String>,
}

pub fn fetch(client: &Client, url: &str) -> Result<Stats> {
    let r = client.get(url).send().with_context(|| format!("serveur injoignable ({url})"))?;
    if !r.status().is_success() {
        bail!("HTTP {} sur {url}", r.status());
    }
    r.json().context("réponse /stats invalide")
}

/// `GET url` en JSON brut. 404 = `None` (endpoint ou run absent). Les erreurs ne citent pas l'adresse.
pub fn fetch_value(client: &Client, url: &str) -> Result<Option<serde_json::Value>> {
    use std::io::Read;
    let r = client.get(url).timeout(std::time::Duration::from_secs(5)).send().map_err(|_| anyhow::anyhow!("serveur injoignable"))?;
    match r.status().as_u16() {
        404 => return Ok(None),
        200..=299 => {}
        c => bail!("le serveur a répondu HTTP {c}"),
    }
    let mut b = Vec::new();
    r.take(2 * 1024 * 1024 + 1).read_to_end(&mut b).context("lecture de la réponse")?;
    if b.len() > 2 * 1024 * 1024 {
        bail!("réponse trop volumineuse");
    }
    Ok(Some(serde_json::from_slice(&b).context("réponse du serveur invalide")?))
}

pub fn format_time(ms: u64) -> String {
    let s = ms / 1000;
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_stats_and_tolerates_missing_fields() {
        let s: Stats = serde_json::from_str(
            r#"{"runNumber":2,"totalTimeMs":65000,"running":true,"deaths":1,"currentSplit":3,
            "currentSplitName":"Nether","reachedSplits":["Start","Wood"],"bestSplit":"End","players":["A","B"]}"#,
        )
        .unwrap();
        assert_eq!((s.run_number, s.deaths, s.players.len()), (2, 1, 2));
        assert_eq!(s.current_split_name.as_deref(), Some("Nether"));
        let d: Stats = serde_json::from_str("{}").unwrap();
        assert!(!d.running);
        let n: Stats = serde_json::from_str(r#"{"bestSplit":null,"currentSplitName":null}"#).unwrap();
        assert!(n.best_split.is_none());
    }

    #[test]
    fn formats_time() {
        assert_eq!(format_time(3_725_000), "01:02:05");
        assert_eq!(format_time(0), "00:00:00");
    }
}
