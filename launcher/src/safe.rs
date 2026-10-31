//! Résolution sécurisée des chemins et vérification de hash.
use anyhow::{Context, Result, bail};
use md5::Md5;
use sha1::{Digest, Sha1};
use sha2::{Sha256, Sha512};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

/// Joint `rel` (venant d'un manifeste, donc non fiable) à `base` sans jamais sortir de `base`.
/// Refuse : chemin vide, absolu, lettre de lecteur / `:`, composante `..`.
pub fn safe_join(base: &Path, rel: &str) -> Result<PathBuf> {
    let norm = rel.replace('\\', "/");
    if norm.is_empty() || norm.starts_with('/') || norm.contains(':') {
        bail!("chemin refusé (vide, absolu ou avec ':') : {rel:?}");
    }
    let mut out = base.to_path_buf();
    for part in norm.split('/') {
        match Path::new(part).components().collect::<Vec<_>>().as_slice() {
            [] => {} // "a//b" ou "./" : ignoré
            [Component::CurDir] => {}
            [Component::Normal(_)] => out.push(part),
            _ => bail!("chemin refusé (composante interdite {part:?}) : {rel:?}"),
        }
    }
    if out == base {
        bail!("chemin refusé (vide après normalisation) : {rel:?}");
    }
    Ok(out)
}

/// Nom de fichier simple (pas de séparateur), pour `filename` d'un .pw.toml.
pub fn safe_file_name(name: &str) -> Result<&str> {
    if name.is_empty() || name.contains(['/', '\\', ':']) || name == ".." || name == "." {
        bail!("nom de fichier refusé : {name:?}");
    }
    Ok(name)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hash {
    Md5(String),
    Sha1(String),
    Sha256(String),
    Sha512(String),
}

impl Hash {
    /// `format` : « sha1 », « sha256 », « sha512 », « md5 » (insensible à la casse).
    pub fn new(format: &str, hex: &str) -> Result<Hash> {
        let h = hex.trim().to_ascii_lowercase();
        Ok(match format.to_ascii_lowercase().as_str() {
            "md5" => Hash::Md5(h),
            "sha1" => Hash::Sha1(h),
            "sha256" => Hash::Sha256(h),
            "sha512" => Hash::Sha512(h),
            other => bail!("algorithme de hash non supporté : {other}"),
        })
    }

    /// (format, hex) : réutilisable avec `Hash::new`.
    pub fn parts(&self) -> (&'static str, &str) {
        match self {
            Hash::Md5(h) => ("md5", h),
            Hash::Sha1(h) => ("sha1", h),
            Hash::Sha256(h) => ("sha256", h),
            Hash::Sha512(h) => ("sha512", h),
        }
    }

    pub fn expected(&self) -> &str {
        match self {
            Hash::Md5(h) | Hash::Sha1(h) | Hash::Sha256(h) | Hash::Sha512(h) => h,
        }
    }

    pub fn compute(&self, r: &mut impl Read) -> Result<String> {
        match self {
            Hash::Md5(_) => digest::<Md5>(r),
            Hash::Sha1(_) => digest::<Sha1>(r),
            Hash::Sha256(_) => digest::<Sha256>(r),
            Hash::Sha512(_) => digest::<Sha512>(r),
        }
    }

    pub fn matches(&self, r: &mut impl Read) -> Result<bool> {
        Ok(self.compute(r)? == self.expected())
    }

    pub fn matches_file(&self, path: &Path) -> Result<bool> {
        let mut f = std::fs::File::open(path).with_context(|| format!("ouverture de {}", path.display()))?;
        self.matches(&mut f)
    }
}

fn digest<D: Digest>(r: &mut impl Read) -> Result<String> {
    let mut d = D::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = r.read(&mut buf)?;
        if n == 0 {
            break;
        }
        d.update(&buf[..n]);
    }
    Ok(hex::encode(d.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_join_accepts_normal_paths() {
        let base = Path::new("base");
        assert_eq!(safe_join(base, "mods/a.jar").unwrap(), Path::new("base/mods/a.jar"));
        assert_eq!(safe_join(base, "mods\\a.jar").unwrap(), Path::new("base/mods/a.jar"));
        assert_eq!(safe_join(base, "./a//b").unwrap(), Path::new("base/a/b"));
    }

    #[test]
    fn safe_join_rejects_escapes() {
        let base = Path::new("base");
        for bad in ["", ".", "../x", "a/../../x", "a/..", "/etc/passwd", "\\x", "C:\\x", "C:x", "a:b", "mods/..\\..\\x"] {
            assert!(safe_join(base, bad).is_err(), "devrait refuser {bad:?}");
        }
    }

    #[test]
    fn safe_file_name_rules() {
        assert!(safe_file_name("fabric-api-0.1+26.3.jar").is_ok());
        for bad in ["", "..", "a/b.jar", "a\\b.jar", "C:x.jar"] {
            assert!(safe_file_name(bad).is_err());
        }
    }

    #[test]
    fn hashes_known_vectors() {
        let data = b"abc";
        let cases = [
            ("md5", "900150983cd24fb0d6963f7d28e17f72"),
            ("sha1", "a9993e364706816aba3e25717850c26c9cd0d89d"),
            ("sha256", "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"),
            ("SHA512", "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"),
        ];
        for (f, h) in cases {
            assert!(Hash::new(f, h).unwrap().matches(&mut &data[..]).unwrap(), "{f}");
            assert!(!Hash::new(f, &"0".repeat(h.len())).unwrap().matches(&mut &data[..]).unwrap());
        }
        assert!(Hash::new("murmur2", "1").is_err());
    }
}
