//! Orchestration de l'installation : Java, Minecraft, Fabric, mods. Les URL de base sont injectables.
use crate::config::{Dirs, fabric_profile_id};
use crate::download::{self, Job, Progress};
use crate::fabric::{FabricProfile, maven_path};
use crate::java;
use crate::mojang::{AssetIndex, Env, VersionJson, VersionManifest, applicable_libraries};
use crate::packwiz::{self, Pack};
use crate::safe::{Hash, safe_file_name, safe_join};
use serde::{Deserialize, Serialize};
use anyhow::{Context, Result, bail};
use reqwest::blocking::Client;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub const CONCURRENCY: usize = 12;

#[derive(Clone, Debug)]
pub struct Endpoints {
    pub mojang_manifest: String,
    pub fabric_meta: String,
    pub adoptium: String,
    pub assets: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Endpoints {
            mojang_manifest: "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json".into(),
            fabric_meta: "https://meta.fabricmc.net".into(),
            adoptium: "https://api.adoptium.net".into(),
            assets: "https://resources.download.minecraft.net".into(),
        }
    }
}

pub struct Installed {
    pub java: PathBuf,
    pub version: VersionJson,
    pub fabric: FabricProfile,
}

pub struct InstallParams {
    pub dirs: Dirs,
    pub pack_dir: PathBuf,
    /// Fichier jar ou dossier ; None = `<pack_dir>/../build/libs`.
    pub mod_jar: Option<PathBuf>,
    pub endpoints: Endpoints,
}

/// Jar classroomrun (hors manifeste) : fichier explicite, ou le plus récent `*.jar` d'un dossier
/// (hors `-sources`, `-dev`, `-javadoc`).
pub fn find_mod_jar(configured: Option<&Path>, pack_dir: &Path) -> Result<PathBuf> {
    let default_dir = pack_dir.parent().map(|p| p.join("build").join("libs"));
    let target = configured.map(Path::to_path_buf).or(default_dir).context("chemin du jar classroomrun inconnu")?;
    if target.is_file() {
        return Ok(target);
    }
    let mut jars: Vec<PathBuf> = fs::read_dir(&target)
        .with_context(|| format!("jar classroomrun introuvable : {} (configurez `mod_jar`)", target.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let n = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_ascii_lowercase();
            n.starts_with("classroomrun") && n.ends_with(".jar") && !["sources", "dev", "javadoc"].iter().any(|x| n.contains(x))
        })
        .collect();
    jars.sort();
    jars.pop().with_context(|| format!("aucun classroomrun*.jar dans {}", target.display()))
}

fn sha1_job(url: &str, dest: PathBuf, sha1: &str, size: Option<u64>) -> Result<Job> {
    Ok(Job { url: url.to_string(), dest, hash: Hash::new("sha1", sha1)?, size })
}

/// Contenu de `installed.ok` : ce pour quoi l'installation a été faite.
#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Marker {
    pack: String,
    minecraft: String,
    fabric: String,
}

fn marker_for(pack: &Pack) -> Result<Marker> {
    Ok(Marker {
        pack: pack.version.clone(),
        minecraft: pack.versions.minecraft.clone(),
        fabric: pack.versions.fabric.clone().context("pack.toml : version de Fabric absente")?,
    })
}

/// Les identifiants venant des manifestes servent de noms de dossier/fichier : un simple nom, rien d'autre.
fn validate_ids(mc: &str, loader: &str) -> Result<()> {
    safe_file_name(mc)?;
    safe_file_name(loader)?;
    Ok(())
}

fn validate_version(v: &VersionJson, mc: &str) -> Result<()> {
    safe_file_name(&v.id)?;
    safe_file_name(&v.asset_index.id)?;
    if v.id != mc {
        bail!("le JSON de version annonce {} au lieu de {mc}", v.id);
    }
    Ok(())
}

pub fn install(client: &Client, p: &InstallParams, progress: &Arc<Progress>) -> Result<Installed> {
    let dirs = &p.dirs;
    let ep = &p.endpoints;
    // Une installation interrompue ne doit jamais passer pour complète.
    match fs::remove_file(dirs.install_marker()) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.into()),
        _ => {}
    }
    progress.set_stage("Lecture du manifeste du pack");
    let pack = packwiz::load_pack(&p.pack_dir)?;
    let marker = marker_for(&pack)?;
    let (mc, loader) = (marker.minecraft.clone(), marker.fabric.clone());
    validate_ids(&mc, &loader)?;
    let mod_jar = find_mod_jar(p.mod_jar.as_deref(), &p.pack_dir)?;

    progress.set_stage("Minecraft : métadonnées");
    let version = fetch_version_json(client, dirs, ep, &mc)?;

    validate_version(&version, &mc)?;
    let major = version.java_version.major_version;
    let java_exe = match java::find_java(&dirs.java(major)) {
        Some(j) => j,
        None => install_java(client, dirs, ep, major, progress)?,
    };

    progress.set_stage("Minecraft : client et bibliothèques");
    let env = Env::current(&[]);
    let mut jobs = vec![sha1_job(&version.downloads.client.url, dirs.client_jar(&version.id), &version.downloads.client.sha1, version.downloads.client.size)?];
    for (_, a) in applicable_libraries(&version.libraries, &env) {
        jobs.push(sha1_job(&a.url, safe_join(&dirs.libraries(), &a.path)?, &a.sha1, a.size)?);
    }
    download::fetch_all(client, jobs, CONCURRENCY, progress)?;

    progress.set_stage("Minecraft : ressources");
    let idx_ref = &version.asset_index;
    let idx_path = dirs.assets().join("indexes").join(format!("{}.json", idx_ref.id));
    download::fetch(client, &sha1_job(&idx_ref.url, idx_path.clone(), &idx_ref.sha1, idx_ref.size)?)?;
    let index: AssetIndex = serde_json::from_slice(&fs::read(&idx_path)?).context("index d'assets invalide")?;
    let objects = dirs.assets().join("objects");
    let mut seen = HashSet::new();
    let mut jobs = Vec::new();
    for o in index.objects.values().filter(|o| seen.insert(o.hash.clone())) {
        let Some(prefix) = o.hash.get(..2) else { bail!("hash d'asset invalide : {}", o.hash) };
        jobs.push(sha1_job(
            &format!("{}/{prefix}/{}", ep.assets.trim_end_matches('/'), o.hash),
            safe_join(&objects, &format!("{prefix}/{}", o.hash))?,
            &o.hash,
            o.size,
        )?);
    }
    download::fetch_all(client, jobs, CONCURRENCY, progress)?;

    progress.set_stage("Fabric Loader");
    let fabric = install_fabric(client, dirs, ep, &mc, &loader, progress)?;

    progress.set_stage("Mods");
    install_mods(client, dirs, &pack, &p.pack_dir, &mod_jar, progress)?;

    fs::create_dir_all(&dirs.root)?;
    fs::write(dirs.install_marker(), serde_json::to_vec_pretty(&marker)?)?;
    progress.set_stage("Terminé");
    Ok(Installed { java: java_exe, version, fabric })
}

fn fetch_version_json(client: &Client, dirs: &Dirs, ep: &Endpoints, mc: &str) -> Result<VersionJson> {
    let manifest: VersionManifest = download::get_json(client, &ep.mojang_manifest)?;
    let entry = manifest.versions.iter().find(|v| v.id == mc).with_context(|| format!("Minecraft {mc} absent du manifeste Mojang"))?;
    let path = dirs.version_json(mc);
    download::fetch(client, &sha1_job(&entry.url, path.clone(), &entry.sha1, None)?)?;
    serde_json::from_slice(&fs::read(&path)?).with_context(|| format!("JSON de version {mc} invalide"))
}

fn install_java(client: &Client, dirs: &Dirs, ep: &Endpoints, major: u32, progress: &Arc<Progress>) -> Result<PathBuf> {
    progress.set_stage(&format!("Java {major} : téléchargement"));
    let pkg = java::parse_assets(&download::get_text(client, &java::assets_url(&ep.adoptium, major))?)?;
    let archive = safe_join(&dirs.downloads(), crate::safe::safe_file_name(&pkg.name)?)?;
    let job = Job { url: pkg.link.clone(), dest: archive.clone(), hash: Hash::new("sha256", &pkg.checksum)?, size: None };
    download::fetch_all(client, vec![job], 1, progress)?;
    progress.set_stage(&format!("Java {major} : extraction"));
    let dest = dirs.java(major);
    let tmp = dest.with_extension("tmp");
    let _ = fs::remove_dir_all(&tmp);
    java::extract(&archive, &tmp)?;
    let _ = fs::remove_dir_all(&dest);
    fs::rename(&tmp, &dest)?;
    java::find_java(&dest).context("java introuvable après extraction")
}

fn install_fabric(client: &Client, dirs: &Dirs, ep: &Endpoints, mc: &str, loader: &str, progress: &Arc<Progress>) -> Result<FabricProfile> {
    let url = format!("{}/v2/versions/loader/{mc}/{loader}/profile/json", ep.fabric_meta.trim_end_matches('/'));
    let text = download::get_text(client, &url)?;
    let profile: FabricProfile = serde_json::from_str(&text).context("profil Fabric invalide")?;
    let mut jobs = Vec::new();
    for l in &profile.libraries {
        let dest = safe_join(&dirs.libraries(), &maven_path(&l.name)?)?;
        let url = l.download_url()?;
        // Sans sha1 dans le profil : fichier .sha1 publié à côté sur le Maven.
        let sha1 = match &l.sha1 {
            Some(s) => s.clone(),
            None => download::get_text(client, &format!("{url}.sha1"))?.split_whitespace().next().unwrap_or("").to_string(),
        };
        jobs.push(sha1_job(&url, dest, &sha1, l.size)?);
    }
    download::fetch_all(client, jobs, CONCURRENCY, progress)?;
    // Profil écrit seulement une fois toutes ses libraries présentes et vérifiées.
    let id = fabric_profile_id(loader, mc);
    let jp = dirs.versions().join(&id).join(format!("{id}.json"));
    fs::create_dir_all(jp.parent().unwrap())?;
    fs::write(&jp, &text)?;
    Ok(profile)
}

/// Lit le profil Fabric déjà installé (lancement sans réinstaller).
pub fn load_installed(dirs: &Dirs, pack: &Pack) -> Result<Installed> {
    let want = marker_for(pack)?;
    let (mc, loader) = (&want.minecraft, &want.fabric);
    validate_ids(mc, loader)?;
    let marker_path = dirs.install_marker();
    let have: Marker = serde_json::from_slice(&fs::read(&marker_path).with_context(|| "installation absente ou incomplète : lancez l'installation".to_string())?)
        .context("installed.ok illisible : relancez l'installation")?;
    if have != want {
        bail!("le pack a changé (installé : {} / MC {} / Fabric {}) : relancez l'installation", have.pack, have.minecraft, have.fabric);
    }
    let id = fabric_profile_id(loader, mc);
    let read = |p: PathBuf| fs::read(&p).with_context(|| format!("{} absent : lancez d'abord l'installation", p.display()));
    let version: VersionJson = serde_json::from_slice(&read(dirs.version_json(mc))?)?;
    validate_version(&version, mc)?;
    let fabric: FabricProfile = serde_json::from_slice(&read(dirs.versions().join(&id).join(format!("{id}.json")))?)?;
    let java = java::find_java(&dirs.java(version.java_version.major_version)).context("Java absent : lancez d'abord l'installation")?;
    Ok(Installed { java, version, fabric })
}

fn install_mods(client: &Client, dirs: &Dirs, pack: &Pack, pack_dir: &Path, mod_jar: &Path, progress: &Arc<Progress>) -> Result<()> {
    let mods_dir = dirs.instance().join("mods");
    fs::create_dir_all(&mods_dir)?;
    let jar_name = crate::safe::safe_file_name(mod_jar.file_name().and_then(|n| n.to_str()).unwrap_or(""))?.to_string();
    let mut keep: HashSet<String> = HashSet::from([jar_name.clone()]);
    let mut jobs = Vec::new();
    for m in &pack.mods {
        keep.insert(m.filename.clone());
        jobs.push(Job { url: m.url.clone(), dest: safe_join(&mods_dir, &m.filename)?, hash: m.hash.clone(), size: None });
    }
    download::fetch_all(client, jobs, CONCURRENCY, progress)?;
    // Fichiers non-méta de l'index (configs...) : copiés depuis le dossier du pack après vérification du hash.
    for f in &pack.files {
        let src = safe_join(pack_dir, &f.path)?;
        let dest = safe_join(&dirs.instance(), &f.path)?;
        let bytes = fs::read(&src).with_context(|| format!("lecture de {}", src.display()))?;
        if !packwiz::bytes_match(&f.hash, &bytes)? {
            bail!("hash invalide pour {} (index du pack)", f.path);
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&dest, &bytes)?;
    }
    fs::copy(mod_jar, mods_dir.join(&jar_name)).with_context(|| format!("copie de {}", mod_jar.display()))?;
    // Ménage dans NOTRE dossier mods uniquement : anciens jars retirés du manifeste.
    for e in fs::read_dir(&mods_dir)?.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        // Fichiers seulement : un dossier nommé « x.jar » est ignoré.
        if name.ends_with(".jar") && !keep.contains(&name) && e.file_type().is_ok_and(|t| t.is_file()) {
            fs::remove_file(e.path())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("crl-inst-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn find_mod_jar_picks_main_jar() {
        let d = tmp("jar");
        for n in ["classroomrun-0.1.0.jar", "classroomrun-0.1.0-sources.jar", "classroomrun-0.1.0-dev.jar", "other.jar"] {
            fs::write(d.join(n), b"x").unwrap();
        }
        assert_eq!(find_mod_jar(Some(&d), &d).unwrap().file_name().unwrap(), "classroomrun-0.1.0.jar");
        assert!(find_mod_jar(Some(&d.join("classroomrun-0.1.0.jar")), &d).is_ok());
        assert!(find_mod_jar(Some(&d.join("nope")), &d).is_err());
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn find_mod_jar_defaults_to_repo_build_libs() {
        let d = tmp("def");
        fs::create_dir_all(d.join("pack")).unwrap();
        fs::create_dir_all(d.join("build/libs")).unwrap();
        fs::write(d.join("build/libs/classroomrun-1.jar"), b"x").unwrap();
        assert!(find_mod_jar(None, &d.join("pack")).is_ok());
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn manifest_paths_cannot_escape() {
        let dirs = Dirs::new("/r");
        assert!(safe_join(&dirs.libraries(), "../../evil.jar").is_err());
        assert!(sha1_job("u", PathBuf::from("x"), "zz", None).is_ok()); // l'hex n'est pas validé, la comparaison échouera
    }

    #[test]
    fn ids_must_be_plain_names() {
        assert!(validate_ids("26.3", "0.20.1").is_ok());
        assert!(validate_ids("../26.3", "0.20.1").is_err());
        assert!(validate_ids("26.3", "a/b").is_err());
    }

    #[test]
    fn mods_cleanup_removes_only_stale_files() {
        let d = tmp("clean");
        let dirs = Dirs::new(d.join("root"));
        let mods = dirs.instance().join("mods");
        fs::create_dir_all(mods.join("dir.jar")).unwrap();
        fs::write(mods.join("old.jar"), b"x").unwrap();
        fs::write(d.join("classroomrun-1.jar"), b"x").unwrap();
        let pack = Pack {
            version: "1".into(),
            versions: packwiz::Versions { minecraft: "26.3".into(), fabric: Some("0.20.1".into()) },
            mods: vec![],
            files: vec![],
        };
        let client = crate::download::http_client().unwrap();
        install_mods(&client, &dirs, &pack, &d, &d.join("classroomrun-1.jar"), &Arc::default()).unwrap();
        assert!(mods.join("dir.jar").is_dir() && !mods.join("old.jar").exists() && mods.join("classroomrun-1.jar").is_file());
        // sans installed.ok, load_installed refuse
        assert!(load_installed(&dirs, &pack).is_err());
        fs::remove_dir_all(&d).unwrap();
    }
}
