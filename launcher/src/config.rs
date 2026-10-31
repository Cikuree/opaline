//! Configuration persistée (aucun secret) et disposition des dossiers du launcher.
use serde::{Deserialize, Serialize};
use crate::perf::PerfMode;
use std::path::PathBuf;

/// Dépôt public unique (releases : pack.zip, launcher.json, binaires).
pub const REPO: &str = "Cikuree/classroom-run";

pub fn pack_zip_url() -> String {
    format!("https://github.com/{REPO}/releases/latest/download/pack.zip")
}

pub fn launcher_json_url() -> String {
    format!("https://github.com/{REPO}/releases/latest/download/launcher.json")
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Config {
    pub nickname: String,
    /// Option avancée (dev) : dossier local du manifeste packwiz (contient pack.toml). Vide = pack.zip de la dernière release.
    pub pack_dir: String,
    /// Jar classroomrun (fichier) ou dossier le contenant ; vide = `<pack_dir>/../build/libs`.
    pub mod_jar: String,
    pub memory_mb: u32,
    pub perf_mode: PerfMode,
    /// « Ne pas mémoriser la connexion sur ce PC » : le refresh token reste en mémoire, jamais dans le trousseau.
    pub no_remember: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            nickname: "Player".into(),
            pack_dir: String::new(),
            mod_jar: String::new(),
            memory_mb: 4096,
            perf_mode: PerfMode::Balanced,
            no_remember: false,
        }
    }
}

impl Config {
    pub fn load(dirs: &Dirs) -> Config {
        let cfg: Config =
            std::fs::read_to_string(dirs.config_file()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        cfg
    }

    pub fn save(&self, dirs: &Dirs) -> anyhow::Result<()> {
        std::fs::create_dir_all(&dirs.root)?;
        std::fs::write(dirs.config_file(), serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn stats_url(&self) -> String {
        stats_url(&server_addr(), STATS_PORT)
    }
}

/// Éléments propres à un pack (MC et loader viennent du manifeste ; instance = `Dirs::instance`).
/// Classroom Run est le profil par défaut ; le multi-pack passera par un catalogue de profils.
#[derive(Clone, Debug)]
pub struct PackProfile {
    pub pack_zip_url: String,
    /// Nom d'hôte de base du serveur (le suffixe MagicDNS Tailscale est ajouté à l'exécution).
    pub server_name: String,
    pub server_port: u16,
}

impl Default for PackProfile {
    fn default() -> Self {
        PackProfile { pack_zip_url: pack_zip_url(), server_name: "classroom".into(), server_port: 25565 }
    }
}

impl PackProfile {
    /// Adresse du serveur, jamais affichée ni saisie : `CLASSROOM_SERVER` (dev), sinon `host:port`
    /// où `host` est fourni par l'appelant (nom de base + suffixe MagicDNS calculé à l'exécution).
    pub fn server_addr_for(&self, host: &str) -> String {
        server_override().unwrap_or_else(|| format!("{host}:{}", self.server_port))
    }

    /// Adresse sans suffixe MagicDNS (repli).
    pub fn server_addr(&self) -> String {
        self.server_addr_for(&self.server_name)
    }
}

pub const STATS_PORT: u16 = 25580;

fn server_override() -> Option<String> {
    std::env::var("CLASSROOM_SERVER").ok().filter(|s| !s.trim().is_empty())
}

/// `CLASSROOM_SERVER` (dev) ou la constante par défaut.
pub fn server_addr() -> String {
    PackProfile::default().server_addr()
}

/// Vrai si l'adresse est surchargée (dev) : Tailscale n'est alors pas exigé.
pub fn server_overridden() -> bool {
    server_override().is_some()
}

pub fn stats_url(server_host: &str, stats_port: u16) -> String {
    let host = server_host.trim();
    let host = match host.rsplit_once(':') {
        Some((h, p)) if !h.is_empty() && p.chars().all(|c| c.is_ascii_digit()) && !h.contains(':') => h,
        _ => host,
    };
    format!("http://{host}:{stats_port}/stats")
}

/// Tout ce que le launcher écrit vit sous `root` ; jamais dans `.minecraft`.
#[derive(Clone, Debug)]
pub struct Dirs {
    pub root: PathBuf,
}

impl Dirs {
    /// `%APPDATA%/ClassroomRunLauncher` (Windows), `~/.local/share/ClassroomRunLauncher` (Linux), etc.
    /// Surcharge possible par la variable d'environnement `CLASSROOM_LAUNCHER_HOME`.
    pub fn default_root() -> PathBuf {
        if let Some(p) = std::env::var_os("CLASSROOM_LAUNCHER_HOME") {
            return PathBuf::from(p);
        }
        directories::BaseDirs::new()
            .map(|d| d.data_dir().join("ClassroomRunLauncher"))
            .unwrap_or_else(|| PathBuf::from("ClassroomRunLauncher"))
    }
    pub fn new(root: impl Into<PathBuf>) -> Dirs {
        Dirs { root: root.into() }
    }
    pub fn config_file(&self) -> PathBuf {
        self.root.join("config.json")
    }
    /// Marqueur écrit en toute fin d'installation réussie.
    pub fn install_marker(&self) -> PathBuf {
        self.root.join("installed.ok")
    }
    pub fn libraries(&self) -> PathBuf {
        self.root.join("libraries")
    }
    pub fn assets(&self) -> PathBuf {
        self.root.join("assets")
    }
    pub fn versions(&self) -> PathBuf {
        self.root.join("versions")
    }
    pub fn java(&self, major: u32) -> PathBuf {
        self.root.join("java").join(major.to_string())
    }
    pub fn downloads(&self) -> PathBuf {
        self.root.join("downloads")
    }
    /// Répertoire de jeu isolé (mods, options, mondes, logs).
    pub fn instance(&self) -> PathBuf {
        self.root.join("instance")
    }
    pub fn natives(&self) -> PathBuf {
        self.instance().join("natives")
    }
    pub fn version_json(&self, id: &str) -> PathBuf {
        self.versions().join(id).join(format!("{id}.json"))
    }
    pub fn client_jar(&self, id: &str) -> PathBuf {
        self.versions().join(id).join(format!("{id}.jar"))
    }
}

pub fn fabric_profile_id(loader: &str, mc: &str) -> String {
    format!("fabric-loader-{loader}-{mc}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_url_strips_game_port() {
        assert_eq!(stats_url("play.example.fr:25565", 25580), "http://play.example.fr:25580/stats");
        assert_eq!(stats_url("play.example.fr", 25580), "http://play.example.fr:25580/stats");
        assert_eq!(stats_url(" 10.0.0.2:1 ", 9), "http://10.0.0.2:9/stats");
    }

    #[test]
    fn config_roundtrip_and_defaults() {
        let c: Config = serde_json::from_str(r#"{"nickname":"A"}"#).unwrap();
        assert_eq!((c.nickname.as_str(), c.memory_mb), ("A", 4096));
        assert_eq!(stats_url(&PackProfile::default().server_addr(), STATS_PORT), "http://classroom:25580/stats");
    }

    #[test]
    fn layout_is_under_root() {
        let d = Dirs::new("/r");
        for p in [d.libraries(), d.assets(), d.java(25), d.instance(), d.natives(), d.client_jar("26.3")] {
            assert!(p.starts_with(&d.root));
        }
    }
}
