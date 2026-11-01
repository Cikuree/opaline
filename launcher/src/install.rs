//! Orchestration de l'installation : Java, Minecraft, loader (Fabric, Quilt, NeoForge, Forge), mods. Les URL de base sont injectables.
use crate::config::{Dirs, fabric_profile_id};
use crate::download::{self, Job, Progress};
use crate::fabric::{FabricProfile, maven_path};
use crate::java;
use crate::loader::{self, Ctx, InstallProfile, LoaderKind, LoaderVersion, check_host, forge_version};
use crate::mojang::{AssetIndex, Env, VersionJson, VersionManifest, applicable_libraries};
use crate::overrides;
use crate::packwiz::{self, Pack};
use crate::safe::{Hash, safe_file_name, safe_join};
use anyhow::{Context, Result, bail};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub const CONCURRENCY: usize = 12;

#[derive(Clone, Debug)]
pub struct Endpoints {
    pub mojang_manifest: String,
    pub fabric_meta: String,
    pub quilt_meta: String,
    /// Racine du Maven NeoForge (contient `net/neoforged/...`).
    pub neoforge_maven: String,
    /// Racine du Maven Forge (contient `net/minecraftforge/...`).
    pub forge_maven: String,
    pub adoptium: String,
    pub assets: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Endpoints {
            mojang_manifest: "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json".into(),
            fabric_meta: "https://meta.fabricmc.net".into(),
            quilt_meta: "https://meta.quiltmc.org".into(),
            neoforge_maven: "https://maven.neoforged.net/releases".into(),
            forge_maven: "https://maven.minecraftforge.net".into(),
            adoptium: "https://api.adoptium.net".into(),
            assets: "https://resources.download.minecraft.net".into(),
        }
    }
}

pub struct Installed {
    pub java: PathBuf,
    pub version: VersionJson,
    /// Profil du loader : Fabric ou Quilt (meta), ou `version.json` de l'installeur Forge / NeoForge.
    pub fabric: FabricProfile,
    /// Identifiant du profil du loader dans `shared/versions` (dossier et `id` du JSON de version).
    pub profile_id: String,
    /// Zip de shaders installé (pour iris.properties).
    pub shader_pack: Option<String>,
    /// Hash de l'index du pack installé (None pour une installation ancienne).
    pub index_hash: Option<String>,
}

pub struct InstallParams {
    pub dirs: Dirs,
    pub pack_dir: PathBuf,
    /// Jar de dev (fichier ou dossier), pour un pack local sans `classroomrun` ; None = rien à ajouter.
    pub mod_jar: Option<PathBuf>,
    pub endpoints: Endpoints,
    /// Archive d'overrides du pack (catalogue) ; None = pack sans overrides.
    pub overrides: Option<crate::catalog::Overrides>,
}

/// Jar classroomrun (hors manifeste) : fichier explicite, ou le plus récent `*.jar` d'un dossier
/// (hors `-sources`, `-dev`, `-javadoc`).
pub fn find_mod_jar(configured: Option<&Path>, pack_dir: &Path) -> Result<PathBuf> {
    // Repli dev : premier `build/libs` trouvé en remontant depuis le pack (`<dépôt>/packs/<id>` -> `<dépôt>/build/libs`).
    let default_dir = pack_dir.ancestors().skip(1).map(|p| p.join("build").join("libs")).find(|p| p.is_dir());
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
    /// Absent dans les anciens marqueurs : Fabric.
    #[serde(default = "default_kind")]
    loader: String,
    #[serde(alias = "fabric")]
    loader_version: String,
    /// Identifiant du profil dans `shared/versions` (absent dans les anciens marqueurs).
    #[serde(default)]
    profile_id: Option<String>,
    #[serde(default)]
    shader_pack: Option<String>,
    /// Hash de l'index du pack installé (détection « le pack a changé »).
    #[serde(default)]
    index_hash: Option<String>,
    /// sha256 de l'archive d'overrides extraite (même sha256 = rien à refaire).
    #[serde(default)]
    overrides_sha256: Option<String>,
    /// Fichiers posés par l'archive d'overrides (chemins relatifs à l'instance) : seuls ceux-là sont retirés à la mise à jour.
    #[serde(default)]
    overrides_files: Vec<String>,
}

fn default_kind() -> String {
    "fabric".into()
}

fn marker_for(pack: &Pack) -> Result<Marker> {
    let (kind, loader_version) = pack.versions.loader()?;
    Ok(Marker {
        pack: pack.version.clone(),
        minecraft: pack.versions.minecraft.clone(),
        loader: kind.name().into(),
        loader_version,
        profile_id: None,
        shader_pack: pack.shader_pack().map(str::to_string),
        index_hash: Some(pack.index_hash.clone()),
        overrides_sha256: None,
        overrides_files: vec![],
    })
}

/// Les identifiants venant des manifestes servent de noms de dossier/fichier : un simple nom, rien d'autre.
fn validate_ids(mc: &str, loader: &str) -> Result<()> {
    safe_file_name(mc)?;
    safe_file_name(loader)?;
    if !loader::valid_version(mc) || !loader::valid_version(loader) {
        bail!("version invalide (caractères autorisés : lettres, chiffres, . _ + -)");
    }
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
    // Lu avant la suppression du marqueur : fichiers d'overrides déjà posés (protégés du ménage, retirés s'ils disparaissent).
    let previous: Option<Marker> = fs::read(dirs.install_marker()).ok().and_then(|b| serde_json::from_slice(&b).ok());
    // Une installation interrompue ne doit jamais passer pour complète.
    match fs::remove_file(dirs.install_marker()) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.into()),
        _ => {}
    }
    progress.set_stage("java");
    let pack = packwiz::load_pack(&p.pack_dir)?;
    let mut marker = marker_for(&pack)?;
    marker.index_hash = Some(crate::catalog::pack_version_hash(&pack.index_hash, p.overrides.as_ref()));
    let (mc, loader) = (marker.minecraft.clone(), marker.loader_version.clone());
    let kind = LoaderKind::parse(&marker.loader).context("loader inconnu")?;
    validate_ids(&mc, &loader)?;
    // Le jar classroomrun vient du pack s'il y est ; sinon seulement si un jar de dev est demandé explicitement.
    let mod_jar = if pack.mods.iter().any(|m| m.dir == "mods" && m.filename.starts_with("classroomrun")) {
        None
    } else {
        p.mod_jar.as_deref().map(|j| find_mod_jar(Some(j), &p.pack_dir)).transpose()?
    };

    let version = fetch_version_json(client, dirs, ep, &mc)?;

    validate_version(&version, &mc)?;
    let major = version.java_version.major_version;
    let java_exe = match java::find_java(&dirs.java(major)) {
        Some(j) => j,
        None => install_java(client, dirs, ep, major, progress)?,
    };

    progress.complete_stage();
    progress.set_stage("minecraft");
    let env = Env::current(&[]);
    let mut jobs = vec![sha1_job(&version.downloads.client.url, dirs.client_jar(&version.id), &version.downloads.client.sha1, version.downloads.client.size)?];
    for (_, a) in applicable_libraries(&version.libraries, &env) {
        jobs.push(sha1_job(&a.url, safe_join(&dirs.libraries(), &a.path)?, &a.sha1, a.size)?);
    }
    download::fetch_all(client, jobs, CONCURRENCY, progress)?;

    progress.set_stage("assets");
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

    progress.set_stage("loader");
    let (fabric, profile_id) = install_loader(client, dirs, ep, kind, &mc, &loader, &java_exe, progress)?;
    let mut marker = Marker { profile_id: Some(profile_id.clone()), ..marker };

    progress.set_stage("mods");
    let old_files: Vec<String> = previous.as_ref().map(|m| m.overrides_files.clone()).unwrap_or_default();
    install_mods(client, dirs, &pack, &p.pack_dir, mod_jar.as_deref(), &old_files.iter().cloned().collect(), progress)?;

    match &p.overrides {
        Some(o) => {
            progress.set_stage("overrides");
            marker.overrides_sha256 = Some(o.sha256.to_ascii_lowercase());
            if previous.as_ref().and_then(|m| m.overrides_sha256.as_deref()) == marker.overrides_sha256.as_deref() {
                marker.overrides_files = old_files; // archive inchangée : rien à télécharger ni à extraire
            } else {
                marker.overrides_files = install_overrides(client, dirs, o, &old_files, progress)?;
            }
        }
        None => overrides::remove_stale(&dirs.instance(), &old_files, &HashSet::new()),
    }

    crate::changelog::save_snapshot(dirs, &crate::changelog::snapshot_of(&pack))?;
    fs::create_dir_all(dirs.instance())?;
    fs::write(dirs.install_marker(), serde_json::to_vec_pretty(&marker)?)?;
    Ok(Installed { java: java_exe, version, fabric, profile_id, shader_pack: marker.shader_pack, index_hash: marker.index_hash })
}

/// Télécharge l'archive (sha256 vérifié, taille bornée par `size`, reprise), l'extrait dans l'instance et renvoie les fichiers posés.
fn install_overrides(client: &Client, dirs: &Dirs, o: &crate::catalog::Overrides, previous: &[String], progress: &Arc<Progress>) -> Result<Vec<String>> {
    let sha = o.sha256.to_ascii_lowercase();
    let archive = safe_join(&dirs.downloads(), &format!("overrides-{}-{}.zip", dirs.pack, &sha[..16]))?;
    let job = Job { url: o.url.clone(), dest: archive.clone(), hash: Hash::new("sha256", &sha)?, size: Some(o.size) };
    download::fetch_all(client, vec![job], 1, progress)?;
    let files = overrides::extract(&archive, &dirs.instance(), previous);
    let _ = fs::remove_file(&archive); // jusqu'à 2 Go : inutile une fois extraite
    files
}

pub fn fetch_version_json(client: &Client, dirs: &Dirs, ep: &Endpoints, mc: &str) -> Result<VersionJson> {
    let manifest: VersionManifest = download::get_json(client, &ep.mojang_manifest)?;
    let entry = manifest.versions.iter().find(|v| v.id == mc).with_context(|| format!("Minecraft {mc} absent du manifeste Mojang"))?;
    let path = dirs.version_json(mc);
    download::fetch(client, &sha1_job(&entry.url, path.clone(), &entry.sha1, None)?)?;
    serde_json::from_slice(&fs::read(&path)?).with_context(|| format!("JSON de version {mc} invalide"))
}

fn install_java(client: &Client, dirs: &Dirs, ep: &Endpoints, major: u32, progress: &Arc<Progress>) -> Result<PathBuf> {
    progress.set_stage("java");
    let pkg = java::parse_assets(&download::get_text(client, &java::assets_url(&ep.adoptium, major))?)?;
    let archive = safe_join(&dirs.downloads(), crate::safe::safe_file_name(&pkg.name)?)?;
    let job = Job { url: pkg.link.clone(), dest: archive.clone(), hash: Hash::new("sha256", &pkg.checksum)?, size: None };
    download::fetch_all(client, vec![job], 1, progress)?;
    // même étape pour l'interface
    let dest = dirs.java(major);
    let tmp = dest.with_extension("tmp");
    let _ = fs::remove_dir_all(&tmp);
    java::extract(&archive, &tmp)?;
    let _ = fs::remove_dir_all(&dest);
    fs::rename(&tmp, &dest)?;
    java::find_java(&dest).context("java introuvable après extraction")
}

/// sha1 fourni par le manifeste, sinon fichier `.sha1` publié à côté sur le Maven.
fn sha1_or_maven(client: &Client, url: &str, given: Option<&str>) -> Result<String> {
    match given.filter(|s| !s.is_empty()) {
        Some(s) => Ok(s.to_string()),
        None => Ok(download::get_text(client, &format!("{url}.sha1"))?.split_whitespace().next().unwrap_or("").to_string()),
    }
}

#[allow(clippy::too_many_arguments)]
fn install_loader(
    client: &Client,
    dirs: &Dirs,
    ep: &Endpoints,
    kind: LoaderKind,
    mc: &str,
    loader: &str,
    java: &Path,
    progress: &Arc<Progress>,
) -> Result<(FabricProfile, String)> {
    match kind {
        LoaderKind::Fabric | LoaderKind::Quilt => install_meta_loader(client, dirs, ep, kind, mc, loader, progress),
        LoaderKind::NeoForge | LoaderKind::Forge => install_installer_loader(dirs, ep, kind, mc, loader, java, progress),
    }
}

/// Fabric (meta v2) et Quilt (meta v3) : profil JSON, libraries vérifiées par leur sha1 (profil ou `.sha1` du Maven).
fn install_meta_loader(
    client: &Client,
    dirs: &Dirs,
    ep: &Endpoints,
    kind: LoaderKind,
    mc: &str,
    loader: &str,
    progress: &Arc<Progress>,
) -> Result<(FabricProfile, String)> {
    let (meta, ver) = if kind == LoaderKind::Quilt { (&ep.quilt_meta, "v3") } else { (&ep.fabric_meta, "v2") };
    let url = format!("{}/{ver}/versions/loader/{mc}/{loader}/profile/json", meta.trim_end_matches('/'));
    let text = download::get_text(client, &url)?;
    let profile: FabricProfile = serde_json::from_str(&text).with_context(|| format!("profil {} invalide", kind.name()))?;
    let strict = loader::loader_client()?;
    let mut jobs = Vec::new();
    for l in &profile.libraries {
        let dest = safe_join(&dirs.libraries(), &maven_path(&l.name)?)?;
        let url = l.download_url()?;
        check_host(&url)?;
        let sha1 = sha1_or_maven(&strict, &url, l.sha1.as_deref())?;
        jobs.push(sha1_job(&url, dest, &sha1, l.size)?);
    }
    download::fetch_all(&strict, jobs, CONCURRENCY, progress)?;
    // Profil écrit seulement une fois toutes ses libraries présentes et vérifiées.
    let id = fabric_profile_id(kind.name(), loader, mc);
    let jp = dirs.versions().join(&id).join(format!("{id}.json"));
    fs::create_dir_all(jp.parent().unwrap())?;
    write_atomic(&jp, text.as_bytes())?;
    Ok((profile, id))
}

/// Écriture atomique (fichier temporaire voisin puis renommage) : un JSON de version n'est jamais à moitié écrit.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

/// Liste des fichiers de `shared/libraries` que le Launcher officiel ne peut pas télécharger (sorties des processors, fichiers
/// `data` `[coords]` côté client, libraries sans URL) : `versions/<id>/opaline-loader-files.json`, lue par le mode officiel.
fn write_loader_files(dirs: &Dirs, id: &str, libs_dir: &Path, ctx: &Ctx, profile: &InstallProfile, local: &[(String, PathBuf, Option<String>)]) -> Result<()> {
    let mut files: Vec<(String, Option<String>)> = local.iter().map(|(rel, _, sha)| (rel.clone(), sha.clone())).collect();
    let rel = |p: &Path| p.strip_prefix(libs_dir).ok().map(|r| r.to_string_lossy().replace('\\', "/"));
    for p in loader::client_processors(profile) {
        for (file, sha) in &p.outputs {
            if let (Ok(f), Ok(s)) = (ctx.resolve(file), ctx.resolve(sha)) {
                files.extend(rel(Path::new(&f)).map(|r| (r, Some(s))));
            }
        }
        files.extend(ctx.processor_files(p).iter().filter_map(|f| rel(f)).map(|r| (r, None)));
    }
    let mut out: Vec<serde_json::Value> = Vec::new();
    let mut seen = HashSet::new();
    files.sort_by_key(|(_, sha)| sha.is_none()); // une entrée avec sha1 l'emporte sur la même sans
    for (path, sha1) in files {
        if seen.insert(path.clone()) {
            out.push(serde_json::json!({ "path": path, "sha1": sha1 }));
        }
    }
    write_atomic(&dirs.versions().join(id).join(crate::official::LOADER_FILES), &serde_json::to_vec_pretty(&out)?)
}

/// URL de l'installeur officiel (Forge : la version peut être donnée avec ou sans le préfixe `<mc>-`).
pub fn installer_url(ep: &Endpoints, kind: LoaderKind, mc: &str, loader: &str) -> Result<String> {
    safe_file_name(mc)?;
    safe_file_name(loader)?;
    Ok(match kind {
        LoaderKind::NeoForge => format!("{0}/net/neoforged/neoforge/{loader}/neoforge-{loader}-installer.jar", ep.neoforge_maven.trim_end_matches('/')),
        LoaderKind::Forge => {
            let v = forge_version(mc, loader);
            format!("{0}/net/minecraftforge/forge/{mc}-{v}/forge-{mc}-{v}-installer.jar", ep.forge_maven.trim_end_matches('/'))
        }
        _ => bail!("{} n'a pas d'installeur", kind.name()),
    })
}

fn parse_entry<T: serde::de::DeserializeOwned>(installer: &Path, name: &str) -> Result<(T, Vec<u8>)> {
    let bytes = loader::read_entry(installer, name)?.with_context(|| format!("{name} absent de l'installeur (version non prise en charge)"))?;
    Ok((serde_json::from_slice(&bytes).with_context(|| format!("{name} invalide"))?, bytes))
}

/// NeoForge et Forge : exécute l'`install_profile.json` de l'installeur officiel (processors client) dans l'instance isolée.
#[allow(clippy::too_many_arguments)]
fn install_installer_loader(
    dirs: &Dirs,
    ep: &Endpoints,
    kind: LoaderKind,
    mc: &str,
    loader: &str,
    java: &Path,
    progress: &Arc<Progress>,
) -> Result<(FabricProfile, String)> {
    let client = &loader::loader_client()?;
    let url = installer_url(ep, kind, mc, loader)?;
    check_host(&url)?;
    let name = url.rsplit('/').next().unwrap_or_default();
    let installer = safe_join(&dirs.downloads(), safe_file_name(name)?)?;
    let installer_sha = sha1_or_maven(client, &url, None)?;
    download::fetch_all(client, vec![sha1_job(&url, installer.clone(), &installer_sha, None)?], 1, progress)?;

    let (profile, _): (InstallProfile, _) = parse_entry(&installer, "install_profile.json")?;
    let (ver, ver_bytes): (LoaderVersion, _) = parse_entry(&installer, "version.json")?;
    ver.check(mc, &profile)?;
    let id = ver.id.clone();
    safe_file_name(&id)?;
    let libs_dir = dirs.libraries();

    // Libraries du profil puis du version.json. Sans URL : fournies par le dossier `maven/` de l'installeur, ou produites par un processor.
    let mut seen = HashSet::new();
    let (mut jobs, mut local) = (Vec::new(), Vec::new());
    for l in profile.libraries.iter().chain(&ver.libraries) {
        let rel = l.rel_path()?;
        let dest = safe_join(&libs_dir, &rel)?;
        if !seen.insert(dest.clone()) {
            continue;
        }
        match l.artifact().filter(|a| !a.url.is_empty()) {
            Some(a) => {
                check_host(&a.url)?;
                let sha1 = sha1_or_maven(client, &a.url, Some(&a.sha1))?;
                jobs.push(sha1_job(&a.url, dest, &sha1, a.size)?);
            }
            None => local.push((rel, dest, l.artifact().map(|a| a.sha1.clone()).filter(|s| !s.is_empty()))),
        }
    }
    download::fetch_all(client, jobs, CONCURRENCY, progress)?;
    for (rel, dest, sha1) in &local {
        let valid = |p: &Path| p.is_file() && sha1.as_ref().is_none_or(|h| Hash::new("sha1", h).and_then(|h| h.matches_file(p)).unwrap_or(false));
        if let (false, Some(bytes)) = (valid(dest), loader::read_entry(&installer, &format!("maven/{rel}"))?) {
            fs::create_dir_all(dest.parent().unwrap())?;
            fs::write(dest, bytes)?;
        }
    }

    let work = dirs.instance().join("loader-work");
    let data_dir = work.join("installer");
    let ctx = Ctx::new(&dirs.shared(), &libs_dir, &dirs.client_jar(mc), &installer, &data_dir, mc, &profile)?;
    let vdir = dirs.versions().join(&id);
    let marker = vdir.join("opaline-loader.ok");
    let sha_text = installer_sha.to_ascii_lowercase();
    // Tout est vérifié : sorties (sha1), fichiers de libraries des arguments, libraries locales (sha1 quand connu).
    let outputs_ok = || {
        loader::client_processors(&profile)
            .iter()
            .all(|p| ctx.outputs_valid(p).ok().flatten() != Some(false) && ctx.processor_files(p).iter().all(|f| f.is_file()))
            && local.iter().all(|(_, d, sha)| loader::file_ok(d, sha.as_deref()))
    };
    if !(outputs_ok() && fs::read_to_string(&marker).is_ok_and(|m| m.trim() == sha_text)) {
        let _lock = loader::InstallLock::acquire(&vdir)?;
        let _ = fs::remove_file(&marker);
        loader::extract_data(&installer, &data_dir)?;
        let log = dirs.instance().join("logs").join("loader-install.log");
        let _ = fs::remove_file(&log);
        let sep = if cfg!(windows) { ";" } else { ":" };
        // Le journal d'un processor peut contenir des chemins personnels : ils sont masqués dans l'erreur.
        loader::run_processors(java, &ctx, &profile, &work, &log, sep, loader::PROCESSOR_TIMEOUT)
            .map_err(|e| anyhow::anyhow!(crate::service::sanitize_error(&format!("{e:#}"), dirs)))?;
        if !outputs_ok() {
            bail!("l'installation du loader n'a pas produit tous ses fichiers (ou un sha1 est incorrect)");
        }
        let tmp = marker.with_extension("tmp");
        fs::write(&tmp, &sha_text)?;
        fs::rename(&tmp, &marker)?;
    }
    // Jar client sous le nom du loader : `${version_name}.jar` figure dans l'ignoreList de Forge et NeoForge.
    let (src, dst) = (dirs.client_jar(mc), dirs.client_jar(&id));
    if fs::metadata(&src)?.len() != fs::metadata(&dst).map(|m| m.len()).unwrap_or(0) {
        let tmp = dst.with_extension("tmp");
        fs::copy(&src, &tmp)?;
        fs::rename(&tmp, &dst)?;
    }
    let jp = dirs.versions().join(&id).join(format!("{id}.json"));
    write_atomic(&jp, &ver_bytes)?;
    write_loader_files(dirs, &id, &libs_dir, &ctx, &profile, &local)?;
    Ok((ver.into_profile(mc)?, id))
}

/// Lit l'installation existante d'après `installed.ok` (lancement sans réseau ni lecture du pack).
/// Hash de l'index du pack installé (None : pas installé ou installation ancienne sans hash).
pub fn installed_hash(dirs: &Dirs) -> Option<String> {
    serde_json::from_slice::<Marker>(&fs::read(dirs.install_marker()).ok()?).ok()?.index_hash
}

/// Shader du pack installé d'après `installed.ok`.
pub fn installed_shader_pack(dirs: &Dirs) -> Option<String> {
    serde_json::from_slice::<Marker>(&fs::read(dirs.install_marker()).ok()?).ok()?.shader_pack
}

pub fn load_installed(dirs: &Dirs) -> Result<Installed> {
    let have: Marker =
        serde_json::from_slice(&fs::read(dirs.install_marker()).with_context(|| "installation absente ou incomplète : lancez l'installation".to_string())?)
            .context("installed.ok illisible : relancez l'installation")?;
    let (mc, loader) = (&have.minecraft, &have.loader_version);
    validate_ids(mc, loader)?;
    let kind = LoaderKind::parse(&have.loader).context("loader inconnu : relancez l'installation")?;
    let id = have.profile_id.clone().unwrap_or_else(|| fabric_profile_id(kind.name(), loader, mc));
    safe_file_name(&id)?;
    let read = |p: PathBuf| fs::read(&p).with_context(|| format!("{} absent : lancez d'abord l'installation", p.display()));
    let version: VersionJson = serde_json::from_slice(&read(dirs.version_json(mc))?)?;
    validate_version(&version, mc)?;
    let profile_json = read(dirs.versions().join(&id).join(format!("{id}.json")))?;
    let fabric: FabricProfile = match kind {
        LoaderKind::Fabric | LoaderKind::Quilt => serde_json::from_slice(&profile_json)?,
        LoaderKind::NeoForge | LoaderKind::Forge => serde_json::from_slice::<LoaderVersion>(&profile_json)?.into_profile(mc)?,
    };
    let java = java::find_java(&dirs.java(version.java_version.major_version)).context("Java absent : lancez d'abord l'installation")?;
    Ok(Installed { java, version, fabric, profile_id: id, shader_pack: have.shader_pack, index_hash: have.index_hash })
}

fn install_mods(
    client: &Client,
    dirs: &Dirs,
    pack: &Pack,
    pack_dir: &Path,
    mod_jar: Option<&Path>,
    keep_paths: &HashSet<String>,
    progress: &Arc<Progress>,
) -> Result<()> {
    let mods_dir = dirs.instance().join("mods");
    fs::create_dir_all(&mods_dir)?;
    let jar_name = mod_jar.map(|j| crate::safe::safe_file_name(j.file_name().and_then(|n| n.to_str()).unwrap_or("")).map(str::to_string)).transpose()?;
    let mut keep: HashSet<String> = jar_name.iter().cloned().collect();
    // « Mes mods » : jamais supprimés par le ménage ci-dessous, recopiés à la fin.
    keep.extend(crate::extramods::names(&dirs.instance()));
    let mut pack_jars: HashSet<String> = jar_name.iter().cloned().collect();
    let mut jobs = Vec::new();
    for m in &pack.mods {
        let dir = safe_join(&dirs.instance(), &m.dir)?;
        if m.dir == "mods" {
            keep.insert(m.filename.clone());
            pack_jars.insert(m.filename.clone());
        }
        jobs.push(Job { url: m.url.clone(), dest: safe_join(&dir, &m.filename)?, hash: m.hash.clone(), size: None });
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
    if let (Some(j), Some(n)) = (mod_jar, &jar_name) {
        fs::copy(j, mods_dir.join(n)).with_context(|| format!("copie de {}", j.display()))?;
    }
    crate::extramods::sync(&dirs.instance(), &pack_jars)?;
    crate::extrapacks::sync(&dirs.instance())?;
    let mut keep_paths = keep_paths.clone();
    keep_paths.extend(crate::extrapacks::keep_paths(&dirs.instance())); // « Mes resource packs et shaders » : jamais supprimés
    // Ménage dans NOTRE instance uniquement : fichiers absents du pack (jars de mods ; zips de shaders et de
    // resource packs). Les fichiers de réglages (`*.zip.txt`) et les dossiers ne sont jamais supprimés.
    for (dir, ext) in [("mods", ".jar"), ("shaderpacks", ".zip"), ("resourcepacks", ".zip")] {
        let target = dirs.instance().join(dir);
        let Ok(rd) = fs::read_dir(&target) else {
            continue;
        };
        let wanted: HashSet<&str> = pack
            .mods
            .iter()
            .filter(|m| m.dir == dir)
            .map(|m| m.filename.as_str())
            .chain(pack.files.iter().filter_map(|f| f.path.strip_prefix(dir)?.strip_prefix('/')))
            .collect();
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let kept = wanted.contains(name.as_str()) || (dir == "mods" && keep.contains(&name)) || keep_paths.contains(&format!("{dir}/{name}"));
            // Fichiers seulement : un dossier nommé « x.jar » est ignoré.
            if name.ends_with(ext) && !kept && e.file_type().is_ok_and(|t| t.is_file()) {
                fs::remove_file(e.path())?;
            }
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
        fs::create_dir_all(d.join("packs/classroom-run")).unwrap();
        fs::create_dir_all(d.join("build/libs")).unwrap();
        fs::write(d.join("build/libs/classroomrun-1.jar"), b"x").unwrap();
        assert!(find_mod_jar(None, &d.join("packs/classroom-run")).is_ok());
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
            index_hash: String::new(),
            versions: packwiz::Versions { minecraft: "26.3".into(), fabric: Some("0.20.1".into()), quilt: None, forge: None, neoforge: None },
            mods: vec![],
            files: vec![],
        };
        let client = crate::download::http_client().unwrap();
        install_mods(&client, &dirs, &pack, &d, Some(&d.join("classroomrun-1.jar")), &HashSet::new(), &Arc::default()).unwrap();
        assert!(mods.join("dir.jar").is_dir() && !mods.join("old.jar").exists() && mods.join("classroomrun-1.jar").is_file());
        // sans installed.ok, load_installed refuse
        assert!(load_installed(&dirs).is_err());
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn extra_mods_survive_a_simulated_pack_update() {
        let d = tmp("extras");
        let dirs = Dirs::new(d.join("root"));
        let mods = dirs.instance().join("mods");
        fs::create_dir_all(&mods).unwrap();
        fs::write(mods.join("old-pack.jar"), b"x").unwrap();
        crate::extramods::add_bytes(&dirs, "mon-mod.jar", b"PKperso").unwrap();
        crate::extramods::sync(&dirs.instance(), &HashSet::new()).unwrap(); // état après un lancement
        assert!(mods.join("mon-mod.jar").is_file());
        let pack = Pack {
            version: "2".into(),
            index_hash: String::new(),
            versions: packwiz::Versions { minecraft: "26.3".into(), fabric: Some("0.20.1".into()), quilt: None, forge: None, neoforge: None },
            mods: vec![],
            files: vec![],
        };
        let client = crate::download::http_client().unwrap();
        install_mods(&client, &dirs, &pack, &d, None, &HashSet::new(), &Arc::default()).unwrap();
        assert!(!mods.join("old-pack.jar").exists(), "le ménage du pack reste actif");
        assert_eq!(fs::read(mods.join("mon-mod.jar")).unwrap(), b"PKperso");
        assert!(dirs.instance().join("opaline-extra-mods/mon-mod.jar").is_file());
        // supprimé de mods/ à la main : recopié par l'installation
        fs::remove_file(mods.join("mon-mod.jar")).unwrap();
        install_mods(&client, &dirs, &pack, &d, None, &HashSet::new(), &Arc::default()).unwrap();
        assert!(mods.join("mon-mod.jar").is_file());
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn extra_packs_survive_a_simulated_pack_update() {
        let d = tmp("extrapacks");
        let dirs = Dirs::new(d.join("root"));
        let inst = dirs.instance();
        let zip = |names: &[&str]| {
            let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
            for n in names {
                w.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
            }
            w.finish().unwrap().into_inner()
        };
        crate::extrapacks::add_bytes(&dirs, "mon-rp.zip", &zip(&["pack.mcmeta"])).unwrap();
        crate::extrapacks::add_bytes(&dirs, "mon-shader.zip", &zip(&["shaders/a.fsh"])).unwrap();
        fs::write(inst.join("resourcepacks/ancien-pack.zip"), b"x").unwrap();
        let pack = Pack {
            version: "2".into(),
            index_hash: String::new(),
            versions: packwiz::Versions { minecraft: "26.3".into(), fabric: Some("0.20.1".into()), quilt: None, forge: None, neoforge: None },
            mods: vec![],
            files: vec![],
        };
        let client = crate::download::http_client().unwrap();
        fs::remove_file(inst.join("shaderpacks/mon-shader.zip")).unwrap();
        install_mods(&client, &dirs, &pack, &d, None, &HashSet::new(), &Arc::default()).unwrap();
        assert!(!inst.join("resourcepacks/ancien-pack.zip").exists(), "le ménage du pack reste actif");
        assert!(inst.join("resourcepacks/mon-rp.zip").is_file() && inst.join("shaderpacks/mon-shader.zip").is_file());
        fs::remove_dir_all(&d).unwrap();
    }
}
