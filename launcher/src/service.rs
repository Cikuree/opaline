//! Opérations de haut niveau (une par commande de l'interface). Tout est synchrone : l'appelant (backend Tauri)
//! les exécute sur un fil dédié. Les erreurs sont des phrases lisibles, à passer par `sanitize_error` avant affichage.
use crate::auth::{self, AuthManager, Session};
use crate::catalog::{self, PackEntry, PackView};
use crate::changelog::{self, Changelog, ModView};
use crate::config::{self, Config, Dirs};
use crate::download::{self, Progress};
use crate::{backup, extramods, friends, identity, install, launch, mojang, official, packwiz, perf, servers, serverview, stats, sysmem, textures, update};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

fn short_client() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder().user_agent(concat!("Opaline/", env!("CARGO_PKG_VERSION"))).timeout(Duration::from_secs(10)).build()?)
}

pub fn packs(root: &Dirs) -> Result<Vec<PackEntry>> {
    catalog::load(root, &short_client()?)
}

pub fn entry(root: &Dirs, id: &str) -> Result<PackEntry> {
    catalog::find(&packs(root)?, id)
}

pub fn packs_list(root: &Dirs) -> Result<Vec<PackView>> {
    packs(root)?
        .iter()
        .map(|e| {
            let d = root.for_pack(&e.id)?;
            Ok(e.view(install::installed_hash(&d).as_deref(), d.install_marker().is_file()))
        })
        .collect()
}

/// Désinstalle un pack : copie des mondes (`backups/<pack>/`, refusée si un monde est ouvert), puis suppression de l'instance de ce
/// pack seulement (mods, Mes mods, configs, mondes). `shared/`, `backups/` et les autres packs ne sont jamais touchés.
/// L'appelant a déjà refusé si le jeu ou une installation de ce pack tourne.
pub fn uninstall_pack(root: &Dirs, id: &str) -> Result<()> {
    let dirs = root.for_pack(id)?;
    let inst = dirs.instance();
    if !inst.is_dir() {
        bail!("ce pack n'est pas installé");
    }
    backup::backup_saves(&dirs, friends::now_secs())?; // refuse si un monde est ouvert : rien n'est alors supprimé
    std::fs::remove_dir_all(&inst).context("suppression du pack impossible (un fichier est-il ouvert ailleurs ?)")
}

/// Installe ou met à jour le pack (revérifie tout : sert aussi de réparation). `local_pack` : dossier packwiz de dev.
/// Une panique devient une erreur : l'interface ne reste jamais figée.
pub fn install_pack(root: &Dirs, id: &str, progress: &Arc<Progress>, local_pack: Option<&Path>) -> Result<()> {
    install_pack_with(root, id, progress, local_pack, &install::Endpoints::default())
}

/// Comme `install_pack`, avec les services Mojang / Fabric / Adoptium injectables (tests sur fixtures).
pub fn install_pack_with(root: &Dirs, id: &str, progress: &Arc<Progress>, local_pack: Option<&Path>, endpoints: &install::Endpoints) -> Result<()> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| install_inner(root, id, progress, local_pack, endpoints)))
        .unwrap_or_else(|_| bail!("erreur interne pendant l'installation"))
}

fn install_inner(root: &Dirs, id: &str, progress: &Arc<Progress>, local_pack: Option<&Path>, endpoints: &install::Endpoints) -> Result<()> {
    let client = download::http_client()?;
    let e = catalog::find(&catalog::load(root, &short_client()?)?, id)?;
    if !e.supported() {
        bail!("ce pack n'est pas pris en charge par cette version d'Opaline (loader {})", e.loader);
    }
    let dirs = root.for_pack(id)?;
    let pack_dir = match local_pack {
        Some(p) => p.to_path_buf(),
        None => packwiz::fetch_remote_pack(&client, &dirs, &e.pack)?,
    };
    if local_pack.is_none() && !catalog::pack_version_hash(&packwiz::load_pack(&pack_dir)?.index_hash, e.overrides.as_ref()).eq_ignore_ascii_case(e.pack_hash.trim()) {
        bail!("le pack téléchargé ne correspond pas au catalogue (packHash) : réessayez plus tard");
    }
    let params = install::InstallParams {
        dirs: dirs.clone(),
        pack_dir: pack_dir.clone(),
        mod_jar: local_pack.and_then(|_| install::find_mod_jar(None, &pack_dir).ok()),
        endpoints: endpoints.clone(),
        overrides: e.overrides.clone(),
    };
    // Avant toute écriture dans l'instance : copie des mondes (l'appelant a déjà refusé si le jeu de ce pack tourne).
    backup::backup_saves(&dirs, friends::now_secs())?;
    let installed = install::install(&client, &params, progress)?;
    progress.set_stage("textures");
    // Cosmétique : un échec ici n'invalide pas l'installation.
    let _ = textures::extract(&dirs.client_jar(&installed.version.id), &root.textures(), std::slice::from_ref(&e.thumbnail));
    progress.complete_stage();
    Ok(())
}

pub fn changelog(root: &Dirs, id: &str) -> Result<Changelog> {
    let e = entry(root, id)?;
    let dirs = root.for_pack(id)?;
    let installed = changelog::load_snapshot(&dirs)?;
    // Dossier de travail unique par appel (distinct aussi de celui de l'installation), nettoyé quoi qu'il arrive.
    static CALLS: AtomicU64 = AtomicU64::new(0);
    let work = dirs.downloads().join(format!("changelog-{id}-{}-{}", std::process::id(), CALLS.fetch_add(1, Ordering::Relaxed)));
    let published =
        packwiz::fetch_remote_pack_in(&download::http_client()?, &work, &e.pack).and_then(|dir| packwiz::load_pack(&dir)).map(|p| changelog::snapshot_of(&p));
    let _ = std::fs::remove_dir_all(&work);
    Ok(changelog::diff(&installed, &published?))
}

// ---------- Mes mods ----------

pub fn extra_mods_list(root: &Dirs, id: &str) -> Result<Vec<extramods::ExtraMod>> {
    Ok(extramods::list(&root.for_pack(id)?))
}

pub fn extra_mod_search(root: &Dirs, id: &str, query: &str, api: &dyn crate::modrinth::Api) -> Result<Vec<crate::modrinth::Hit>> {
    let e = entry(root, id)?;
    anyhow::ensure!(!query.trim().is_empty() && query.len() <= 100, "saisis un nom de mod");
    api.search(query.trim(), &e.mc, &e.loader)
}

/// Ajoute un projet Modrinth (et ses dépendances requises) à Mes mods.
pub fn extra_mod_add_modrinth(root: &Dirs, id: &str, project: &str, api: &dyn crate::modrinth::Api) -> Result<extramods::AddResult> {
    let e = entry(root, id)?;
    let dirs = root.for_pack(id)?;
    anyhow::ensure!(dirs.install_marker().is_file(), "installe le pack avant d'ajouter des mods");
    let client = download::http_client()?;
    extramods::add_modrinth(&dirs, api, project, &e.mc, &e.loader, &|r, dest| extramods::download_job(&client, r, dest))
}

pub fn pack_mods(root: &Dirs, id: &str) -> Result<Vec<ModView>> {
    let dirs = root.for_pack(id)?;
    Ok(changelog::mod_views(&dirs, &changelog::load_snapshot(&dirs)?))
}

/// Au démarrage : si les textures manquent, télécharge le client.jar officiel (sha1, partagé avec l'installation)
/// et les extrait. `Ok(Some(dossier))` si des textures viennent d'être extraites, `Ok(None)` si rien à faire.
/// Hors ligne : erreur, à ignorer (nouvel essai au prochain démarrage).
pub fn ensure_textures(root: &Dirs, endpoints: &install::Endpoints) -> Result<Option<PathBuf>> {
    if textures::complete(&root.textures()) {
        return Ok(None);
    }
    let packs = packs(root).unwrap_or_default();
    let extra: Vec<String> = packs.iter().map(|p| p.thumbnail.clone()).filter(|t| !t.is_empty()).collect();
    let client = download::http_client()?;
    let v = install::fetch_version_json(&client, root, endpoints, textures::pick_mc(&packs))?;
    textures::ensure_from_jar(&client, root, &v.id, &v.downloads.client, &extra)?;
    Ok(Some(root.textures()))
}

/// Dossier des textures extraites, s'il en contient.
pub fn textures_dir(root: &Dirs) -> Option<PathBuf> {
    let d = root.textures();
    std::fs::read_dir(&d).ok()?.flatten().any(|e| e.path().extension().is_some_and(|x| x == "png")).then_some(d)
}

// ---------- Carte du serveur ----------

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MapInfo {
    pub name: String,
    pub url: String,
    /// Le site a répondu.
    pub reachable: bool,
    /// Aucun en-tête n'interdit l'intégration dans un cadre (`X-Frame-Options`, `frame-ancestors`).
    pub embeddable: bool,
}

/// Nom du pack et URL de carte à ouvrir (nom court Tailscale complété par le suffixe MagicDNS connu).
pub fn map_target(root: &Dirs, id: &str) -> Result<(String, String)> {
    let e = entry(root, id)?;
    let raw = e.server.and_then(|s| s.map).context("ce serveur n'a pas de carte")?;
    let url = catalog::resolve_map(&raw, magic_suffix().as_deref()).context("adresse de carte invalide")?;
    Ok((e.name, url))
}

/// Le site interdit-il l'intégration dans un cadre ? `X-Frame-Options` (DENY, SAMEORIGIN) ou `frame-ancestors` sans `*` ni http(s).
pub fn frame_allowed(xfo: Option<&str>, csp: Option<&str>) -> bool {
    if xfo.is_some_and(|v| !v.trim().is_empty()) {
        return false;
    }
    let ancestors = csp.into_iter().flat_map(|c| c.split(';')).map(str::trim).find_map(|d| d.strip_prefix("frame-ancestors"));
    ancestors.is_none_or(|a| a.split_whitespace().any(|t| t == "*" || t.starts_with("http:") || t.starts_with("https:")))
}

/// Sonde la carte (un GET, corps ignoré) : joignable, et intégrable ou non. Sert à choisir entre le cadre et un message clair.
pub fn map_info(root: &Dirs, id: &str) -> Result<MapInfo> {
    let (name, url) = map_target(root, id)?;
    let h = |r: &reqwest::blocking::Response, k: &str| r.headers().get(k).and_then(|v| v.to_str().ok()).map(str::to_string);
    Ok(match short_client()?.get(&url).send() {
        Ok(r) if !r.status().is_server_error() => {
            let embeddable = frame_allowed(h(&r, "x-frame-options").as_deref(), h(&r, "content-security-policy").as_deref());
            MapInfo { name, url, reachable: true, embeddable }
        }
        _ => MapInfo { name, url, reachable: false, embeddable: true },
    })
}

// ---------- Réglages ----------

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    pub perf_mode: perf::PerfMode,
    pub memory_mb: u32,
    pub no_remember: bool,
    pub launch_via: crate::config::LaunchVia,
    pub join_server: bool,
    pub notify_server: bool,
    pub discord_presence: bool,
    pub tray_on_game: bool,
    /// Mode et mémoire réglés automatiquement au premier lancement (et pas encore modifiés).
    pub auto_tuned: bool,
    pub system_ram_mb: Option<u64>,
}

pub fn settings_get(root: &Dirs) -> SettingsView {
    let c = Config::load(root);
    SettingsView {
        perf_mode: c.perf_mode,
        memory_mb: c.memory_mb,
        no_remember: c.no_remember,
        launch_via: c.launch_via,
        join_server: c.join_server,
        notify_server: c.notify_server,
        discord_presence: c.discord_presence,
        tray_on_game: c.tray_on_game,
        auto_tuned: c.auto_tuned,
        system_ram_mb: sysmem::total_ram_mb(),
    }
}

/// Mémoire bornée : au moins 1 Go, au plus 75 % de la RAM physique (si connue).
pub fn clamp_memory(mb: u32, ram_mb: Option<u64>) -> u32 {
    let max = ram_mb.map_or(u32::MAX, |r| ((r * 3 / 4) as u32).max(1024));
    mb.clamp(1024, max)
}

/// Modifications demandées par l'interface : seuls les champs présents changent.
#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct SettingsPatch {
    pub perf_mode: Option<perf::PerfMode>,
    pub memory_mb: Option<u32>,
    pub no_remember: Option<bool>,
    pub launch_via: Option<crate::config::LaunchVia>,
    pub join_server: Option<bool>,
    pub notify_server: Option<bool>,
    pub discord_presence: Option<bool>,
    pub tray_on_game: Option<bool>,
}

pub fn settings_set(root: &Dirs, p: SettingsPatch) -> Result<SettingsView> {
    let mut c = Config::load(root);
    if let Some(m) = p.perf_mode {
        c.perf_mode = m;
    }
    if let Some(m) = p.memory_mb {
        c.memory_mb = clamp_memory(m, sysmem::total_ram_mb());
    }
    if p.perf_mode.is_some() || p.memory_mb.is_some() {
        c.auto_tuned = false; // choix de l'utilisateur : le message « réglé automatiquement » disparaît
    }
    c.no_remember = p.no_remember.unwrap_or(c.no_remember);
    c.launch_via = p.launch_via.unwrap_or(c.launch_via);
    c.join_server = p.join_server.unwrap_or(c.join_server);
    c.notify_server = p.notify_server.unwrap_or(c.notify_server);
    c.discord_presence = p.discord_presence.unwrap_or(c.discord_presence);
    c.tray_on_game = p.tray_on_game.unwrap_or(c.tray_on_game);
    c.save(root)?;
    Ok(settings_get(root))
}

// ---------- Jeu ----------

/// Session pour jouer : hors ligne en dev (`CLASSROOM_DEV_OFFLINE=1`), sinon Microsoft (mémoire, puis trousseau).
/// `None` : pas de session Opaline (non connecté, ou connexion non configurée) : JOUER passe par le Launcher officiel.
pub fn try_session(cfg: &Config, mgr: &AuthManager) -> Result<Option<Session>> {
    if auth::dev_offline() {
        return Ok(Some(Session::offline(cfg.nickname.trim())));
    }
    if !auth::configured() {
        return Ok(None);
    }
    mgr.ensure_session(&auth::http_client()?, &auth::Endpoints::default(), auth::CLIENT_ID, !cfg.no_remember)
}

/// Comme `try_session`, mais l'absence de session est une erreur (commandes qui exigent le jeton).
pub fn session_for_play(cfg: &Config, mgr: &AuthManager) -> Result<Session> {
    if !auth::dev_offline() && !auth::configured() {
        bail!("Connexion Microsoft non configurée");
    }
    try_session(cfg, mgr)?.context("connexion Microsoft requise : connectez-vous avec Microsoft")
}

fn magic_suffix() -> Option<String> {
    serverview::tailscale_info().magic_dns_suffix
}

pub struct Launched {
    pub child: Child,
    /// Adresse du serveur du pack, ou vide (à masquer des journaux : le jeu peut s'y connecter plus tard).
    pub server: String,
    /// Le serveur du pack figure dans `servers.dat` (faux : pas de serveur, suffixe MagicDNS inconnu, ou écriture impossible).
    pub server_listed: bool,
    /// Le jeu a reçu `--quickPlayMultiplayer` : il rejoint le serveur du pack au lieu de s'arrêter au menu principal.
    pub joining: bool,
    pub log: PathBuf,
}

/// Lance le jeu : menu principal, ou serveur du pack si `cfg.join_server` (Quick Play, aucune vérification réseau). Si le pack
/// a un serveur, il est d'abord ajouté à la liste Multijoueur de l'instance ; un échec de cet ajout ne bloque pas le lancement.
pub fn launch_pack(root: &Dirs, e: &PackEntry, cfg: &Config, session: &Session) -> Result<Launched> {
    if !e.supported() {
        bail!("ce pack n'est pas pris en charge par cette version d'Opaline");
    }
    let dirs = root.for_pack(&e.id)?;
    let inst = install::load_installed(&dirs)?;
    let (server, server_listed) = e.server.as_ref().map(|s| list_entry(&dirs.instance(), &e.name, s)).unwrap_or_default();
    let memory_mb = perf::apply(&dirs.instance(), cfg.perf_mode, inst.shader_pack.as_deref(), sysmem::total_ram_mb())?
        .unwrap_or(clamp_memory(cfg.memory_mb, sysmem::total_ram_mb()));
    let env = mojang::Env::current(&[]);
    let _ = extramods::sync_for_launch(&dirs); // Mes mods : recopiés dans mods/ ; un échec ne bloque pas le lancement
    let quick_play = cfg.join_server.then(|| e.server.as_ref().and_then(quick_play_target)).flatten();
    let cmd = launch::build_command_with(&launch::LaunchInput {
        java: &inst.java,
        version: &inst.version,
        fabric: &inst.fabric,
        dirs: &dirs,
        session,
        memory_mb,
        env: &env,
    }, quick_play.as_deref())?;
    let child = launch::spawn(&cmd, &dirs)?;
    Ok(Launched { child, server, server_listed, joining: quick_play.is_some(), log: dirs.instance().join("logs").join("launcher-game.log") })
}

pub struct OfficialLaunched {
    /// Le Launcher Minecraft a pu être ouvert (sinon l'utilisateur l'ouvre lui-même).
    pub opened: bool,
    pub server: String,
    pub server_listed: bool,
    pub profile_name: String,
}

/// Raison typée renvoyée par `play` quand le Launcher officiel tourne seul : l'interface propose de le fermer.
pub const LAUNCHER_OPEN: &str = "officialLauncherOpen";
/// Une partie lancée par le Launcher officiel tourne : rien n'est fermé.
pub const OFFICIAL_GAME_RUNNING: &str = "Une partie est en cours dans le Launcher Minecraft : quitte le jeu d'abord";

/// Si le Launcher tourne : erreur typée (`LAUNCHER_OPEN`) ou refus (`OFFICIAL_GAME_RUNNING`) ; `Ok` sinon.
fn check_launcher(root: &Dirs, mc: &Path) -> Result<()> {
    if !official::launcher_running() {
        return Ok(());
    }
    match official::decide(true, official::game_running(root, mc)) {
        official::LauncherDecision::GameRunning => bail!("{OFFICIAL_GAME_RUNNING}"),
        _ => bail!("{LAUNCHER_OPEN}"),
    }
}

/// Ferme le Launcher Minecraft (jamais le jeu) après accord de l'utilisateur ; refusé si une partie tourne.
pub fn close_official_launcher(root: &Dirs) -> Result<()> {
    let mc = official::minecraft_dir().context("dossier .minecraft introuvable")?;
    if official::game_running(root, &mc) {
        bail!("{OFFICIAL_GAME_RUNNING}");
    }
    official::close_launcher()
}

/// Mode officiel : prépare la version `opaline-<pack>` et le profil du même nom dans le `.minecraft` du Launcher officiel (qui
/// connecte le compte et lance le jeu), puis l'ouvre. `interact` : refuse si le Launcher tourne (il réécrirait
/// `launcher_profiles.json`) et l'ouvre à la fin ; faux pour les tests. Rien n'est écrit tant que `launcher_profiles.json`
/// n'est pas lisible ; il est relu juste avant la fusion.
pub fn launch_official(root: &Dirs, e: &PackEntry, cfg: &Config, mc: &Path, interact: bool) -> Result<OfficialLaunched> {
    if !e.supported() {
        bail!("ce pack n'est pas pris en charge par cette version d'Opaline");
    }
    if interact {
        check_launcher(root, mc)?;
    }
    let profiles = official::profiles_path(mc)?;
    official::read_profiles(&profiles)?;
    let dirs = root.for_pack(&e.id)?;
    let inst = install::load_installed(&dirs)?;
    let (server, server_listed) = e.server.as_ref().map(|s| list_entry(&dirs.instance(), &e.name, s)).unwrap_or_default();
    let memory_mb = perf::apply(&dirs.instance(), cfg.perf_mode, inst.shader_pack.as_deref(), sysmem::total_ram_mb())?
        .unwrap_or(clamp_memory(cfg.memory_mb, sysmem::total_ram_mb()));
    let _ = extramods::sync_for_launch(&dirs);
    let vid = official::version_id(&e.id);
    official::install_version(&dirs, mc, &inst.profile_id, &vid, &inst.fabric.libraries, inst.fabric.version_name.is_some())?;
    let profile_name = format!("Opaline · {}", e.name);
    let now = official::iso_utc(friends::now_secs());
    let java_args = official::java_args(memory_mb);
    let game_dir = dirs.instance();
    let spec = official::ProfileSpec { name: &profile_name, version_id: &vid, game_dir: &game_dir, java_args: &java_args, icon: "Furnace", now: &now };
    let mut doc = official::read_profiles(&profiles)?; // relu après les copies : on ne fusionne jamais dans une copie périmée
    official::merge_profile(&mut doc, &official::profile_key(&e.id), &spec)?;
    official::write_profiles(&profiles, &doc)?;
    Ok(OfficialLaunched { opened: interact && official::open_launcher(), server, server_listed, profile_name })
}

// ---------- Identité sans authentification (mode officiel) ----------

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct IdentityView {
    pub name: String,
    pub uuid: String,
}

pub fn identity_get(root: &Dirs) -> Option<IdentityView> {
    let c = Config::load(root);
    (!c.mc_name.is_empty() && !c.mc_uuid.is_empty()).then_some(IdentityView { name: c.mc_name, uuid: c.mc_uuid })
}

/// Mémorise le pseudo Minecraft (résolu via l'API publique Mojang). Pseudo vide : l'oublie.
pub fn identity_set(root: &Dirs, name: &str, api: &str) -> Result<Option<IdentityView>> {
    let mut c = Config::load(root);
    if name.trim().is_empty() {
        c.mc_name.clear();
        c.mc_uuid.clear();
    } else {
        (c.mc_name, c.mc_uuid) = identity::lookup(&short_client()?, api, name)?;
    }
    c.save(root)?;
    Ok(identity_get(root))
}

/// Adresse Quick Play du serveur (la même que dans la liste Multijoueur) ; `None` si le suffixe MagicDNS est inconnu.
fn quick_play_target(s: &catalog::ServerInfo) -> Option<String> {
    let suffix = if config::is_short_host(&s.host) { serverview::cached_magic_suffix() } else { None };
    config::quick_play_addr(&s.host, s.port, suffix.as_deref())
}

/// Ajoute (ou met à jour) le serveur dans `servers.dat` ; renvoie son adresse (pour masquage des journaux) et si l'entrée y figure.
/// Aucun appel à Tailscale : un nom court utilise le suffixe MagicDNS déjà connu ; inconnu, l'entrée n'est pas écrite cette fois-ci
/// (nouvel essai au lancement suivant). Rien n'est loggé de l'adresse ; un échec n'empêche pas le lancement.
fn list_entry(instance: &Path, pack_name: &str, s: &catalog::ServerInfo) -> (String, bool) {
    let suffix = if config::is_short_host(&s.host) { serverview::cached_magic_suffix() } else { None };
    let addr = config::server_addr(&s.host, s.port, suffix.as_deref());
    if suffix.is_none() && config::is_short_host(&s.host) && !config::server_overridden() {
        return (addr, false);
    }
    let ip = config::list_addr(&s.host, s.port, suffix.as_deref());
    let listed = servers::add_server(instance, pack_name, &ip).unwrap_or_else(|err| {
        eprintln!("liste Multijoueur non modifiée : {}", sanitize_error_with(&format!("{err:#}"), &Dirs::new(instance), &[&ip]));
        false
    });
    (addr, listed)
}

/// Dernières lignes du journal du jeu, sans jeton, sans adresse du serveur, sans chemin personnel.
pub fn log_tail(log: &Path, token: &str, server: &str, root: &Dirs, max_lines: usize) -> Option<String> {
    let bytes = std::fs::read(log).ok()?;
    let text = String::from_utf8_lossy(&bytes[bytes.len().saturating_sub(64 * 1024)..]).into_owned();
    let lines: Vec<&str> = text.lines().collect();
    let tail = lines[lines.len().saturating_sub(max_lines)..].join("\n");
    Some(sanitize_error(&launch::redact_text(&tail, token, Some(server)), root))
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CrashView {
    pub findings: Vec<crate::crash::Finding>,
    /// Nom du rapport de crash analysé (`crash-reports/<nom>`), s'il date de cette session.
    pub crash_file: Option<String>,
    /// Texte à copier : résumé + fin du journal, sans jeton, adresse ni chemin personnel.
    pub report: String,
}

/// Fin du fichier (au plus `max` octets) : on se place avant la fin au lieu de tout lire.
fn read_tail(p: &Path, max: usize) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(p).ok()?;
    let len = f.metadata().ok()?.len();
    f.seek(SeekFrom::Start(len.saturating_sub(max as u64))).ok()?;
    let mut b = Vec::new();
    f.take(max as u64).read_to_end(&mut b).ok()?;
    Some(String::from_utf8_lossy(&b).into_owned())
}

/// Dernier `crash-reports/*.txt` modifié depuis `since`.
fn latest_crash_report(instance: &Path, since: std::time::SystemTime) -> Option<(String, String)> {
    let mut found: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(instance.join("crash-reports"))
        .ok()?
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "txt"))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .filter(|(t, _)| *t >= since)
        .collect();
    found.sort();
    let (_, p) = found.pop()?;
    Some((p.file_name()?.to_string_lossy().into_owned(), read_tail(&p, 512 * 1024)?))
}

/// Résumé lisible d'un plantage (code de sortie non nul) : `latest.log`, fin du journal déjà transmise et rapport de crash de cette session.
pub fn crash_view(root: &Dirs, pack_id: &str, code: Option<i32>, log_tail: Option<&str>, since: std::time::SystemTime, token: &str, server: &str) -> Result<CrashView> {
    let inst = root.for_pack(pack_id)?.instance();
    let latest = read_tail(&inst.join("logs").join("latest.log"), 256 * 1024).unwrap_or_default();
    let report = latest_crash_report(&inst, since);
    let findings = crate::crash::analyze(&[&latest, log_tail.unwrap_or_default()], report.as_ref().map(|r| r.1.as_str()));
    let clean = |t: &str| sanitize_error(&launch::redact_text(t, token, Some(server)), root);
    let mut text = format!("Opaline : plantage du pack {pack_id} (code {})\n", code.map_or("inconnu".into(), |c| c.to_string()));
    for f in &findings {
        text += &format!("- {}\n", f.text);
    }
    if findings.is_empty() {
        text += "Cause non identifiée.\n";
    }
    let last_lines = |t: &str| t.lines().rev().take(40).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
    let tail = log_tail.map(str::to_string).unwrap_or_else(|| last_lines(&latest));
    if !tail.is_empty() {
        text += &format!("\n--- fin du journal ---\n{tail}\n");
    }
    let findings = findings.into_iter().map(|f| crate::crash::Finding { text: clean(&f.text), ..f }).collect();
    Ok(CrashView { findings, crash_file: report.map(|r| r.0), report: clean(&text) })
}

/// Remplace tout mot qui est un nom d'hôte `*.ts.net` (avec ou sans port) par `<serveur>`.
fn mask_tailnet_hosts(s: &str) -> String {
    s.split_inclusive(|c: char| c.is_whitespace() || "()\"',;<>".contains(c))
        .map(|w| {
            let core = w.trim_end_matches(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '-'));
            let (scheme, rest) = core.rsplit_once("://").map_or(("", core), |(sch, r)| (&core[..sch.len() + 3], r));
            let host = rest.split([':', '/']).next().unwrap_or(rest).to_ascii_lowercase();
            if host.len() > 7 && host.ends_with(".ts.net") {
                let tail = &rest[host.len()..];
                format!("{scheme}<serveur>{tail}{}", &w[core.len()..])
            } else {
                w.to_string()
            }
        })
        .collect()
}

/// Retire des textes affichés les chemins personnels : dossier de données -> `<Opaline>`, dossier utilisateur -> `~`.
pub fn sanitize_error(msg: &str, root: &Dirs) -> String {
    sanitize_error_with(msg, root, &[])
}

/// Comme `sanitize_error`, et masque aussi les secrets connus (jeton, adresse résolue du serveur) ainsi que tout hôte `*.ts.net`.
pub fn sanitize_error_with(msg: &str, root: &Dirs, secrets: &[&str]) -> String {
    let mut s = msg.to_string();
    for sec in secrets.iter().filter(|x| x.len() >= 4) {
        s = s.replace(sec, "<masqué>");
        // adresse `hôte:port` : l'hôte seul aussi (s'il est assez distinctif)
        if let Some((h, _)) = sec.rsplit_once(':').filter(|(h, _)| h.contains('.')) {
            s = s.replace(h, "<masqué>");
        }
    }
    s = mask_tailnet_hosts(&s);
    let mut subst = |p: &Path, to: &str| {
        let d = p.display().to_string();
        if d.len() < 4 {
            return;
        }
        for v in [d.clone(), d.replace('\\', "/"), d.replace('/', "\\")] {
            s = s.replace(&v, to);
        }
    };
    subst(&root.root, "<Opaline>");
    if let Some(h) = directories::BaseDirs::new() {
        subst(h.home_dir(), "~");
    }
    s
}

// ---------- Serveur, stats, amis ----------

/// `None` : le pack n'a pas de serveur.
pub fn server_status(root: &Dirs, id: &str) -> Result<Option<serverview::ServerView>> {
    Ok(serverview::check(&entry(root, id)?).map(|c| c.0))
}

fn stats_get(root: &Dirs, id: &str, path: &str) -> Result<Option<serde_json::Value>> {
    let e = entry(root, id)?;
    let Some(srv) = e.server.as_ref().filter(|s| s.stats_port != 0 && e.widgets.iter().any(|w| w == "classroomRun")) else {
        return Ok(None);
    };
    let suffix = if config::is_short_host(&srv.host) { magic_suffix() } else { None };
    let host = config::stats_host(&srv.host, suffix.as_deref());
    stats::fetch_value(&short_client()?, &config::stats_url(&host, srv.stats_port, path))
}

pub fn run_stats(root: &Dirs, id: &str) -> Result<Option<serde_json::Value>> {
    stats_get(root, id, "stats")
}

pub fn run_history(root: &Dirs, id: &str, run: Option<u32>) -> Result<Option<serde_json::Value>> {
    stats_get(root, id, &run.map_or("history".to_string(), |n| format!("history/{n}")))
}

/// Dernier état connu (en ligne ou non) de chaque serveur de pack, pour détecter la transition hors ligne -> en ligne.
pub type ServerWatch = std::collections::HashMap<String, bool>;

/// Intègre un relevé `(id, nom du pack, en ligne)` et renvoie les noms des packs dont le serveur vient de passer de hors ligne à
/// en ligne. Premier relevé d'un serveur (démarrage d'Opaline) : jamais de notification.
pub fn watch_step(watch: &mut ServerWatch, observed: &[(String, String, bool)]) -> Vec<String> {
    observed.iter().filter(|(id, _, now)| watch.insert(id.clone(), *now) == Some(false) && *now).map(|(_, name, _)| name.clone()).collect()
}

/// Relève (ping, Tailscale) les serveurs des packs installés. Une erreur de catalogue renvoie un relevé vide.
pub fn observe_servers(root: &Dirs) -> Vec<(String, String, bool)> {
    let Ok(entries) = packs(root) else { return Vec::new() };
    entries
        .iter()
        .filter(|e| e.supported() && root.for_pack(&e.id).is_ok_and(|d| d.install_marker().is_file()))
        .filter_map(|e| Some((e.id.clone(), e.name.clone(), serverview::check(e)?.0.online)))
        .collect()
}

/// Relève les joueurs en ligne sur tous les serveurs des packs puis renvoie la liste mémorisée.
pub fn friends_list(root: &Dirs) -> Result<Vec<friends::Friend>> {
    let mut seen = Vec::new();
    for e in packs(root)? {
        // Seuls les serveurs dont le MOTD correspond au pack comptent (pas d'inscription depuis un serveur inattendu).
        let Some((view, st)) = serverview::check(&e) else { continue };
        if view.motd_ok
            && let Some(st) = st
        {
            seen.extend(st.sample.into_iter().zip(st.sample_ids).filter(|(_, u)| !u.is_empty()));
        }
    }
    Ok(friends::record(root, &seen, friends::now_secs()))
}

pub fn app_update(current: &str) -> Option<update::Update> {
    update::check(&short_client().ok()?, &config::launcher_json_url(), current)
}

/// Télécharge l'installeur de la version publiée (relue ici : l'interface ne fournit ni URL ni hash).
/// `progress(octets reçus, total si connu)`. Dossier temporaire : `<racine>/update`.
pub fn app_update_download(root: &Dirs, current: &str, progress: impl FnMut(u64, Option<u64>)) -> Result<PathBuf> {
    let info = update::fetch_info(&short_client()?, &config::launcher_json_url())?;
    let u = update::pick(&info, current).context("Aucune mise à jour disponible")?;
    anyhow::ensure!(update::can_auto(&u), "Mise à jour automatique indisponible sur cette installation : utilisez le lien de téléchargement");
    update::download(&crate::download::http_client()?, &u, &root.root.join("update"), progress)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_policy() {
        assert!(frame_allowed(None, None));
        assert!(frame_allowed(None, Some("default-src 'self'; script-src 'self'")));
        assert!(!frame_allowed(Some("DENY"), None));
        assert!(!frame_allowed(Some("SAMEORIGIN"), None));
        assert!(!frame_allowed(None, Some("default-src 'self'; frame-ancestors 'none'")));
        assert!(!frame_allowed(None, Some("frame-ancestors 'self'")));
        assert!(frame_allowed(None, Some("frame-ancestors *")));
        assert!(frame_allowed(None, Some("frame-ancestors https://opaline.example")));
    }

    #[test]
    fn uninstall_backs_up_saves_then_removes_only_this_instance() {
        let d = Dirs::new(std::env::temp_dir().join(format!("opaline-uninst-{}", std::process::id())));
        let _ = std::fs::remove_dir_all(&d.root);
        let (a, b) = (d.for_pack("a").unwrap(), d.for_pack("b").unwrap());
        for p in [&a, &b] {
            std::fs::create_dir_all(p.instance().join("saves/Monde")).unwrap();
            std::fs::write(p.instance().join("saves/Monde/level.dat"), "x").unwrap();
            std::fs::write(p.install_marker(), "ok").unwrap();
        }
        std::fs::create_dir_all(d.libraries()).unwrap();
        std::fs::write(d.libraries().join("lib.jar"), "j").unwrap();
        // monde ouvert : refus, rien de supprimé
        let lock = std::fs::File::create(a.instance().join("saves/Monde/session.lock")).unwrap();
        lock.lock().unwrap();
        assert!(uninstall_pack(&d, "a").unwrap_err().to_string().contains("monde est ouvert"));
        assert!(a.install_marker().is_file());
        drop(lock);
        uninstall_pack(&d, "a").unwrap();
        assert!(!a.instance().exists() && !a.install_marker().exists());
        assert_eq!(backup::list(&a).len(), 1, "mondes sauvegardés avant suppression");
        assert!(b.install_marker().is_file() && d.libraries().join("lib.jar").is_file());
        assert!(uninstall_pack(&d, "a").unwrap_err().to_string().contains("pas installé"));
        assert!(uninstall_pack(&d, "../b").is_err());
        std::fs::remove_dir_all(&d.root).unwrap();
    }

    #[test]
    fn settings_patch_changes_only_given_fields_and_clears_auto_tuned() {
        let d = Dirs::new(std::env::temp_dir().join(format!("opaline-patch-{}", std::process::id())));
        let _ = std::fs::remove_dir_all(&d.root);
        let c = Config { auto_tuned: true, ..Config::default() };
        c.save(&d).unwrap();
        let v = settings_set(&d, serde_json::from_str(r#"{"discordPresence":false,"trayOnGame":false}"#).unwrap()).unwrap();
        assert!(!v.discord_presence && !v.tray_on_game && v.auto_tuned && v.join_server);
        let v = settings_set(&d, SettingsPatch { perf_mode: Some(perf::PerfMode::Low), ..Default::default() }).unwrap();
        assert!(!v.auto_tuned && v.perf_mode == perf::PerfMode::Low && !v.discord_presence);
        std::fs::remove_dir_all(&d.root).unwrap();
    }

    #[test]
    fn list_entry_needs_known_suffix_for_short_hosts_only() {
        if config::server_overridden() || serverview::cached_magic_suffix().is_some() {
            return;
        }
        let d = std::env::temp_dir().join(format!("opaline-list-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let info = |host: &str| catalog::ServerInfo { host: host.into(), port: 25565, motd: "m".into(), stats_port: 0, map: None };
        let (_, listed) = list_entry(&d, "Pack", &info("home-server"));
        assert!(!listed && !d.join("servers.dat").exists(), "suffixe inconnu : rien d'écrit");
        let (addr, listed) = list_entry(&d, "Pack", &info("mon.minestrator.com"));
        assert!(listed && d.join("servers.dat").is_file());
        assert_eq!(addr, "mon.minestrator.com:25565");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn crash_view_reads_fresh_report_and_hides_secrets() {
        let root = Dirs::new(std::env::temp_dir().join(format!("opaline-crash-{}", std::process::id())));
        let _ = std::fs::remove_dir_all(&root.root);
        let inst = root.for_pack("p").unwrap().instance();
        std::fs::create_dir_all(inst.join("logs")).unwrap();
        std::fs::create_dir_all(inst.join("crash-reports")).unwrap();
        std::fs::write(inst.join("logs/latest.log"), "	 - Mod 'Legendary Monuments' (legendarymonuments) 8.1 requires any version of lithostitched, which is missing!
joined secret.host token SECRETTOKEN").unwrap();
        let before = std::time::SystemTime::now() - Duration::from_secs(3600);
        std::fs::write(inst.join("crash-reports/crash-1.txt"), "Description: x
	Suspected Mods: Sodium (sodium)").unwrap();
        let v = crash_view(&root, "p", Some(1), Some("fin SECRETTOKEN"), before, "SECRETTOKEN", "secret.host:25565").unwrap();
        assert_eq!(v.crash_file.as_deref(), Some("crash-1.txt"));
        assert!(v.findings.iter().any(|f| f.kind == "missingMod") && v.findings.iter().any(|f| f.kind == "suspect"));
        assert!(!v.report.contains("SECRETTOKEN") && !v.report.contains("secret.host") && v.report.contains("lithostitched"), "{}", v.report);
        // rapport antérieur à la session : ignoré
        let after = std::time::SystemTime::now() + Duration::from_secs(60);
        assert_eq!(crash_view(&root, "p", None, None, after, "t", "").unwrap().crash_file, None);
        std::fs::remove_dir_all(&root.root).unwrap();
    }

    #[test]
    fn notifies_only_on_offline_to_online_transition() {
        let obs = |online: bool| vec![("a".to_string(), "Pack A".to_string(), online)];
        let mut w = ServerWatch::new();
        assert!(watch_step(&mut w, &obs(true)).is_empty(), "premier relevé en ligne : pas de notification au démarrage");
        assert!(watch_step(&mut w, &obs(true)).is_empty());
        assert!(watch_step(&mut w, &obs(false)).is_empty());
        assert!(watch_step(&mut w, &obs(false)).is_empty());
        assert_eq!(watch_step(&mut w, &obs(true)), ["Pack A"]);
        assert!(watch_step(&mut w, &obs(true)).is_empty(), "une seule notification par transition");
        let mut w = ServerWatch::new();
        watch_step(&mut w, &obs(false));
        assert_eq!(watch_step(&mut w, &obs(true)), ["Pack A"], "hors ligne au démarrage puis en ligne : notifié");
    }

    #[test]
    fn read_tail_is_bounded() {
        let d = std::env::temp_dir().join(format!("opaline-tail-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("x.log");
        std::fs::write(&f, format!("{}FIN", "a".repeat(5000))).unwrap();
        let t = read_tail(&f, 100).unwrap();
        assert_eq!(t.len(), 100);
        assert!(t.ends_with("aaFIN"));
        assert_eq!(read_tail(&f, 1 << 20).unwrap().len(), 5003);
        assert!(read_tail(&d.join("absent"), 10).is_none());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn mask_tailnet_hosts_with_scheme() {
        assert_eq!(mask_tailnet_hosts("GET http://home-server.tail1.ts.net:25580/stats a échoué"), "GET http://<serveur>:25580/stats a échoué");
        assert_eq!(mask_tailnet_hosts("vers home-server.tail1.ts.net:25565"), "vers <serveur>:25565");
    }

    #[test]
    fn sanitize_masks_secrets_and_tailnet_hosts() {
        let root = Dirs::new(std::env::temp_dir().join("OpalineTest2"));
        let m = sanitize_error_with("échec avec SECRETTOKEN sur home-server.tail1234.ts.net:25565 (réessayez)", &root, &["SECRETTOKEN"]);
        assert!(!m.contains("SECRETTOKEN") && !m.contains("tail1234") && m.contains("<serveur>:25565") && m.contains("(réessayez)"), "{m}");
        let m = sanitize_error_with("connexion à 10.1.2.3:25565 refusée", &root, &["10.1.2.3:25565"]);
        assert!(!m.contains("10.1.2.3"), "{m}");
        assert_eq!(sanitize_error("HTTP 503 pour le pack", &root), "HTTP 503 pour le pack");
    }

    #[test]
    fn memory_is_clamped() {
        assert_eq!(clamp_memory(100, Some(16384)), 1024);
        assert_eq!(clamp_memory(20000, Some(16384)), 12288);
        assert_eq!(clamp_memory(4096, Some(2048)), 1536);
        assert_eq!(clamp_memory(500, Some(512)), 1024);
        assert_eq!(clamp_memory(99999, None), 99999);
    }

    #[test]
    fn errors_and_logs_hide_personal_paths_and_secrets() {
        let root = Dirs::new(std::env::temp_dir().join("OpalineTest"));
        let p = root.root.join("instances").join("x");
        let m = sanitize_error(&format!("ouverture de {} impossible", p.display()), &root);
        assert!(m.contains("<Opaline>") && !m.contains("OpalineTest"));
        let d = std::env::temp_dir().join(format!("opaline-log-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("g.log");
        std::fs::write(&f, "a\nconnexion à secret.tail.ts.net:25565 avec SECRETTOKEN\nfin").unwrap();
        let t = log_tail(&f, "SECRETTOKEN", "secret.tail.ts.net:25565", &root, 2).unwrap();
        assert!(!t.contains("SECRETTOKEN") && !t.contains("secret.tail") && t.ends_with("fin") && !t.starts_with('a'));
        std::fs::remove_dir_all(&d).unwrap();
    }
}
