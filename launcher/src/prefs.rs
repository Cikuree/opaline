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
            "narrator" | "highContrast" | "highContrastBlockOutline" | "showSubtitles" | "directionalAudio" | "textBackgroundOpacity" | "backgroundForChatOnly" | "chatOpacity" | "chatLineSpacing" | "chatScale"
                | "chatWidth" | "chatHeightFocused" | "chatHeightUnfocused" | "chatDelay" | "notificationDisplayTime" | "toggleCrouch" | "toggleSprint" | "hideLightningFlashes" | "damageTiltStrength"
                | "distortionEffectScale" | "menuBackgroundBlurriness" | "panoramaScrollSpeed" | "monochromeLogo"
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

/// Copie les réglages des catégories de `from` vers `to` (fusion : le reste de la destination est conservé) ; l'ancien
/// `options.txt` de la destination est gardé dans `options.txt.bak`. Renvoie le nombre de réglages copiés.
/// L'appelant refuse si le jeu de la destination tourne.
pub fn copy_options(from: &Dirs, to: &Dirs, cats: &[String]) -> Result<usize> {
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
        std::fs::copy(&dest, to.instance().join("options.txt.bak")).context("sauvegarde d'options.txt impossible : rien n'a été modifié")?;
    }
    let kv: Vec<(&str, String)> = pairs.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
    perf::merge_file(&dest, ':', &kv)?;
    Ok(pairs.len())
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
        let n = copy_options(&a, &b, &["keys".into(), "language".into()]).unwrap();
        assert_eq!(n, 2);
        let out = fs::read_to_string(b.instance().join("options.txt")).unwrap();
        assert_eq!(out, "version:4000\nkey_key.jump:key.keyboard.x\nrenderDistance:6\nparticles:2\nlang:fr_fr\ngamma:1.0\n");
        assert_eq!(fs::read_to_string(b.instance().join("options.txt.bak")).unwrap(), old);
        // même avec toutes les catégories, les clés de performance de la destination restent
        copy_options(&a, &b, &CATEGORIES.map(String::from)).unwrap();
        let out = fs::read_to_string(b.instance().join("options.txt")).unwrap();
        assert!(out.contains("renderDistance:6") && out.contains("particles:2") && !out.contains("graphicsPreset") && out.contains("narrator:1"));
        fs::remove_dir_all(&a.root).unwrap();
    }

    #[test]
    fn copy_refusals_change_nothing() {
        let (a, b) = dirs("refuse");
        assert!(copy_options(&a, &b, &["keys".into()]).unwrap_err().to_string().contains("aucun réglage"));
        fs::write(a.instance().join("options.txt"), SRC).unwrap();
        assert!(copy_options(&a, &a, &["keys".into()]).is_err());
        assert!(copy_options(&a, &b, &[]).is_err() && copy_options(&a, &b, &["renderDistance".into()]).is_err());
        assert!(!b.instance().join("options.txt").exists());
        let c = a.for_pack("c").unwrap(); // destination non installée
        assert!(copy_options(&a, &c, &["keys".into()]).unwrap_err().to_string().contains("pas installé"));
        fs::write(a.instance().join("options.txt"), "version:1\n").unwrap();
        assert!(copy_options(&a, &b, &["keys".into()]).is_err() && !b.instance().join("options.txt").exists());
        fs::remove_dir_all(&a.root).unwrap();
    }
}
