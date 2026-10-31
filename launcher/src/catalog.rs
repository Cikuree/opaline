//! Catalogue `packs.json` (dernière release) : parsing, validation, vues pour l'interface.
use crate::config::Dirs;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    /// Nom court (sans point) = nom MagicDNS Tailscale, suffixe du tailnet ajouté à l'exécution ; nom complet ou IP =
    /// serveur public, utilisé tel quel. Jamais affiché.
    pub host: String,
    pub port: u16,
    pub motd: String,
    #[serde(default)]
    pub stats_port: u16,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PackEntry {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub mc: String,
    pub loader: String,
    #[serde(default)]
    pub loader_version: String,
    #[serde(default)]
    pub mods_count: u32,
    #[serde(default)]
    pub thumbnail: String,
    #[serde(default)]
    pub widgets: Vec<String>,
    /// URL du `pack-<id>.zip`.
    pub pack: String,
    pub pack_hash: String,
    /// Absent ou `null` : pack sans serveur (ni carte serveur, ni amis, ni statistiques).
    #[serde(default)]
    pub server: Option<ServerInfo>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PackView {
    pub id: String,
    pub name: String,
    pub description: String,
    pub mc: String,
    pub loader: String,
    pub loader_version: String,
    pub mods_count: u32,
    pub thumbnail: String,
    pub widgets: Vec<String>,
    pub has_server: bool,
    /// Serveur avec endpoint de statistiques (`statsPort` ≠ 0).
    pub has_stats: bool,
    pub supported: bool,
    pub installed: bool,
    pub update_available: bool,
    pub installed_version_hash: Option<String>,
}

impl PackEntry {
    /// Fabric, Quilt, NeoForge et Forge sont installables.
    pub fn supported(&self) -> bool {
        crate::loader::LoaderKind::parse(&self.loader).is_some()
    }

    pub fn view(&self, installed_hash: Option<&str>, installed: bool) -> PackView {
        let update_available = installed && !installed_hash.is_some_and(|h| h.eq_ignore_ascii_case(self.pack_hash.trim()));
        PackView {
            id: self.id.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            mc: self.mc.clone(),
            loader: self.loader.clone(),
            loader_version: self.loader_version.clone(),
            mods_count: self.mods_count,
            thumbnail: self.thumbnail.clone(),
            widgets: self.widgets.clone(),
            has_server: self.server.is_some(),
            has_stats: self.server.as_ref().is_some_and(|s| s.stats_port != 0),
            supported: self.supported(),
            installed,
            update_available,
            installed_version_hash: installed_hash.map(str::to_string),
        }
    }
}

/// Même règle que `pack-tools.py` : segments non vides `[a-z0-9-]+` séparés par des points.
fn valid_host(h: &str) -> bool {
    !h.is_empty() && h.split('.').all(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'))
}

fn plain_token(s: &str, extra: &[char]) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || extra.contains(&c))
}

/// Entrée utilisable : identifiants simples (ils deviennent dossiers, fichiers, hôtes d'URL), URL du zip en https
/// (http seulement vers la boucle locale, pour les tests).
fn validate(e: &PackEntry) -> Result<()> {
    if !plain_token(&e.id, &[]) || !e.server.as_ref().is_none_or(|s| valid_host(&s.host)) || !(e.thumbnail.is_empty() || plain_token(&e.thumbnail, &[])) {
        bail!("identifiant invalide dans le catalogue");
    }
    let local = e.pack.starts_with("http://127.0.0.1") || e.pack.starts_with("http://localhost");
    if !(e.pack.starts_with("https://") || local) {
        bail!("URL de pack refusée (https uniquement)");
    }
    if e.pack_hash.trim().len() < 8 {
        bail!("packHash absent");
    }
    Ok(())
}

/// Entrées invalides ignorées une à une (un pack cassé ne masque pas les autres).
pub fn parse_catalog(json: &str) -> Result<Vec<PackEntry>> {
    let v: serde_json::Value = serde_json::from_str(json).context("catalogue invalide")?;
    let list = v.get("packs").and_then(|p| p.as_array()).context("catalogue : champ « packs » absent")?;
    Ok(list.iter().filter_map(|x| serde_json::from_value::<PackEntry>(x.clone()).ok()).filter(|e| validate(e).is_ok()).collect())
}

fn cache_file(dirs: &Dirs) -> std::path::PathBuf {
    dirs.cache().join("packs.json")
}

/// Surcharge de développement : `OPALINE_PACKS_URL` (URL ou chemin de fichier) remplace l'URL de la release.
fn source() -> String {
    crate::config::dev_env("OPALINE_PACKS_URL").unwrap_or_else(crate::config::packs_json_url)
}

/// Catalogue publié, sinon dernier catalogue en cache (hors ligne).
pub fn load(dirs: &Dirs, client: &reqwest::blocking::Client) -> Result<Vec<PackEntry>> {
    let src = source();
    let text = if std::path::Path::new(&src).is_file() { std::fs::read_to_string(&src).map_err(Into::into) } else { crate::download::get_text(client, &src) };
    match text.and_then(|t| parse_catalog(&t).map(|p| (t, p))) {
        Ok((t, packs)) => {
            if std::fs::create_dir_all(dirs.cache()).is_ok() {
                let _ = std::fs::write(cache_file(dirs), t);
            }
            Ok(packs)
        }
        Err(e) => {
            let cached = std::fs::read_to_string(cache_file(dirs)).ok().and_then(|t| parse_catalog(&t).ok());
            cached.with_context(|| format!("catalogue des packs indisponible : {e}"))
        }
    }
}

pub fn find(packs: &[PackEntry], id: &str) -> Result<PackEntry> {
    packs.iter().find(|p| p.id == id).cloned().with_context(|| format!("pack inconnu : {id}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const J: &str = r#"{"packs":[
      {"id":"classroom-run","name":"Classroom Run","description":"d","mc":"26.3","loader":"fabric","loaderVersion":"0.20.1","modsCount":4,
       "thumbnail":"ender_eye","widgets":["classroomRun","server"],"pack":"https://github.com/x/pack-classroom-run.zip","packHash":"ABCDEF0123",
       "server":{"host":"home-server","port":25565,"motd":"Classroom Run","statsPort":25580}},
      {"id":"../evil","name":"x","mc":"1","loader":"fabric","pack":"https://x","packHash":"abcdef0123","server":{"host":"a","port":1,"motd":"m"}},
      {"id":"neo","name":"Neo","mc":"1","loader":"bedrock","pack":"https://x/n.zip","packHash":"abcdef0123","server":{"host":"neo","port":1,"motd":"m"}},
      {"id":"http","name":"x","mc":"1","loader":"fabric","pack":"http://evil/x.zip","packHash":"abcdef0123","server":{"host":"a","port":1,"motd":"m"}},
      {"id":"nosrv","name":"Sans","mc":"1","loader":"fabric","pack":"https://x/s.zip","packHash":"abcdef0123","server":null},
      {"id":"nosrv2","name":"Sans2","mc":"1","loader":"fabric","pack":"https://x/s.zip","packHash":"abcdef0123"},
      {"broken":true}]}"#;

    #[test]
    fn parses_and_drops_invalid_entries() {
        let p = parse_catalog(J).unwrap();
        assert_eq!(p.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ["classroom-run", "neo", "nosrv", "nosrv2"]);
        assert_eq!(p[0].server.as_ref().unwrap().stats_port, 25580);
        assert_eq!(p[1].server.as_ref().unwrap().stats_port, 0);
        assert!(p[2].server.is_none() && p[3].server.is_none());
        assert!(!p[2].view(None, false).has_server && p[0].view(None, false).has_server);
        assert!(p[0].view(None, false).has_stats && !p[1].view(None, false).has_stats && !p[2].view(None, false).has_stats);
        assert!(parse_catalog("{}").is_err() && parse_catalog("nope").is_err());
    }

    #[test]
    fn host_validation_matches_pack_tools() {
        for ok in ["home-server", "mon.minestrator.com", "10.0.0.2"] {
            assert!(valid_host(ok), "{ok}");
        }
        for bad in ["", ".a", "a.", "a..b", "Mon.Host", "a b", "a:25565", "::1", "a_b"] {
            assert!(!valid_host(bad), "{bad}");
        }
    }

    #[test]
    fn views_follow_install_state_and_loader() {
        let p = parse_catalog(J).unwrap();
        let v = p[0].view(None, false);
        assert!(v.supported && !v.installed && !v.update_available);
        assert!(!p[0].view(Some("abcdef0123"), true).update_available);
        assert!(p[0].view(Some("0000000000"), true).update_available);
        assert!(p[0].view(None, true).update_available);
        assert!(!p[1].view(None, false).supported);
        let j = serde_json::to_value(&v).unwrap();
        assert!(j.get("installedVersionHash").is_some() && j.get("updateAvailable").is_some() && j.get("loaderVersion").is_some());
        assert!(find(&p, "nope").is_err());
    }
}
