//! Java via l'API Adoptium (Temurin JRE) : sélection de l'asset, extraction zip / tar.gz.
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Debug)]
struct Asset {
    binary: Binary,
}
#[derive(Deserialize, Debug)]
struct Binary {
    package: Package,
}
#[derive(Deserialize, Debug, Clone)]
pub struct Package {
    pub link: String,
    pub name: String,
    pub checksum: String, // sha256
}

pub fn adoptium_os() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        "linux"
    }
}

pub fn adoptium_arch() -> &'static str {
    if std::env::consts::ARCH == "aarch64" { "aarch64" } else { "x64" }
}

/// `GET {base}/v3/assets/latest/{major}/hotspot?architecture=..&image_type=jre&os=..`
pub fn assets_url(base: &str, major: u32) -> String {
    format!(
        "{}/v3/assets/latest/{major}/hotspot?architecture={}&image_type=jre&os={}",
        base.trim_end_matches('/'),
        adoptium_arch(),
        adoptium_os()
    )
}

pub fn parse_assets(json: &str) -> Result<Package> {
    let v: Vec<Asset> = serde_json::from_str(json).context("réponse Adoptium invalide")?;
    match v.into_iter().next() {
        Some(a) => Ok(a.binary.package),
        None => bail!("Adoptium ne propose aucun JRE pour cette plateforme"),
    }
}

/// Cherche `bin/java[.exe]` (au plus 4 niveaux : `jdk-25+8-jre/bin`, ou `Contents/Home/bin` sur macOS).
pub fn find_java(dir: &Path) -> Option<PathBuf> {
    let exe = if cfg!(windows) { "java.exe" } else { "java" };
    fn walk(d: &Path, exe: &str, depth: u32) -> Option<PathBuf> {
        let cand = d.join("bin").join(exe);
        if cand.is_file() {
            return Some(cand);
        }
        if depth == 0 {
            return None;
        }
        fs::read_dir(d).ok()?.flatten().filter(|e| e.path().is_dir()).find_map(|e| walk(&e.path(), exe, depth - 1))
    }
    walk(dir, exe, 4)
}

/// Extrait `archive` (.zip ou .tar.gz) dans `dest`. Les chemins d'entrées qui sortiraient de `dest` sont refusés.
pub fn extract(archive: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)?;
    let name = archive.to_string_lossy();
    if name.ends_with(".zip") {
        let mut z = zip::ZipArchive::new(File::open(archive)?)?;
        for i in 0..z.len() {
            let mut e = z.by_index(i)?;
            let Some(rel) = e.enclosed_name() else {
                bail!("entrée zip refusée (chemin hors du dossier, index {i})");
            };
            let out = dest.join(rel);
            if e.is_dir() {
                fs::create_dir_all(&out)?;
            } else {
                if let Some(p) = out.parent() {
                    fs::create_dir_all(p)?;
                }
                io::copy(&mut e, &mut File::create(&out)?)?;
            }
        }
    } else if name.ends_with(".tar.gz") {
        let mut t = tar::Archive::new(flate2::read::GzDecoder::new(File::open(archive)?));
        for e in t.entries()? {
            let mut e = e?;
            if e.header().entry_type().is_symlink() || e.header().entry_type().is_hard_link() {
                let target = e.link_name()?.map(|t| t.into_owned()).unwrap_or_default();
                let escapes = target.has_root() || target.components().any(|c| matches!(c, std::path::Component::ParentDir | std::path::Component::Prefix(_)));
                if escapes {
                    bail!("lien tar refusé (cible hors du dossier) : {} -> {}", e.path()?.display(), target.display());
                }
            }
            // unpack_in refuse (false) tout chemin qui sort de `dest`.
            if !e.unpack_in(dest)? {
                bail!("entrée tar refusée : {}", e.path()?.display());
            }
        }
    } else {
        bail!("format d'archive non supporté : {name}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_adoptium_json() {
        let j = r#"[{"binary":{"architecture":"x64","package":{"checksum":"ab","link":"https://x/a.zip","name":"a.zip","size":1}}}]"#;
        let p = parse_assets(j).unwrap();
        assert_eq!((p.link.as_str(), p.name.as_str(), p.checksum.as_str()), ("https://x/a.zip", "a.zip", "ab"));
        assert!(parse_assets("[]").is_err());
    }

    #[test]
    fn assets_url_shape() {
        let u = assets_url("http://h/", 25);
        assert!(u.starts_with("http://h/v3/assets/latest/25/hotspot?architecture="));
        assert!(u.contains("image_type=jre"));
    }

    #[test]
    fn zip_extract_and_find_java() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("crl-java-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let zp = dir.join("j.zip");
        let exe = if cfg!(windows) { "java.exe" } else { "java" };
        {
            let mut w = zip::ZipWriter::new(File::create(&zp).unwrap());
            let o = zip::write::SimpleFileOptions::default();
            w.start_file(format!("jdk-25-jre/bin/{exe}"), o).unwrap();
            w.write_all(b"x").unwrap();
            w.finish().unwrap();
        }
        let out = dir.join("out");
        extract(&zp, &out).unwrap();
        assert!(find_java(&out).unwrap().ends_with(format!("bin/{exe}")));
        fs::remove_dir_all(&dir).unwrap();
    }

    fn tar_gz(entries: &[(&str, tar::EntryType, &str)]) -> Vec<u8> {
        let mut b = tar::Builder::new(Vec::new());
        for (name, ty, link) in entries {
            let mut h = tar::Header::new_gnu();
            h.set_entry_type(*ty);
            h.set_size(if *ty == tar::EntryType::Regular { 1 } else { 0 });
            h.set_mode(0o644);
            if !link.is_empty() {
                h.set_link_name(link).unwrap();
            }
            b.append_data(&mut h, name, if *ty == tar::EntryType::Regular { &b"x"[..] } else { &b""[..] }).unwrap();
        }
        let raw = b.into_inner().unwrap();
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        io::Write::write_all(&mut gz, &raw).unwrap();
        gz.finish().unwrap()
    }

    #[test]
    fn tar_gz_extracts_and_refuses_escaping_links() {
        let dir = std::env::temp_dir().join(format!("crl-tar-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let ok = dir.join("ok.tar.gz");
        fs::write(&ok, tar_gz(&[("jdk/bin/java", tar::EntryType::Regular, "")])).unwrap();
        extract(&ok, &dir.join("out")).unwrap();
        assert!(find_java(&dir.join("out")).is_some() || cfg!(windows)); // java.exe attendu sous Windows
        assert!(dir.join("out/jdk/bin/java").is_file());
        for (i, (ty, target)) in [(tar::EntryType::Symlink, "../../etc/passwd"), (tar::EntryType::Symlink, "/etc/passwd"), (tar::EntryType::Link, "../x")].into_iter().enumerate() {
            let bad = dir.join(format!("bad{i}.tar.gz"));
            fs::write(&bad, tar_gz(&[("jdk/l", ty, target)])).unwrap();
            assert!(extract(&bad, &dir.join(format!("out{i}"))).is_err(), "{target}");
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn zip_slip_is_refused() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("crl-slip-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let zp = dir.join("e.zip");
        {
            let mut w = zip::ZipWriter::new(File::create(&zp).unwrap());
            w.start_file("../evil.txt", zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(b"x").unwrap();
            w.finish().unwrap();
        }
        assert!(extract(&zp, &dir.join("out")).is_err());
        assert!(!dir.join("evil.txt").exists());
        fs::remove_dir_all(&dir).unwrap();
    }
}
