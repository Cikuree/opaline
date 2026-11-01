//! Opérations de haut niveau (une par commande de l'interface). Tout est synchrone : l'appelant (backend Tauri)
//! les exécute sur un fil dédié. Les erreurs sont des phrases lisibles, à passer par `sanitize_error` avant affichage.
use crate::auth::{self, AuthManager, Session};
use crate::catalog::{self, PackEntry, PackView};
use crate::changelog::{self, Changelog, ModView};
use crate::config::{self, Config, Dirs};
use crate::download::{self, Progress};
use crate::{friends, identity, install, launch, mojang, official, packwiz, perf, servers, serverview, stats, sysmem, textures, update};
use anyhow::{Context, Result, bail};
use serde::Serialize;
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
    pub system_ram_mb: Option<u64>,
}

pub fn settings_get(root: &Dirs) -> SettingsView {
    let c = Config::load(root);
    SettingsView { perf_mode: c.perf_mode, memory_mb: c.memory_mb, no_remember: c.no_remember, launch_via: c.launch_via, join_server: c.join_server, notify_server: c.notify_server, system_ram_mb: sysmem::total_ram_mb() }
}

/// Mémoire bornée : au moins 1 Go, au plus 75 % de la RAM physique (si connue).
pub fn clamp_memory(mb: u32, ram_mb: Option<u64>) -> u32 {
    let max = ram_mb.map_or(u32::MAX, |r| ((r * 3 / 4) as u32).max(1024));
    mb.clamp(1024, max)
}

pub fn settings_set(root: &Dirs, mode: Option<perf::PerfMode>, memory_mb: Option<u32>, no_remember: Option<bool>, launch_via: Option<crate::config::LaunchVia>, join_server: Option<bool>, notify_server: Option<bool>) -> Result<SettingsView> {
    let mut c = Config::load(root);
    if let Some(m) = mode {
        c.perf_mode = m;
    }
    if let Some(m) = memory_mb {
        c.memory_mb = clamp_memory(m, sysmem::total_ram_mb());
    }
    if let Some(n) = no_remember {
        c.no_remember = n;
    }
    if let Some(v) = launch_via {
        c.launch_via = v;
    }
    c.join_server = join_server.unwrap_or(c.join_server);
    c.notify_server = notify_server.unwrap_or(c.notify_server);
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

/// Message quand le Launcher officiel tourne déjà.
pub const LAUNCHER_RUNNING: &str = "Ferme le Launcher Minecraft puis relance JOUER (il écraserait le profil Opaline)";

/// Mode officiel : prépare la version `opaline-<pack>` et le profil du même nom dans le `.minecraft` du Launcher officiel (qui
/// connecte le compte et lance le jeu), puis l'ouvre. `interact` : refuse si le Launcher tourne (il réécrirait
/// `launcher_profiles.json`) et l'ouvre à la fin ; faux pour les tests. Rien n'est écrit tant que `launcher_profiles.json`
/// n'est pas lisible ; il est relu juste avant la fusion.
pub fn launch_official(root: &Dirs, e: &PackEntry, cfg: &Config, mc: &Path, interact: bool) -> Result<OfficialLaunched> {
    if !e.supported() {
        bail!("ce pack n'est pas pris en charge par cette version d'Opaline");
    }
    if interact && official::launcher_running() {
        bail!("{LAUNCHER_RUNNING}");
    }
    let profiles = official::profiles_path(mc)?;
    official::read_profiles(&profiles)?;
    let dirs = root.for_pack(&e.id)?;
    let inst = install::load_installed(&dirs)?;
    let (server, server_listed) = e.server.as_ref().map(|s| list_entry(&dirs.instance(), &e.name, s)).unwrap_or_default();
    let memory_mb = perf::apply(&dirs.instance(), cfg.perf_mode, inst.shader_pack.as_deref(), sysmem::total_ram_mb())?
        .unwrap_or(clamp_memory(cfg.memory_mb, sysmem::total_ram_mb()));
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
    fn list_entry_needs_known_suffix_for_short_hosts_only() {
        if config::server_overridden() || serverview::cached_magic_suffix().is_some() {
            return;
        }
        let d = std::env::temp_dir().join(format!("opaline-list-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let info = |host: &str| catalog::ServerInfo { host: host.into(), port: 25565, motd: "m".into(), stats_port: 0 };
        let (_, listed) = list_entry(&d, "Pack", &info("home-server"));
        assert!(!listed && !d.join("servers.dat").exists(), "suffixe inconnu : rien d'écrit");
        let (addr, listed) = list_entry(&d, "Pack", &info("mon.minestrator.com"));
        assert!(listed && d.join("servers.dat").is_file());
        assert_eq!(addr, "mon.minestrator.com:25565");
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
