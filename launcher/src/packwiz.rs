//! Lecture du manifeste packwiz depuis un dossier local (ou le pack.zip de la release, extrait puis lu ici).
use crate::safe::{Hash, safe_file_name, safe_join};
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::path::Path;

#[derive(Deserialize)]
struct PackToml {
    index: PackIndex,
    versions: Versions,
}
#[derive(Deserialize)]
struct PackIndex {
    file: String,
    #[serde(rename = "hash-format")]
    hash_format: String,
    hash: String,
}
#[derive(Deserialize, Clone, Debug)]
pub struct Versions {
    pub minecraft: String,
    pub fabric: Option<String>,
}
#[derive(Deserialize)]
struct IndexToml {
    #[serde(rename = "hash-format", default)]
    hash_format: Option<String>,
    #[serde(default)]
    files: Vec<IndexFile>,
}
#[derive(Deserialize)]
struct IndexFile {
    file: String,
    hash: String,
    #[serde(rename = "hash-format")]
    hash_format: Option<String>,
    #[serde(default)]
    metafile: bool,
}
#[derive(Deserialize)]
struct MetaFile {
    filename: String,
    #[serde(default)]
    side: Option<String>,
    download: MetaDownload,
}
#[derive(Deserialize)]
struct MetaDownload {
    url: Option<String>,
    #[serde(rename = "hash-format")]
    hash_format: String,
    hash: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModEntry {
    /// Dossier d'instance de destination, d'après le dossier du métafichier (`mods`, `shaderpacks`...).
    pub dir: String,
    pub filename: String,
    pub url: String,
    pub hash: Hash,
}

/// Fichier non-méta de l'index (config, etc.) : copié du dossier du pack vers l'instance.
#[derive(Debug, Clone, PartialEq)]
pub struct PackFile {
    pub path: String,
    pub hash: Hash,
}

#[derive(Debug)]
pub struct Pack {
    pub version: String,
    pub versions: Versions,
    pub mods: Vec<ModEntry>,
    pub files: Vec<PackFile>,
}

impl Pack {
    /// Nom du zip de shaders (premier fichier .zip du dossier `shaderpacks`).
    pub fn shader_pack(&self) -> Option<&str> {
        self.mods.iter().find(|m| m.dir == "shaderpacks" && m.filename.ends_with(".zip")).map(|m| m.filename.as_str())
    }
}

/// Packwiz : `side` absent = both ; on garde client et both.
pub fn side_wanted(side: Option<&str>) -> bool {
    matches!(side.unwrap_or("both"), "client" | "both")
}

/// Hash d'un fichier texte du pack. Git sous Windows (autocrlf) convertit les fins de ligne en CRLF
/// alors que packwiz hashe la version LF : on accepte l'une ou l'autre.
fn text_matches(h: &Hash, text: &str) -> Result<bool> {
    Ok(h.matches(&mut text.as_bytes())? || h.matches(&mut text.replace("\r\n", "\n").as_bytes())?)
}

/// Pareil pour un fichier du pack lu en octets (la normalisation ne s'applique qu'au texte UTF-8).
pub fn bytes_match(h: &Hash, bytes: &[u8]) -> Result<bool> {
    match std::str::from_utf8(bytes) {
        Ok(t) => text_matches(h, t),
        Err(_) => h.matches(&mut &bytes[..]),
    }
}

pub fn parse_pack(pack_toml: &str) -> Result<(Versions, String, Hash)> {
    let p: PackToml = toml::from_str(pack_toml).context("pack.toml invalide")?;
    Ok((p.versions, p.index.file, Hash::new(&p.index.hash_format, &p.index.hash)?))
}

/// Parse index.toml + lit chaque métafichier via `read(chemin_relatif)`, vérifie leurs hashes
/// d'index, filtre par side.
pub fn parse_index(index_toml: &str, read: impl Fn(&str) -> Result<String>) -> Result<(Vec<ModEntry>, Vec<PackFile>)> {
    let idx: IndexToml = toml::from_str(index_toml).context("index.toml invalide")?;
    let mut mods = Vec::new();
    let mut files = Vec::new();
    for f in &idx.files {
        let fmt = f.hash_format.as_deref().or(idx.hash_format.as_deref()).unwrap_or("sha256");
        if !f.metafile {
            safe_join(Path::new("x"), &f.file)?; // refus précoce des chemins qui sortent du dossier
            files.push(PackFile { path: f.file.clone(), hash: Hash::new(fmt, &f.hash)? });
            continue;
        }
        let text = read(&f.file)?;
        if !text_matches(&Hash::new(fmt, &f.hash)?, &text)? {
            bail!("hash de l'index invalide pour {}", f.file);
        }
        let m: MetaFile = toml::from_str(&text).with_context(|| format!("{} invalide", f.file))?;
        if !side_wanted(m.side.as_deref()) {
            continue;
        }
        let Some(url) = m.download.url else {
            bail!("{} : pas de download.url (CurseForge non supporté)", f.file);
        };
        let dir = f.file.rsplit_once('/').map_or("mods", |(d, _)| d).to_string();
        safe_join(Path::new("x"), &dir)?;
        mods.push(ModEntry {
            dir,
            filename: safe_file_name(&m.filename)?.to_string(),
            url,
            hash: Hash::new(&m.download.hash_format, &m.download.hash)?,
        });
    }
    Ok((mods, files))
}

/// Télécharge `pack.zip` (release), l'extrait dans `<downloads>/pack-remote` (extraction sûre : zip-slip refusé)
/// et renvoie le dossier contenant `pack.toml`. Les hashes sont vérifiés ensuite par `load_pack`.
pub fn fetch_remote_pack(client: &reqwest::blocking::Client, dirs: &crate::config::Dirs, url: &str) -> Result<std::path::PathBuf> {
    let r = client.get(url).send().with_context(|| format!("requête {url}"))?;
    if !r.status().is_success() {
        bail!("HTTP {} pour {url}", r.status());
    }
    let bytes = r.bytes()?;
    let work = dirs.downloads().join("pack-remote");
    let _ = std::fs::remove_dir_all(&work); // notre propre dossier de travail
    std::fs::create_dir_all(&work)?;
    let zip = work.with_extension("zip");
    std::fs::write(&zip, &bytes)?;
    crate::java::extract(&zip, &work)?;
    let _ = std::fs::remove_file(&zip);
    [work.join("pack"), work.clone()].into_iter().find(|d| d.join("pack.toml").is_file()).context("pack.zip : pack.toml introuvable")
}

pub fn load_pack(dir: &Path) -> Result<Pack> {
    if dir.as_os_str().is_empty() {
        anyhow::bail!("dossier du pack non renseigné");
    }
    let read_in = |rel: &str| -> Result<String> {
        let p = safe_join(dir, rel)?;
        std::fs::read_to_string(&p).with_context(|| format!("lecture de {}", p.display()))
    };
    let (versions, index_file, index_hash) = parse_pack(&read_in("pack.toml")?)?;
    let index_text = read_in(&index_file)?;
    if !text_matches(&index_hash, &index_text)? {
        bail!("hash de {index_file} différent de celui de pack.toml");
    }
    let (mods, files) = parse_index(&index_text, read_in)?;
    #[derive(Deserialize)]
    struct V {
        #[serde(default)]
        version: String,
    }
    let version = toml::from_str::<V>(&read_in("pack.toml")?)?.version;
    Ok(Pack { version, versions, mods, files })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn sha256(s: &str) -> String {
        hex::encode(Sha256::digest(s.as_bytes()))
    }

    fn meta(name: &str, side: &str) -> String {
        format!(
            "name = \"{name}\"\nfilename = \"{name}.jar\"\nside = \"{side}\"\n[download]\nurl = \"http://h/{name}.jar\"\nhash-format = \"sha512\"\nhash = \"ab\"\n"
        )
    }

    fn index_for(metas: &[(&str, String)]) -> String {
        let mut s = String::from("hash-format = \"sha256\"\n");
        for (file, content) in metas {
            s += &format!("[[files]]\nfile = \"{file}\"\nhash = \"{}\"\nmetafile = true\n", sha256(content));
        }
        s
    }

    #[test]
    fn filters_sides() {
        let metas = [
            ("mods/a.pw.toml", meta("a", "both")),
            ("mods/b.pw.toml", meta("b", "server")),
            ("mods/c.pw.toml", meta("c", "client")),
        ];
        let idx = index_for(&metas);
        let read = |p: &str| Ok(metas.iter().find(|(f, _)| *f == p).unwrap().1.clone());
        let (mods, files) = parse_index(&idx, read).unwrap();
        assert!(files.is_empty());
        let names: Vec<_> = mods.iter().map(|m| m.filename.as_str()).collect();
        assert_eq!(names, ["a.jar", "c.jar"]);
        assert_eq!(mods[0].url, "http://h/a.jar");
    }

    #[test]
    fn accepts_crlf_checkouts() {
        let m = meta("a", "both");
        let idx = index_for(&[("mods/a.pw.toml", m.clone())]);
        let crlf = m.replace('\n', "\r\n");
        assert_eq!(parse_index(&idx, |_| Ok(crlf.clone())).unwrap().0.len(), 1);
    }

    #[test]
    fn plain_files_are_listed_and_checked() {
        let idx = "hash-format = \"sha256\"\n[[files]]\nfile = \"config/a.json\"\nhash = \"ab\"\n";
        let (mods, files) = parse_index(idx, |_| unreachable!()).unwrap();
        assert!(mods.is_empty());
        assert_eq!(files[0].path, "config/a.json");
        let bad = idx.replace("config/a.json", "../x");
        assert!(parse_index(&bad, |_| unreachable!()).is_err());
    }

    #[test]
    fn side_default_is_both() {
        assert!(side_wanted(None));
        assert!(!side_wanted(Some("server")));
    }

    #[test]
    fn rejects_wrong_index_hash_and_bad_filename() {
        let m = meta("a", "both");
        let idx = format!("hash-format = \"sha256\"\n[[files]]\nfile = \"mods/a.pw.toml\"\nhash = \"{}\"\nmetafile = true\n", "0".repeat(64));
        assert!(parse_index(&idx, |_| Ok(m.clone())).is_err());
        let evil = m.replace("a.jar\"\nside", "../../evil.jar\"\nside");
        let idx = index_for(&[("mods/a.pw.toml", evil.clone())]);
        assert!(parse_index(&idx, |_| Ok(evil.clone())).is_err());
    }

    #[test]
    fn parses_pack_toml() {
        let t = "[index]\nfile=\"index.toml\"\nhash-format=\"sha256\"\nhash=\"AB\"\n[versions]\nfabric=\"0.20.1\"\nminecraft=\"26.3\"\n";
        let (v, f, h) = parse_pack(t).unwrap();
        assert_eq!((v.minecraft.as_str(), v.fabric.as_deref(), f.as_str()), ("26.3", Some("0.20.1"), "index.toml"));
        assert_eq!(h.expected(), "ab");
    }
}
