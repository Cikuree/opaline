//! « Mes resource packs et shaders » : zips personnels stockés dans `<instance>/opaline-extra-packs/{resourcepacks,shaderpacks}/`,
//! copiés dans `resourcepacks/` / `shaderpacks/` à l'ajout et au lancement. Le ménage d'`install` les conserve
//! (voir `keep_paths`). Un fichier du pack n'est jamais écrasé.
use crate::config::Dirs;
use crate::safe::safe_file_name;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

pub const DIR: &str = "opaline-extra-packs";
const INDEX: &str = "index.json";
pub const MAX_BYTES: u64 = 500 * 1024 * 1024;
const MAX_ENTRIES: usize = 100_000;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    ResourcePack,
    Shader,
}

impl Kind {
    /// Dossier de l'instance (et sous-dossier d'`opaline-extra-packs/`).
    pub fn dir(self) -> &'static str {
        match self {
            Kind::ResourcePack => "resourcepacks",
            Kind::Shader => "shaderpacks",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExtraPack {
    pub file: String,
    pub kind: Kind,
    /// Calculée à la lecture (absente de l'index).
    #[serde(default)]
    pub size_bytes: u64,
}

pub fn extra_dir(instance: &Path) -> PathBuf {
    instance.join(DIR)
}

fn stored(instance: &Path, kind: Kind, file: &str) -> PathBuf {
    extra_dir(instance).join(kind.dir()).join(file)
}

pub fn validate_name(name: &str) -> Result<&str> {
    let n = safe_file_name(name).map_err(|_| anyhow::anyhow!("nom de fichier refusé : pas de chemin"))?;
    if !n.to_ascii_lowercase().ends_with(".zip") || n.len() > 150 || n.starts_with('.') || n.chars().any(|c| c.is_control() || "<>\"|?*".contains(c)) {
        bail!("seuls les fichiers .zip au nom simple sont acceptés");
    }
    Ok(n)
}

/// Type d'un zip d'après son contenu : `pack.mcmeta` à la racine = resource pack, dossier `shaders/` à la racine = shader.
pub fn detect(path: &Path) -> Result<Kind> {
    let mut magic = [0u8; 2];
    fs::File::open(path)?.read_exact(&mut magic).ok();
    if &magic != b"PK" {
        bail!("ce fichier n'est pas un zip valide");
    }
    let z = zip::ZipArchive::new(fs::File::open(path)?).map_err(|_| anyhow::anyhow!("ce fichier n'est pas un zip valide"))?;
    if z.len() > MAX_ENTRIES {
        bail!("zip refusé : trop d'entrées");
    }
    let mut shader = false;
    for n in z.file_names().flatten() {
        if n == "pack.mcmeta" {
            return Ok(Kind::ResourcePack);
        }
        shader |= n.starts_with("shaders/");
    }
    if shader { Ok(Kind::Shader) } else { bail!("ni resource pack ni shader (ni pack.mcmeta, ni dossier shaders/ à la racine)") }
}

// ---------- Index ----------

#[derive(Serialize, Deserialize, Default)]
struct Index {
    packs: Vec<ExtraPack>,
}

fn load(instance: &Path) -> Vec<ExtraPack> {
    let idx: Index = fs::read_to_string(extra_dir(instance).join(INDEX)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    let mut v: Vec<ExtraPack> = idx
        .packs
        .into_iter()
        .filter_map(|mut p| {
            validate_name(&p.file).ok()?; // index modifié à la main : nom non simple ignoré
            p.size_bytes = fs::metadata(stored(instance, p.kind, &p.file)).ok().filter(|m| m.is_file())?.len();
            Some(p)
        })
        .collect();
    v.sort_by_key(|p| (p.kind.dir(), p.file.to_lowercase()));
    v
}

fn save(instance: &Path, packs: &[ExtraPack]) -> Result<()> {
    let dir = extra_dir(instance);
    fs::create_dir_all(&dir)?;
    let tmp = dir.join("index.json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(&Index { packs: packs.to_vec() })?)?;
    Ok(fs::rename(&tmp, dir.join(INDEX))?)
}

pub fn list(dirs: &Dirs) -> Vec<ExtraPack> {
    load(&dirs.instance())
}

/// Chemins relatifs à l'instance (`resourcepacks/x.zip`) que le ménage d'`install` ne doit pas supprimer.
pub fn keep_paths(instance: &Path) -> HashSet<String> {
    load(instance).into_iter().map(|p| format!("{}/{}", p.kind.dir(), p.file)).collect()
}

/// Copie les zips perso absents de `resourcepacks/` / `shaderpacks/` (un fichier déjà présent n'est jamais écrasé).
pub fn sync(instance: &Path) -> Result<()> {
    for p in load(instance) {
        let dest = instance.join(p.kind.dir()).join(&p.file);
        if dest.exists() {
            continue;
        }
        fs::create_dir_all(instance.join(p.kind.dir()))?;
        fs::copy(stored(instance, p.kind, &p.file), &dest).with_context(|| format!("copie de {} vers {}/", p.file, p.kind.dir()))?;
    }
    Ok(())
}

pub fn sync_for_launch(dirs: &Dirs) -> Result<()> {
    sync(&dirs.instance())
}

/// Ajoute un zip : `write` l'écrit dans le fichier temporaire donné, puis type et taille sont vérifiés avant de le ranger.
fn place(dirs: &Dirs, name: &str, write: impl FnOnce(&Path) -> Result<u64>) -> Result<ExtraPack> {
    let name = validate_name(name)?.to_string();
    let inst = dirs.instance();
    let tmp_dir = extra_dir(&inst);
    fs::create_dir_all(&tmp_dir)?;
    let tmp = tmp_dir.join(format!("{name}.part"));
    let r = write(&tmp).and_then(|len| {
        if len == 0 || len > MAX_BYTES {
            bail!("« {name} » est vide ou dépasse 500 Mo");
        }
        let kind = detect(&tmp).with_context(|| format!("« {name} »"))?;
        let (dest, live) = (stored(&inst, kind, &name), inst.join(kind.dir()).join(&name));
        if dest.exists() || live.exists() {
            bail!("« {name} » existe déjà dans {}/ : renomme le fichier", kind.dir());
        }
        fs::create_dir_all(dest.parent().unwrap())?;
        fs::create_dir_all(live.parent().unwrap())?;
        fs::copy(&tmp, &live)?; // l'instance d'abord : en cas d'échec ensuite, rien n'est indexé
        fs::rename(&tmp, &dest)?;
        let mut packs = load(&inst);
        packs.push(ExtraPack { file: name.clone(), kind, size_bytes: len });
        save(&inst, &packs)?;
        Ok(ExtraPack { file: name.clone(), kind, size_bytes: len })
    });
    let _ = fs::remove_file(&tmp);
    r
}

/// Glisser-déposer : chemin choisi par l'utilisateur ; taille vérifiée avant toute copie.
pub fn add_path(dirs: &Dirs, src: &Path) -> Result<ExtraPack> {
    let name = src.file_name().and_then(|n| n.to_str()).context("nom de fichier illisible")?;
    validate_name(name)?;
    let meta = fs::metadata(src).ok().filter(|m| m.is_file()).context("ce n'est pas un fichier")?;
    if meta.len() > MAX_BYTES {
        bail!("fichier trop lourd (500 Mo maximum)");
    }
    place(dirs, name, |tmp| Ok(fs::copy(src, tmp)?))
}

/// Sélecteur de fichier de l'interface : contenu en mémoire.
pub fn add_bytes(dirs: &Dirs, name: &str, bytes: &[u8]) -> Result<ExtraPack> {
    place(dirs, name, |tmp| fs::write(tmp, bytes).map(|()| bytes.len() as u64).map_err(Into::into))
}

/// Télécharge un shader Modrinth (cdn seulement, sha512 vérifié par `fetch`), le range comme un zip perso puis l'active. Renvoie son nom.
pub fn add_modrinth_shader(
    dirs: &Dirs,
    api: &dyn crate::modrinth::Api,
    project: &str,
    mc: &str,
    fetch: &dyn Fn(&crate::extramods::Resolved, &Path) -> Result<()>,
) -> Result<String> {
    let versions = api.versions(project, mc, crate::modrinth::SHADERS)?;
    let v = crate::modrinth::choose_version(&versions).with_context(|| format!("aucune version de ce shader pour Minecraft {mc}"))?;
    let f = v.primary_file().context("aucun fichier")?;
    if !f.url.starts_with(crate::modrinth::CDN) {
        bail!("téléchargement refusé : seul cdn.modrinth.com est autorisé");
    }
    let size = f.size.context("taille du fichier inconnue : téléchargement refusé")?;
    if size > MAX_BYTES {
        bail!("fichier trop lourd (500 Mo maximum)");
    }
    let sha512 = f.hashes.get("sha512").context("hash manquant")?.to_ascii_lowercase();
    let r = crate::extramods::Resolved {
        project_id: project.to_string(),
        title: String::new(),
        version: v.version_number.clone(),
        file: f.filename.clone(),
        url: f.url.clone(),
        sha512,
        size: Some(size),
    };
    let p = place(dirs, &f.filename, |tmp| {
        fetch(&r, tmp)?;
        Ok(fs::metadata(tmp)?.len())
    })?;
    crate::prefs::shader_set(dirs, Some(&p.file))?;
    Ok(p.file)
}

/// Retire un zip perso d'`opaline-extra-packs/` et de l'instance (ce fichier seulement, sauf si le pack fournit le même nom).
/// Un shader actif retiré est remplacé par le shader du pack, ou « Aucun ». Renvoie le nom retiré.
pub fn remove(dirs: &Dirs, kind: Kind, file: &str) -> Result<String> {
    let file = validate_name(file)?;
    let inst = dirs.instance();
    let mut packs = load(&inst);
    if !packs.iter().any(|p| p.kind == kind && p.file == file) {
        bail!("ce fichier n'est pas dans Mes resource packs et shaders");
    }
    let was_active = kind == Kind::Shader && crate::prefs::shaders_get(dirs).active.as_deref() == Some(file);
    packs.retain(|p| !(p.kind == kind && p.file == file));
    save(&inst, &packs)?;
    fs::remove_file(stored(&inst, kind, file)).or_else(|e| if e.kind() == std::io::ErrorKind::NotFound { Ok(()) } else { Err(e) })?;
    // Sans instantané lisible, on ne sait pas ce que fournit le pack : l'instance n'est pas touchée.
    let from_pack = crate::changelog::load_snapshot(dirs).map_or(true, |s| s.mods.iter().any(|m| m.dir == kind.dir() && m.file == file));
    if !from_pack {
        let _ = fs::remove_file(inst.join(kind.dir()).join(file));
    }
    if was_active {
        let fallback = crate::install::installed_shader_pack(dirs);
        if crate::prefs::shader_set(dirs, fallback.as_deref()).is_err() {
            crate::prefs::shader_set(dirs, None)?;
        }
    }
    Ok(file.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn dirs(tag: &str) -> Dirs {
        let d = Dirs::new(std::env::temp_dir().join(format!("opaline-xpacks-{tag}-{}", std::process::id())));
        let _ = fs::remove_dir_all(&d.root);
        let d = d.for_pack("p").unwrap();
        fs::create_dir_all(d.instance()).unwrap();
        d
    }

    fn zip_bytes(names: &[&str]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for n in names {
            w.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(b"x").unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    #[test]
    fn detection() {
        let d = dirs("detect");
        let p = d.instance().join("t.zip");
        for (names, want) in [
            (&["pack.mcmeta", "assets/a.png"][..], Some(Kind::ResourcePack)),
            (&["shaders/final.fsh"], Some(Kind::Shader)),
            (&["readme.txt", "sub/pack.mcmeta"], None),
        ] {
            fs::write(&p, zip_bytes(names)).unwrap();
            assert_eq!(detect(&p).ok(), want, "{names:?}");
        }
        fs::write(&p, b"PKfaux").unwrap();
        assert!(detect(&p).is_err());
        fs::write(&p, b"pas un zip").unwrap();
        assert!(detect(&p).is_err());
    }

    #[test]
    fn add_refusals_and_size_checked_before_copy() {
        let d = dirs("refus");
        assert!(add_bytes(&d, "a.jar", &zip_bytes(&["pack.mcmeta"])).is_err());
        assert!(add_bytes(&d, "../a.zip", &zip_bytes(&["pack.mcmeta"])).is_err());
        assert!(add_bytes(&d, "a.zip", &zip_bytes(&["x.txt"])).is_err());
        assert!(list(&d).is_empty() && !d.instance().join("resourcepacks/a.zip").exists());
        // trop gros : refusé avant copie (fichier creux de 500 Mo + 1)
        let big = d.instance().join("gros.zip");
        fs::File::create(&big).unwrap().set_len(MAX_BYTES + 1).unwrap();
        let e = add_path(&d, &big).unwrap_err().to_string();
        assert!(e.contains("trop lourd") && !extra_dir(&d.instance()).join("gros.zip.part").exists());
        // même nom qu'un fichier du pack : refus, fichier du pack intact
        fs::create_dir_all(d.instance().join("resourcepacks")).unwrap();
        fs::write(d.instance().join("resourcepacks/pack.zip"), b"du pack").unwrap();
        assert!(add_bytes(&d, "pack.zip", &zip_bytes(&["pack.mcmeta"])).is_err());
        assert_eq!(fs::read(d.instance().join("resourcepacks/pack.zip")).unwrap(), b"du pack");
    }

    #[test]
    fn survive_update_and_remove_leaves_pack_files() {
        let d = dirs("cycle");
        let inst = d.instance();
        let a = add_bytes(&d, "rp.zip", &zip_bytes(&["pack.mcmeta"])).unwrap();
        assert_eq!(a.kind, Kind::ResourcePack);
        add_bytes(&d, "sh.zip", &zip_bytes(&["shaders/a.fsh"])).unwrap();
        assert!(inst.join("resourcepacks/rp.zip").is_file() && inst.join("shaderpacks/sh.zip").is_file());
        fs::write(inst.join("shaderpacks/du-pack.zip"), b"pack").unwrap();
        let keep = keep_paths(&inst);
        assert!(keep.contains("resourcepacks/rp.zip") && keep.contains("shaderpacks/sh.zip"));
        // supprimé de l'instance à la main : recopié
        fs::remove_file(inst.join("resourcepacks/rp.zip")).unwrap();
        sync(&inst).unwrap();
        assert!(inst.join("resourcepacks/rp.zip").is_file());
        crate::changelog::save_snapshot(&d, &Default::default()).unwrap(); // instantané du pack lisible (vide)
        // shader actif retiré : réinitialisé à « Aucun » (pas de shader de pack)
        crate::prefs::shader_set(&d, Some("sh.zip")).unwrap();
        assert!(remove(&d, Kind::Shader, "du-pack.zip").is_err(), "fichier du pack : pas dans l'index");
        remove(&d, Kind::Shader, "sh.zip").unwrap();
        let iris = fs::read_to_string(inst.join("config/iris.properties")).unwrap();
        assert!(iris.contains("enableShaders=false"), "{iris}");
        assert!(!inst.join("shaderpacks/sh.zip").exists() && !stored(&inst, Kind::Shader, "sh.zip").exists());
        assert_eq!(fs::read(inst.join("shaderpacks/du-pack.zip")).unwrap(), b"pack");
        assert_eq!(list(&d).len(), 1);
    }

    #[test]
    fn modrinth_shader_is_stored_and_activated() {
        use crate::modrinth::{Hit, Version, VersionFile};
        struct F(Version);
        impl crate::modrinth::Api for F {
            fn search(&self, _: &str, _: &str, _: &str) -> Result<Vec<Hit>> {
                Ok(vec![])
            }
            fn versions(&self, _: &str, _: &str, loader: &str) -> Result<Vec<Version>> {
                assert_eq!(loader, crate::modrinth::SHADERS);
                Ok(vec![self.0.clone()])
            }
            fn title(&self, _: &str) -> Option<String> {
                None
            }
        }
        let d = dirs("mrshader");
        let file = |url: &str| VersionFile {
            url: url.into(),
            filename: "BSL.zip".into(),
            primary: true,
            size: Some(10),
            hashes: [("sha512".to_string(), "AB".to_string())].into(),
        };
        let ver =
            |url: &str| Version { id: "v".into(), version_number: "1".into(), version_type: "release".into(), files: vec![file(url)], dependencies: vec![] };
        let fetch = |_: &crate::extramods::Resolved, dest: &Path| Ok(fs::write(dest, zip_bytes(&["shaders/final.fsh"]))?);
        assert!(add_modrinth_shader(&d, &F(ver("https://evil.example/BSL.zip")), "bsl", "1.21.1", &fetch).is_err());
        let name = add_modrinth_shader(&d, &F(ver("https://cdn.modrinth.com/data/x/BSL.zip")), "bsl", "1.21.1", &fetch).unwrap();
        assert_eq!(name, "BSL.zip");
        assert!(d.instance().join("shaderpacks").join("BSL.zip").is_file());
    }
}
