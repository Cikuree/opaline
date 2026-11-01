//! Manifeste du pack installé (instantané), liste des mods et différence installé / publié.
use crate::config::Dirs;
use crate::packwiz::Pack;
use crate::safe::{Hash, safe_join};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SnapMod {
    pub name: String,
    pub version: String,
    pub side: String,
    /// Dossier d'instance (`mods`, `shaderpacks`...).
    pub dir: String,
    pub file: String,
    pub hash_format: String,
    pub hash: String,
    /// Projet Modrinth (absent des instantanés anciens et des mods hors Modrinth).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub mods: Vec<SnapMod>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Updated {
    pub name: String,
    pub from: String,
    pub to: String,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct Changelog {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub updated: Vec<Updated>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct ModView {
    pub name: String,
    pub version: String,
    pub side: String,
    pub file: String,
    pub verified: bool,
}

/// Version lisible d'après le nom de fichier : `sodium-fabric-0.9.2+mc26.3.jar` -> `0.9.2+mc26.3`.
/// Premier segment (séparé par `-` ou `_`) qui commence par un chiffre, puis tout ce qui suit ;
/// à défaut, `r5` / `v2` ; à défaut, le nom sans extension.
pub fn version_of(file: &str) -> String {
    let stem = file.rsplit_once('.').filter(|(_, e)| matches!(*e, "jar" | "zip")).map_or(file, |(s, _)| s);
    let parts: Vec<&str> = stem.split(['-', '_']).collect();
    let starts_digit = |p: &&str| p.chars().next().is_some_and(|c| c.is_ascii_digit());
    let rel = |p: &&str| matches!(p.chars().next(), Some('r' | 'v' | 'R' | 'V')) && p.chars().nth(1).is_some_and(|c| c.is_ascii_digit());
    match parts.iter().position(starts_digit).or_else(|| parts.iter().position(rel)) {
        Some(i) => parts[i..].join("-"),
        None => stem.to_string(),
    }
}

pub fn snapshot_of(pack: &Pack) -> Snapshot {
    Snapshot {
        mods: pack
            .mods
            .iter()
            .map(|m| {
                let (hash_format, hash) = m.hash.parts();
                SnapMod {
                    name: m.name.clone(),
                    version: version_of(&m.filename),
                    side: m.side.clone(),
                    dir: m.dir.clone(),
                    file: m.filename.clone(),
                    hash_format: hash_format.into(),
                    hash: hash.into(),
                    project_id: m.project_id.clone(),
                }
            })
            .collect(),
    }
}

pub fn diff(installed: &Snapshot, published: &Snapshot) -> Changelog {
    let find = |s: &'_ Snapshot, m: &SnapMod| s.mods.iter().find(|o| o.name == m.name && o.dir == m.dir).cloned();
    let mut c = Changelog::default();
    for n in &published.mods {
        match find(installed, n) {
            None => c.added.push(n.name.clone()),
            Some(o) if o.file != n.file || o.hash != n.hash => c.updated.push(Updated { name: n.name.clone(), from: o.version, to: n.version.clone() }),
            Some(_) => {}
        }
    }
    c.removed = installed.mods.iter().filter(|o| find(published, o).is_none()).map(|o| o.name.clone()).collect();
    c
}

pub fn load_snapshot(dirs: &Dirs) -> Result<Snapshot> {
    let b = std::fs::read(dirs.pack_snapshot()).context("pack non installé (manifeste absent) : lancez l'installation")?;
    serde_json::from_slice(&b).context("manifeste installé illisible : relancez l'installation")
}

pub fn save_snapshot(dirs: &Dirs, s: &Snapshot) -> Result<()> {
    std::fs::create_dir_all(dirs.instance())?;
    std::fs::write(dirs.pack_snapshot(), serde_json::to_vec_pretty(s)?)?;
    Ok(())
}

/// Mods de l'instance avec vérification du hash du fichier présent.
/// Résultat de vérification mémorisé tant que le fichier garde la même taille et la même date : sans ce cache, changer de pack
/// dans « Fichiers & mods » relisait tous les jars (centaines de Mo pour un gros pack).
fn verified_cached(path: &std::path::Path, hash: &str, check: impl FnOnce() -> Result<bool>) -> Result<bool> {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    type Key = (std::path::PathBuf, String, u64, Option<std::time::SystemTime>);
    static CACHE: OnceLock<Mutex<HashMap<Key, bool>>> = OnceLock::new();
    let meta = std::fs::metadata(path)?;
    let key = (path.to_path_buf(), hash.to_string(), meta.len(), meta.modified().ok());
    let cache = CACHE.get_or_init(Default::default);
    if let Some(&ok) = cache.lock().unwrap().get(&key) {
        return Ok(ok);
    }
    let ok = check()?;
    cache.lock().unwrap().insert(key, ok);
    Ok(ok)
}

pub fn mod_views(dirs: &Dirs, s: &Snapshot) -> Vec<ModView> {
    s.mods
        .iter()
        .map(|m| {
            let verified = (|| -> Result<bool> {
                let p = safe_join(&safe_join(&dirs.instance(), &m.dir)?, &m.file)?;
                verified_cached(&p, &m.hash, || Hash::new(&m.hash_format, &m.hash)?.matches_file(&p))
            })()
            .unwrap_or(false);
            ModView { name: m.name.clone(), version: m.version.clone(), side: m.side.clone(), file: m.file.clone(), verified }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(name: &str, file: &str, hash: &str) -> SnapMod {
        SnapMod {
            name: name.into(),
            version: version_of(file),
            side: "both".into(),
            dir: "mods".into(),
            file: file.into(),
            hash_format: "sha1".into(),
            hash: hash.into(),
            project_id: None,
        }
    }

    #[test]
    fn versions_from_file_names() {
        assert_eq!(version_of("sodium-fabric-0.9.2+mc26.3.jar"), "0.9.2+mc26.3");
        assert_eq!(version_of("fabric-api-0.165.0+26.3.jar"), "0.165.0+26.3");
        assert_eq!(version_of("ComplementaryReimagined_r5.7.1.zip"), "r5.7.1");
        assert_eq!(version_of("classroomrun-0.2.0.jar"), "0.2.0");
        assert_eq!(version_of("weird.jar"), "weird");
    }

    #[test]
    fn diff_detects_added_removed_updated() {
        let old = Snapshot { mods: vec![m("Sodium", "sodium-0.9.2.jar", "a"), m("Iris", "iris-1.0.jar", "b"), m("Gone", "gone-1.jar", "c")] };
        let new = Snapshot { mods: vec![m("Sodium", "sodium-0.9.3.jar", "d"), m("Iris", "iris-1.0.jar", "b"), m("Fresh", "fresh-2.jar", "e")] };
        let c = diff(&old, &new);
        assert_eq!(c.added, ["Fresh"]);
        assert_eq!(c.removed, ["Gone"]);
        assert_eq!(c.updated, [Updated { name: "Sodium".into(), from: "0.9.2".into(), to: "0.9.3".into() }]);
        assert_eq!(diff(&old, &old), Changelog::default());
    }

    #[test]
    fn same_file_new_hash_counts_as_update() {
        let old = Snapshot { mods: vec![m("A", "a-1.jar", "x")] };
        let new = Snapshot { mods: vec![m("A", "a-1.jar", "y")] };
        assert_eq!(diff(&old, &new).updated.len(), 1);
    }
}
