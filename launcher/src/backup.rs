//! Sauvegarde automatique des mondes : copie de `saves/` de l'instance vers `<racine>/backups/<pack>/<horodatage>/`.
//! Copie complète dans un dossier temporaire puis renommage ; liens symboliques ignorés ; rien n'est supprimé de `saves/`
//! avant que la copie soit complète. Les 3 plus récentes sont conservées.
use crate::config::Dirs;
use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

pub const KEEP: usize = 3;
const TMP_SUFFIX: &str = ".tmp";

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BackupView {
    pub id: String,
    /// ISO 8601 UTC, relu depuis le nom du dossier.
    pub created_at: String,
    pub size_bytes: u64,
}

fn saves(dirs: &Dirs) -> PathBuf {
    dirs.instance().join("saves")
}

/// `20261101-123456` d'après des secondes Unix.
fn stamp(secs: u64) -> String {
    crate::official::iso_utc(secs)[..19].replace(['-', ':'], "").replace('T', "-")
}

fn iso_of(id: &str) -> Option<String> {
    let b = id.as_bytes();
    (b.len() >= 15 && b[8] == b'-' && b[..8].iter().chain(&b[9..15]).all(u8::is_ascii_digit))
        .then(|| format!("{}-{}-{}T{}:{}:{}Z", &id[..4], &id[4..6], &id[6..8], &id[9..11], &id[11..13], &id[13..15]))
}

fn non_empty(dir: &Path) -> bool {
    fs::read_dir(dir).is_ok_and(|mut r| r.next().is_some())
}

/// Copie récursive sans suivre les liens (ignorés). Renvoie le nombre d'octets copiés.
fn copy_tree(src: &Path, dst: &Path) -> Result<u64> {
    fs::create_dir_all(dst)?;
    let mut total = 0;
    for e in fs::read_dir(src).with_context(|| format!("lecture de {}", src.display()))? {
        let e = e?;
        let t = e.file_type()?;
        let to = dst.join(e.file_name());
        if t.is_dir() {
            total += copy_tree(&e.path(), &to)?;
        } else if t.is_file() {
            total += fs::copy(e.path(), &to).with_context(|| format!("copie de {}", e.path().display()))?;
        }
    }
    Ok(total)
}

fn dir_size(p: &Path) -> u64 {
    let size = |e: &fs::DirEntry| match e.file_type() {
        Ok(t) if t.is_dir() => dir_size(&e.path()),
        Ok(t) if t.is_file() => e.metadata().map_or(0, |m| m.len()),
        _ => 0,
    };
    fs::read_dir(p).map_or(0, |rd| rd.flatten().map(|e| size(&e)).sum())
}

/// Sauvegardes du pack, la plus récente d'abord.
pub fn list(dirs: &Dirs) -> Vec<BackupView> {
    let mut v: Vec<BackupView> = fs::read_dir(dirs.backups())
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| {
            let id = e.file_name().to_string_lossy().into_owned();
            let created_at = iso_of(&id).filter(|_| !id.ends_with(TMP_SUFFIX))?;
            Some(BackupView { created_at, size_bytes: dir_size(&e.path()), id })
        })
        .collect();
    v.sort_by(|a, b| b.id.cmp(&a.id));
    v
}

/// Supprime les plus anciennes au-delà de `KEEP` (jamais `protect`).
fn rotate(dirs: &Dirs, protect: Option<&str>) {
    let all = list(dirs);
    let mut extra = all.len().saturating_sub(KEEP);
    for b in all.iter().rev() {
        if extra == 0 {
            break;
        }
        if Some(b.id.as_str()) != protect && fs::remove_dir_all(dirs.backups().join(&b.id)).is_ok() {
            extra -= 1;
        }
    }
}

/// Sauvegarde `saves/` (s'il existe et n'est pas vide). `Ok(None)` : rien à sauvegarder.
pub fn backup_saves(dirs: &Dirs, now_secs: u64) -> Result<Option<String>> {
    backup_inner(dirs, now_secs, None)
}

fn backup_inner(dirs: &Dirs, now_secs: u64, protect: Option<&str>) -> Result<Option<String>> {
    let src = saves(dirs);
    if !non_empty(&src) {
        return Ok(None);
    }
    let base = stamp(now_secs);
    let mut id = base.clone();
    let mut n = 1;
    while dirs.backups().join(&id).exists() {
        id = format!("{base}-{n}");
        n += 1;
    }
    let tmp = dirs.backups().join(format!("{id}{TMP_SUFFIX}"));
    let _ = fs::remove_dir_all(&tmp);
    if let Err(e) = copy_tree(&src, &tmp).and_then(|_| fs::rename(&tmp, dirs.backups().join(&id)).map_err(Into::into)) {
        let _ = fs::remove_dir_all(&tmp);
        return Err(e.context("sauvegarde des mondes impossible"));
    }
    rotate(dirs, protect);
    Ok(Some(id))
}

/// Restaure `backup_id` : copie complète dans un dossier voisin, sauvegarde du `saves/` actuel, puis échange.
/// Le jeu de ce pack ne doit pas tourner (vérifié par l'appelant).
pub fn restore(dirs: &Dirs, backup_id: &str, now_secs: u64) -> Result<()> {
    if iso_of(backup_id).is_none() || backup_id.contains(['/', '\\', '.']) {
        bail!("sauvegarde inconnue");
    }
    let from = dirs.backups().join(backup_id);
    if !from.is_dir() {
        bail!("sauvegarde introuvable");
    }
    let inst = dirs.instance();
    let (cur, new, old) = (saves(dirs), inst.join("saves.opaline-restore"), inst.join("saves.opaline-old"));
    let _ = fs::remove_dir_all(&new);
    let _ = fs::remove_dir_all(&old);
    if let Err(e) = copy_tree(&from, &new) {
        let _ = fs::remove_dir_all(&new);
        return Err(e.context("restauration impossible : copie de la sauvegarde"));
    }
    if let Err(e) = backup_inner(dirs, now_secs, Some(backup_id)) {
        let _ = fs::remove_dir_all(&new);
        return Err(e.context("le monde actuel n'a pas pu être sauvegardé : restauration annulée"));
    }
    let had = cur.exists();
    if had {
        fs::rename(&cur, &old).context("remplacement de saves/ impossible (un monde est-il ouvert ailleurs ?)")?;
    }
    if let Err(e) = fs::rename(&new, &cur) {
        if had {
            let _ = fs::rename(&old, &cur);
        }
        return Err(anyhow::Error::from(e).context("restauration impossible : saves/ remis en l'état"));
    }
    let _ = fs::remove_dir_all(&old);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(tag: &str) -> Dirs {
        let d = std::env::temp_dir().join(format!("opaline-bk-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        Dirs::new(&d).for_pack("p").unwrap()
    }

    fn world(dirs: &Dirs, content: &str) {
        let w = saves(dirs).join("Monde").join("region");
        fs::create_dir_all(&w).unwrap();
        fs::write(w.join("r.0.0.mca"), content).unwrap();
    }

    fn region(base: &Path) -> PathBuf {
        base.join("Monde").join("region").join("r.0.0.mca")
    }

    #[test]
    fn stamp_roundtrip() {
        let s = stamp(1_793_536_496);
        assert_eq!(s.len(), 15);
        assert!(iso_of(&s).unwrap().ends_with('Z'));
        assert_eq!(iso_of("20261101-120816").unwrap(), "2026-11-01T12:08:16Z");
        assert_eq!(iso_of("x"), None);
    }

    #[test]
    fn empty_or_missing_saves_are_not_backed_up() {
        let d = setup("empty");
        assert_eq!(backup_saves(&d, 1000).unwrap(), None);
        fs::create_dir_all(saves(&d)).unwrap();
        assert_eq!(backup_saves(&d, 1000).unwrap(), None);
        let _ = fs::remove_dir_all(&d.root);
    }

    #[test]
    fn keeps_three_most_recent() {
        let d = setup("rot");
        world(&d, "abc");
        let ids: Vec<String> = (0..5).map(|i| backup_saves(&d, 1_800_000_000 + i * 10).unwrap().unwrap()).collect();
        let l = list(&d);
        assert_eq!(l.iter().map(|b| b.id.clone()).collect::<Vec<_>>(), [ids[4].clone(), ids[3].clone(), ids[2].clone()]);
        assert_eq!(l[0].size_bytes, 3);
        // même seconde : pas d'écrasement
        let a = backup_saves(&d, 1_800_000_100).unwrap().unwrap();
        let b = backup_saves(&d, 1_800_000_100).unwrap().unwrap();
        assert_ne!(a, b);
        let _ = fs::remove_dir_all(&d.root);
    }

    #[test]
    fn restore_backs_up_current_then_replaces_and_keeps_target() {
        let d = setup("res");
        world(&d, "v1");
        let b1 = backup_saves(&d, 1_800_000_000).unwrap().unwrap();
        for i in 1..3 {
            backup_saves(&d, 1_800_000_000 + i * 10).unwrap();
        }
        fs::write(region(&saves(&d)), "v2").unwrap();
        restore(&d, &b1, 1_800_001_000).unwrap(); // b1 est la plus ancienne : elle doit survivre à la rotation
        assert_eq!(fs::read_to_string(region(&saves(&d))).unwrap(), "v1");
        let ids: Vec<String> = list(&d).into_iter().map(|b| b.id).collect();
        assert!(ids.contains(&b1));
        assert_eq!(fs::read_to_string(region(&d.backups().join(&ids[0]))).unwrap(), "v2", "l'état avant restauration est sauvegardé");
        assert!(!d.instance().join("saves.opaline-old").exists() && !d.instance().join("saves.opaline-restore").exists());
        assert!(restore(&d, "../x", 1).is_err() && restore(&d, "20200101-000000", 1).is_err());
        let _ = fs::remove_dir_all(&d.root);
    }
}
