//! Mise à jour du launcher : détection (`launcher.json` de la dernière release), téléchargement vérifié de l'installeur
//! (sha256 obligatoire, URL limitée aux releases d'Opaline) et application (NSIS sous Windows, remplacement de l'AppImage sous Linux).
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Seules les releases du dépôt d'Opaline sont acceptées pour un téléchargement automatique.
pub const RELEASE_PREFIX: &str = "https://github.com/Cikuree/opaline/releases/download/";

#[derive(Deserialize, Debug, Clone)]
pub struct LauncherInfo {
    pub version: String,
    pub windows: Option<String>,
    pub linux: Option<String>,
    /// sha256 (hex) des installeurs ; absents (anciennes releases) : pas de mise à jour automatique.
    #[serde(rename = "windowsSha256")]
    pub windows_sha256: Option<String>,
    #[serde(rename = "linuxSha256")]
    pub linux_sha256: Option<String>,
    /// Hash de l'index du pack publié (celui de `pack.toml`) ; sert à détecter « le pack a changé ».
    #[serde(rename = "pack_hash")]
    pub pack: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Update {
    pub version: String,
    pub url: String,
    pub sha256: Option<String>,
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
    let (url, sha) = if cfg!(windows) { (&info.windows, &info.windows_sha256) } else { (&info.linux, &info.linux_sha256) };
    Some(Update { version: info.version.clone(), url: url.clone()?, sha256: sha.clone() })
}

pub fn fetch_info(client: &reqwest::blocking::Client, url: &str) -> Result<LauncherInfo> {
    crate::download::get_json(client, url)
}

/// Le pack publié diffère de l'installé. Sans hash publié : faux. Installation ancienne sans hash : vrai.
pub fn pack_changed(info: &LauncherInfo, installed_index_hash: Option<&str>) -> bool {
    match (info.pack.as_deref().map(str::trim).filter(|p| !p.is_empty()), installed_index_hash) {
        (Some(remote), Some(local)) => !remote.eq_ignore_ascii_case(local),
        (Some(_), None) => true,
        (None, _) => false,
    }
}

/// Erreur réseau ou JSON invalide : silencieux (trace seulement avec CLASSROOM_DEBUG=1).
pub fn check(client: &reqwest::blocking::Client, url: &str, current: &str) -> Option<Update> {
    match fetch_info(client, url) {
        Ok(info) => pick(&info, current),
        Err(e) => {
            if crate::config::dev_env("CLASSROOM_DEBUG").is_some() {
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

/// Chemin de l'AppImage en cours d'exécution (Linux seulement).
fn appimage_path() -> Option<PathBuf> {
    if cfg!(windows) { None } else { std::env::var_os("APPIMAGE").map(PathBuf::from).filter(|p| p.is_file()) }
}

/// Le bouton « Mettre à jour » est proposé : hash publié, et (Windows) ou (Linux lancé depuis une AppImage).
pub fn can_auto(u: &Update) -> bool {
    check_download(u).is_ok() && (cfg!(windows) || appimage_path().is_some())
}

/// Valide URL (releases d'Opaline, nom de fichier simple) et sha256 ; renvoie `(nom de fichier, hash)`.
fn check_download(u: &Update) -> Result<(String, crate::safe::Hash)> {
    let rest = u.url.strip_prefix(RELEASE_PREFIX).context("URL de mise à jour refusée (hors des releases d'Opaline)")?;
    anyhow::ensure!(!rest.contains(['?', '#', '\\']) && !rest.contains(".."), "URL de mise à jour refusée");
    let name = crate::safe::safe_file_name(rest.rsplit('/').next().unwrap_or(""))?.to_string();
    let hex = u.sha256.as_deref().map(str::trim).filter(|h| !h.is_empty()).context("pas de sha256 publié : mise à jour manuelle")?;
    anyhow::ensure!(hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit()), "sha256 publié invalide");
    Ok((name, crate::safe::Hash::new("sha256", hex)?))
}

/// Télécharge l'installeur dans `dir` (reprise, taille bornée à celle annoncée, hash vérifié).
/// `progress(octets reçus, taille totale si connue)` est appelé environ toutes les 150 ms.
pub fn download(client: &reqwest::blocking::Client, u: &Update, dir: &Path, mut progress: impl FnMut(u64, Option<u64>)) -> Result<PathBuf> {
    let (name, hash) = check_download(u)?;
    let dest = dir.join(name);
    let size = client.head(&u.url).send().ok().filter(|r| r.status().is_success()).and_then(|r| r.content_length()).filter(|n| *n > 0);
    let job = crate::download::Job { url: u.url.clone(), dest: dest.clone(), hash, size };
    let part = crate::download::part_path(&dest);
    std::thread::scope(|sc| {
        let h = sc.spawn(|| crate::download::fetch(client, &job));
        while !h.is_finished() {
            progress(std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0), size);
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
        h.join().unwrap_or_else(|_| Err(anyhow::anyhow!("erreur interne")))
    })?;
    let n = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
    progress(n, Some(n));
    Ok(dest)
}

/// Remplace `target` par `new` de façon atomique (copie voisine `.tmp`, droits d'exécution, renommage).
fn replace_file_atomic(new: &Path, target: &Path) -> Result<()> {
    let mut tmp = target.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let r = (|| -> Result<()> {
        std::fs::copy(new, &tmp).with_context(|| format!("copie vers {}", tmp.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?;
        }
        std::fs::rename(&tmp, target).with_context(|| format!("remplacement de {}", target.display()))
    })();
    if r.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    r
}

/// Windows : installeur NSIS en mode passif (`/P`), relance de l'app (`/R`), sans désinstallation préalable (`/UPDATE`).
pub fn installer_args() -> [&'static str; 3] {
    ["/P", "/R", "/UPDATE"]
}

/// Applique l'installeur téléchargé. L'appelant doit quitter Opaline juste après (exe libéré ; la nouvelle version est relancée).
pub fn apply(installer: &Path) -> Result<()> {
    if cfg!(windows) {
        std::process::Command::new(installer).args(installer_args()).spawn().context("lancement de l'installeur")?;
        return Ok(());
    }
    let Some(target) = appimage_path() else { bail!("mise à jour automatique impossible hors AppImage") };
    replace_file_atomic(installer, &target)?;
    let _ = std::fs::remove_file(installer);
    std::process::Command::new(&target).spawn().context("relance d'Opaline")?;
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
        let i: LauncherInfo = serde_json::from_str(r#"{"version":"0.3.0","windows":"https://w","linux":"https://l","pack_hash":"1"}"#).unwrap();
        let u = pick(&i, "0.1.0").unwrap();
        assert_eq!(u.url, if cfg!(windows) { "https://w" } else { "https://l" });
        assert!(pick(&i, "0.3.0").is_none());
    }

    #[test]
    fn detects_changed_pack() {
        let i = |p: Option<&str>| LauncherInfo { version: "1.0.0".into(), windows: None, linux: None, windows_sha256: None, linux_sha256: None, pack: p.map(str::to_string) };
        assert!(!pack_changed(&i(Some("ABC")), Some("abc")));
        assert!(pack_changed(&i(Some("abd")), Some("abc")));
        assert!(pack_changed(&i(Some("abd")), None));
        assert!(!pack_changed(&i(None), Some("abc")));
    }

    fn upd(url: &str, sha: Option<&str>) -> Update {
        Update { version: "1.0.0".into(), url: url.into(), sha256: sha.map(str::to_string) }
    }
    const H: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    const OK: &str = "https://github.com/Cikuree/opaline/releases/download/v1.0.0/Opaline_1.0.0_x64-setup.exe";

    #[test]
    fn parses_info_with_and_without_hash() {
        let a: LauncherInfo = serde_json::from_str(r#"{"version":"1.0.0","windows":"w","linux":"l","windowsSha256":"aa","linuxSha256":"bb"}"#).unwrap();
        assert_eq!((a.windows_sha256.as_deref(), a.linux_sha256.as_deref()), (Some("aa"), Some("bb")));
        let b: LauncherInfo = serde_json::from_str(r#"{"version":"1.0.0","windows":"w","linux":"l"}"#).unwrap();
        assert!(b.windows_sha256.is_none() && pick(&b, "0.1.0").unwrap().sha256.is_none());
    }

    #[test]
    fn download_validation() {
        assert!(check_download(&upd(OK, Some(H))).is_ok());
        assert!(check_download(&upd(OK, None)).is_err(), "sans hash : refus");
        assert!(check_download(&upd(OK, Some("zz"))).is_err());
        for bad in ["https://evil.example/Cikuree/opaline/releases/download/x.exe", "http://github.com/Cikuree/opaline/releases/download/x.exe",
            "https://github.com/Cikuree/opaline-evil/releases/download/x.exe", "https://github.com/Cikuree/opaline/releases/download/../../x.exe",
            "https://github.com/Cikuree/opaline/releases/download/v1/x.exe?a=b", "https://github.com/Cikuree/opaline/releases/download/"] {
            assert!(check_download(&upd(bad, Some(H))).is_err(), "{bad}");
        }
    }

    #[test]
    fn download_refuses_before_network() {
        let c = reqwest::blocking::Client::new();
        let d = std::env::temp_dir();
        assert!(download(&c, &upd("https://evil.example/x.exe", Some(H)), &d, |_, _| {}).is_err());
        assert!(download(&c, &upd(OK, None), &d, |_, _| {}).is_err());
    }

    #[test]
    fn invalid_hash_is_an_error_and_removes_file() {
        use std::io::{Read, Write};
        let d = std::env::temp_dir().join(format!("opaline-upd-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = l.accept() {
                let _ = s.read(&mut [0u8; 2048]);
                let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nabc");
            }
        });
        // `download` refuse les URL locales : on exerce `fetch` avec le hash validé par `check_download`.
        let (_, hash) = check_download(&upd(OK, Some(H))).unwrap();
        let dest = d.join("x.exe");
        let job = crate::download::Job { url: format!("http://127.0.0.1:{port}/x.exe"), dest: dest.clone(), hash, size: Some(3) };
        let e = crate::download::fetch(&reqwest::blocking::Client::new(), &job).unwrap_err();
        assert!(format!("{e}").contains("hash invalide") && !dest.exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn replaces_appimage_atomically() {
        let d = std::env::temp_dir().join(format!("opaline-appimg-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let (new, target) = (d.join("new"), d.join("Opaline.AppImage"));
        std::fs::write(&new, b"v2").unwrap();
        std::fs::write(&target, b"v1").unwrap();
        replace_file_atomic(&new, &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"v2");
        assert!(!d.join("Opaline.AppImage.tmp").exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&target).unwrap().permissions().mode() & 0o111, 0o111);
        }
        assert!(replace_file_atomic(&d.join("absent"), &target).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"v2", "cible intacte en cas d'échec");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn windows_installer_is_passive_with_relaunch() {
        assert_eq!(installer_args(), ["/P", "/R", "/UPDATE"]);
    }

    #[test]
    fn refuses_non_https() {
        assert!(open_url("file:///C:/x.exe").is_err());
    }
}
