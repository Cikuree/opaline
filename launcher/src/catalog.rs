//! Catalogue `packs.json` (dernière release) : parsing, validation, vues pour l'interface.
use crate::config::Dirs;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    /// Nom MagicDNS court (le suffixe du tailnet est ajouté à l'exécution). Jamais affiché.
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
    pub server: ServerInfo,
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
    pub supported: bool,
    pub installed: bool,
    pub update_available: bool,
    pub installed_version_hash: Option<String>,
}

impl PackEntry {
    /// Seul Fabric est installable pour l'instant.
    pub fn supported(&self) -> bool {
        self.loader.eq_ignore_ascii_case("fabric")
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
            supported: self.supported(),
            installed,
            update_available,
            installed_version_hash: installed_hash.map(str::to_string),
        }
    }
}

fn plain_token(s: &str, extra: &[char]) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || extra.contains(&c))
}

/// Entrée utilisable : identifiants simples (ils deviennent dossiers, fichiers, hôtes d'URL), URL du zip en https
/// (http seulement vers la boucle locale, pour les tests).
fn validate(e: &PackEntry) -> Result<()> {
    if !plain_token(&e.id, &[]) || !plain_token(&e.server.host, &['.']) || !(e.thumbnail.is_empty() || plain_token(&e.thumbnail, &[])) {
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
      {"id":"neo","name":"Neo","mc":"1","loader":"neoforge","pack":"https://x/n.zip","packHash":"abcdef0123","server":{"host":"neo","port":1,"motd":"m"}},
      {"id":"http","name":"x","mc":"1","loader":"fabric","pack":"http://evil/x.zip","packHash":"abcdef0123","server":{"host":"a","port":1,"motd":"m"}},
      {"broken":true}]}"#;

    #[test]
    fn parses_and_drops_invalid_entries() {
        let p = parse_catalog(J).unwrap();
        assert_eq!(p.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ["classroom-run", "neo"]);
        assert_eq!(p[0].server.stats_port, 25580);
        assert_eq!(p[1].server.stats_port, 0);
        assert!(parse_catalog("{}").is_err() && parse_catalog("nope").is_err());
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
