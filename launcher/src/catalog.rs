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
    /// Carte du serveur (BlueMap...), URL http(s) validée par `valid_map` ; ouverte dans une fenêtre séparée sans accès à l'application.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map: Option<String>,
}

/// URL de carte à ouvrir : un nom d'hôte court (nom MagicDNS Tailscale, comme pour le serveur de jeu) reçoit le suffixe du tailnet
/// s'il est connu ; `localhost`, les noms complets et les IP sont gardés tels quels. `None` : URL invalide.
pub fn resolve_map(u: &str, suffix: Option<&str>) -> Option<String> {
    if !valid_map(u) {
        return None;
    }
    let mut url = reqwest::Url::parse(u).ok()?;
    let host = url.host_str()?.to_string();
    if crate::config::is_short_host(&host) {
        url.set_host(Some(&crate::config::choose_host(&host, suffix, None))).ok()?;
    }
    Some(url.to_string())
}

/// Longueur maximale d'une URL de carte : même limite que `pack-tools.py`.
pub const MAP_MAX_LEN: usize = 300;

/// Même règle que `pack-tools.py` : http(s) avec hôte, aucun identifiant, ASCII visible sans antislash, longueur bornée.
pub fn valid_map(u: &str) -> bool {
    let lower = u.to_ascii_lowercase();
    let rest = lower.strip_prefix("http://").or_else(|| lower.strip_prefix("https://"));
    if u.is_empty() || u.len() > MAP_MAX_LEN || !u.bytes().all(|b| (0x21..=0x7E).contains(&b) && b != b'\\') || rest.is_none_or(|r| r.starts_with('/')) {
        return false;
    }
    // Hôtes locaux refusés : une carte du catalogue ne doit jamais viser la machine de l'utilisateur.
    let local = |h: &str| {
        let h = h.to_ascii_lowercase();
        h == "localhost" || h.ends_with(".localhost") || h.starts_with("127.") || h == "[::1]"
    };
    reqwest::Url::parse(u).is_ok_and(|p| p.host_str().is_some_and(|h| !h.is_empty() && !local(h)) && p.username().is_empty() && p.password().is_none() && p.port() != Some(0))
}

/// Archive des fichiers « overrides » d'un pack (resourcepacks, datapacks, shaderpacks...) publiée à côté du zip du pack.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct Overrides {
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

/// Taille maximale annoncée d'une archive d'overrides (2 Go) : même limite que `pack-tools.py`.
pub const OVERRIDES_MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Hash de version d'un pack : `packHash` du catalogue. Sans overrides, c'est le hash d'index (inchangé) ; avec,
/// `sha256_hex(utf8("<hash d'index>\n<sha256 de l'archive>"))` (`\n` = saut de ligne), en minuscules (formule de `pack-tools.py`).
pub fn pack_version_hash(index_hash: &str, overrides: Option<&Overrides>) -> String {
    use sha2::{Digest, Sha256};
    match overrides {
        None => index_hash.to_ascii_lowercase(),
        Some(o) => hex::encode(Sha256::digest(format!("{}\n{}", index_hash.to_ascii_lowercase(), o.sha256.to_ascii_lowercase()))),
    }
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
    /// Absent ou `null` : pack sans overrides.
    #[serde(default)]
    pub overrides: Option<Overrides>,
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
    /// Le serveur annonce une carte (bouton « Carte »).
    pub has_map: bool,
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
            has_map: self.server.as_ref().is_some_and(|s| s.map.is_some()),
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
    if let Some(o) = &e.overrides {
        let local = o.url.starts_with("http://127.0.0.1") || o.url.starts_with("http://localhost");
        if !(o.url.starts_with("https://") || local) || o.sha256.len() != 64 || !o.sha256.chars().all(|c| c.is_ascii_hexdigit()) || o.size == 0 || o.size > OVERRIDES_MAX_BYTES {
            bail!("overrides invalides dans le catalogue");
        }
    }
    Ok(())
}

/// Entrées invalides ignorées une à une (un pack cassé ne masque pas les autres).
pub fn parse_catalog(json: &str) -> Result<Vec<PackEntry>> {
    let v: serde_json::Value = serde_json::from_str(json).context("catalogue invalide")?;
    let list = v.get("packs").and_then(|p| p.as_array()).context("catalogue : champ « packs » absent")?;
    let mut packs: Vec<PackEntry> = list.iter().filter_map(|x| serde_json::from_value::<PackEntry>(x.clone()).ok()).filter(|e| validate(e).is_ok()).collect();
    // Carte invalide : seule la carte est ignorée, le pack reste utilisable.
    for s in packs.iter_mut().filter_map(|p| p.server.as_mut()) {
        s.map = s.map.take().filter(|m| valid_map(m));
    }
    Ok(packs)
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
    fn overrides_are_validated_and_change_the_version_hash() {
        let e = |o: &str| format!(r#"{{"packs":[{{"id":"a","name":"A","mc":"1","loader":"fabric","pack":"https://x/a.zip","packHash":"abcdef0123"{o}}}]}}"#);
        let sha = "ab".repeat(32);
        let ok = parse_catalog(&e(&format!(r#","overrides":{{"url":"https://x/o.zip","sha256":"{sha}","size":10}}"#))).unwrap();
        assert_eq!(ok[0].overrides.as_ref().unwrap().size, 10);
        for bad in [
            format!(r#","overrides":{{"url":"http://evil/o.zip","sha256":"{sha}","size":10}}"#),
            r#","overrides":{"url":"https://x/o.zip","sha256":"zz","size":10}"#.to_string(),
            format!(r#","overrides":{{"url":"https://x/o.zip","sha256":"{sha}","size":0}}"#),
            format!(r#","overrides":{{"url":"https://x/o.zip","sha256":"{sha}","size":{}}}"#, OVERRIDES_MAX_BYTES + 1),
        ] {
            assert!(parse_catalog(&e(&bad)).unwrap().is_empty(), "{bad}");
        }
        assert_eq!(parse_catalog(&e("")).unwrap()[0].overrides, None);
        let o = ok[0].overrides.as_ref();
        assert_eq!(pack_version_hash("ABC", None), "abc");
        assert_ne!(pack_version_hash("abc", o), "abc");
        assert_eq!(pack_version_hash("abc", o).len(), 64);
        // vecteur partagé avec scripts/test_pack_tools.py (même formule des deux côtés)
        assert_eq!(pack_version_hash("ABCDEF0123", o), "43d3f77780af2ff9fa4f009f1b6ea2cb6bb5cac91bb33342c2fce45866e35288");
    }

    #[test]
    fn map_url_validation_matches_pack_tools() {
        for ok in ["https://map.example.fr/", "http://91.197.6.154:8100/#world:0:0:0", "https://m.example.fr/map?x=1&y=2", "HTTP://X.FR/"] {
            assert!(valid_map(ok), "{ok}");
        }
        let long = format!("https://x.fr/{}", "a".repeat(300));
        for bad in ["ftp://x.fr/", "javascript:alert(1)", "map.example.fr", "https://u:p@x.fr/", "https://u@x.fr/", "https://x.fr@evil.fr/", "https://x.fr/a b", "", &long, "https://x.fr/é", "https:///x", "file:///c:/x", "https://x.fr:0/", "http://localhost:8100/", "http://a.localhost/", "http://127.0.0.1:8100/", "http://127.1.2.3/", "http://[::1]:8100/", "https://x.fr\\@evil.fr/", "https:/x.fr/", "https://"] {
            assert!(!valid_map(bad), "{bad}");
        }
    }

    #[test]
    fn short_tailscale_names_get_the_magic_dns_suffix() {
        assert_eq!(resolve_map("http://home-server:8100/", Some("x.ts.net")).as_deref(), Some("http://home-server.x.ts.net:8100/"));
        assert_eq!(resolve_map("http://home-server:8100", None).as_deref(), Some("http://home-server:8100/"));
        assert_eq!(resolve_map("https://map.example.fr/a?b=1", Some("x.ts.net")).as_deref(), Some("https://map.example.fr/a?b=1"));
        assert_eq!(resolve_map("http://10.0.0.2:8100/", Some("x.ts.net")).as_deref(), Some("http://10.0.0.2:8100/"));
        assert_eq!(resolve_map("http://localhost:8100/", Some("x.ts.net")), None);
        assert_eq!(resolve_map("http://u:p@home-server/", Some("x.ts.net")), None);
    }

    #[test]
    fn map_is_exposed_and_an_invalid_map_is_dropped_without_dropping_the_pack() {
        let e = |m: &str| format!(r#"{{"packs":[{{"id":"a","name":"A","mc":"1","loader":"fabric","pack":"https://x/a.zip","packHash":"abcdef0123","server":{{"host":"h","port":1,"motd":"m"{m}}}}}]}}"#);
        let p = parse_catalog(&e(r#","map":"https://map.example.fr/""#)).unwrap();
        assert_eq!(p[0].server.as_ref().unwrap().map.as_deref(), Some("https://map.example.fr/"));
        assert!(p[0].view(None, false).has_map);
        for m in [r#","map":"https://u:p@x.fr/""#, r#","map":"ftp://x""#, r#","map":12"#] {
            let p = parse_catalog(&e(m)).unwrap_or_default();
            assert!(p.is_empty() || !p[0].view(None, false).has_map, "{m}");
        }
        assert_eq!(parse_catalog(&e(r#","map":"ftp://x""#)).unwrap().len(), 1);
        assert!(!parse_catalog(&e("")).unwrap()[0].view(None, false).has_map);
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
