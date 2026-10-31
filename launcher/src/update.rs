//! Vérification de mise à jour du launcher (`launcher.json` de la dernière release). Pas d'auto-remplacement.
use anyhow::Result;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct LauncherInfo {
    pub version: String,
    pub windows: Option<String>,
    pub linux: Option<String>,
    /// Version du pack (informative).
    #[allow(dead_code)]
    pub pack: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Update {
    pub version: String,
    pub url: String,
}

/// `X.Y.Z` (préfixe `v`, suffixes `-pre` / `+build` ignorés).
pub fn parse_semver(s: &str) -> Option<(u64, u64, u64)> {
    let core = s.trim().trim_start_matches('v').split(['-', '+']).next()?;
    let mut it = core.split('.');
    let v = (it.next()?.parse().ok()?, it.next()?.parse().ok()?, it.next()?.parse().ok()?);
    it.next().is_none().then_some(v)
}

pub fn is_newer(remote: &str, local: &str) -> bool {
    matches!((parse_semver(remote), parse_semver(local)), (Some(r), Some(l)) if r > l)
}

/// Mise à jour disponible pour l'OS courant, si `info.version` > `current`.
pub fn pick(info: &LauncherInfo, current: &str) -> Option<Update> {
    if !is_newer(&info.version, current) {
        return None;
    }
    let url = if cfg!(windows) { info.windows.as_ref() } else { info.linux.as_ref() }?;
    Some(Update { version: info.version.clone(), url: url.clone() })
}

/// Erreur réseau ou JSON invalide : silencieux (trace seulement avec CLASSROOM_DEBUG=1).
pub fn check(client: &reqwest::blocking::Client, url: &str, current: &str) -> Option<Update> {
    let r: Result<LauncherInfo> = crate::download::get_json(client, url);
    match r {
        Ok(info) => pick(&info, current),
        Err(e) => {
            if std::env::var_os("CLASSROOM_DEBUG").is_some() {
                eprintln!("[debug] vérification de mise à jour : {e:#}");
            }
            None
        }
    }
}

/// Ouvre une URL https dans le navigateur par défaut (rien d'autre n'est accepté).
pub fn open_url(url: &str) -> Result<()> {
    anyhow::ensure!(url.starts_with("https://"), "URL refusée (https uniquement)");
    open::that(url)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_compare() {
        assert!(is_newer("0.2.0", "0.1.9") && is_newer("v1.0.0", "0.9.9") && is_newer("0.1.10", "0.1.9"));
        assert!(!is_newer("0.1.0", "0.1.0") && !is_newer("0.0.9", "0.1.0") && !is_newer("x", "0.1.0"));
        assert_eq!(parse_semver("1.2.3-beta+5"), Some((1, 2, 3)));
        assert_eq!(parse_semver("1.2"), None);
    }

    #[test]
    fn picks_os_url_only_when_newer() {
        let i: LauncherInfo = serde_json::from_str(r#"{"version":"0.3.0","windows":"https://w","linux":"https://l","pack":"1"}"#).unwrap();
        let u = pick(&i, "0.1.0").unwrap();
        assert_eq!(u.url, if cfg!(windows) { "https://w" } else { "https://l" });
        assert!(pick(&i, "0.3.0").is_none());
    }

    #[test]
    fn refuses_non_https() {
        assert!(open_url("file:///C:/x.exe").is_err());
    }
}
