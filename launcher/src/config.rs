//! Configuration persistée (aucun secret) et disposition des dossiers du launcher.
use crate::perf::PerfMode;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Dépôt public unique (releases : launcher.json, installeurs ; packs.json et pack-<id>.zip sur la release « packs »).
pub const REPO: &str = "Cikuree/opaline";

fn release_asset(name: &str) -> String {
    format!("https://github.com/{REPO}/releases/latest/download/{name}")
}

pub fn packs_json_url() -> String {
    format!("https://github.com/{REPO}/releases/download/packs/packs.json")
}

pub fn launcher_json_url() -> String {
    release_asset("launcher.json")
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Config {
    /// Pseudo du mode hors ligne (développement seulement).
    pub nickname: String,
    pub memory_mb: u32,
    pub perf_mode: PerfMode,
    /// « Ne pas mémoriser la connexion sur ce PC » : le refresh token reste en mémoire, jamais dans le trousseau.
    pub no_remember: bool,
    /// Pseudo Minecraft saisi en mode officiel (tête et skin affichés sans authentification) et son UUID résolu.
    pub mc_name: String,
    pub mc_uuid: String,
    /// Avec une session Opaline : lancer le jeu directement ou via le Launcher Minecraft officiel.
    pub launch_via: LaunchVia,
    /// Lancement direct : rejoindre le serveur du pack (Quick Play) au lieu de s'arrêter au menu principal.
    pub join_server: bool,
    /// Notification quand le serveur d'un pack passe de hors ligne à en ligne.
    pub notify_server: bool,
    /// Statut Discord pendant une partie (sans effet si l'application a été compilée sans identifiant Discord).
    pub discord_presence: bool,
    /// Opaline se cache dans la zone de notification pendant une partie lancée en direct.
    pub tray_on_game: bool,
    /// Mode et mémoire choisis automatiquement au premier lancement ; remis à faux dès que l'utilisateur les change.
    pub auto_tuned: bool,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum LaunchVia {
    #[default]
    Opaline,
    Official,
}

impl Default for Config {
    fn default() -> Self {
        Config { nickname: "Player".into(), memory_mb: 4096, perf_mode: PerfMode::Balanced, no_remember: false, mc_name: String::new(), mc_uuid: String::new(), launch_via: LaunchVia::Opaline, join_server: true, notify_server: true, discord_presence: true, tray_on_game: true, auto_tuned: false }
    }
}

impl Config {
    pub fn load(dirs: &Dirs) -> Config {
        std::fs::read_to_string(dirs.config_file()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    pub fn save(&self, dirs: &Dirs) -> anyhow::Result<()> {
        std::fs::create_dir_all(&dirs.root)?;
        std::fs::write(dirs.config_file(), serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}

/// Variable d'environnement de développement : lue seulement dans un build debug (jamais en release).
pub fn dev_env(name: &str) -> Option<String> {
    if !cfg!(debug_assertions) {
        return None;
    }
    std::env::var(name).ok().filter(|s| !s.trim().is_empty())
}

/// Surcharge de développement : `CLASSROOM_SERVER` (`hôte[:port]`) remplace l'adresse de tout serveur de pack.
fn server_override() -> Option<String> {
    dev_env("CLASSROOM_SERVER")
}

/// Vrai si l'adresse est surchargée (dev) : Tailscale n'est alors pas exigé.
pub fn server_overridden() -> bool {
    server_override().is_some()
}

/// Nom court (sans point ni `:`) = nom MagicDNS Tailscale ; sinon nom complet ou IP, utilisé tel quel.
pub fn is_short_host(host: &str) -> bool {
    !host.contains('.') && !host.contains(':')
}

/// Hôte (sans port) à contacter pour un serveur de pack : surcharge de dev, sinon nom court + suffixe MagicDNS, sinon tel quel.
pub fn choose_host(base: &str, suffix: Option<&str>, override_addr: Option<&str>) -> String {
    match override_addr.map(str::trim).filter(|s| !s.is_empty()) {
        Some(o) => crate::ping::split_host(o).0,
        None if is_short_host(base) => crate::tailscale::server_host(base, suffix),
        None => base.to_string(),
    }
}

fn join_port(host: &str, port: u16) -> String {
    if host.contains(':') { format!("[{host}]:{port}") } else { format!("{host}:{port}") }
}

/// Adresse `hôte:port` du serveur de jeu pour cette exécution (ping). Jamais affichée ni loggée.
pub fn server_addr(base: &str, port: u16, suffix: Option<&str>) -> String {
    match server_override() {
        Some(o) => o,
        None => join_port(&choose_host(base, suffix, None), port),
    }
}

/// Adresse écrite dans `servers.dat` : comme `server_addr`, sans le port s'il vaut 25565.
pub fn list_addr(base: &str, port: u16, suffix: Option<&str>) -> String {
    match server_override() {
        Some(o) => o,
        None if port == crate::ping::DEFAULT_PORT => choose_host(base, suffix, None),
        None => join_port(&choose_host(base, suffix, None), port),
    }
}

/// Adresse Quick Play : celle de `servers.dat` (`list_addr`). `None` : nom court dont le suffixe MagicDNS est inconnu (pas de connexion automatique).
pub fn quick_play_addr(base: &str, port: u16, suffix: Option<&str>) -> Option<String> {
    if is_short_host(base) && suffix.is_none() && !server_overridden() {
        return None;
    }
    Some(list_addr(base, port, suffix))
}

/// Hôte (sans port) pour l'endpoint HTTP de stats.
pub fn stats_host(base: &str, suffix: Option<&str>) -> String {
    choose_host(base, suffix, server_override().as_deref())
}

/// `http://hôte:port/chemin` (le port éventuel de `server_host` est ignoré).
pub fn stats_url(server_host: &str, stats_port: u16, path: &str) -> String {
    let host = server_host.trim();
    let host = match host.rsplit_once(':') {
        Some((h, p)) if !h.is_empty() && p.chars().all(|c| c.is_ascii_digit()) && !h.contains(':') => h,
        _ => host,
    };
    format!("http://{host}:{stats_port}/{}", path.trim_start_matches('/'))
}

/// Tout ce que le launcher écrit vit sous `root` ; jamais dans `.minecraft`.
/// Disposition : `shared/` (Java, bibliothèques, assets, versions : communs à tous les packs) et
/// `instances/<pack>/` (dossier de jeu isolé, `installed.ok`).
#[derive(Clone, Debug)]
pub struct Dirs {
    pub root: PathBuf,
    /// Identifiant du pack (nom de dossier simple).
    pub pack: String,
}

impl Dirs {
    /// `%APPDATA%/Opaline` (Windows), `~/.local/share/Opaline` (Linux), etc.
    /// Surcharge possible par la variable d'environnement `OPALINE_HOME`.
    pub fn default_root() -> PathBuf {
        if let Some(p) = dev_env("OPALINE_HOME") {
            return PathBuf::from(p);
        }
        directories::BaseDirs::new().map(|d| d.data_dir().join("Opaline")).unwrap_or_else(|| PathBuf::from("Opaline"))
    }
    pub fn new(root: impl Into<PathBuf>) -> Dirs {
        Dirs { root: root.into(), pack: "default".into() }
    }
    /// Même racine, autre pack. L'identifiant doit être un simple nom (il devient un dossier).
    pub fn for_pack(&self, id: &str) -> anyhow::Result<Dirs> {
        Ok(Dirs { root: self.root.clone(), pack: crate::safe::safe_file_name(id)?.to_string() })
    }
    pub fn config_file(&self) -> PathBuf {
        self.root.join("config.json")
    }
    pub fn shared(&self) -> PathBuf {
        self.root.join("shared")
    }
    /// Marqueur écrit en toute fin d'installation réussie (dans l'instance).
    pub fn install_marker(&self) -> PathBuf {
        self.instance().join("installed.ok")
    }
    /// Manifeste (mods, versions, hashes) du pack installé : sert au changelog et à la liste des mods.
    pub fn pack_snapshot(&self) -> PathBuf {
        self.instance().join("opaline-pack.json")
    }
    pub fn libraries(&self) -> PathBuf {
        self.shared().join("libraries")
    }
    pub fn assets(&self) -> PathBuf {
        self.shared().join("assets")
    }
    pub fn versions(&self) -> PathBuf {
        self.shared().join("versions")
    }
    pub fn java(&self, major: u32) -> PathBuf {
        self.shared().join("java").join(major.to_string())
    }
    pub fn downloads(&self) -> PathBuf {
        self.shared().join("downloads")
    }
    /// Dossier de travail du pack distant de ce pack.
    pub fn pack_work(&self) -> PathBuf {
        self.downloads().join(format!("pack-{}", self.pack))
    }
    /// Répertoire de jeu isolé (mods, options, mondes, logs).
    pub fn instance(&self) -> PathBuf {
        self.root.join("instances").join(&self.pack)
    }
    pub fn natives(&self) -> PathBuf {
        self.instance().join("natives")
    }
    /// PNG extraits du client.jar installé (jamais redistribués).
    pub fn textures(&self) -> PathBuf {
        self.root.join("textures")
    }
    /// Garde-robe de skins.
    pub fn skins(&self) -> PathBuf {
        self.root.join("skins")
    }
    /// Sauvegardes des mondes : `backups/<pack>/<horodatage>/`.
    pub fn backups(&self) -> PathBuf {
        self.root.join("backups").join(&self.pack)
    }
    /// Caches (catalogue, têtes, amis).
    pub fn cache(&self) -> PathBuf {
        self.root.join("cache")
    }
    pub fn version_json(&self, id: &str) -> PathBuf {
        self.versions().join(id).join(format!("{id}.json"))
    }
    pub fn client_jar(&self, id: &str) -> PathBuf {
        self.versions().join(id).join(format!("{id}.jar"))
    }
}

/// Identifiant du profil Fabric ou Quilt (`<kind>-loader-<version>-<mc>`, comme dans leurs JSON).
pub fn fabric_profile_id(kind: &str, loader: &str, mc: &str) -> String {
    format!("{kind}-loader-{loader}-{mc}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_and_launcher_urls() {
        assert_eq!(packs_json_url(), "https://github.com/Cikuree/opaline/releases/download/packs/packs.json");
        assert_eq!(launcher_json_url(), "https://github.com/Cikuree/opaline/releases/latest/download/launcher.json");
    }

    #[test]
    fn stats_url_strips_game_port() {
        assert_eq!(stats_url("play.example.fr:25565", 25580, "stats"), "http://play.example.fr:25580/stats");
        assert_eq!(stats_url("play.example.fr", 25580, "/history/3"), "http://play.example.fr:25580/history/3");
        assert_eq!(stats_url(" 10.0.0.2:1 ", 9, "stats"), "http://10.0.0.2:9/stats");
    }

    #[test]
    fn host_choice() {
        assert_eq!(choose_host("home-server", Some("x.ts.net"), None), "home-server.x.ts.net");
        assert_eq!(choose_host("home-server", None, None), "home-server");
        assert_eq!(choose_host("home-server", Some("x.ts.net"), Some("127.0.0.1:9999")), "127.0.0.1");
        assert_eq!(choose_host("home-server", Some("x.ts.net"), Some("  ")), "home-server.x.ts.net");
        assert_eq!(choose_host("mon.minestrator.com", Some("x.ts.net"), None), "mon.minestrator.com");
        assert_eq!(choose_host("10.0.0.2", Some("x.ts.net"), None), "10.0.0.2");
        assert!(is_short_host("home-server") && !is_short_host("a.b") && !is_short_host("::1"));
    }

    #[test]
    fn list_addr_omits_default_port() {
        if server_override().is_some() {
            return;
        }
        assert_eq!(list_addr("home-server", 25565, Some("x.ts.net")), "home-server.x.ts.net");
        assert_eq!(list_addr("home-server", 25565, None), "home-server");
        assert_eq!(list_addr("mon.minestrator.com", 25565, Some("x.ts.net")), "mon.minestrator.com");
        assert_eq!(list_addr("mon.minestrator.com", 25999, None), "mon.minestrator.com:25999");
        assert_eq!(list_addr("::1", 25999, None), "[::1]:25999");
        assert_eq!(server_addr("mon.minestrator.com", 25565, None), "mon.minestrator.com:25565");
    }

    #[test]
    fn quick_play_needs_known_suffix_for_short_hosts() {
        if server_override().is_some() {
            return;
        }
        assert_eq!(quick_play_addr("home-server", 25565, Some("x.ts.net")).as_deref(), Some("home-server.x.ts.net"));
        assert_eq!(quick_play_addr("home-server", 25565, None), None);
        assert_eq!(quick_play_addr("mon.minestrator.com", 25565, None).as_deref(), Some("mon.minestrator.com"));
        assert_eq!(quick_play_addr("mon.minestrator.com", 25999, None).as_deref(), Some("mon.minestrator.com:25999"));
    }

    #[test]
    fn extras_default_on_and_missing_keys_ok() {
        let c: Config = serde_json::from_str(r#"{"nickname":"A"}"#).unwrap();
        assert!(c.join_server && c.notify_server && c.discord_presence && c.tray_on_game && !c.auto_tuned);
    }

    #[test]
    fn config_roundtrip_and_defaults() {
        let c: Config = serde_json::from_str(r#"{"nickname":"A","pack_dir":"ancien"}"#).unwrap();
        assert_eq!((c.nickname.as_str(), c.memory_mb), ("A", 4096));
    }

    #[test]
    fn layout_is_under_root_and_shared_between_packs() {
        let a = Dirs::new("/r").for_pack("classroom-run").unwrap();
        let b = a.for_pack("autre").unwrap();
        for p in [a.libraries(), a.assets(), a.java(25), a.instance(), a.natives(), a.client_jar("26.3"), a.textures(), a.skins()] {
            assert!(p.starts_with(&a.root));
        }
        assert_eq!(a.libraries(), b.libraries());
        assert_eq!(a.java(25), b.java(25));
        assert_ne!(a.instance(), b.instance());
        assert!(a.install_marker().starts_with(a.instance()));
        assert!(a.for_pack("../x").is_err() && a.for_pack("a/b").is_err());
    }
}
