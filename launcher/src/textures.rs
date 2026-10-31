//! Extraction locale de quelques PNG du client.jar installé (jamais redistribués, jamais commités).
use anyhow::{Context, Result};
use std::io::Read;
use std::path::Path;

/// Textures utilisées par l'interface (maquette `design/launcher-mockup/index.html`) : `item/` puis `block/`.
pub const UI_TEXTURES: &[&str] = &[
    "ender_eye",
    "blaze_rod",
    "diamond",
    "diamond_pickaxe",
    "wooden_pickaxe",
    "iron_ingot",
    "flint_and_steel",
    "nether_wart",
    "ender_pearl",
    "end_stone",
    "dragon_egg",
    "arrow",
    "feather",
    "grass_block_side",
    "gunpowder",
    "lava_bucket",
    "piston_side",
];

const MAX_PNG: u64 = 1024 * 1024;
const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";

fn plain(n: &str) -> bool {
    !n.is_empty() && n.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
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
}
