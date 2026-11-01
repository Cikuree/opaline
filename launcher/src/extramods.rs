//! « Mes mods » : jars personnels (ajoutés à la main ou depuis Modrinth, avec leurs dépendances requises) stockés dans
//! `<instance>/opaline-extra-mods/`, copiés dans `mods/` à l'installation et au lancement. Le ménage de `mods/` fait par la mise
//! à jour du pack les conserve (voir `install::install_mods`). Le pack partagé n'est jamais modifié.
use crate::changelog;
use crate::config::Dirs;
use crate::modrinth::{self, Api};
use crate::safe::{Hash, safe_file_name};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

pub const DIR: &str = "opaline-extra-mods";
const INDEX: &str = "index.json";
pub const MAX_JAR_BYTES: u64 = 200 * 1024 * 1024;
const MAX_DEPTH: usize = 4;
const MAX_MODS_PER_ADD: usize = 15;

pub fn extra_dir(instance: &Path) -> PathBuf {
    instance.join(DIR)
}

/// Nom de fichier d'un jar perso : simple (aucun chemin), `.jar` seulement.
pub fn validate_name(name: &str) -> Result<&str> {
    let n = safe_file_name(name).map_err(|_| anyhow::anyhow!("nom de fichier refusé : pas de chemin"))?;
    if !n.to_ascii_lowercase().ends_with(".jar") || n.len() > 150 || n.starts_with('.') || n.chars().any(|c| c.is_control() || "<>\"|?*".contains(c)) {
        bail!("seuls les fichiers .jar au nom simple sont acceptés");
    }
    Ok(n)
}

// ---------- Index local ----------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub file: String,
    pub project_id: String,
    pub title: String,
    pub version: String,
    pub sha512: String,
    /// Ajouté automatiquement comme dépendance requise (retiré avec le dernier mod qui en a besoin).
    pub auto: bool,
    /// Projets Modrinth des mods perso qui requièrent celui-ci.
    pub required_by: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Index {
    pub mods: Vec<Entry>,
}

impl Index {
    /// Les entrées dont le fichier n'existe plus sont ignorées (jar supprimé à la main, retrait interrompu).
    pub fn load(instance: &Path) -> Index {
        let dir = extra_dir(instance);
        let mut idx: Index = fs::read_to_string(dir.join(INDEX)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        idx.mods.retain(|e| dir.join(&e.file).is_file());
        idx
    }

    pub fn save(&self, instance: &Path) -> Result<()> {
        let dir = extra_dir(instance);
        fs::create_dir_all(&dir)?;
        let tmp = dir.join("index.json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        fs::rename(&tmp, dir.join(INDEX))?;
        Ok(())
    }

    fn by_project(&self, p: &str) -> Option<&Entry> {
        self.mods.iter().find(|e| e.project_id == p)
    }

    /// Retire `file` puis, en cascade, les dépendances automatiques dont plus aucun mod perso n'a besoin. Renvoie les fichiers retirés.
    fn remove(&mut self, file: &str) -> Vec<String> {
        let mut removed = Vec::new();
        let mut gone: Vec<String> = self.mods.iter().filter(|e| e.file == file).map(|e| e.project_id.clone()).collect();
        self.mods.retain(|e| e.file != file);
        removed.push(file.to_string());
        while let Some(p) = gone.pop() {
            for e in &mut self.mods {
                e.required_by.retain(|r| *r != p);
            }
            let orphans: Vec<Entry> = self.mods.iter().filter(|e| e.auto && e.required_by.is_empty()).cloned().collect();
            for o in orphans {
                self.mods.retain(|e| e.file != o.file);
                removed.push(o.file);
                gone.push(o.project_id);
            }
        }
        removed
    }
}

// ---------- Ce que fournit le pack ----------

#[derive(Debug, Default)]
pub struct PackInfo {
    pub projects: HashSet<String>,
    pub sha512: HashSet<String>,
    pub files: HashSet<String>,
    /// Noms de mods en minuscules.
    pub names: HashSet<String>,
    /// Fichiers `mods/` du pack, chemins des jars installés.
    pub jars: Vec<(String, PathBuf)>,
}

pub fn pack_info(dirs: &Dirs) -> PackInfo {
    pack_info_opt(dirs).unwrap_or_default()
}

/// `None` : instantané du pack illisible ou absent (on ne sait pas quels fichiers de `mods/` sont ceux du pack).
fn pack_info_opt(dirs: &Dirs) -> Option<PackInfo> {
    let mut p = PackInfo::default();
    let snap = changelog::load_snapshot(dirs).ok()?;
    for m in snap.mods.iter().filter(|m| m.dir == "mods") {
        p.projects.extend(m.project_id.clone());
        if m.hash_format == "sha512" {
            p.sha512.insert(m.hash.to_ascii_lowercase());
        }
        p.files.insert(m.file.clone());
        p.names.insert(m.name.to_lowercase());
        p.jars.push((m.name.clone(), dirs.instance().join("mods").join(&m.file)));
    }
    Some(p)
}

/// Identifiant de mod lu dans le jar (fabric.mod.json, quilt.mod.json, neoforge.mods.toml / mods.toml).
pub fn jar_mod_id(path: &Path) -> Option<String> {
    type Cache = HashMap<(PathBuf, u64, u64), Option<String>>;
    static CACHE: std::sync::Mutex<Option<Cache>> = std::sync::Mutex::new(None);
    let m = fs::metadata(path).ok()?;
    let mtime = m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
    let key = (path.to_path_buf(), m.len(), mtime);
    if let Some(v) = CACHE.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).get(&key) {
        return v.clone();
    }
    let id = read_jar_mod_id(path);
    CACHE.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(key, id.clone());
    id
}

fn read_jar_mod_id(path: &Path) -> Option<String> {
    let mut z = zip::ZipArchive::new(fs::File::open(path).ok()?).ok()?;
    let mut read = |name: &str| -> Option<String> {
        let mut s = String::new();
        z.by_name(name).ok()?.take(1 << 20).read_to_string(&mut s).ok()?;
        Some(s)
    };
    if let Some(t) = read("fabric.mod.json") {
        return serde_json::from_str::<serde_json::Value>(&t).ok()?["id"].as_str().map(str::to_string);
    }
    if let Some(t) = read("quilt.mod.json") {
        return serde_json::from_str::<serde_json::Value>(&t).ok()?["quilt_loader"]["id"].as_str().map(str::to_string);
    }
    for f in ["META-INF/neoforge.mods.toml", "META-INF/mods.toml"] {
        if let Some(t) = read(f) {
            let v: toml::Value = toml::from_str(&t).ok()?;
            return v.get("mods")?.as_array()?.first()?.get("modId")?.as_str().map(str::to_string);
        }
    }
    None
}

// ---------- Liste, copie vers mods/, retrait ----------

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExtraMod {
    pub file: String,
    pub size_bytes: u64,
    /// Nom Modrinth si le mod vient de Modrinth.
    pub title: Option<String>,
    pub auto: bool,
    /// Titres des mods perso qui requièrent celui-ci.
    pub required_by: Vec<String>,
    pub warning: Option<String>,
}

fn jars_in(dir: &Path) -> Vec<(String, u64)> {
    let mut v: Vec<(String, u64)> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| Some((e.file_name().to_string_lossy().into_owned(), e.metadata().ok()?.len())))
        .filter(|(n, _)| validate_name(n).is_ok())
        .collect();
    v.sort();
    v
}

pub fn names(instance: &Path) -> HashSet<String> {
    jars_in(&extra_dir(instance)).into_iter().map(|(n, _)| n).collect()
}

pub fn list(dirs: &Dirs) -> Vec<ExtraMod> {
    let inst = dirs.instance();
    let jars = jars_in(&extra_dir(&inst));
    if jars.is_empty() {
        return Vec::new();
    }
    let (idx, pack) = (Index::load(&inst), pack_info(dirs));
    let pack_ids: HashMap<String, String> = pack.jars.iter().filter_map(|(n, p)| Some((jar_mod_id(p)?, n.clone()))).collect();
    jars.into_iter()
        .map(|(file, size_bytes)| {
            let e = idx.mods.iter().find(|e| e.file == file);
            let own_id = || jar_mod_id(&extra_dir(&inst).join(&file));
            let warning = if pack.files.contains(&file) {
                Some("Même nom de fichier qu'un mod du pack : la version du pack est conservée, ce jar n'est pas copié.".to_string())
            } else if e.is_some_and(|e| pack.projects.contains(&e.project_id)) {
                Some("Ce mod est déjà fourni par le pack.".to_string())
            } else {
                own_id().and_then(|id| pack_ids.get(&id).map(|n| format!("Même identifiant (« {id} ») que le mod « {n} » du pack : risque de conflit.")))
            };
            ExtraMod {
                title: e.map(|e| e.title.clone()),
                auto: e.is_some_and(|e| e.auto),
                required_by: e.map(|e| e.required_by.iter().filter_map(|p| idx.by_project(p)).map(|p| p.title.clone()).collect()).unwrap_or_default(),
                file,
                size_bytes,
                warning,
            }
        })
        .collect()
}

/// Copie les jars perso dans `mods/` (absents ou de taille différente), sauf les noms `reserved` (fichiers du pack : jamais écrasés).
pub fn sync(instance: &Path, reserved: &HashSet<String>) -> Result<()> {
    let dir = extra_dir(instance);
    let mods = instance.join("mods");
    for (name, size) in jars_in(&dir) {
        if reserved.contains(&name) {
            continue;
        }
        fs::create_dir_all(&mods)?;
        let dest = mods.join(&name);
        if dest.metadata().is_ok_and(|m| m.is_file() && m.len() == size) {
            continue;
        }
        fs::copy(dir.join(&name), &dest).with_context(|| format!("copie de {name} vers mods/"))?;
    }
    Ok(())
}

/// `sync` avec les noms de fichiers du pack installé (instantané).
/// Sans instantané lisible, rien n'est copié (on risquerait d'écraser un jar du pack).
pub fn sync_for_launch(dirs: &Dirs) -> Result<()> {
    match pack_info_opt(dirs) {
        Some(p) => sync(&dirs.instance(), &p.files),
        None => Ok(()),
    }
}

/// Retire un jar perso (et ses dépendances automatiques devenues inutiles) d'`opaline-extra-mods/` et de `mods/`.
pub fn remove(dirs: &Dirs, file: &str) -> Result<Vec<String>> {
    let file = validate_name(file)?;
    let inst = dirs.instance();
    if !extra_dir(&inst).join(file).is_file() {
        bail!("ce mod n'est pas dans Mes mods");
    }
    let mut idx = Index::load(&inst);
    if let Some(e) = idx.mods.iter().find(|e| e.file == file) {
        let by: Vec<&str> = e.required_by.iter().filter_map(|p| idx.by_project(p)).map(|p| p.title.as_str()).collect();
        if !by.is_empty() {
            bail!("« {} » est requis par {} : retire d'abord ce(s) mod(s)", e.title, by.join(", "));
        }
    }
    let removed = idx.remove(file);
    idx.save(&inst)?; // l'index d'abord : en cas d'échec ensuite, il reste au pire des jars sans entrée (traités comme des jars perso)
    // Sans instantané lisible, `mods/` n'est jamais touché (on ne sait pas quels jars sont ceux du pack).
    let pack = pack_info_opt(dirs);
    for f in &removed {
        fs::remove_file(extra_dir(&inst).join(f)).or_else(|e| if e.kind() == std::io::ErrorKind::NotFound { Ok(()) } else { Err(e) })?;
        if pack.as_ref().is_some_and(|p| !p.files.contains(f)) {
            let _ = fs::remove_file(inst.join("mods").join(f));
        }
    }
    Ok(removed)
}

fn place(dirs: &Dirs, name: &str, write: impl FnOnce(&Path) -> Result<u64>) -> Result<String> {
    let name = validate_name(name)?.to_string();
    let dir = extra_dir(&dirs.instance());
    fs::create_dir_all(&dir)?;
    let (dest, tmp) = (dir.join(&name), dir.join(format!("{name}.part")));
    if dest.exists() {
        bail!("« {name} » est déjà dans Mes mods");
    }
    let r = write(&tmp).and_then(|len| {
        let mut magic = [0u8; 2];
        fs::File::open(&tmp)?.read_exact(&mut magic).ok();
        if len == 0 || len > MAX_JAR_BYTES || &magic != b"PK" {
            bail!("« {name} » n'est pas un jar valide (ou dépasse 200 Mo)");
        }
        Ok(fs::rename(&tmp, &dest)?)
    });
    if r.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    r.map(|()| name)
}

/// Ajoute un jar choisi par l'utilisateur (glisser-déposer : chemin). Le nom du fichier est validé.
pub fn add_path(dirs: &Dirs, src: &Path) -> Result<String> {
    let name = src.file_name().and_then(|n| n.to_str()).context("nom de fichier illisible")?;
    let meta = fs::metadata(src).ok().filter(|m| m.is_file()).context("ce n'est pas un fichier")?;
    if meta.len() > MAX_JAR_BYTES {
        bail!("fichier trop lourd (200 Mo maximum)");
    }
    place(dirs, name, |tmp| Ok(fs::copy(src, tmp)?))
}

/// Ajoute un jar reçu en mémoire (sélecteur de fichier de l'interface).
pub fn add_bytes(dirs: &Dirs, name: &str, bytes: &[u8]) -> Result<String> {
    place(dirs, name, |tmp| fs::write(tmp, bytes).map(|()| bytes.len() as u64).map_err(Into::into))
}

// ---------- Modrinth : plan d'installation avec dépendances ----------

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Resolved {
    pub project_id: String,
    pub title: String,
    pub version: String,
    pub file: String,
    #[serde(skip)]
    pub url: String,
    #[serde(skip)]
    pub sha512: String,
    #[serde(skip)]
    pub size: Option<u64>,
}

#[derive(Debug, Default)]
pub struct Plan {
    pub root: Option<Resolved>,
    pub deps: Vec<Resolved>,
    /// Titres des dépendances requises déjà fournies (pack ou Mes mods) : rien n'est installé pour elles.
    pub provided: Vec<String>,
    /// `(dépendance, mod qui la requiert)`, projets Modrinth.
    pub links: Vec<(String, String)>,
}

struct Planner<'a> {
    api: &'a dyn Api,
    pack: &'a PackInfo,
    mine: &'a Index,
    mc: &'a str,
    loader: &'a str,
    plan: Plan,
}

impl Planner<'_> {
    fn title(&self, project: &str) -> String {
        self.api.title(project).unwrap_or_else(|| project.to_string())
    }

    fn planned(&self, p: &str) -> Option<&Resolved> {
        self.plan.root.iter().chain(&self.plan.deps).find(|r| r.project_id == p)
    }

    fn resolve(&mut self, project: &str, parent: Option<&str>, depth: usize) -> Result<()> {
        if depth > MAX_DEPTH {
            bail!("chaîne de dépendances trop profonde");
        }
        if let Some(par) = parent {
            self.plan.links.push((project.to_string(), par.to_string()));
        }
        if self.planned(project).is_some() {
            return Ok(());
        }
        let provided = |me: &mut Self, t: String| {
            me.plan.provided.push(t);
            Ok(())
        };
        if let Some(e) = self.mine.by_project(project) {
            return if parent.is_some() { provided(self, e.title.clone()) } else { bail!("« {} » est déjà dans Mes mods", e.title) };
        }
        if self.pack.projects.contains(project) {
            let t = self.title(project);
            return if parent.is_some() { provided(self, t) } else { bail!("« {t} » est déjà fourni par le pack") };
        }
        let versions = self.api.versions(project, self.mc, self.loader)?;
        let Some(v) = modrinth::choose_version(&versions) else {
            let t = self.title(project);
            if parent.is_some() {
                bail!("la dépendance « {t} » n'a aucune version compatible avec Minecraft {} ({})", self.mc, self.loader);
            }
            bail!("« {t} » n'a aucune version compatible avec Minecraft {} ({})", self.mc, self.loader);
        };
        let file = v.primary_file().context("aucun fichier")?;
        if !file.url.starts_with(modrinth::CDN) {
            bail!("téléchargement refusé : seul cdn.modrinth.com est autorisé");
        }
        let size = file.size.context("taille du fichier inconnue : téléchargement refusé")?;
        if size > MAX_JAR_BYTES {
            bail!("« {} » dépasse 200 Mo : téléchargement refusé", file.filename);
        }
        let sha512 = file.hashes.get("sha512").context("hash manquant")?.to_ascii_lowercase();
        let title = self.title(project);
        validate_name(&file.filename)?;
        if self.pack.sha512.contains(&sha512) || self.pack.files.contains(&file.filename) || self.pack.names.contains(&title.to_lowercase()) {
            return if parent.is_some() { provided(self, title) } else { bail!("« {title} » est déjà fourni par le pack") };
        }
        for d in v.dependencies.iter().filter(|d| d.dependency_type == "incompatible") {
            let Some(other) = d.project_id.as_deref() else { continue };
            if self.pack.projects.contains(other) || self.mine.by_project(other).is_some() || self.planned(other).is_some() {
                bail!("« {title} » est incompatible avec « {} », déjà présent : ajout refusé", self.title(other));
            }
        }
        let r = Resolved { project_id: project.to_string(), title, version: v.version_number.clone(), file: file.filename.clone(), url: file.url.clone(), sha512, size: Some(size) };
        if parent.is_none() {
            self.plan.root = Some(r);
        } else {
            self.plan.deps.push(r);
        }
        if self.plan.deps.len() + 1 > MAX_MODS_PER_ADD {
            bail!("trop de dépendances à installer (plus de {MAX_MODS_PER_ADD})");
        }
        for d in v.dependencies.iter().filter(|d| d.dependency_type == "required") {
            if let Some(dep) = d.project_id.as_deref() {
                self.resolve(dep, Some(project), depth + 1)?;
            }
        }
        Ok(())
    }
}

/// Résout `project` et ses dépendances requises (`optional` ignorée, `incompatible` refuse si le mod en cause est présent).
pub fn plan(api: &dyn Api, pack: &PackInfo, mine: &Index, project: &str, mc: &str, loader: &str) -> Result<Plan> {
    let mut p = Planner { api, pack, mine, mc, loader, plan: Plan::default() };
    p.resolve(project, None, 0)?;
    Ok(p.plan)
}

#[derive(Serialize, Debug, PartialEq)]
pub struct AddResult {
    pub added: Resolved,
    pub deps: Vec<Resolved>,
    pub provided: Vec<String>,
}

/// Télécharge (via `fetch(url, dest, sha512, size)`, hash vérifié) puis enregistre un plan. En cas d'échec, les fichiers de cet ajout sont retirés.
pub fn install_plan(dirs: &Dirs, plan: Plan, fetch: &dyn Fn(&Resolved, &Path) -> Result<()>) -> Result<AddResult> {
    let inst = dirs.instance();
    let dir = extra_dir(&inst);
    fs::create_dir_all(&dir)?;
    let root = plan.root.context("rien à installer")?;
    let mut created: Vec<PathBuf> = Vec::new();
    let mut idx = Index::load(&inst);
    let result: Result<()> = plan.deps.iter().chain(std::iter::once(&root)).try_for_each(|r| {
        let dest = dir.join(&r.file);
        if dest.exists() {
            bail!("le fichier « {} » existe déjà dans Mes mods", r.file);
        }
        created.push(dest.clone());
        fetch(r, &dest)?;
        idx.mods.push(Entry { file: r.file.clone(), project_id: r.project_id.clone(), title: r.title.clone(), version: r.version.clone(), sha512: r.sha512.clone(), auto: r.project_id != root.project_id, required_by: vec![] });
        Ok(())
    });
    if let Err(e) = result.and_then(|()| {
        for (dep, parent) in &plan.links {
            if let Some(e) = idx.mods.iter_mut().find(|e| e.project_id == *dep)
                && !e.required_by.contains(parent)
            {
                e.required_by.push(parent.clone());
            }
        }
        idx.save(&inst)
    }) {
        created.iter().for_each(|p| drop(fs::remove_file(p)));
        return Err(e);
    }
    Ok(AddResult { added: root, deps: plan.deps, provided: plan.provided })
}

/// Ajoute un projet Modrinth et ses dépendances requises. Un mod déjà présent comme dépendance automatique devient un mod à part entière.
pub fn add_modrinth(dirs: &Dirs, api: &dyn Api, project: &str, mc: &str, loader: &str, fetch: &dyn Fn(&Resolved, &Path) -> Result<()>) -> Result<AddResult> {
    let inst = dirs.instance();
    let mut idx = Index::load(&inst);
    if let Some(e) = idx.mods.iter_mut().find(|e| e.project_id == project && e.auto) {
        e.auto = false;
        let added = Resolved { project_id: e.project_id.clone(), title: e.title.clone(), version: e.version.clone(), file: e.file.clone(), url: String::new(), sha512: e.sha512.clone(), size: None };
        idx.save(&inst)?;
        return Ok(AddResult { added, deps: vec![], provided: vec![] });
    }
    install_plan(dirs, plan(api, &pack_info(dirs), &idx, project, mc, loader)?, fetch)
}

/// Téléchargeur de production : hash sha512 de l'API vérifié, reprise gérée par `download::fetch`.
pub fn download_job(client: &reqwest::blocking::Client, r: &Resolved, dest: &Path) -> Result<()> {
    anyhow::ensure!(r.url.starts_with(modrinth::CDN), "téléchargement refusé : seul cdn.modrinth.com est autorisé");
    let job = crate::download::Job { url: r.url.clone(), dest: dest.to_path_buf(), hash: Hash::new("sha512", &r.sha512)?, size: r.size };
    crate::download::fetch(client, &job)?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::type_complexity)]
mod tests {
    use super::*;
    use crate::modrinth::{Dependency, Hit, Version, VersionFile};
    use std::cell::RefCell;

    struct Fake {
        versions: HashMap<String, Vec<Version>>,
        titles: HashMap<String, String>,
        calls: RefCell<Vec<String>>,
    }

    impl Api for Fake {
        fn search(&self, _: &str, _: &str, _: &str) -> Result<Vec<Hit>> {
            Ok(vec![])
        }
        fn versions(&self, p: &str, _: &str, _: &str) -> Result<Vec<Version>> {
            self.calls.borrow_mut().push(p.to_string());
            Ok(self.versions.get(p).cloned().unwrap_or_default())
        }
        fn title(&self, p: &str) -> Option<String> {
            self.titles.get(p).cloned()
        }
    }

    /// `(projet, titre, dépendances [(projet, type)])` -> faux API.
    fn fake(mods: &[(&str, &str, &[(&str, &str)])]) -> Fake {
        let mut f = Fake { versions: HashMap::new(), titles: HashMap::new(), calls: RefCell::default() };
        for (p, t, deps) in mods {
            f.titles.insert(p.to_string(), t.to_string());
            f.versions.insert(
                p.to_string(),
                vec![Version {
                    id: format!("v-{p}"),
                    version_number: "1.0".into(),
                    version_type: "release".into(),
                    files: vec![VersionFile { url: format!("{}data/{p}/{p}.jar", modrinth::CDN), filename: format!("{p}.jar"), primary: true, size: Some(10), hashes: [("sha512".to_string(), format!("hash-{p}"))].into() }],
                    dependencies: deps.iter().map(|(d, ty)| Dependency { project_id: Some(d.to_string()), dependency_type: ty.to_string() }).collect(),
                }],
            );
        }
        f
    }

    fn dirs(tag: &str) -> Dirs {
        let d = std::env::temp_dir().join(format!("opaline-xm-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        Dirs::new(&d).for_pack("p").unwrap()
    }

    fn jar(dirs: &Dirs, rel: &str, extra: &str) {
        let p = dirs.instance().join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, format!("PK{extra}")).unwrap();
    }

    fn fake_fetch(r: &Resolved, dest: &Path) -> Result<()> {
        fs::write(dest, format!("PK{}", r.project_id))?;
        Ok(())
    }

    #[test]
    fn index_entries_without_a_file_are_ignored() {
        let d = dirs("orphan");
        let api = fake(&[("iris", "Iris", &[])]);
        add_modrinth(&d, &api, "iris", "1.21.1", "fabric", &fake_fetch).unwrap();
        assert_eq!(Index::load(&d.instance()).mods.len(), 1);
        fs::remove_file(extra_dir(&d.instance()).join("iris.jar")).unwrap();
        assert!(Index::load(&d.instance()).mods.is_empty());
        assert!(list(&d).is_empty());
        // on peut le rajouter : l'entrée fantôme ne bloque rien
        add_modrinth(&d, &api, "iris", "1.21.1", "fabric", &fake_fetch).unwrap();
        let _ = fs::remove_dir_all(&d.root);
    }

    #[test]
    fn a_mod_needed_by_another_cannot_be_removed_first() {
        let d = dirs("needed");
        changelog::save_snapshot(&d, &changelog::Snapshot::default()).unwrap();
        let api = fake(&[("iris", "Iris", &[("sodium", "required")]), ("sodium", "Sodium", &[])]);
        add_modrinth(&d, &api, "sodium", "1.21.1", "fabric", &fake_fetch).unwrap(); // ajouté à la main (pas auto)
        add_modrinth(&d, &api, "iris", "1.21.1", "fabric", &fake_fetch).unwrap();
        let e = remove(&d, "sodium.jar").unwrap_err().to_string();
        assert!(e.contains("« Sodium » est requis par Iris"), "{e}");
        assert!(names(&d.instance()).contains("sodium.jar"));
        assert_eq!(remove(&d, "iris.jar").unwrap(), ["iris.jar"]);
        assert_eq!(remove(&d, "sodium.jar").unwrap(), ["sodium.jar"]);
        let _ = fs::remove_dir_all(&d.root);
    }

    #[test]
    fn remove_never_touches_mods_without_a_readable_snapshot() {
        let d = dirs("nosnap");
        add_bytes(&d, "a.jar", b"PKa").unwrap();
        jar(&d, "mods/a.jar", "a");
        assert_eq!(remove(&d, "a.jar").unwrap(), ["a.jar"]);
        assert!(d.instance().join("mods/a.jar").is_file(), "mods/ intact : on ne sait pas ce qui vient du pack");
        assert!(!extra_dir(&d.instance()).join("a.jar").exists());
        add_bytes(&d, "b.jar", b"PKb").unwrap();
        sync_for_launch(&d).unwrap();
        assert!(!d.instance().join("mods/b.jar").exists(), "pas de copie sans instantané");
        let _ = fs::remove_dir_all(&d.root);
    }

    #[test]
    fn oversized_jars_are_refused_before_copy_and_before_download() {
        let d = dirs("big");
        let big = d.root.join("gros.jar");
        fs::create_dir_all(&d.root).unwrap();
        let f = fs::File::create(&big).unwrap();
        f.set_len(MAX_JAR_BYTES + 1).unwrap(); // fichier creux : aucune écriture réelle
        drop(f);
        let e = add_path(&d, &big).unwrap_err().to_string();
        assert!(e.contains("trop lourd"), "{e}");
        assert!(!extra_dir(&d.instance()).exists() || names(&d.instance()).is_empty());
        let mut api = fake(&[("x", "X", &[])]);
        api.versions.get_mut("x").unwrap()[0].files[0].size = Some(MAX_JAR_BYTES + 1);
        assert!(plan(&api, &PackInfo::default(), &Index::default(), "x", "1.21.1", "fabric").unwrap_err().to_string().contains("200 Mo"));
        api.versions.get_mut("x").unwrap()[0].files[0].size = None;
        assert!(plan(&api, &PackInfo::default(), &Index::default(), "x", "1.21.1", "fabric").is_err());
        let _ = fs::remove_dir_all(&d.root);
    }

    #[test]
    fn names_are_validated() {
        for ok in ["iris-1.8.jar", "A B.JAR"] {
            assert!(validate_name(ok).is_ok(), "{ok}");
        }
        for bad in ["a.zip", "../x.jar", "a/b.jar", "a\\b.jar", "c:x.jar", ".hidden.jar", "x.jar.exe", "", "a\u{7}.jar", "a|b.jar"] {
            assert!(validate_name(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn dependencies_are_installed_and_orphans_removed_with_their_parent() {
        let d = dirs("dep");
        changelog::save_snapshot(&d, &changelog::Snapshot::default()).unwrap(); // pack installé, sans mod
        let api = fake(&[("iris", "Iris", &[("sodium", "required"), ("opt", "optional")]), ("sodium", "Sodium", &[]), ("opt", "Opt", &[])]);
        let plan1 = plan(&api, &PackInfo::default(), &Index::default(), "iris", "1.21.1", "fabric").unwrap();
        assert_eq!(plan1.deps.iter().map(|r| r.title.as_str()).collect::<Vec<_>>(), ["Sodium"]);
        let r = install_plan(&d, plan1, &fake_fetch).unwrap();
        assert_eq!((r.added.title.as_str(), r.deps.len()), ("Iris", 1));
        let l = list(&d);
        assert_eq!(l.iter().map(|m| (m.file.as_str(), m.auto)).collect::<Vec<_>>(), [("iris.jar", false), ("sodium.jar", true)]);
        assert_eq!(l[1].required_by, ["Iris"]);
        // un second mod requiert aussi Sodium : il n'est pas réinstallé, et survit au retrait d'Iris
        let api2 = fake(&[("other", "Other", &[("sodium", "required")]), ("sodium", "Sodium", &[])]);
        let p2 = plan(&api2, &PackInfo::default(), &Index::load(&d.instance()), "other", "1.21.1", "fabric").unwrap();
        assert!(p2.deps.is_empty() && p2.provided == ["Sodium"] && !api2.calls.borrow().contains(&"sodium".to_string()));
        install_plan(&d, p2, &fake_fetch).unwrap();
        sync(&d.instance(), &HashSet::new()).unwrap();
        assert!(d.instance().join("mods/sodium.jar").is_file());
        assert_eq!(remove(&d, "iris.jar").unwrap(), ["iris.jar"]);
        assert!(d.instance().join(DIR).join("sodium.jar").is_file() && !d.instance().join("mods/iris.jar").exists());
        // retrait du dernier : sodium (auto) part aussi, de DIR et de mods/
        let mut gone = remove(&d, "other.jar").unwrap();
        gone.sort();
        assert_eq!(gone, ["other.jar", "sodium.jar"]);
        assert!(names(&d.instance()).is_empty() && !d.instance().join("mods/sodium.jar").exists());
        let _ = fs::remove_dir_all(&d.root);
    }

    /// Essai réseau réel : `cargo test --lib real_modrinth -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn real_modrinth_iris_pulls_sodium() {
        let d = dirs("real");
        let api = modrinth::Http::default_api().unwrap();
        let hits = api.search("iris", "1.21.1", "fabric").unwrap();
        assert!(hits.iter().any(|h| h.title.starts_with("Iris")), "{hits:?}");
        let client = crate::download::http_client().unwrap();
        let r = add_modrinth(&d, &api, "YL57xq9U", "1.21.1", "fabric", &|r, dest| download_job(&client, r, dest)).unwrap();
        println!("ajouté {:?} avec {:?}, fournis {:?}", r.added.file, r.deps.iter().map(|x| &x.file).collect::<Vec<_>>(), r.provided);
        assert!(r.deps.iter().any(|x| x.title == "Sodium"));
        for m in list(&d) {
            assert_eq!(&fs::read(extra_dir(&d.instance()).join(&m.file)).unwrap()[..2], b"PK");
        }
        // le mod est complet et son hash a été vérifié par download_job ; retrait en cascade
        assert_eq!(remove(&d, &r.added.file).unwrap().len(), 1 + r.deps.len());
        assert!(names(&d.instance()).is_empty());
        let _ = fs::remove_dir_all(&d.root);
    }

    #[test]
    fn adding_an_auto_dependency_by_hand_keeps_it_when_the_parent_goes() {
        let d = dirs("promote");
        let api = fake(&[("iris", "Iris", &[("sodium", "required")]), ("sodium", "Sodium", &[])]);
        add_modrinth(&d, &api, "iris", "1.21.1", "fabric", &fake_fetch).unwrap();
        let r = add_modrinth(&d, &api, "sodium", "1.21.1", "fabric", &fake_fetch).unwrap();
        assert_eq!(r.added.file, "sodium.jar");
        remove(&d, "iris.jar").unwrap();
        assert_eq!(names(&d.instance()), HashSet::from(["sodium.jar".to_string()]));
        assert!(add_modrinth(&d, &api, "sodium", "1.21.1", "fabric", &fake_fetch).unwrap_err().to_string().contains("déjà dans Mes mods"));
        let _ = fs::remove_dir_all(&d.root);
    }

    #[test]
    fn dependency_already_in_the_pack_is_not_installed() {
        let api = fake(&[("iris", "Iris", &[("sodium", "required"), ("fabric-api", "required")]), ("sodium", "Sodium", &[]), ("fabric-api", "Fabric API", &[])]);
        let mut pack = PackInfo::default();
        pack.projects.insert("sodium".into()); // connu par project_id
        pack.sha512.insert("hash-fabric-api".into()); // sinon, reconnu par hash
        let p = plan(&api, &pack, &Index::default(), "iris", "1.21.1", "fabric").unwrap();
        assert!(p.deps.is_empty(), "{p:?}");
        assert_eq!(p.provided, ["Sodium", "Fabric API"]);
        // par nom de fichier
        let mut pack = PackInfo::default();
        pack.files.insert("sodium.jar".into());
        let p = plan(&api, &pack, &Index::default(), "iris", "1.21.1", "fabric").unwrap();
        assert_eq!(p.deps.len(), 1);
        assert_eq!(p.deps[0].title, "Fabric API");
        // le mod demandé lui-même déjà fourni : refus clair
        let mut pack = PackInfo::default();
        pack.projects.insert("iris".into());
        assert!(plan(&api, &pack, &Index::default(), "iris", "1.21.1", "fabric").unwrap_err().to_string().contains("déjà fourni par le pack"));
    }

    #[test]
    fn dependency_cycles_terminate() {
        let api = fake(&[("a", "A", &[("b", "required")]), ("b", "B", &[("a", "required")])]);
        let p = plan(&api, &PackInfo::default(), &Index::default(), "a", "1.21.1", "fabric").unwrap();
        assert_eq!((p.root.unwrap().title.as_str(), p.deps.len()), ("A", 1));
        assert_eq!(api.calls.borrow().len(), 2);
    }

    #[test]
    fn deep_or_huge_chains_are_bounded() {
        let names: Vec<String> = (0..30).map(|i| format!("m{i}")).collect();
        let defs: Vec<(String, String, Vec<(String, &str)>)> = (0..30).map(|i| (names[i].clone(), names[i].to_uppercase(), if i < 29 { vec![(names[i + 1].clone(), "required")] } else { vec![] })).collect();
        let owned: Vec<Vec<(&str, &str)>> = defs.iter().map(|d| d.2.iter().map(|(a, b)| (a.as_str(), *b)).collect()).collect();
        let list: Vec<(&str, &str, &[(&str, &str)])> = defs.iter().zip(&owned).map(|(d, o)| (d.0.as_str(), d.1.as_str(), o.as_slice())).collect();
        let api = fake(&list);
        let e = plan(&api, &PackInfo::default(), &Index::default(), "m0", "1.21.1", "fabric").unwrap_err().to_string();
        assert!(e.contains("trop profonde"), "{e}");
    }

    #[test]
    fn incompatible_present_mod_refuses_but_absent_one_does_not() {
        let api = fake(&[("iris", "Iris", &[("optifine", "incompatible")]), ("optifine", "OptiFine", &[])]);
        assert!(plan(&api, &PackInfo::default(), &Index::default(), "iris", "1.21.1", "fabric").is_ok());
        let mut pack = PackInfo::default();
        pack.projects.insert("optifine".into());
        let e = plan(&api, &pack, &Index::default(), "iris", "1.21.1", "fabric").unwrap_err().to_string();
        assert!(e.contains("« Iris » est incompatible avec « OptiFine »"), "{e}");
    }

    #[test]
    fn missing_compatible_version_and_foreign_hosts_are_refused() {
        let api = fake(&[("iris", "Iris", &[("ghost", "required")])]);
        let e = plan(&api, &PackInfo::default(), &Index::default(), "iris", "1.21.1", "fabric").unwrap_err().to_string();
        assert!(e.contains("dépendance") && e.contains("aucune version compatible"), "{e}");
        let mut evil = fake(&[("x", "X", &[])]);
        evil.versions.get_mut("x").unwrap()[0].files[0].url = "https://evil.example/x.jar".into();
        assert!(plan(&evil, &PackInfo::default(), &Index::default(), "x", "1.21.1", "fabric").is_err());
    }

    #[test]
    fn failed_download_rolls_back_this_addition_only() {
        let d = dirs("roll");
        let api = fake(&[("iris", "Iris", &[("sodium", "required")]), ("sodium", "Sodium", &[])]);
        jar(&d, "opaline-extra-mods/mine.jar", "");
        let p = plan(&api, &PackInfo::default(), &Index::default(), "iris", "1.21.1", "fabric").unwrap();
        let failing = |r: &Resolved, dest: &Path| if r.project_id == "iris" { bail!("réseau coupé") } else { fake_fetch(r, dest) };
        assert!(install_plan(&d, p, &failing).is_err());
        assert_eq!(names(&d.instance()), HashSet::from(["mine.jar".to_string()]));
        assert!(Index::load(&d.instance()).mods.is_empty());
        let _ = fs::remove_dir_all(&d.root);
    }

    #[test]
    fn manual_jars_are_validated_and_survive_a_pack_update() {
        let d = dirs("keep");
        assert!(add_bytes(&d, "../evil.jar", b"PKxx").is_err() && add_bytes(&d, "x.txt", b"PKxx").is_err());
        assert!(add_bytes(&d, "fake.jar", b"pas un zip").is_err());
        assert_eq!(add_bytes(&d, "mon-mod.jar", b"PKcontenu").unwrap(), "mon-mod.jar");
        assert!(add_bytes(&d, "mon-mod.jar", b"PKbis").is_err(), "pas d'écrasement");
        assert!(!extra_dir(&d.instance()).join("mon-mod.jar.part").exists());
        // copie dans mods/ ; un jar du pack de même nom n'est pas écrasé
        jar(&d, "mods/pack.jar", "pack");
        changelog::save_snapshot(&d, &changelog::Snapshot { mods: vec![changelog::SnapMod { name: "Pack".into(), version: "1".into(), side: "both".into(), dir: "mods".into(), file: "pack.jar".into(), hash_format: "sha1".into(), hash: "ab".into(), project_id: None }] }).unwrap();
        add_bytes(&d, "pack.jar", b"PKperso").unwrap();
        sync(&d.instance(), &HashSet::from(["pack.jar".to_string()])).unwrap();
        assert_eq!(fs::read(d.instance().join("mods/pack.jar")).unwrap(), b"PKpack");
        assert_eq!(fs::read(d.instance().join("mods/mon-mod.jar")).unwrap(), b"PKcontenu");
        // retirer un jar perso n'enlève pas le jar du pack de même nom
        remove(&d, "pack.jar").unwrap();
        assert!(d.instance().join("mods/pack.jar").is_file() && remove(&d, "pack.jar").is_err());
        // glisser-déposer par chemin
        let src = d.root.join("dl").join("depuis-disque.jar");
        fs::create_dir_all(src.parent().unwrap()).unwrap();
        fs::write(&src, b"PKdisque").unwrap();
        assert_eq!(add_path(&d, &src).unwrap(), "depuis-disque.jar");
        assert!(add_path(&d, &d.root).is_err());
        let _ = fs::remove_dir_all(&d.root);
    }

    #[test]
    fn duplicate_names_and_ids_are_reported() {
        let d = dirs("warn");
        // jar du pack avec fabric.mod.json
        let mut zip_bytes = Vec::new();
        {
            use std::io::Write;
            let mut w = zip::ZipWriter::new(std::io::Cursor::new(&mut zip_bytes));
            w.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(br#"{"schemaVersion":1,"id":"sodium","version":"1"}"#).unwrap();
            w.finish().unwrap();
        }
        fs::create_dir_all(d.instance().join("mods")).unwrap();
        fs::write(d.instance().join("mods/sodium-pack.jar"), &zip_bytes).unwrap();
        changelog::save_snapshot(&d, &changelog::Snapshot { mods: vec![changelog::SnapMod { name: "Sodium".into(), version: "1".into(), side: "both".into(), dir: "mods".into(), file: "sodium-pack.jar".into(), hash_format: "sha512".into(), hash: "AB".into(), project_id: Some("AANobbMI".into()) }] }).unwrap();
        add_bytes(&d, "sodium-perso.jar", &zip_bytes).unwrap();
        add_bytes(&d, "sodium-pack.jar", b"PKx").unwrap();
        add_bytes(&d, "autre.jar", b"PKx").unwrap();
        let l = list(&d);
        let w = |f: &str| l.iter().find(|m| m.file == f).unwrap().warning.clone();
        assert!(w("sodium-perso.jar").unwrap().contains("Même identifiant (« sodium »)"), "{:?}", w("sodium-perso.jar"));
        assert!(w("sodium-pack.jar").unwrap().contains("Même nom de fichier"));
        assert_eq!(w("autre.jar"), None);
        let info = pack_info(&d);
        assert!(info.projects.contains("AANobbMI") && info.sha512.contains("ab"));
        let _ = fs::remove_dir_all(&d.root);
    }
}
