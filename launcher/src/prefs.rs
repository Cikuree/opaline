//! Réglages d'instance : copie d'`options.txt` d'un pack vers un autre (par catégories) et shaders Iris.
//! Toute écriture passe par la fusion de `perf` : les autres lignes du fichier sont conservées telles quelles.
use crate::config::Dirs;
use crate::perf;
use anyhow::{Context, Result, bail};

/// Catégories proposées à l'utilisateur (identifiants du contrat).
pub const CATEGORIES: [&str; 6] = ["keys", "mouse", "fov", "sound", "language", "accessibility"];

fn in_category(cat: &str, key: &str) -> bool {
    match cat {
        "keys" => key.starts_with("key_"),
        "mouse" => matches!(key, "mouseSensitivity" | "invertYMouse" | "mouseWheelSensitivity" | "discrete_mouse_scroll" | "rawMouseInput"),
        "fov" => matches!(key, "fov" | "fovEffectScale" | "screenEffectScale" | "darknessEffectScale"),
        "sound" => key.starts_with("soundCategory_"),
        "language" => key == "lang",
        "accessibility" => matches!(
            key,
            "narrator"
                | "highContrast"
                | "highContrastBlockOutline"
                | "showSubtitles"
                | "directionalAudio"
                | "textBackgroundOpacity"
                | "backgroundForChatOnly"
                | "chatOpacity"
                | "chatLineSpacing"
                | "chatScale"
                | "chatWidth"
                | "chatHeightFocused"
                | "chatHeightUnfocused"
                | "chatDelay"
                | "notificationDisplayTime"
                | "toggleCrouch"
                | "toggleSprint"
                | "hideLightningFlashes"
                | "damageTiltStrength"
                | "distortionEffectScale"
                | "menuBackgroundBlurriness"
                | "panoramaScrollSpeed"
                | "monochromeLogo"
        ),
        _ => false,
    }
}

/// Lignes `clé:valeur` de `options.txt` retenues par les catégories, sans jamais les clés des modes de performance.
pub fn select(options: &str, cats: &[&str]) -> Vec<(String, String)> {
    options
        .lines()
        .filter_map(|l| l.split_once(':'))
        .filter(|(k, _)| !perf::MANAGED_OPTIONS.contains(k) && cats.iter().any(|c| in_category(c, k)))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/// Garde les 3 dernières sauvegardes `options.txt.<date>.bak` (la date dans le nom trie dans l'ordre du temps).
fn prune_bak(instance: &std::path::Path) {
    let mut v: Vec<String> = std::fs::read_dir(instance)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("options.txt.") && n.ends_with(".bak"))
        .collect();
    v.sort();
    let extra = v.len().saturating_sub(3);
    for n in &v[..extra] {
        let _ = std::fs::remove_file(instance.join(n));
    }
}

/// Copie les réglages des catégories de `from` vers `to` (fusion : le reste de la destination est conservé) ; l'ancien
/// `options.txt` de la destination est gardé dans `options.txt.<date>.bak` (3 au plus). Renvoie le nombre de réglages copiés.
/// L'appelant refuse si le jeu de la destination tourne.
pub fn copy_options(from: &Dirs, to: &Dirs, cats: &[String], now_secs: u64) -> Result<usize> {
    if from.pack == to.pack {
        bail!("choisis deux packs différents");
    }
    if cats.is_empty() || cats.iter().any(|c| !CATEGORIES.contains(&c.as_str())) {
        bail!("catégorie de réglages inconnue");
    }
    if !to.instance().is_dir() {
        bail!("le pack de destination n'est pas installé");
    }
    let src = perf::read_text(&from.instance().join("options.txt")).context("aucun réglage trouvé dans le pack source : lance-le au moins une fois")?;
    let pairs = select(&src, &cats.iter().map(String::as_str).collect::<Vec<_>>());
    if pairs.is_empty() {
        bail!("aucun de ces réglages n'est enregistré dans le pack source");
    }
    let dest = to.instance().join("options.txt");
    if dest.is_file() {
        std::fs::copy(&dest, to.instance().join(format!("options.txt.{}.bak", crate::backup::stamp(now_secs))))
            .context("sauvegarde d'options.txt impossible : rien n'a été modifié")?;
        prune_bak(&to.instance());
    }
    let kv: Vec<(&str, String)> = pairs.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
    perf::merge_file(&dest, ':', &kv)?;
    Ok(pairs.len())
}

// ---------- Shaders (Iris) ----------

#[derive(serde::Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ShaderView {
    /// Le pack contient Iris : sans lui, la section est masquée.
    pub available: bool,
    /// Contenu de `shaderpacks/` (zips et dossiers), trié.
    pub files: Vec<String>,
    /// Shader actif d'après `config/iris.properties` (`shaderPack=` et `enableShaders=`), `None` : aucun.
    pub active: Option<String>,
}

fn iris_file(dirs: &Dirs) -> std::path::PathBuf {
    dirs.instance().join("config").join("iris.properties")
}

fn shader_files(dirs: &Dirs) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dirs.instance().join("shaderpacks"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()) || e.file_name().to_string_lossy().to_ascii_lowercase().ends_with(".zip"))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    v.sort_by_key(|n| n.to_lowercase());
    v
}

/// Iris est présent si un jar `iris*` figure dans `mods/` (pack ou Mes mods).
fn has_iris(dirs: &Dirs) -> bool {
    std::fs::read_dir(dirs.instance().join("mods")).into_iter().flatten().flatten().any(|e| {
        let n = e.file_name().to_string_lossy().to_ascii_lowercase();
        n.starts_with("iris") && n.ends_with(".jar")
    })
}

pub fn shaders_get(dirs: &Dirs) -> ShaderView {
    let text = perf::read_text(&iris_file(dirs)).unwrap_or_default();
    let files = shader_files(dirs);
    let on = perf::prop(&text, "enableShaders").is_none_or(|v| v != "false");
    let active = perf::prop(&text, "shaderPack").filter(|p| on && !p.is_empty() && p != "(internal)" && files.contains(p));
    ShaderView { available: has_iris(dirs), files, active }
}

/// Choisit le shader (`None` : aucun) dans `iris.properties` par la fusion habituelle. L'appelant refuse si le jeu tourne
/// (Iris réécrit ce fichier en quittant).
pub fn shader_set(dirs: &Dirs, name: Option<&str>) -> Result<()> {
    if !dirs.instance().is_dir() {
        bail!("ce pack n'est pas installé");
    }
    let kv = match name {
        Some(n) => {
            let n = crate::safe::safe_file_name(n)?;
            if !shader_files(dirs).iter().any(|f| f == n) {
                bail!("shader introuvable dans shaderpacks/");
            }
            [("enableShaders", "true".to_string()), ("shaderPack", n.to_string())]
        }
        None => [("enableShaders", "false".to_string()), ("shaderPack", String::new())],
    };
    perf::merge_file(&iris_file(dirs), '=', &kv)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn dirs(tag: &str) -> (Dirs, Dirs) {
        let d = Dirs::new(std::env::temp_dir().join(format!("opaline-prefs-{tag}-{}", std::process::id())));
        let _ = fs::remove_dir_all(&d.root);
        let (a, b) = (d.for_pack("a").unwrap(), d.for_pack("b").unwrap());
        fs::create_dir_all(a.instance()).unwrap();
        fs::create_dir_all(b.instance()).unwrap();
        (a, b)
    }

    const SRC: &str = "version:4000\nkey_key.jump:key.keyboard.x\nmouseSensitivity:0.7\nfov:0.5\nsoundCategory_music:0.2\nlang:fr_fr\nnarrator:1\nrenderDistance:32\nparticles:0\ngraphicsPreset:\"fabulous\"\nresourcePacks:[\"vanilla\"]\n";

    #[test]
    fn categories_select_only_their_keys_and_never_performance_keys() {
        let keys = |c: &[&str]| select(SRC, c).into_iter().map(|p| p.0).collect::<Vec<_>>();
        assert_eq!(keys(&["keys"]), ["key_key.jump"]);
        assert_eq!(keys(&["mouse", "fov"]), ["mouseSensitivity", "fov"]);
        assert_eq!(keys(&["sound", "language", "accessibility"]), ["soundCategory_music", "lang", "narrator"]);
        assert_eq!(keys(&CATEGORIES).len(), 6);
    }

    #[test]
    fn copy_merges_into_destination_keeps_its_other_lines_and_writes_bak() {
        let (a, b) = dirs("copy");
        fs::write(a.instance().join("options.txt"), SRC).unwrap();
        let old = "version:4000\nkey_key.jump:key.keyboard.space\nrenderDistance:6\nparticles:2\nlang:en_us\ngamma:1.0\n";
        fs::write(b.instance().join("options.txt"), old).unwrap();
        let n = copy_options(&a, &b, &["keys".into(), "language".into()], 1_900_000_000).unwrap();
        assert_eq!(n, 2);
        let out = fs::read_to_string(b.instance().join("options.txt")).unwrap();
        assert_eq!(out, "version:4000\nkey_key.jump:key.keyboard.x\nrenderDistance:6\nparticles:2\nlang:fr_fr\ngamma:1.0\n");
        let first = fs::read_dir(b.instance()).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).find(|n| n.ends_with(".bak")).unwrap();
        assert!(first.starts_with("options.txt.") && first.len() > "options.txt..bak".len());
        assert_eq!(fs::read_to_string(b.instance().join(&first)).unwrap(), old);
        for i in 1..5 {
            copy_options(&a, &b, &["keys".into()], 1_900_000_000 + i * 60).unwrap();
        }
        let baks = fs::read_dir(b.instance()).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().ends_with(".bak")).count();
        assert_eq!(baks, 3, "3 sauvegardes au plus");
        // même avec toutes les catégories, les clés de performance de la destination restent
        copy_options(&a, &b, &CATEGORIES.map(String::from), 1_900_001_000).unwrap();
        let out = fs::read_to_string(b.instance().join("options.txt")).unwrap();
        assert!(out.contains("renderDistance:6") && out.contains("particles:2") && !out.contains("graphicsPreset") && out.contains("narrator:1"));
        fs::remove_dir_all(&a.root).unwrap();
    }

    #[test]
    fn shaders_list_read_and_write_iris_properties() {
        let (a, _) = dirs("shaders");
        let sp = a.instance().join("shaderpacks");
        fs::create_dir_all(&sp).unwrap();
        fs::write(sp.join("Zeta.zip"), "z").unwrap();
        fs::write(sp.join("alpha.ZIP"), "z").unwrap();
        fs::write(sp.join("Zeta.zip.txt"), "réglages").unwrap();
        fs::create_dir_all(sp.join("Dossier")).unwrap();
        let v = shaders_get(&a);
        assert_eq!(
            (v.available, v.files.as_slice(), v.active.as_deref()),
            (false, ["alpha.ZIP".to_string(), "Dossier".into(), "Zeta.zip".into()].as_slice(), None)
        );
        fs::create_dir_all(a.instance().join("mods")).unwrap();
        fs::write(a.instance().join("mods/iris-fabric-1.9.3.jar"), "j").unwrap();
        assert!(shaders_get(&a).available);
        fs::create_dir_all(a.instance().join("config")).unwrap();
        fs::write(iris_file(&a), "# Iris\nmaxShadowRenderDistance=32\nshaderPack=Zeta.zip\nenableShaders=true\n").unwrap();
        assert_eq!(shaders_get(&a).active.as_deref(), Some("Zeta.zip"));
        shader_set(&a, Some("alpha.ZIP")).unwrap();
        let t = fs::read_to_string(iris_file(&a)).unwrap();
        assert!(t.contains("shaderPack=alpha.ZIP") && t.contains("maxShadowRenderDistance=32") && t.contains("# Iris"), "{t}");
        assert_eq!(shaders_get(&a).active.as_deref(), Some("alpha.ZIP"));
        shader_set(&a, None).unwrap();
        assert_eq!(shaders_get(&a).active, None);
        assert!(fs::read_to_string(iris_file(&a)).unwrap().contains("enableShaders=false"));
        fs::write(iris_file(&a), "shaderPack=Zeta.zip\nenableShaders=false\n").unwrap();
        assert_eq!(shaders_get(&a).active, None, "enableShaders=false : aucun");
        for bad in ["../x", "Inconnu.zip", "a/b", ""] {
            assert!(shader_set(&a, Some(bad)).is_err(), "{bad}");
        }
        fs::remove_dir_all(&a.root).unwrap();
    }

    #[test]
    fn copy_refusals_change_nothing() {
        let (a, b) = dirs("refuse");
        assert!(copy_options(&a, &b, &["keys".into()], 1).unwrap_err().to_string().contains("aucun réglage"));
        fs::write(a.instance().join("options.txt"), SRC).unwrap();
        assert!(copy_options(&a, &a, &["keys".into()], 1).is_err());
        assert!(copy_options(&a, &b, &[], 1).is_err() && copy_options(&a, &b, &["renderDistance".into()], 1).is_err());
        assert!(!b.instance().join("options.txt").exists());
        let c = a.for_pack("c").unwrap(); // destination non installée
        assert!(copy_options(&a, &c, &["keys".into()], 1).unwrap_err().to_string().contains("pas installé"));
        fs::write(a.instance().join("options.txt"), "version:1\n").unwrap();
        assert!(copy_options(&a, &b, &["keys".into()], 1).is_err() && !b.instance().join("options.txt").exists());
        fs::remove_dir_all(&a.root).unwrap();
    }
}
