//! Loaders : types, liste blanche d'hôtes, exécution de l'`install_profile.json` (NeoForge, Forge).
//! Tout ici est pur ou local (aucun accès réseau) : le téléchargement est dans `install.rs`.
use crate::fabric::{FabricLib, FabricProfile, maven_path};
use crate::mojang::Arguments;
use crate::safe::{Hash, safe_join};
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoaderKind {
    Fabric,
    Quilt,
    NeoForge,
    Forge,
}

impl LoaderKind {
    pub fn parse(s: &str) -> Option<LoaderKind> {
        match s.trim().to_ascii_lowercase().as_str() {
            "fabric" => Some(LoaderKind::Fabric),
            "quilt" => Some(LoaderKind::Quilt),
            "neoforge" => Some(LoaderKind::NeoForge),
            "forge" => Some(LoaderKind::Forge),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            LoaderKind::Fabric => "fabric",
            LoaderKind::Quilt => "quilt",
            LoaderKind::NeoForge => "neoforge",
            LoaderKind::Forge => "forge",
        }
    }
}

/// Seuls hôtes depuis lesquels du code (libraries, installeurs, processors) est téléchargé.
pub const TRUSTED_HOSTS: [&str; 5] = ["maven.neoforged.net", "maven.minecraftforge.net", "libraries.minecraft.net", "maven.fabricmc.net", "maven.quiltmc.org"];

/// https vers un hôte de `TRUSTED_HOSTS` ; http seulement vers la boucle locale (serveurs de test, comme pour le catalogue).
pub fn check_host(url: &str) -> Result<()> {
    let (scheme, rest) = url.split_once("://").with_context(|| format!("URL invalide : {url}"))?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let loopback = scheme == "http" && ["127.0.0.1", "localhost"].contains(&authority.split(':').next().unwrap_or(""));
    if loopback || (scheme == "https" && TRUSTED_HOSTS.contains(&authority)) {
        return Ok(());
    }
    bail!("téléchargement refusé : hôte non autorisé ({})", authority.rsplit('@').next().unwrap_or(authority))
}

/// `pack.toml` de Forge peut porter la version avec ou sans le préfixe `<mc>-` : on garde la version du loader seule.
pub fn forge_version(mc: &str, v: &str) -> String {
    v.strip_prefix(&format!("{mc}-")).unwrap_or(v).to_string()
}

// ---------------------------------------------------------------- JSON de l'installeur

#[derive(Deserialize, Debug, Clone)]
pub struct Art {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub sha1: String,
    #[serde(default)]
    pub size: Option<u64>,
}
#[derive(Deserialize, Debug, Clone)]
pub struct Dl {
    pub artifact: Option<Art>,
}
#[derive(Deserialize, Debug, Clone)]
pub struct ProfileLib {
    pub name: String,
    pub downloads: Option<Dl>,
}

impl ProfileLib {
    /// Chemin relatif dans `libraries/` : celui du manifeste, sinon déduit des coordonnées.
    pub fn rel_path(&self) -> Result<String> {
        match self.artifact().map(|a| a.path.as_str()).filter(|p| !p.is_empty()) {
            Some(p) => Ok(p.to_string()),
            None => maven_path(&self.name),
        }
    }
    pub fn artifact(&self) -> Option<&Art> {
        self.downloads.as_ref().and_then(|d| d.artifact.as_ref())
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct DataEntry {
    pub client: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Processor {
    #[serde(default)]
    pub sides: Option<Vec<String>>,
    pub jar: String,
    #[serde(default)]
    pub classpath: Vec<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub outputs: HashMap<String, String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct InstallProfile {
    #[serde(default)]
    pub data: HashMap<String, DataEntry>,
    #[serde(default)]
    pub processors: Vec<Processor>,
    #[serde(default)]
    pub libraries: Vec<ProfileLib>,
}

/// `version.json` de l'installeur (hérite de la version vanilla).
#[derive(Deserialize, Debug, Clone)]
pub struct LoaderVersion {
    pub id: String,
    #[serde(rename = "inheritsFrom")]
    pub inherits_from: Option<String>,
    #[serde(rename = "mainClass")]
    pub main_class: String,
    #[serde(default)]
    pub arguments: Arguments,
    #[serde(default)]
    pub libraries: Vec<ProfileLib>,
}

impl LoaderVersion {
    /// Profil de lancement : la fusion avec la version vanilla (`inheritsFrom`) se fait dans `launch::build_command`.
    pub fn into_profile(self, mc: &str) -> Result<FabricProfile> {
        if let Some(i) = self.inherits_from.as_deref().filter(|i| *i != mc) {
            bail!("le loader hérite de {i} au lieu de {mc}");
        }
        let libraries = self
            .libraries
            .iter()
            .map(|l| {
                let a = l.artifact();
                Ok(FabricLib {
                    name: l.name.clone(),
                    url: a.map(|a| a.url.clone()).unwrap_or_default(),
                    sha1: a.map(|a| a.sha1.clone()).filter(|s| !s.is_empty()),
                    size: a.and_then(|a| a.size),
                    path: Some(l.rel_path()?),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(FabricProfile { main_class: self.main_class, arguments: self.arguments, libraries, version_name: Some(self.id) })
    }
}

// ---------------------------------------------------------------- variables et processors

pub struct Ctx {
    pub libs: PathBuf,
    pub data_dir: PathBuf,
    /// Variables résolues : intégrées (`MINECRAFT_JAR`, `SIDE`...) puis entrées `data` côté client.
    pub vars: HashMap<String, String>,
}

fn unwrap_pair(s: &str, open: char, close: char) -> Option<&str> {
    s.strip_prefix(open)?.strip_suffix(close)
}

impl Ctx {
    pub fn new(shared: &Path, libs: &Path, minecraft_jar: &Path, installer: &Path, data_dir: &Path, mc: &str, profile: &InstallProfile) -> Result<Ctx> {
        let s = |p: &Path| p.to_string_lossy().into_owned();
        let mut ctx = Ctx {
            libs: libs.to_path_buf(),
            data_dir: data_dir.to_path_buf(),
            vars: HashMap::from([
                ("MINECRAFT_JAR".into(), s(minecraft_jar)),
                ("SIDE".into(), "client".into()),
                ("ROOT".into(), s(shared)),
                ("LIBRARY_DIR".into(), s(libs)),
                ("INSTALLER".into(), s(installer)),
                ("MINECRAFT_VERSION".into(), mc.to_string()),
            ]),
        };
        for (k, v) in &profile.data {
            let value = ctx.resolve(&v.client).with_context(|| format!("entrée data {k}"))?;
            ctx.vars.insert(k.clone(), value);
        }
        Ok(ctx)
    }

    pub fn lib_path(&self, coords: &str) -> Result<PathBuf> {
        safe_join(&self.libs, &maven_path(coords)?)
    }

    /// `[groupe:artefact:version@ext]` -> chemin dans libraries ; `'littéral'` ; `/data/...` -> fichier extrait de l'installeur ;
    /// sinon texte où `{NOM}` est remplacé (variable inconnue : erreur).
    pub fn resolve(&self, arg: &str) -> Result<String> {
        if let Some(c) = unwrap_pair(arg, '[', ']') {
            return Ok(self.lib_path(c)?.to_string_lossy().into_owned());
        }
        if let Some(l) = unwrap_pair(arg, '\'', '\'') {
            return Ok(l.to_string());
        }
        if arg.starts_with("/data/") {
            return Ok(safe_join(&self.data_dir, arg.trim_start_matches('/'))?.to_string_lossy().into_owned());
        }
        let mut out = String::new();
        let mut rest = arg;
        while let Some(start) = rest.find('{') {
            let (head, tail) = rest.split_at(start);
            let end = tail.find('}').with_context(|| format!("accolade non fermée : {arg}"))?;
            let key = &tail[1..end];
            out.push_str(head);
            out.push_str(self.vars.get(key).with_context(|| format!("variable inconnue : {{{key}}}"))?);
            rest = &tail[end + 1..];
        }
        out.push_str(rest);
        Ok(out)
    }

    /// `jar` puis `classpath` du processor, en chemins de libraries.
    pub fn processor_classpath(&self, p: &Processor) -> Result<Vec<PathBuf>> {
        std::iter::once(&p.jar).chain(&p.classpath).map(|c| self.lib_path(c)).collect()
    }

    /// `None` : le processor ne déclare aucune sortie. `Some(true)` : toutes existent avec le bon sha1.
    pub fn outputs_valid(&self, p: &Processor) -> Result<Option<bool>> {
        if p.outputs.is_empty() {
            return Ok(None);
        }
        for (file, sha) in &p.outputs {
            let (file, sha) = (PathBuf::from(self.resolve(file)?), self.resolve(sha)?);
            if !file.is_file() || !Hash::new("sha1", &sha)?.matches_file(&file).unwrap_or(false) {
                return Ok(Some(false));
            }
        }
        Ok(Some(true))
    }
}

/// Processors exécutés : ceux sans `sides` ou dont `sides` contient `client`.
pub fn client_processors(p: &InstallProfile) -> Vec<&Processor> {
    p.processors.iter().filter(|x| x.sides.as_ref().is_none_or(|s| s.iter().any(|v| v == "client"))).collect()
}

pub fn join_classpath(entries: &[PathBuf], sep: &str) -> String {
    entries.iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>().join(sep)
}

/// `Main-Class` d'un MANIFEST.MF (lignes repliées par un espace de tête).
pub fn manifest_main_class(manifest: &str) -> Option<String> {
    let unfolded = manifest.replace("\r\n", "\n").replace("\n ", "");
    unfolded.lines().find_map(|l| l.strip_prefix("Main-Class:").map(|v| v.trim().to_string())).filter(|m| !m.is_empty())
}

pub fn jar_main_class(jar: &Path) -> Result<String> {
    let mut z = zip::ZipArchive::new(File::open(jar).with_context(|| format!("ouverture de {}", jar.display()))?)?;
    let mut text = String::new();
    z.by_name("META-INF/MANIFEST.MF").with_context(|| format!("pas de MANIFEST dans {}", jar.display()))?.read_to_string(&mut text)?;
    manifest_main_class(&text).with_context(|| format!("pas de Main-Class dans {}", jar.display()))
}

/// Contenu d'une entrée de l'installeur (`None` si absente).
pub fn read_entry(installer: &Path, name: &str) -> Result<Option<Vec<u8>>> {
    let mut z = zip::ZipArchive::new(File::open(installer)?)?;
    let mut f = match z.by_name(name.trim_start_matches('/')) {
        Ok(f) => f,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    Ok(Some(buf))
}

/// Extrait `data/*` de l'installeur dans `dest` (chemins préservés, jamais en dehors de `dest`).
pub fn extract_data(installer: &Path, dest: &Path) -> Result<()> {
    let mut z = zip::ZipArchive::new(File::open(installer)?)?;
    for i in 0..z.len() {
        let mut f = z.by_index(i)?;
        let name = f.name()?.into_owned();
        if f.is_dir() || !name.starts_with("data/") {
            continue;
        }
        let out = safe_join(dest, &name)?;
        fs::create_dir_all(out.parent().context("chemin sans parent")?)?;
        std::io::copy(&mut f, &mut File::create(&out)?)?;
    }
    Ok(())
}

// ---------------------------------------------------------------- exécution

pub const PROCESSOR_TIMEOUT: Duration = Duration::from_secs(900);

fn log_tail(log: &Path) -> String {
    let t = fs::read_to_string(log).unwrap_or_default();
    let lines: Vec<&str> = t.lines().collect();
    lines[lines.len().saturating_sub(15)..].join("\n")
}

/// Lance `cmd` (stdout et stderr ajoutés à `log`) et attend au plus `timeout` ; le processus est tué au-delà.
pub fn run_logged(mut cmd: Command, log: &Path, timeout: Duration) -> Result<()> {
    let out = fs::OpenOptions::new().create(true).append(true).open(log)?;
    cmd.stdin(Stdio::null()).stdout(out.try_clone()?).stderr(out);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let mut child = cmd.spawn().context("lancement du processor")?;
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            if status.success() {
                return Ok(());
            }
            bail!("le processor a échoué ({status}) :\n{}", log_tail(log));
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            bail!("le processor dépasse {} s, arrêté :\n{}", timeout.as_secs(), log_tail(log));
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// Exécute les processors client dans l'ordre. Ceux dont les sorties sont déjà valides sont ignorés ;
/// après exécution, des sorties invalides sont une erreur.
pub fn run_processors(java: &Path, ctx: &Ctx, profile: &InstallProfile, cwd: &Path, log: &Path, sep: &str, timeout: Duration) -> Result<()> {
    fs::create_dir_all(cwd)?;
    if let Some(parent) = log.parent() {
        fs::create_dir_all(parent)?;
    }
    for p in client_processors(profile) {
        if ctx.outputs_valid(p)? == Some(true) {
            continue;
        }
        let cp = ctx.processor_classpath(p)?;
        let main = jar_main_class(&cp[0])?;
        let args = p.args.iter().map(|a| ctx.resolve(a)).collect::<Result<Vec<_>>>()?;
        writeln!(fs::OpenOptions::new().create(true).append(true).open(log)?, "== {} {main}", p.jar)?;
        let mut cmd = Command::new(java);
        cmd.current_dir(cwd).arg("-cp").arg(join_classpath(&cp, sep)).arg(&main).args(&args);
        run_logged(cmd, log, timeout).with_context(|| format!("processor {}", p.jar))?;
        if ctx.outputs_valid(p)? == Some(false) {
            bail!("processor {} : sortie absente ou sha1 incorrect", p.jar);
        }
    }
    Ok(())
}
