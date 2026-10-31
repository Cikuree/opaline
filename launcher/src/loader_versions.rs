//! Listes de versions des loaders (parsing pur, sans réseau). Inclus aussi par l'Atelier via `#[path]` : seulement `std` et `serde_json`.
use serde_json::Value;

pub const FABRIC_META: &str = "https://meta.fabricmc.net/v2/versions/loader";
pub const QUILT_META: &str = "https://meta.quiltmc.org/v3/versions/loader";
pub const NEOFORGE_METADATA: &str = "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml";
pub const FORGE_PROMOTIONS: &str = "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";

/// Réponse de `…/versions/loader/{mc}` (Fabric et Quilt) : `[{"loader":{"version":"…"}}]`, ordre du serveur (récent d'abord).
pub fn parse_meta_loaders(json: &str) -> Vec<String> {
    let Ok(Value::Array(a)) = serde_json::from_str::<Value>(json) else { return vec![] };
    a.iter().filter_map(|e| Some(e.get("loader")?.get("version")?.as_str()?.to_string())).collect()
}

/// Les `<version>` d'un `maven-metadata.xml`.
pub fn parse_maven_versions(xml: &str) -> Vec<String> {
    xml.split("<version>").skip(1).filter_map(|s| Some(s.split_once("</version>")?.0.trim().to_string())).filter(|v| !v.is_empty()).collect()
}

/// Préfixe des versions NeoForge pour une version de Minecraft : 1.21.1 -> `21.1.`, 1.21 -> `21.0.`, 26.3 -> `26.3.0.`, 26.1.2 -> `26.1.2.`.
pub fn neoforge_prefix(mc: &str) -> Option<String> {
    let p: Vec<&str> = mc.trim().split('.').collect();
    if p.iter().any(|s| s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit())) {
        return None;
    }
    match p.as_slice() {
        ["1", minor, patch] => Some(format!("{minor}.{patch}.")),
        ["1", minor] => Some(format!("{minor}.0.")),
        [a, b] => Some(format!("{a}.{b}.0.")),
        [a, b, c] => Some(format!("{a}.{b}.{c}.")),
        _ => None,
    }
}

/// Clé de tri : composantes numériques de tête, puis « sans suffixe » (une release passe avant `-beta`).
fn version_key(v: &str) -> (Vec<u64>, bool) {
    let core = v.split(['-', '+']).next().unwrap_or(v);
    (core.split('.').map_while(|s| s.parse().ok()).collect(), core.len() == v.len())
}

/// Versions NeoForge de ce Minecraft, la plus récente d'abord.
pub fn neoforge_versions(metadata_xml: &str, mc: &str) -> Vec<String> {
    let Some(prefix) = neoforge_prefix(mc) else { return vec![] };
    let mut v: Vec<String> = parse_maven_versions(metadata_xml).into_iter().filter(|x| x.starts_with(&prefix)).collect();
    v.sort_by_key(|x| std::cmp::Reverse(version_key(x)));
    v
}

/// `promotions_slim.json` : « recommended » puis « latest » (sans doublon) pour ce Minecraft.
pub fn forge_versions(promotions_json: &str, mc: &str) -> Vec<String> {
    let Ok(v) = serde_json::from_str::<Value>(promotions_json) else { return vec![] };
    let mut out: Vec<String> = Vec::new();
    for kind in ["recommended", "latest"] {
        if let Some(s) = v.get("promos").and_then(|p| p.get(format!("{mc}-{kind}"))).and_then(Value::as_str) {
            if !out.iter().any(|o| o == s) {
                out.push(s.to_string());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meta_list_for_fabric_and_quilt() {
        let j = r#"[{"loader":{"version":"0.20.0-beta.9","build":9},"launcherMeta":{}},{"loader":{"version":"0.20.0-beta.8"}},{"nope":1}]"#;
        assert_eq!(parse_meta_loaders(j), ["0.20.0-beta.9", "0.20.0-beta.8"]);
        assert!(parse_meta_loaders("pas du json").is_empty());
    }

    #[test]
    fn neoforge_metadata_filtered_and_sorted() {
        let x = "<metadata><versioning><latest>26.3.0.100</latest><versions><version>26.3.0.9</version><version>26.3.0.100</version>\
                 <version>21.1.265</version><version>26.1.2.125</version><version>26.3.0.101-beta</version><version>26.3.1.2</version></versions></versioning></metadata>";
        assert_eq!(neoforge_versions(x, "26.3"), ["26.3.0.101-beta", "26.3.0.100", "26.3.0.9"]);
        assert_eq!(neoforge_versions(x, "1.21.1"), ["21.1.265"]);
        assert_eq!(neoforge_versions(x, "26.1.2"), ["26.1.2.125"]);
        assert!(neoforge_versions(x, "n'importe quoi").is_empty());
    }

    #[test]
    fn neoforge_prefixes() {
        assert_eq!(neoforge_prefix("1.21").as_deref(), Some("21.0."));
        assert_eq!(neoforge_prefix("1.20.1").as_deref(), Some("20.1."));
        assert_eq!(neoforge_prefix("26.3").as_deref(), Some("26.3.0."));
        assert_eq!(neoforge_prefix("1.x"), None);
    }

    #[test]
    fn forge_promotions_recommended_then_latest() {
        let j = r#"{"homepage":"h","promos":{"26.3-latest":"66.1.2","26.3-recommended":"66.1.0","1.20.1-latest":"47.4.0"}}"#;
        assert_eq!(forge_versions(j, "26.3"), ["66.1.0", "66.1.2"]);
        assert_eq!(forge_versions(j, "1.20.1"), ["47.4.0"]);
        assert!(forge_versions(j, "9.9").is_empty());
        assert_eq!(forge_versions(r#"{"promos":{"1.1-latest":"1","1.1-recommended":"1"}}"#, "1.1"), ["1"]);
    }
}
