//! Client minimal de l'API Modrinth (recherche, versions d'un projet) pour « Mes mods ». Le parsing est pur ; le HTTP passe par
//! le trait `Api` (remplaçable par un faux dans les tests). Seuls les fichiers de `https://cdn.modrinth.com/` sont acceptés.
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

pub const API: &str = "https://api.modrinth.com/v2";
pub const CDN: &str = "https://cdn.modrinth.com/";

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    pub project_id: String,
    pub title: String,
    pub description: String,
    pub downloads: u64,
    /// Seulement une URL `cdn.modrinth.com` en https ; sinon `None` (pas d'image).
    pub icon_url: Option<String>,
}

#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct VersionFile {
    pub url: String,
    pub filename: String,
    #[serde(default)]
    pub primary: bool,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub hashes: std::collections::HashMap<String, String>,
}

#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct Dependency {
    #[serde(default)]
    pub project_id: Option<String>,
    pub dependency_type: String,
}

#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct Version {
    pub id: String,
    pub version_number: String,
    #[serde(default)]
    pub version_type: String,
    #[serde(default)]
    pub files: Vec<VersionFile>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
}

impl Version {
    /// Fichier principal (marqué `primary`, sinon le premier `.jar`).
    pub fn primary_file(&self) -> Option<&VersionFile> {
        self.files.iter().find(|f| f.primary).or_else(|| self.files.iter().find(|f| f.filename.ends_with(".jar")))
    }
}

/// Release de préférence (la liste de l'API est triée de la plus récente à la plus ancienne), sinon la plus récente.
pub fn choose_version(versions: &[Version]) -> Option<&Version> {
    let usable = |v: &&Version| v.primary_file().is_some_and(|f| f.hashes.contains_key("sha512"));
    versions.iter().filter(usable).find(|v| v.version_type == "release").or_else(|| versions.iter().find(usable))
}

pub fn cdn_icon(url: &str) -> Option<String> {
    (url.starts_with(CDN) && !url[CDN.len()..].contains(['"', '\'', '<', '>', ' '])).then(|| url.to_string())
}

pub fn parse_search(json: &str) -> Result<Vec<Hit>> {
    #[derive(Deserialize)]
    struct Raw {
        project_id: String,
        title: String,
        #[serde(default)]
        description: String,
        #[serde(default)]
        downloads: u64,
        #[serde(default)]
        icon_url: Option<String>,
        #[serde(default)]
        client_side: String,
    }
    #[derive(Deserialize)]
    struct Resp {
        hits: Vec<Raw>,
    }
    let r: Resp = serde_json::from_str(json).context("réponse Modrinth illisible")?;
    Ok(r.hits
        .into_iter()
        .filter(|h| h.client_side != "unsupported")
        .map(|h| Hit { project_id: h.project_id, title: h.title, description: h.description, downloads: h.downloads, icon_url: h.icon_url.as_deref().and_then(cdn_icon) })
        .collect())
}

pub fn parse_versions(json: &str) -> Result<Vec<Version>> {
    serde_json::from_str(json).context("réponse Modrinth illisible")
}

/// Loaders Modrinth acceptés pour un loader de pack (Quilt exécute aussi les mods Fabric).
pub fn loaders_for(loader: &str) -> Vec<&str> {
    if loader == "quilt" { vec!["quilt", "fabric"] } else { vec![loader] }
}

pub fn search_url(base: &str, query: &str, mc: &str, loader: &str) -> Result<String> {
    let cats = loaders_for(loader).iter().map(|l| format!("\"categories:{l}\"")).collect::<Vec<_>>().join(",");
    let facets = format!("[[\"project_type:mod\"],[\"versions:{mc}\"],[{cats}],[\"client_side:required\",\"client_side:optional\"]]");
    Ok(reqwest::Url::parse_with_params(&format!("{base}/search"), [("query", query), ("facets", facets.as_str()), ("limit", "20")])?.to_string())
}

fn check_project(project: &str) -> Result<()> {
    if project.is_empty() || !project.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        bail!("identifiant de projet Modrinth invalide");
    }
    Ok(())
}

pub fn versions_url(base: &str, project: &str, mc: &str, loader: &str) -> Result<String> {
    check_project(project)?;
    let loaders = format!("[{}]", loaders_for(loader).iter().map(|l| format!("\"{l}\"")).collect::<Vec<_>>().join(","));
    let games = format!("[\"{mc}\"]");
    Ok(reqwest::Url::parse_with_params(&format!("{base}/project/{project}/version"), [("loaders", loaders.as_str()), ("game_versions", games.as_str())])?.to_string())
}

pub trait Api {
    fn search(&self, query: &str, mc: &str, loader: &str) -> Result<Vec<Hit>>;
    fn versions(&self, project: &str, mc: &str, loader: &str) -> Result<Vec<Version>>;
    fn title(&self, project: &str) -> Option<String>;
}

pub struct Http {
    client: reqwest::blocking::Client,
    base: String,
}

impl Http {
    pub fn new(base: &str) -> Result<Http> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(concat!("Cikuree/opaline/", env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_secs(20))
            .build()?;
        Ok(Http { client, base: base.trim_end_matches('/').to_string() })
    }

    pub fn default_api() -> Result<Http> {
        Http::new(API)
    }

    fn get(&self, url: &str) -> Result<String> {
        crate::download::get_text(&self.client, url).map_err(|e| e.context("Modrinth injoignable"))
    }
}

impl Api for Http {
    fn search(&self, query: &str, mc: &str, loader: &str) -> Result<Vec<Hit>> {
        parse_search(&self.get(&search_url(&self.base, query, mc, loader)?)?)
    }
    fn versions(&self, project: &str, mc: &str, loader: &str) -> Result<Vec<Version>> {
        parse_versions(&self.get(&versions_url(&self.base, project, mc, loader)?)?)
    }
    fn title(&self, project: &str) -> Option<String> {
        #[derive(Deserialize)]
        struct P {
            title: String,
        }
        check_project(project).ok()?;
        let p: P = serde_json::from_str(&self.get(&format!("{}/project/{project}", self.base)).ok()?).ok()?;
        Some(p.title)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_filters_client_unsupported_and_foreign_icons() {
        let json = r#"{"hits":[
          {"project_id":"YL57xq9U","title":"Iris Shaders","description":"Shaders","downloads":1234567,"icon_url":"https://cdn.modrinth.com/data/YL57xq9U/icon.png","client_side":"required"},
          {"project_id":"srv","title":"ServerOnly","description":"x","downloads":1,"icon_url":null,"client_side":"unsupported"},
          {"project_id":"evil","title":"Evil","description":"x","downloads":2,"icon_url":"https://evil.example/x.png\" onerror=\"1","client_side":"optional"}],"total_hits":3}"#;
        let h = parse_search(json).unwrap();
        assert_eq!(h.len(), 2);
        assert_eq!(h[0].icon_url.as_deref(), Some("https://cdn.modrinth.com/data/YL57xq9U/icon.png"));
        assert_eq!((h[1].title.as_str(), h[1].icon_url.as_deref()), ("Evil", None));
        assert!(parse_search("pas du json").is_err());
    }

    fn v(id: &str, ty: &str, sha: Option<&str>) -> Version {
        let hashes = sha.map(|s| [("sha512".to_string(), s.to_string())].into()).unwrap_or_default();
        Version { id: id.into(), version_number: id.into(), version_type: ty.into(), files: vec![VersionFile { url: format!("{CDN}data/x/{id}.jar"), filename: format!("{id}.jar"), primary: true, size: None, hashes }], dependencies: vec![] }
    }

    #[test]
    fn prefers_release_then_newest_and_needs_a_hash() {
        let list = [v("beta2", "beta", Some("a")), v("rel1", "release", Some("b")), v("rel0", "release", Some("c"))];
        assert_eq!(choose_version(&list).unwrap().id, "rel1");
        let list = [v("beta2", "beta", Some("a")), v("alpha", "alpha", Some("b"))];
        assert_eq!(choose_version(&list).unwrap().id, "beta2");
        assert!(choose_version(&[v("nohash", "release", None)]).is_none());
        assert!(choose_version(&[]).is_none());
    }

    #[test]
    fn parses_versions_with_dependencies() {
        let json = r#"[{"id":"v1","project_id":"p","version_number":"1.8.0","version_type":"release",
          "files":[{"hashes":{"sha512":"abc","sha1":"d"},"url":"https://cdn.modrinth.com/data/p/versions/v1/iris.jar","filename":"iris.jar","primary":true,"size":10}],
          "dependencies":[{"version_id":null,"project_id":"AANobbMI","file_name":null,"dependency_type":"required"},{"project_id":"x","dependency_type":"optional"}]}]"#;
        let vs = parse_versions(json).unwrap();
        assert_eq!(vs[0].dependencies[0].project_id.as_deref(), Some("AANobbMI"));
        assert_eq!(vs[0].primary_file().unwrap().filename, "iris.jar");
    }

    #[test]
    fn urls_carry_facets_and_filters() {
        let u = search_url(API, "iris", "1.21.1", "fabric").unwrap();
        assert!(u.starts_with("https://api.modrinth.com/v2/search?query=iris&facets="));
        let dec = reqwest::Url::parse(&u).unwrap().query_pairs().find(|(k, _)| k == "facets").unwrap().1.into_owned();
        assert_eq!(dec, r#"[["project_type:mod"],["versions:1.21.1"],["categories:fabric"],["client_side:required","client_side:optional"]]"#);
        let u = versions_url(API, "YL57xq9U", "1.21.1", "quilt").unwrap();
        let q: std::collections::HashMap<_, _> = reqwest::Url::parse(&u).unwrap().query_pairs().into_owned().collect();
        assert_eq!(q["loaders"], r#"["quilt","fabric"]"#);
        assert_eq!(q["game_versions"], r#"["1.21.1"]"#);
        assert!(versions_url(API, "../x", "1", "fabric").is_err());
    }
}
