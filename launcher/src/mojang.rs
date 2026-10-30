//! JSON de version Mojang : libraries (règles OS), arguments, assets.
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Debug)]
pub struct VersionManifest {
    pub versions: Vec<ManifestVersion>,
}
#[derive(Deserialize, Debug)]
pub struct ManifestVersion {
    pub id: String,
    pub url: String,
    pub sha1: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Artifact {
    pub path: String,
    pub sha1: String,
    pub url: String,
}
#[derive(Deserialize, Debug, Clone)]
pub struct FileRef {
    pub sha1: String,
    pub url: String,
}
#[derive(Deserialize, Debug, Clone)]
pub struct AssetIndexRef {
    pub id: String,
    pub sha1: String,
    pub url: String,
}
#[derive(Deserialize, Debug, Clone)]
pub struct JavaVersion {
    #[serde(rename = "majorVersion")]
    pub major_version: u32,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct OsRule {
    pub name: Option<String>,
    pub arch: Option<String>,
}
#[derive(Deserialize, Debug, Clone)]
pub struct Rule {
    pub action: String,
    pub os: Option<OsRule>,
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct LibDownloads {
    pub artifact: Option<Artifact>,
}
#[derive(Deserialize, Debug, Clone)]
pub struct Library {
    pub name: String,
    pub downloads: Option<LibDownloads>,
    pub rules: Option<Vec<Rule>>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum ArgValue {
    One(String),
    Many(Vec<String>),
}
#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum Arg {
    Plain(String),
    Ruled { rules: Vec<Rule>, value: ArgValue },
}
#[derive(Deserialize, Debug, Clone, Default)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<Arg>,
    #[serde(default)]
    pub jvm: Vec<Arg>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct VersionJson {
    pub id: String,
    #[serde(rename = "mainClass")]
    pub main_class: String,
    pub arguments: Arguments,
    #[serde(rename = "assetIndex")]
    pub asset_index: AssetIndexRef,
    pub downloads: VersionDownloads,
    #[serde(rename = "javaVersion")]
    pub java_version: JavaVersion,
    #[serde(default)]
    pub libraries: Vec<Library>,
}
#[derive(Deserialize, Debug, Clone)]
pub struct VersionDownloads {
    pub client: FileRef,
}

#[derive(Deserialize, Debug)]
pub struct AssetIndex {
    pub objects: HashMap<String, AssetObject>,
}
#[derive(Deserialize, Debug)]
pub struct AssetObject {
    pub hash: String,
}

/// Environnement d'évaluation des règles.
#[derive(Clone, Debug)]
pub struct Env {
    pub os: &'static str,   // "windows" | "osx" | "linux"
    pub arch: &'static str, // "x86_64" | "arm64" | "x86"
    pub features: HashMap<String, bool>,
}

impl Env {
    pub fn current(features: &[(&str, bool)]) -> Env {
        Env {
            os: if cfg!(windows) {
                "windows"
            } else if cfg!(target_os = "macos") {
                "osx"
            } else {
                "linux"
            },
            arch: match std::env::consts::ARCH {
                "x86" => "x86",
                "aarch64" => "arm64",
                _ => "x86_64",
            },
            features: features.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        }
    }
}

fn rule_matches(r: &Rule, env: &Env) -> bool {
    if let Some(os) = &r.os {
        if os.name.as_deref().is_some_and(|n| n != env.os) {
            return false;
        }
        // Mojang n'utilise `arch` que pour "x86" (32 bits).
        if os.arch.as_deref().is_some_and(|a| a != env.arch) {
            return false;
        }
    }
    if let Some(feats) = &r.features
        && feats.iter().any(|(k, want)| env.features.get(k).copied().unwrap_or(false) != *want)
    {
        return false;
    }
    true
}

/// Sans règle : autorisé. Sinon la dernière règle qui correspond décide (défaut : refusé).
pub fn rules_allow(rules: &Option<Vec<Rule>>, env: &Env) -> bool {
    let Some(rules) = rules else { return true };
    let mut allowed = false;
    for r in rules.iter().filter(|r| rule_matches(r, env)) {
        allowed = r.action == "allow";
    }
    allowed
}

/// Libraries applicables, avec leur artefact. Les natives de 26.3 sont de simples artefacts
/// `natives-<os>` filtrés par règle (pas d'extraction) ; LWJGL les extrait lui-même.
pub fn applicable_libraries<'a>(libs: &'a [Library], env: &Env) -> Vec<(&'a Library, &'a Artifact)> {
    libs.iter()
        .filter(|l| rules_allow(&l.rules, env))
        .filter_map(|l| Some((l, l.downloads.as_ref()?.artifact.as_ref()?)))
        .collect()
}

/// Aplatit les arguments (règles évaluées) puis substitue les `${var}` ; inconnues laissées telles quelles.
pub fn expand_args(args: &[Arg], env: &Env, vars: &HashMap<&str, String>) -> Vec<String> {
    let mut out = Vec::new();
    for a in args {
        match a {
            Arg::Plain(s) => out.push(substitute(s, vars)),
            Arg::Ruled { rules, value } => {
                if rules_allow(&Some(rules.clone()), env) {
                    match value {
                        ArgValue::One(s) => out.push(substitute(s, vars)),
                        ArgValue::Many(v) => out.extend(v.iter().map(|s| substitute(s, vars))),
                    }
                }
            }
        }
    }
    out
}

pub fn substitute(s: &str, vars: &HashMap<&str, String>) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find('}') {
            Some(end) => {
                match vars.get(&after[..end]) {
                    Some(v) => out.push_str(v),
                    None => out.push_str(&rest[start..start + 3 + end]),
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(os: &'static str, feats: &[(&str, bool)]) -> Env {
        Env { os, arch: "x86_64", features: feats.iter().map(|(k, v)| (k.to_string(), *v)).collect() }
    }

    fn lib(json: &str) -> Library {
        serde_json::from_str(json).unwrap()
    }

    const LIB: &str = r#"{"name":"x:y:1:natives-windows","rules":[{"action":"allow","os":{"name":"windows"}}],
        "downloads":{"artifact":{"path":"x/y.jar","sha1":"aa","size":1,"url":"http://u/y.jar"}}}"#;

    #[test]
    fn os_rules_filter_libraries() {
        let libs = [
            lib(LIB),
            lib(r#"{"name":"plain","downloads":{"artifact":{"path":"p.jar","sha1":"b","size":1,"url":"u"}}}"#),
        ];
        assert_eq!(applicable_libraries(&libs, &env("windows", &[])).len(), 2);
        let linux = applicable_libraries(&libs, &env("linux", &[]));
        assert_eq!(linux.len(), 1);
        assert_eq!(linux[0].0.name, "plain");
    }

    #[test]
    fn last_matching_rule_wins() {
        let l = lib(r#"{"name":"n","rules":[{"action":"allow"},{"action":"disallow","os":{"name":"osx"}}]}"#);
        assert!(rules_allow(&l.rules, &env("windows", &[])));
        assert!(!rules_allow(&l.rules, &env("osx", &[])));
    }

    #[test]
    fn x86_arch_rule_does_not_match_x64() {
        let l = lib(r#"{"name":"n","rules":[{"action":"allow","os":{"arch":"x86"}}]}"#);
        assert!(!rules_allow(&l.rules, &env("windows", &[])));
    }

    fn args() -> Vec<Arg> {
        serde_json::from_str(
            r#"["--username","${auth_player_name}","pre-${x}-post","${unknown}",
            {"rules":[{"action":"allow","features":{"is_quick_play_multiplayer":true}}],"value":["--quickPlayMultiplayer","${quickPlayMultiplayer}"]},
            {"rules":[{"action":"allow","features":{"is_demo_user":true}}],"value":"--demo"},
            {"rules":[{"action":"allow","os":{"name":"windows"}}],"value":"-Dwin"}]"#,
        )
        .unwrap()
    }

    #[test]
    fn substitutes_and_evaluates_features() {
        let vars = HashMap::from([
            ("auth_player_name", "Bob".to_string()),
            ("x", "1".to_string()),
            ("quickPlayMultiplayer", "h:25565".to_string()),
        ]);
        let out = expand_args(&args(), &env("windows", &[("is_quick_play_multiplayer", true)]), &vars);
        assert_eq!(out, ["--username", "Bob", "pre-1-post", "${unknown}", "--quickPlayMultiplayer", "h:25565", "-Dwin"]);
        let out = expand_args(&args(), &env("linux", &[]), &vars);
        assert_eq!(out, ["--username", "Bob", "pre-1-post", "${unknown}"]);
    }

    #[test]
    fn substitute_edge_cases() {
        let vars = HashMap::from([("a", "A".to_string())]);
        assert_eq!(substitute("${a}${a}", &vars), "AA");
        assert_eq!(substitute("${a", &vars), "${a");
        assert_eq!(substitute("no vars", &vars), "no vars");
    }
}
