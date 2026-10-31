//! Profil Fabric Loader (meta.fabricmc.net) et chemins Maven.
use crate::mojang::Arguments;
use anyhow::{Result, bail};
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct FabricLib {
    pub name: String,
    pub url: String,
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    /// Chemin relatif dans `libraries/` quand le manifeste le donne (Forge, NeoForge) ; sinon déduit du nom.
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct FabricProfile {
    #[serde(rename = "mainClass")]
    pub main_class: String,
    #[serde(default)]
    pub arguments: Arguments,
    pub libraries: Vec<FabricLib>,
    /// `${version_name}` au lancement quand le loader impose le sien (Forge, NeoForge) ; sinon la version vanilla.
    #[serde(skip)]
    pub version_name: Option<String>,
}

/// `group:artifact:version[:classifier][@ext]` -> `group/path/artifact/version/artifact-version[-classifier].ext` (ext = jar par défaut)
pub fn maven_path(name: &str) -> Result<String> {
    let (name, ext) = name.split_once('@').unwrap_or((name, "jar"));
    let p: Vec<&str> = name.split(':').collect();
    if !(3..=4).contains(&p.len()) || p.iter().any(|s| s.is_empty()) || ext.is_empty() {
        bail!("coordonnées Maven invalides : {name}");
    }
    let (g, a, v) = (p[0].replace('.', "/"), p[1], p[2]);
    let cl = p.get(3).map(|c| format!("-{c}")).unwrap_or_default();
    Ok(format!("{g}/{a}/{v}/{a}-{v}{cl}.{ext}"))
}

impl FabricLib {
    pub fn rel_path(&self) -> Result<String> {
        self.path.clone().map_or_else(|| maven_path(&self.name), Ok)
    }

    /// URL de téléchargement = base Maven + chemin.
    pub fn download_url(&self) -> Result<String> {
        Ok(format!("{}/{}", self.url.trim_end_matches('/'), maven_path(&self.name)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maven_paths() {
        assert_eq!(maven_path("org.ow2.asm:asm:9.10.1").unwrap(), "org/ow2/asm/asm/9.10.1/asm-9.10.1.jar");
        assert_eq!(maven_path("a.b:c:1:natives").unwrap(), "a/b/c/1/c-1-natives.jar");
        assert!(maven_path("a:b").is_err());
        assert_eq!(maven_path("net.minecraft:client:26.3:mappings@txt").unwrap(), "net/minecraft/client/26.3/client-26.3-mappings.txt");
        assert_eq!(maven_path("a:b:1@zip").unwrap(), "a/b/1/b-1.zip");
        assert!(maven_path("a:b:1@").is_err());
        assert!(maven_path("a:..:1").is_ok()); // le chemin est ensuite passé à safe_join qui le refuse
    }

    #[test]
    fn download_url_joins_base() {
        let l = FabricLib { name: "g:a:1".into(), url: "https://m.example/".into(), sha1: None, size: None, path: None };
        assert_eq!(l.download_url().unwrap(), "https://m.example/g/a/1/a-1.jar");
    }

    #[test]
    fn parses_profile() {
        let p: FabricProfile = serde_json::from_str(
            r#"{"id":"x","inheritsFrom":"26.3","mainClass":"net.fabricmc.loader.impl.launch.knot.KnotClient",
            "arguments":{"game":[],"jvm":["-DFabricMcEmu= net.minecraft.client.main.Main "]},
            "libraries":[{"name":"g:a:1","url":"https://m/","sha1":"ab","size":1}]}"#,
        )
        .unwrap();
        assert!(p.main_class.ends_with("KnotClient"));
        assert_eq!(p.arguments.jvm.len(), 1);
        assert_eq!(p.libraries[0].sha1.as_deref(), Some("ab"));
    }
}
