//! Archive d'overrides d'un pack (resourcepacks, datapacks, shaderpacks...) : extraction sûre dans l'instance et
//! suppression des fichiers qu'une archive précédente avait posés et que la nouvelle n'a plus.
use crate::safe::safe_join;
use anyhow::{Context, Result, bail};
use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;

pub const MAX_ENTRIES: usize = 60_000;
pub const MAX_UNPACKED: u64 = 1024 * 1024 * 1024;

/// Chemin relatif normalisé (`/`) d'une entrée, ou `None` si l'entrée n'est jamais extraite : `options.txt` (géré par les
/// modes de performance, que l'archive ne doit pas écraser), `installed.ok`, `opaline-pack.json` (état d'Opaline) et
/// `mods/` (géré par le manifeste du pack). Erreur si le chemin sort du dossier (absolu, `..`, lecteur Windows).
fn rel_path(name: &str) -> Result<Option<String>> {
    safe_join(Path::new("x"), name)?;
    let rel = name.replace('\\', "/").split('/').filter(|p| !p.is_empty() && *p != ".").collect::<Vec<_>>().join("/");
    let first = rel.split('/').next().unwrap_or("").to_ascii_lowercase();
    let reserved = matches!(rel.to_ascii_lowercase().as_str(), "options.txt" | "installed.ok" | "opaline-pack.json") || first == "mods";
    Ok((!reserved).then_some(rel))
}

/// Supprime de l'instance les fichiers de `previous` absents de `keep` (jamais d'autre fichier, jamais de dossier non vide).
pub fn remove_stale(instance: &Path, previous: &[String], keep: &HashSet<String>) {
    for rel in previous.iter().filter(|r| !keep.contains(*r)) {
        let Ok(Some(rel)) = rel_path(rel) else { continue };
        let Ok(path) = safe_join(instance, &rel) else { continue };
        if fs::symlink_metadata(&path).is_ok_and(|m| m.is_file()) && fs::remove_file(&path).is_ok() {
            for dir in path.ancestors().skip(1).take_while(|d| *d != instance) {
                if fs::remove_dir(dir).is_err() {
                    break;
                }
            }
        }
    }
}

/// Extrait `archive` dans `instance` après validation complète (rien n'est écrit si une entrée est refusée), retire les
/// fichiers de `previous` que l'archive n'a plus, et renvoie la liste triée des fichiers posés.
pub fn extract(archive: &Path, instance: &Path, previous: &[String]) -> Result<Vec<String>> {
    extract_with(archive, instance, previous, MAX_ENTRIES, MAX_UNPACKED)
}

fn extract_with(archive: &Path, instance: &Path, previous: &[String], max_entries: usize, max_unpacked: u64) -> Result<Vec<String>> {
    let mut zip = zip::ZipArchive::new(fs::File::open(archive).context("archive d'overrides illisible")?).context("archive d'overrides invalide")?;
    if zip.len() > max_entries {
        bail!("archive d'overrides : {} entrées, limite {max_entries}", zip.len());
    }
    let (mut total, mut wanted) = (0u64, Vec::new());
    for i in 0..zip.len() {
        let f = zip.by_index(i)?;
        let name = f.name()?.into_owned();
        if f.is_dir() {
            continue; // les dossiers ne sont créés que comme parents de fichiers validés
        }
        if f.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
            bail!("archive d'overrides : lien symbolique refusé ({name:?})");
        }
        total = total.saturating_add(f.size());
        if total > max_unpacked {
            bail!("archive d'overrides : plus de {} octets décompressés", max_unpacked);
        }
        if let Some(rel) = rel_path(&name)? {
            wanted.push((i, rel));
        }
    }
    let keep: HashSet<String> = wanted.iter().map(|(_, r)| r.clone()).collect();
    remove_stale(instance, previous, &keep);
    let mut written = 0u64;
    for (i, rel) in &wanted {
        let dest = safe_join(instance, rel)?;
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = fs::File::create(&dest).with_context(|| format!("écriture de {rel}"))?;
        // La taille annoncée peut mentir : on borne aussi les octets réellement lus.
        written += std::io::copy(&mut zip.by_index(*i)?.take(max_unpacked + 1 - written), &mut out)?;
        out.flush()?;
        if written > max_unpacked {
            bail!("archive d'overrides : plus de {max_unpacked} octets décompressés");
        }
    }
    let mut files: Vec<String> = keep.into_iter().collect();
    files.sort();
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("crl-ovr-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn zip_of(path: &Path, entries: &[(&str, &[u8])]) {
        let mut w = zip::ZipWriter::new(fs::File::create(path).unwrap());
        for (n, b) in entries {
            w.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(b).unwrap();
        }
        w.finish().unwrap();
    }

    #[test]
    fn extracts_skips_reserved_and_removes_only_stale_files() {
        let d = tmp("ok");
        let inst = d.join("inst");
        fs::create_dir_all(inst.join("saves/w")).unwrap();
        fs::write(inst.join("saves/w/level.dat"), b"keep").unwrap();
        fs::write(inst.join("options.txt"), b"mine").unwrap();
        zip_of(&d.join("a.zip"), &[("resourcepacks/r.zip", b"r1"), ("datapacks/d/pack.mcmeta", b"{}"), ("options.txt", b"theirs"), ("mods/x.jar", b"x")]);
        let files = extract(&d.join("a.zip"), &inst, &[]).unwrap();
        assert_eq!(files, ["datapacks/d/pack.mcmeta", "resourcepacks/r.zip"]);
        assert_eq!(fs::read(inst.join("options.txt")).unwrap(), b"mine");
        assert!(!inst.join("mods").exists());
        zip_of(&d.join("b.zip"), &[("resourcepacks/r.zip", b"r2")]);
        let files2 = extract(&d.join("b.zip"), &inst, &files).unwrap();
        assert_eq!(files2, ["resourcepacks/r.zip"]);
        assert_eq!(fs::read(inst.join("resourcepacks/r.zip")).unwrap(), b"r2");
        assert!(!inst.join("datapacks").exists(), "fichier et dossier vide retirés");
        assert_eq!(fs::read(inst.join("saves/w/level.dat")).unwrap(), b"keep");
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn booby_trapped_archives_are_refused_without_writing() {
        let d = tmp("evil");
        let inst = d.join("inst");
        for bad in ["../evil.txt", "a/../../evil.txt", "/abs.txt", "C:/win.txt", "C:\\win.txt", "..\\evil.txt"] {
            zip_of(&d.join("e.zip"), &[("ok.txt", b"1"), (bad, b"x")]);
            assert!(extract(&d.join("e.zip"), &inst, &[]).is_err(), "{bad}");
            assert!(!inst.join("ok.txt").exists() && !d.join("evil.txt").exists(), "{bad}");
        }
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn symlinks_too_many_entries_and_too_much_data_are_refused() {
        let d = tmp("lim");
        let inst = d.join("inst");
        let mut w = zip::ZipWriter::new(fs::File::create(d.join("l.zip")).unwrap());
        w.add_symlink("link", "/etc", zip::write::SimpleFileOptions::default()).unwrap();
        w.finish().unwrap();
        assert!(extract(&d.join("l.zip"), &inst, &[]).is_err());
        zip_of(&d.join("n.zip"), &[("a", b"1"), ("b", b"1"), ("c", b"1")]);
        assert!(extract_with(&d.join("n.zip"), &inst, &[], 2, 100).unwrap_err().to_string().contains("entrées"));
        zip_of(&d.join("b.zip"), &[("a", &[0u8; 60]), ("b", &[0u8; 60])]);
        assert!(extract_with(&d.join("b.zip"), &inst, &[], 10, 100).unwrap_err().to_string().contains("décompressés"));
        assert!(!inst.join("a").exists());
        assert!(extract_with(&d.join("b.zip"), &inst, &[], 10, 120).is_ok());
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn stale_list_cannot_delete_outside_or_reserved_files() {
        let d = tmp("stale");
        let inst = d.join("inst");
        fs::create_dir_all(&inst).unwrap();
        fs::write(d.join("victim.txt"), b"v").unwrap();
        fs::write(inst.join("options.txt"), b"o").unwrap();
        remove_stale(&inst, &["../victim.txt".into(), "options.txt".into(), "/x".into()], &HashSet::new());
        assert!(d.join("victim.txt").exists() && inst.join("options.txt").exists());
        fs::remove_dir_all(&d).unwrap();
    }
}
