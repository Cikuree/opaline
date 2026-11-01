//! Sauvegarde automatique des mondes : copie de `saves/` de l'instance vers `<racine>/backups/<pack>/<horodatage>/`.
//! Copie complète dans un dossier temporaire puis renommage ; liens symboliques ignorés ; rien n'est supprimé de `saves/`
//! avant que la copie soit complète. Rotation : les 2 plus récentes plus la plus ancienne (jamais la copie qu'on vient de créer).
//! Pas de nouvelle copie si `saves/` est inchangé depuis la dernière (empreinte mémorisée dans `<id>.fp`). Refus si un monde est ouvert.
use crate::config::Dirs;
use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const KEEP: usize = 3;
const TMP_SUFFIX: &str = ".tmp";
const OPEN_WORLD: &str = "Un monde est ouvert : ferme Minecraft puis réessaie";

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

/// (taille totale, mtime max en secondes, nombre de fichiers) sans suivre les liens.
fn scan(p: &Path) -> (u64, u64, u64) {
    let mut acc = (0u64, 0u64, 0u64);
    for e in fs::read_dir(p).into_iter().flatten().flatten() {
        match e.file_type() {
            Ok(t) if t.is_dir() => {
                let s = scan(&e.path());
                acc = (acc.0 + s.0, acc.1.max(s.1), acc.2 + s.2);
            }
            Ok(t) if t.is_file() => {
                let m = e.metadata().ok();
                let mt = m.as_ref().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
                acc = (acc.0 + m.map_or(0, |m| m.len()), acc.1.max(mt), acc.2 + 1);
            }
            _ => {}
        }
    }
    acc
}

fn fingerprint(p: &Path) -> String {
    let (size, mtime, count) = scan(p);
    format!("{size}:{mtime}:{count}")
}

fn fp_file(dirs: &Dirs, id: &str) -> PathBuf {
    dirs.backups().join(format!("{id}.fp"))
}

/// Une sauvegarde ne change plus une fois créée : sa taille est calculée une seule fois par session.
fn dir_size(p: &Path) -> u64 {
    static CACHE: Mutex<Option<HashMap<PathBuf, u64>>> = Mutex::new(None);
    let mut c = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    *c.get_or_insert_with(HashMap::new).entry(p.to_path_buf()).or_insert_with(|| scan(p).0)
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

/// Au-delà de `KEEP` : garde les `KEEP - 1` plus récentes et la plus ancienne, retire les autres (de la plus ancienne des
/// intermédiaires à la plus récente), jamais un id de `protect` (copie qu'on vient de créer, copie en cours de restauration).
fn rotate(dirs: &Dirs, protect: &[&str]) {
    let all = list(dirs);
    let mut len = all.len();
    if len <= KEEP {
        return;
    }
    for b in all[KEEP - 1..len - 1].iter().rev() {
        if len <= KEEP {
            break;
        }
        if !protect.contains(&b.id.as_str()) && fs::remove_dir_all(dirs.backups().join(&b.id)).is_ok() {
            let _ = fs::remove_file(fp_file(dirs, &b.id));
            len -= 1;
        }
    }
}

/// Un monde est ouvert si un `saves/*/session.lock` est verrouillé par un autre processus (Minecraft tient ce verrou).
/// Windows : verrou exclusif de `std` (même mécanisme que Java). Unix : `fcntl(F_GETLK)` (les verrous de Java sont des verrous POSIX).
pub fn world_open(dirs: &Dirs) -> bool {
    fs::read_dir(saves(dirs)).into_iter().flatten().flatten().any(|w| locked(&w.path().join("session.lock")))
}

#[cfg(windows)]
fn locked(p: &Path) -> bool {
    let Ok(f) = fs::File::open(p) else { return false };
    matches!(f.try_lock(), Err(fs::TryLockError::WouldBlock))
}

#[cfg(unix)]
fn locked(p: &Path) -> bool {
    use std::os::fd::AsRawFd;
    let Ok(f) = fs::File::open(p) else { return false };
    // SAFETY : `flock` est une structure de C entièrement initialisée à zéro puis remplie ; `f` reste ouvert pendant l'appel.
    unsafe {
        let mut l: libc::flock = std::mem::zeroed();
        l.l_type = libc::F_WRLCK as _;
        l.l_whence = libc::SEEK_SET as _;
        libc::fcntl(f.as_raw_fd(), libc::F_GETLK, &mut l) == 0 && l.l_type != libc::F_UNLCK as _
    }
}

#[cfg(not(any(windows, unix)))]
fn locked(_: &Path) -> bool {
    false
}

/// Restauration interrompue : `saves.opaline-old` sans `saves/` est remis en place ; un `old` périmé n'est supprimé que si `saves/` existe.
fn recover(dirs: &Dirs) -> Result<()> {
    let (cur, old) = (saves(dirs), dirs.instance().join("saves.opaline-old"));
    if old.exists() {
        if cur.is_dir() {
            let _ = fs::remove_dir_all(&old);
        } else {
            fs::rename(&old, &cur).context("récupération de saves/ après une restauration interrompue impossible")?;
        }
    }
    Ok(())
}

/// Sauvegarde `saves/` (s'il existe et n'est pas vide). `Ok(None)` : rien à sauvegarder, ou `saves/` inchangé depuis la dernière copie.
/// Refuse si un monde est ouvert.
pub fn backup_saves(dirs: &Dirs, now_secs: u64) -> Result<Option<String>> {
    backup_inner(dirs, now_secs, None)
}

fn backup_inner(dirs: &Dirs, now_secs: u64, protect: Option<&str>) -> Result<Option<String>> {
    recover(dirs)?;
    let src = saves(dirs);
    if !non_empty(&src) {
        return Ok(None);
    }
    if world_open(dirs) {
        bail!(OPEN_WORLD);
    }
    let fp = fingerprint(&src);
    if list(dirs).first().is_some_and(|b| fs::read_to_string(fp_file(dirs, &b.id)).is_ok_and(|s| s == fp)) {
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
    let _ = fs::write(fp_file(dirs, &id), &fp);
    let keep: Vec<&str> = [Some(id.as_str()), protect].into_iter().flatten().collect();
    rotate(dirs, &keep);
    Ok(Some(id))
}

/// Restaure `backup_id` : copie complète dans un dossier voisin, sauvegarde du `saves/` actuel, puis échange.
/// Le jeu de ce pack ne doit pas tourner (vérifié par l'appelant) ; un monde ouvert (Launcher officiel) est refusé.
pub fn restore(dirs: &Dirs, backup_id: &str, now_secs: u64) -> Result<()> {
    if iso_of(backup_id).is_none() || backup_id.contains(['/', '\\', '.']) {
        bail!("sauvegarde inconnue");
    }
    let from = dirs.backups().join(backup_id);
    if !from.is_dir() {
        bail!("sauvegarde introuvable");
    }
    recover(dirs)?;
    if world_open(dirs) {
        bail!(OPEN_WORLD);
    }
    let inst = dirs.instance();
    let (cur, new, old) = (saves(dirs), inst.join("saves.opaline-restore"), inst.join("saves.opaline-old"));
    let _ = fs::remove_dir_all(&new);
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
    if cur.is_dir() {
        let _ = fs::remove_dir_all(&old);
    }
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

    /// Modifie le monde de façon visible pour l'empreinte (taille différente).
    fn touch(dirs: &Dirs, i: u64) {
        fs::write(region(&saves(dirs)), "x".repeat(i as usize + 1)).unwrap();
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
    fn keeps_two_most_recent_plus_the_oldest() {
        let d = setup("rot");
        world(&d, "abc");
        let ids: Vec<String> = (0..5)
            .map(|i| {
                touch(&d, i + 3);
                backup_saves(&d, 1_800_000_000 + i * 10).unwrap().unwrap()
            })
            .collect();
        let got: Vec<String> = list(&d).into_iter().map(|b| b.id).collect();
        assert_eq!(got, [ids[4].clone(), ids[3].clone(), ids[0].clone()]);
        assert!(!fp_file(&d, &ids[1]).exists(), "l'empreinte part avec la sauvegarde");
        // même seconde : pas d'écrasement
        touch(&d, 20);
        let a = backup_saves(&d, 1_800_000_100).unwrap().unwrap();
        touch(&d, 21);
        let b = backup_saves(&d, 1_800_000_100).unwrap().unwrap();
        assert_ne!(a, b);
        let _ = fs::remove_dir_all(&d.root);
    }

    #[test]
    fn clock_behind_never_deletes_the_fresh_copy() {
        let d = setup("clock");
        world(&d, "abc");
        for i in 0..3 {
            touch(&d, i + 3);
            backup_saves(&d, 1_800_000_000 + i * 10).unwrap();
        }
        touch(&d, 50);
        let fresh = backup_saves(&d, 1_700_000_000).unwrap().unwrap(); // horloge en retard : id plus ancien que toutes les autres
        assert!(list(&d).iter().any(|b| b.id == fresh), "la copie qu'on vient de créer survit");
        assert_eq!(list(&d).len(), KEEP);
        let _ = fs::remove_dir_all(&d.root);
    }

    #[test]
    fn unchanged_saves_do_not_create_a_new_copy() {
        let d = setup("fp");
        world(&d, "abc");
        let first = backup_saves(&d, 1_800_000_000).unwrap().unwrap();
        for i in 1..6 {
            assert_eq!(backup_saves(&d, 1_800_000_000 + i * 10).unwrap(), None, "réparation répétée : rien de neuf");
        }
        assert_eq!(list(&d).len(), 1);
        touch(&d, 9);
        assert!(backup_saves(&d, 1_800_000_100).unwrap().is_some());
        assert!(list(&d).iter().any(|b| b.id == first));
        let _ = fs::remove_dir_all(&d.root);
    }

    #[test]
    fn restore_backs_up_current_then_replaces_and_keeps_target() {
        let d = setup("res");
        world(&d, "v1");
        let b1 = backup_saves(&d, 1_800_000_000).unwrap().unwrap();
        for i in 1..3 {
            touch(&d, i + 5);
            backup_saves(&d, 1_800_000_000 + i * 10).unwrap();
        }
        fs::write(region(&saves(&d)), "v2-different").unwrap();
        restore(&d, &b1, 1_800_001_000).unwrap();
        assert_eq!(fs::read_to_string(region(&saves(&d))).unwrap(), "v1");
        let ids: Vec<String> = list(&d).into_iter().map(|b| b.id).collect();
        assert!(ids.contains(&b1));
        assert_eq!(fs::read_to_string(region(&d.backups().join(&ids[0]))).unwrap(), "v2-different", "l'état avant restauration est sauvegardé");
        assert!(!d.instance().join("saves.opaline-old").exists() && !d.instance().join("saves.opaline-restore").exists());
        assert!(restore(&d, "../x", 1).is_err() && restore(&d, "20200101-000000", 1).is_err());
        let _ = fs::remove_dir_all(&d.root);
    }

    #[test]
    fn interrupted_restore_puts_the_old_saves_back() {
        let d = setup("recover");
        world(&d, "monde-precieux");
        let b = backup_saves(&d, 1_800_000_000).unwrap().unwrap();
        // restauration interrompue entre les deux renommages : `saves/` absent, `saves.opaline-old` porte le monde
        fs::rename(saves(&d), d.instance().join("saves.opaline-old")).unwrap();
        restore(&d, &b, 1_800_000_500).unwrap();
        assert_eq!(fs::read_to_string(region(&saves(&d))).unwrap(), "monde-precieux");
        assert!(!d.instance().join("saves.opaline-old").exists());
        // même chose via une simple sauvegarde (installation)
        fs::rename(saves(&d), d.instance().join("saves.opaline-old")).unwrap();
        backup_saves(&d, 1_800_000_900).unwrap();
        assert_eq!(fs::read_to_string(region(&saves(&d))).unwrap(), "monde-precieux");
        // `old` périmé avec un `saves/` valide : supprimé
        fs::create_dir_all(d.instance().join("saves.opaline-old").join("x")).unwrap();
        recover(&d).unwrap();
        assert!(!d.instance().join("saves.opaline-old").exists() && saves(&d).is_dir());
        let _ = fs::remove_dir_all(&d.root);
    }

    #[cfg(windows)]
    #[test]
    fn open_world_blocks_backup_and_restore() {
        let d = setup("lock");
        world(&d, "abc");
        let b = backup_saves(&d, 1_800_000_000).unwrap().unwrap();
        let lock = saves(&d).join("Monde").join("session.lock");
        fs::write(&lock, "x").unwrap();
        assert!(!world_open(&d));
        let f = fs::File::create(&lock).unwrap();
        f.lock().unwrap(); // comme Minecraft pendant une partie
        assert!(world_open(&d));
        touch(&d, 30);
        assert!(backup_saves(&d, 1_800_000_100).unwrap_err().to_string().contains("ferme Minecraft"));
        assert!(restore(&d, &b, 1_800_000_200).unwrap_err().to_string().contains("ferme Minecraft"));
        drop(f);
        assert!(!world_open(&d));
        let _ = fs::remove_dir_all(&d.root);
    }
}
