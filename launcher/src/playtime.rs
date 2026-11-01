//! Temps de jeu par pack, mesuré par Opaline du lancement direct à la sortie du jeu (`playtime.json`, écriture atomique).
//! Le mode officiel n'est pas mesuré (le processus n'appartient pas à Opaline).
use crate::config::Dirs;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Mutex;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Playtime {
    pub total_secs: u64,
    /// Secondes Unix de la fin de la dernière session.
    pub last_played: u64,
}

type Store = BTreeMap<String, Playtime>;

/// Sérialise les lectures-modifications-écritures (deux jeux de packs différents peuvent se terminer ensemble).
static LOCK: Mutex<()> = Mutex::new(());

fn path(dirs: &Dirs) -> std::path::PathBuf {
    dirs.root.join("playtime.json")
}

pub fn load(dirs: &Dirs) -> Store {
    std::fs::read_to_string(path(dirs)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

/// Ajoute une session terminée à `now_secs`. Une session de 0 s n'est pas enregistrée.
pub fn record(dirs: &Dirs, pack: &str, duration_secs: u64, now_secs: u64) -> Result<()> {
    if duration_secs == 0 {
        return Ok(());
    }
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut s = load(dirs);
    let e = s.entry(pack.to_string()).or_default();
    e.total_secs = e.total_secs.saturating_add(duration_secs);
    e.last_played = now_secs;
    std::fs::create_dir_all(&dirs.root)?;
    let (p, tmp) = (path(dirs), dirs.root.join("playtime.json.tmp"));
    std::fs::write(&tmp, serde_json::to_vec_pretty(&s)?)?;
    std::fs::rename(&tmp, &p)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sessions_accumulate_and_persist() {
        let d = Dirs::new(std::env::temp_dir().join(format!("opaline-pt-{}", std::process::id())));
        let _ = std::fs::remove_dir_all(&d.root);
        assert!(load(&d).is_empty());
        record(&d, "a", 600, 1000).unwrap();
        record(&d, "a", 60, 2000).unwrap();
        record(&d, "b", 5, 1500).unwrap();
        record(&d, "b", 0, 9999).unwrap();
        let s = load(&d);
        assert_eq!(s["a"], Playtime { total_secs: 660, last_played: 2000 });
        assert_eq!(s["b"], Playtime { total_secs: 5, last_played: 1500 });
        assert!(!d.root.join("playtime.json.tmp").exists());
        std::fs::write(d.root.join("playtime.json"), "pas du json").unwrap();
        assert!(load(&d).is_empty(), "fichier corrompu : repart de zéro");
        std::fs::remove_dir_all(&d.root).unwrap();
    }
}
