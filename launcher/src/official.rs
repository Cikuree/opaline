//! Second mode de lancement : Opaline prépare une version et un profil dans le dossier `.minecraft` du Launcher Minecraft
//! officiel, qui connecte le compte et lance le jeu. Aucune action destructive : `launcher_profiles.json` n'est modifié que
//! par fusion (sauvegarde unique, écriture atomique) et une version existante d'un autre outil n'est jamais écrasée.
use crate::config::{Dirs, dev_env};
use crate::fabric::FabricLib;
use crate::safe::{Hash, safe_join};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

/// Famille du paquet Microsoft Store du Launcher Minecraft.
pub const STORE_FAMILY: &str = "Microsoft.4297127D64EC6_8wekyb3d8bbwe";
const MANAGED: &str = "opaline.managed";
const NEED_LAUNCHER: &str = "Installe et ouvre une fois le Launcher Minecraft";

pub fn profile_key(pack_id: &str) -> String {
    format!("opaline-{pack_id}")
}

/// `.minecraft` selon l'OS : `data_dir` = `%APPDATA%` (Windows) ou `~/Library/Application Support` (macOS).
pub fn minecraft_dir_for(os: &str, data_dir: &Path, home: &Path) -> PathBuf {
    match os {
        "windows" => data_dir.join(".minecraft"),
        "macos" => data_dir.join("minecraft"),
        _ => home.join(".minecraft"),
    }
}

/// Dossier `.minecraft` de l'utilisateur (surcharge de dev : `OPALINE_MINECRAFT_DIR`).
pub fn minecraft_dir() -> Option<PathBuf> {
    if let Some(p) = dev_env("OPALINE_MINECRAFT_DIR") {
        return Some(PathBuf::from(p));
    }
    let b = directories::BaseDirs::new()?;
    Some(minecraft_dir_for(std::env::consts::OS, b.data_dir(), b.home_dir()))
}

/// Le dossier doit déjà contenir `launcher_profiles.json` (créé par le Launcher au premier démarrage).
pub fn profiles_path(mc: &Path) -> Result<PathBuf> {
    let p = mc.join("launcher_profiles.json");
    if !p.is_file() {
        bail!("{NEED_LAUNCHER} (launcher_profiles.json introuvable)");
    }
    Ok(p)
}

/// Date ISO 8601 UTC (`2026-10-31T12:00:00.000Z`) à partir de secondes Unix.
pub fn iso_utc(secs: u64) -> String {
    let (days, rem) = ((secs / 86400) as i64, secs % 86400);
    // jours depuis 1970 -> date civile (algorithme de H. Hinnant)
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.000Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

pub struct ProfileSpec<'a> {
    pub name: &'a str,
    pub version_id: &'a str,
    pub game_dir: &'a Path,
    pub java_args: &'a str,
    /// Data URL PNG ou nom d'icône du Launcher.
    pub icon: &'a str,
    /// Horodatage ISO (`created` si le profil est nouveau, `lastUsed` toujours).
    pub now: &'a str,
}

/// Ajoute ou met à jour le profil `key` ; tout le reste du document (autres profils, réglages, clés inconnues) est conservé.
/// Mise à jour idempotente : `created` et `icon` d'un profil existant sont gardés.
pub fn merge_profile(doc: &mut Value, key: &str, p: &ProfileSpec) -> Result<()> {
    let root = doc.as_object_mut().context("launcher_profiles.json n'est pas un objet JSON")?;
    let profiles =
        root.entry("profiles").or_insert_with(|| json!({})).as_object_mut().context("« profiles » n'est pas un objet dans launcher_profiles.json")?;
    let o = profiles
        .entry(key)
        .or_insert_with(|| json!({ "created": p.now, "icon": p.icon }))
        .as_object_mut()
        .context("le profil Opaline existant n'est pas un objet")?;
    for (k, v) in [
        ("name", p.name),
        ("type", "custom"),
        ("lastVersionId", p.version_id),
        ("gameDir", &p.game_dir.to_string_lossy()),
        ("javaArgs", p.java_args),
        ("lastUsed", p.now),
    ] {
        o.insert(k.into(), json!(v));
    }
    for (k, v) in [("created", p.now), ("icon", p.icon)] {
        o.entry(k).or_insert_with(|| json!(v));
    }
    Ok(())
}

pub fn read_profiles(path: &Path) -> Result<Value> {
    let bytes = fs::read(path).with_context(|| format!("lecture de {}", path.display()))?;
    serde_json::from_slice(&bytes).context("launcher_profiles.json illisible : Opaline n'y touche pas")
}

/// Sauvegarde `launcher_profiles.json.opaline.bak` (une seule fois), puis écriture atomique (`.tmp` + renommage).
pub fn write_profiles(path: &Path, doc: &Value) -> Result<()> {
    let bak = path.with_file_name("launcher_profiles.json.opaline.bak");
    if !bak.exists() {
        fs::copy(path, &bak).context("sauvegarde de launcher_profiles.json")?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(doc)?)?;
    fs::rename(&tmp, path).context("écriture de launcher_profiles.json")
}

fn atomic_copy(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst.parent().context("chemin sans dossier parent")?)?;
    let tmp = dst.with_extension("opaline-tmp");
    fs::copy(src, &tmp)?;
    fs::rename(&tmp, dst)?;
    Ok(())
}

fn sha1_ok(p: &Path, sha1: &str) -> bool {
    Hash::new("sha1", sha1).and_then(|h| h.matches_file(p)).unwrap_or(false)
}

/// Copie `src` vers `dst` si `dst` manque, ou (sha1 connu) ne correspond pas alors que `src` correspond.
/// Sans sha1, un fichier déjà présent est laissé tel quel. Renvoie vrai si une copie a eu lieu.
pub fn sync_library(src: &Path, dst: &Path, sha1: Option<&str>) -> Result<bool> {
    if !src.is_file() {
        return Ok(false);
    }
    let need = match sha1.filter(|s| !s.is_empty()) {
        Some(h) => !sha1_ok(dst, h) && sha1_ok(src, h),
        None => !dst.exists(),
    };
    if need {
        atomic_copy(src, dst)?;
    }
    Ok(need)
}

/// Copie les libraries du loader déjà présentes dans `shared/libraries` vers `.minecraft/libraries` (celles issues des
/// processors Forge / NeoForge n'ont pas d'URL : le Launcher ne peut pas les télécharger).
pub fn sync_libraries(src_libs: &Path, dst_libs: &Path, libs: &[FabricLib]) -> Result<usize> {
    let mut n = 0;
    for l in libs {
        let rel = l.rel_path()?;
        n += usize::from(sync_library(&safe_join(src_libs, &rel)?, &safe_join(dst_libs, &rel)?, l.sha1.as_deref())?);
    }
    Ok(n)
}

/// Installe `versions/<id>/<id>.json` (et le jar du loader s'il existe) dans `.minecraft`. Une version qui existe déjà avec un
/// autre contenu n'est remplacée que si Opaline l'a écrite (marqueur `opaline.managed`) : sinon erreur.
pub fn install_version(dirs: &Dirs, mc: &Path, id: &str, libs: &[FabricLib]) -> Result<()> {
    crate::safe::safe_file_name(id)?;
    let json_src = fs::read(dirs.version_json(id)).with_context(|| format!("version {id} absente : lance d'abord l'installation du pack"))?;
    let vdir = mc.join("versions").join(id);
    let json_dst = vdir.join(format!("{id}.json"));
    let managed = vdir.join(MANAGED);
    match fs::read(&json_dst) {
        Ok(old) if old == json_src => {}
        Ok(_) if !managed.exists() => {
            bail!("une version « {id} » d'un autre outil existe déjà dans .minecraft avec un contenu différent : Opaline ne la modifie pas")
        }
        _ => {
            fs::create_dir_all(&vdir)?;
            let tmp = json_dst.with_extension("opaline-tmp");
            fs::write(&tmp, &json_src)?;
            fs::rename(&tmp, &json_dst)?;
        }
    }
    if !managed.exists() {
        fs::write(&managed, "Version écrite par Opaline.\n")?;
    }
    // Forge / NeoForge : jar client sous le nom du loader (copie faite à l'installation, `${version_name}.jar`).
    let (jar_src, jar_dst) = (dirs.client_jar(id), vdir.join(format!("{id}.jar")));
    if jar_src.is_file() && fs::metadata(&jar_src)?.len() != fs::metadata(&jar_dst).map(|m| m.len()).unwrap_or(0) {
        atomic_copy(&jar_src, &jar_dst)?;
    }
    sync_libraries(&dirs.libraries(), &mc.join("libraries"), libs)?;
    Ok(())
}

/// Ouvre le Launcher Minecraft. Faux si introuvable (le profil est écrit quand même).
pub fn open_launcher() -> bool {
    use std::process::{Command, Stdio};
    let spawn = |mut c: Command| c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().is_ok();
    if cfg!(windows) {
        let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
        if local.is_some_and(|l| l.join("Packages").join(STORE_FAMILY).is_dir()) {
            let mut c = Command::new("explorer.exe");
            c.arg(format!("shell:AppsFolder\\{STORE_FAMILY}!Minecraft"));
            if spawn(c) {
                return true;
            }
        }
        // shortcut: seuls les deux emplacements usuels de l'installeur classique, pas de lecture du registre.
        return ["ProgramFiles(x86)", "ProgramFiles"]
            .iter()
            .filter_map(std::env::var_os)
            .map(|p| PathBuf::from(p).join("Minecraft Launcher").join("MinecraftLauncher.exe"))
            .find(|e| e.is_file())
            .is_some_and(|e| spawn(Command::new(e)));
    }
    // shortcut: macOS sans ouverture automatique (le profil est écrit, l'utilisateur ouvre le Launcher).
    cfg!(target_os = "linux") && spawn(Command::new("minecraft-launcher"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("opaline-off-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn spec<'a>(now: &'a str, dir: &'a Path) -> ProfileSpec<'a> {
        ProfileSpec { name: "Opaline · Classroom Run", version_id: "fabric-loader-0.20.1-26.3", game_dir: dir, java_args: "-Xmx4096M", icon: "Furnace", now }
    }

    #[test]
    fn minecraft_dir_per_os() {
        let (d, h) = (Path::new("/r"), Path::new("/h"));
        assert_eq!(minecraft_dir_for("windows", d, h), d.join(".minecraft"));
        assert_eq!(minecraft_dir_for("macos", d, h), d.join("minecraft"));
        assert_eq!(minecraft_dir_for("linux", d, h), h.join(".minecraft"));
    }

    #[test]
    fn missing_profiles_file_is_a_readable_error() {
        let d = tmp("missing");
        assert!(profiles_path(&d).unwrap_err().to_string().contains("Installe et ouvre une fois le Launcher Minecraft"));
        fs::write(d.join("launcher_profiles.json"), "{}").unwrap();
        assert!(profiles_path(&d).is_ok());
    }

    #[test]
    fn iso_dates() {
        assert_eq!(iso_utc(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(iso_utc(1_782_864_000 + 3661), "2026-07-01T01:01:01.000Z");
        assert_eq!(iso_utc(951_782_400), "2000-02-29T00:00:00.000Z");
    }

    #[test]
    fn merge_keeps_everything_else_and_is_idempotent() {
        let mut doc: Value = serde_json::from_str(r#"{"profiles":{"abc":{"name":"x","custom":1}},"settings":{"a":true},"version":6,"inconnu":[1]}"#).unwrap();
        let before = doc.clone();
        let dir = Path::new("/inst");
        merge_profile(&mut doc, "opaline-classroom-run", &spec("2026-01-01T00:00:00.000Z", dir)).unwrap();
        let p = &doc["profiles"]["opaline-classroom-run"];
        assert_eq!(
            (p["type"].as_str(), p["lastVersionId"].as_str(), p["javaArgs"].as_str()),
            (Some("custom"), Some("fabric-loader-0.20.1-26.3"), Some("-Xmx4096M"))
        );
        assert_eq!(p["created"], "2026-01-01T00:00:00.000Z");
        for k in ["settings", "version", "inconnu"] {
            assert_eq!(doc[k], before[k]);
        }
        assert_eq!(doc["profiles"]["abc"], before["profiles"]["abc"]);
        // deuxième passe plus tardive : created conservé, lastUsed mis à jour, un seul profil Opaline
        merge_profile(&mut doc, "opaline-classroom-run", &spec("2026-02-02T00:00:00.000Z", dir)).unwrap();
        let p = &doc["profiles"]["opaline-classroom-run"];
        assert_eq!((p["created"].as_str(), p["lastUsed"].as_str()), (Some("2026-01-01T00:00:00.000Z"), Some("2026-02-02T00:00:00.000Z")));
        assert_eq!(doc["profiles"].as_object().unwrap().len(), 2);
    }

    #[test]
    fn merge_rejects_non_objects() {
        let mut doc = json!([1]);
        assert!(merge_profile(&mut doc, "k", &spec("n", Path::new("/i"))).is_err());
        let mut doc = json!({"profiles": 3});
        assert!(merge_profile(&mut doc, "k", &spec("n", Path::new("/i"))).is_err());
    }

    #[test]
    fn unreadable_json_is_an_error_and_file_untouched() {
        let d = tmp("bad");
        let f = d.join("launcher_profiles.json");
        fs::write(&f, "{ pas du json").unwrap();
        assert!(read_profiles(&f).is_err());
        assert_eq!(fs::read_to_string(&f).unwrap(), "{ pas du json");
    }

    #[test]
    fn write_backs_up_once_and_replaces_atomically() {
        let d = tmp("write");
        let f = d.join("launcher_profiles.json");
        fs::write(&f, r#"{"a":1}"#).unwrap();
        write_profiles(&f, &json!({"a": 2})).unwrap();
        write_profiles(&f, &json!({"a": 3})).unwrap();
        assert_eq!(fs::read_to_string(d.join("launcher_profiles.json.opaline.bak")).unwrap(), r#"{"a":1}"#);
        assert_eq!(read_profiles(&f).unwrap()["a"], 3);
        assert!(!d.join("launcher_profiles.json.tmp").exists());
    }

    #[test]
    fn library_copy_uses_sha1() {
        let d = tmp("libs");
        let (src, dst) = (d.join("s.jar"), d.join("o/d.jar"));
        fs::write(&src, b"abc").unwrap();
        let sha = "a9993e364706816aba3e25717850c26c9cd0d89d"; // sha1("abc")
        assert!(sync_library(&src, &dst, Some(sha)).unwrap(), "absent : copié");
        assert!(!sync_library(&src, &dst, Some(sha)).unwrap(), "déjà bon : rien");
        fs::write(&dst, b"zzz").unwrap();
        assert!(sync_library(&src, &dst, Some(sha)).unwrap(), "sha1 différent : remplacé");
        assert_eq!(fs::read(&dst).unwrap(), b"abc");
        fs::write(&dst, b"zzz").unwrap();
        assert!(!sync_library(&src, &dst, None).unwrap(), "sans sha1, existant conservé");
        assert!(!sync_library(&src, &dst, Some("0000000000000000000000000000000000000000")).unwrap(), "source elle-même invalide : jamais propagée");
        assert!(!sync_library(&d.join("absent"), &d.join("x"), None).unwrap());
    }

    #[test]
    fn version_install_never_overwrites_foreign_versions() {
        let d = tmp("ver");
        let (dirs, mc) = (Dirs::new(d.join("opaline")).for_pack("p").unwrap(), d.join("mc"));
        let id = "fabric-loader-0.20.1-26.3";
        fs::create_dir_all(dirs.versions().join(id)).unwrap();
        fs::write(dirs.version_json(id), r#"{"id":"v1"}"#).unwrap();
        let foreign = mc.join("versions").join(id);
        fs::create_dir_all(&foreign).unwrap();
        fs::write(foreign.join(format!("{id}.json")), r#"{"id":"autre"}"#).unwrap();
        assert!(install_version(&dirs, &mc, id, &[]).is_err());
        assert_eq!(fs::read_to_string(foreign.join(format!("{id}.json"))).unwrap(), r#"{"id":"autre"}"#);
        // même contenu : accepté, marqué, puis mise à jour autorisée
        fs::write(foreign.join(format!("{id}.json")), r#"{"id":"v1"}"#).unwrap();
        install_version(&dirs, &mc, id, &[]).unwrap();
        fs::write(dirs.version_json(id), r#"{"id":"v2"}"#).unwrap();
        install_version(&dirs, &mc, id, &[]).unwrap();
        assert_eq!(fs::read_to_string(foreign.join(format!("{id}.json"))).unwrap(), r#"{"id":"v2"}"#);
    }
}
