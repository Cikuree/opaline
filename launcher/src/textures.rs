//! Extraction locale de quelques PNG du client.jar installé (jamais redistribués, jamais commités).
use anyhow::{Context, Result};
use std::io::Read;
use std::path::Path;

/// Version de repli quand aucun catalogue (réseau ou cache) n'est disponible au démarrage.
pub const DEFAULT_MC: &str = "26.3";

/// Toutes les textures affichées par l'interface (`item/` puis `block/`), une fois chacune.
/// Splits (portion « SPLITS »), vignette/hero, causes de mort, mods (`MODTEX`), dossiers (`FOLDERS`) : voir `opaline/ui/data.js`.
pub const UI_TEXTURES: &[&str] = &[
    // splits
    "wooden_pickaxe",
    "iron_ingot",
    "diamond",
    "flint_and_steel",
    "nether_wart",
    "blaze_rod",
    "ender_eye",
    "end_stone",
    "end_crystal",
    // hero, vignettes, causes de mort
    "ender_pearl",
    "diamond_pickaxe",
    "iron_sword",
    "feather",
    "lava_bucket",
    "gunpowder",
    "arrow",
    "bone",
    "fire_charge",
    "water_bucket",
    "rotten_flesh",
    "glowstone_dust",
    "totem_of_undying",
    "splash_potion",
    "grass_block_side",
    "piston_side",
    // mods
    "string",
    "book",
    "redstone",
    "lapis_lazuli",
    "spyglass",
    "crafting_table_front",
    "clock_00",
    "filled_map",
    "emerald",
    "golden_apple",
    // dossiers
    "barrel_side",
    "painting",
    "writable_book",
    "furnace_front",
];

const MAX_PNG: u64 = 1024 * 1024;
const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";

fn plain(n: &str) -> bool {
    !n.is_empty() && n.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Vrai si toutes les textures de `UI_TEXTURES` sont déjà extraites dans `dest`.
pub fn complete(dest: &Path) -> bool {
    UI_TEXTURES.iter().all(|n| dest.join(format!("{n}.png")).is_file())
}

/// Version Minecraft dont on prend le client.jar : celle du premier pack du catalogue, sinon `DEFAULT_MC`.
pub fn pick_mc(packs: &[crate::catalog::PackEntry]) -> &str {
    packs.iter().map(|p| p.mc.trim()).find(|m| !m.is_empty()).unwrap_or(DEFAULT_MC)
}

/// Télécharge (reprise + sha1 ; aucun téléchargement si le jar vérifié est déjà là, ex. après une installation)
/// le client.jar dans `shared/versions/<id>/`, puis extrait les textures. Renvoie le nombre de PNG écrits.
pub fn ensure_from_jar(client: &reqwest::blocking::Client, dirs: &crate::config::Dirs, id: &str, jar: &crate::mojang::FileRef, extra: &[String]) -> Result<usize> {
    let job = crate::download::Job { url: jar.url.clone(), dest: dirs.client_jar(id), hash: crate::safe::Hash::new("sha1", &jar.sha1)?, size: jar.size };
    crate::download::fetch(client, &job)?;
    extract(&job.dest, &dirs.textures(), extra)
}

/// Extrait `names` (plus `UI_TEXTURES`) vers `dest/<nom>.png`. Une texture absente est ignorée ;
/// renvoie le nombre de fichiers écrits.
pub fn extract(client_jar: &Path, dest: &Path, extra: &[String]) -> Result<usize> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(client_jar).with_context(|| format!("ouverture de {}", client_jar.display()))?)
        .context("client.jar illisible")?;
    std::fs::create_dir_all(dest)?;
    let mut n = 0;
    for name in UI_TEXTURES.iter().copied().chain(extra.iter().map(String::as_str)).filter(|n| plain(n)) {
        for dir in ["item", "block"] {
            let Ok(f) = zip.by_name(&format!("assets/minecraft/textures/{dir}/{name}.png")) else {
                continue;
            };
            let mut buf = Vec::new();
            f.take(MAX_PNG + 1).read_to_end(&mut buf)?;
            if buf.len() as u64 > MAX_PNG || !buf.starts_with(PNG_MAGIC) {
                continue;
            }
            let out = dest.join(format!("{name}.png"));
            let tmp = dest.join(format!("{name}.png.part"));
            std::fs::write(&tmp, &buf)?;
            std::fs::rename(&tmp, &out)?;
            n += 1;
            break;
        }
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn extracts_item_then_block_and_skips_bad_files() {
        let d = std::env::temp_dir().join(format!("opaline-tex-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let jar = d.join("c.jar");
        let mut w = zip::ZipWriter::new(std::fs::File::create(&jar).unwrap());
        let o = zip::write::SimpleFileOptions::default();
        let mut png = PNG_MAGIC.to_vec();
        png.extend_from_slice(b"data");
        for p in ["item/diamond.png", "block/end_stone.png", "item/arrow.png", "item/custom.png"] {
            w.start_file(format!("assets/minecraft/textures/{p}"), o).unwrap();
            w.write_all(if p.contains("arrow") { b"pas un png" } else { &png }).unwrap();
        }
        w.finish().unwrap();
        let out = d.join("out");
        let n = extract(&jar, &out, &["custom".into(), "../evil".into()]).unwrap();
        assert_eq!(n, 3);
        assert!(out.join("diamond.png").is_file() && out.join("end_stone.png").is_file() && out.join("custom.png").is_file());
        assert!(!out.join("arrow.png").exists() && !d.join("evil.png").exists());
        std::fs::remove_dir_all(&d).unwrap();
    }

    fn jar_with(path: &Path, names: &[&str], dirs_for: impl Fn(&str) -> &'static str) {
        let mut w = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        let mut png = PNG_MAGIC.to_vec();
        png.extend_from_slice(b"data");
        for n in names {
            w.start_file(format!("assets/minecraft/textures/{}/{n}.png", dirs_for(n)), zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(&png).unwrap();
        }
        w.finish().unwrap();
    }

    #[test]
    fn list_has_no_duplicates_and_only_plain_names() {
        let mut v: Vec<_> = UI_TEXTURES.to_vec();
        v.sort_unstable();
        v.dedup();
        assert_eq!(v.len(), UI_TEXTURES.len());
        assert!(UI_TEXTURES.iter().all(|n| plain(n)));
    }

    #[test]
    fn whole_list_is_extracted_from_a_fixture_jar_without_network() {
        let d = std::env::temp_dir().join(format!("opaline-tex-all-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let dirs = crate::config::Dirs::new(&d);
        std::fs::create_dir_all(dirs.client_jar("26.3").parent().unwrap()).unwrap();
        let jar = dirs.client_jar("26.3");
        let block = ["end_stone", "grass_block_side", "piston_side", "crafting_table_front", "barrel_side", "furnace_front"];
        jar_with(&jar, UI_TEXTURES, |n| if block.contains(&n) { "block" } else { "item" });
        assert!(!complete(&dirs.textures()));
        // L'URL est injoignable : si le jar vérifié n'était pas réutilisé, le téléchargement échouerait.
        let sha1 = {
            let mut f = std::fs::File::open(&jar).unwrap();
            crate::safe::Hash::new("sha1", &"0".repeat(40)).unwrap().compute(&mut f).unwrap()
        };
        let fr = crate::mojang::FileRef { sha1, url: "http://127.0.0.1:9/client.jar".into(), size: None };
        let c = crate::download::http_client().unwrap();
        assert_eq!(ensure_from_jar(&c, &dirs, "26.3", &fr, &[]).unwrap(), UI_TEXTURES.len());
        assert!(complete(&dirs.textures()));
        // Hash différent : le jar est jugé invalide et re-téléchargé (échec réseau ici).
        let bad = crate::mojang::FileRef { sha1: "0".repeat(40), ..fr };
        assert!(ensure_from_jar(&c, &dirs, "26.3", &bad, &[]).is_err());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn mc_version_comes_from_first_pack_else_default() {
        let mk = |mc: &str| crate::catalog::parse_catalog(&format!(
            r#"{{"packs":[{{"id":"a","name":"A","mc":"{mc}","loader":"fabric","pack":"https://x/p.zip","packHash":"{}","server":{{"host":"h","port":25565,"motd":"m","statsPort":25580}}}}]}}"#,
            "0".repeat(64)
        )).unwrap();
        assert_eq!(pick_mc(&mk("1.21.5")), "1.21.5");
        assert_eq!(pick_mc(&[]), DEFAULT_MC);
    }
}
