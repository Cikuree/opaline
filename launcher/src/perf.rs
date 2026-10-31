//! Modes de performance : table des modes, fusion dans `options.txt` et `config/iris.properties`.
//! Valeurs vérifiées dans les sources 26.3 (`Options.java`, `GraphicsPreset`, `CloudStatus`, `ParticleStatus`) :
//! les enums sont écrits en JSON (donc entre guillemets), les entiers tels quels.
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum PerfMode {
    Low,
    #[default]
    Balanced,
    Quality,
    /// Le launcher ne touche à rien.
    Custom,
}

impl PerfMode {
    pub const ALL: [PerfMode; 4] = [PerfMode::Low, PerfMode::Balanced, PerfMode::Quality, PerfMode::Custom];

    pub fn label(self) -> &'static str {
        match self {
            PerfMode::Low => "Faible",
            PerfMode::Balanced => "Équilibré",
            PerfMode::Quality => "Qualité",
            PerfMode::Custom => "Personnalisé",
        }
    }
}

/// Réglages d'un mode.
#[derive(Debug, PartialEq)]
pub struct Settings {
    pub shaders: bool,
    pub render_distance: u32,
    pub simulation_distance: u32,
    pub particles: u32,
    pub render_clouds: &'static str,
    pub graphics_preset: &'static str,
    pub memory_mb: u32,
}

pub fn settings(mode: PerfMode) -> Option<Settings> {
    Some(match mode {
        PerfMode::Low => Settings {
            shaders: false,
            render_distance: 6,
            simulation_distance: 5,
            particles: 2,
            render_clouds: "false",
            graphics_preset: "fast",
            memory_mb: 2048,
        },
        PerfMode::Balanced => Settings {
            shaders: true,
            render_distance: 10,
            simulation_distance: 8,
            particles: 1,
            render_clouds: "fast",
            graphics_preset: "fancy",
            memory_mb: 4096,
        },
        PerfMode::Quality => Settings {
            shaders: true,
            render_distance: 16,
            simulation_distance: 10,
            particles: 0,
            render_clouds: "true",
            graphics_preset: "fabulous",
            memory_mb: 6144,
        },
        PerfMode::Custom => return None,
    })
}

/// Mémoire (Mo) pour `-Xmx` : None = mode Personnalisé (garder la valeur de la config).
/// Tous les modes sont plafonnés à 70 % de la RAM physique si elle est connue (jamais sous 2048).
pub fn memory_mb(mode: PerfMode, ram_mb: Option<u64>) -> Option<u32> {
    let s = settings(mode)?;
    Some(match ram_mb {
        // clamp avant le cast : pas de troncature sur une valeur énorme
        Some(r) => s.memory_mb.min((r.saturating_mul(70) / 100).min(u64::from(u32::MAX)) as u32).max(2048),
        None => s.memory_mb,
    })
}

/// Fusionne `kv` dans `existing` (lignes `clé<sep>valeur`) : les clés connues sont remplacées sur place,
/// les nouvelles ajoutées à la fin, toutes les autres lignes sont conservées telles quelles.
pub fn merge_lines(existing: &str, sep: char, kv: &[(&str, String)]) -> String {
    let nl = if existing.contains("\r\n") { "\r\n" } else { "\n" };
    let mut done = vec![false; kv.len()];
    let mut out: Vec<String> = existing
        .lines()
        .map(|l| {
            let key = l.split(sep).next().unwrap_or("").trim();
            match kv.iter().position(|(k, _)| *k == key) {
                Some(i) if l.contains(sep) => {
                    done[i] = true;
                    format!("{}{sep}{}", kv[i].0, kv[i].1)
                }
                _ => l.to_string(),
            }
        })
        .collect();
    for (i, (k, v)) in kv.iter().enumerate() {
        if !done[i] {
            out.push(format!("{k}{sep}{v}"));
        }
    }
    out.join(nl) + nl
}

pub fn options_pairs(s: &Settings) -> Vec<(&'static str, String)> {
    vec![
        ("graphicsPreset", format!("\"{}\"", s.graphics_preset)),
        ("renderDistance", s.render_distance.to_string()),
        ("simulationDistance", s.simulation_distance.to_string()),
        ("particles", s.particles.to_string()),
        ("renderClouds", format!("\"{}\"", s.render_clouds)),
    ]
}

pub fn iris_pairs(s: &Settings, shader_pack: Option<&str>) -> Vec<(&'static str, String)> {
    let mut v = vec![("enableShaders", s.shaders.to_string())];
    if s.shaders
        && let Some(p) = shader_pack
    {
        v.push(("shaderPack", p.to_string()));
    }
    v
}

fn merge_file(path: &Path, sep: char, kv: &[(&str, String)]) -> Result<()> {
    let old = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e.into()), // illisible : on n'écrase rien
    };
    let out = match String::from_utf8(old) {
        Ok(text) => merge_lines(&text, sep, kv).into_bytes(),
        Err(e) => {
            // Pas de l'UTF-8 (ex. Latin-1) : chaque octet devient un caractère (sans perte), fusion, retour en octets.
            if kv.iter().any(|(k, v)| k.chars().chain(v.chars()).any(|c| c as u32 > 0xFF)) {
                anyhow::bail!("{} n'est pas en UTF-8 : non modifié", path.display());
            }
            let text: String = e.as_bytes().iter().map(|&b| b as char).collect();
            merge_lines(&text, sep, kv).chars().map(|c| c as u32 as u8).collect()
        }
    };
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    std::fs::write(path, out)?;
    Ok(())
}

/// À appeler avant chaque lancement. Mode Personnalisé : aucune écriture. Retourne la mémoire à utiliser.
pub fn apply(instance: &Path, mode: PerfMode, shader_pack: Option<&str>, ram_mb: Option<u64>) -> Result<Option<u32>> {
    let Some(s) = settings(mode) else {
        return Ok(None);
    };
    merge_file(&instance.join("options.txt"), ':', &options_pairs(&s))?;
    merge_file(&instance.join("config").join("iris.properties"), '=', &iris_pairs(&s, shader_pack))?;
    Ok(memory_mb(mode, ram_mb))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_keeps_other_lines_and_replaces_in_place() {
        let old = "version:4000\nrenderDistance:12\nlang:fr_fr\n";
        let kv = [("renderDistance", "6".to_string()), ("particles", "2".to_string())];
        assert_eq!(merge_lines(old, ':', &kv), "version:4000\nrenderDistance:6\nlang:fr_fr\nparticles:2\n");
        assert_eq!(merge_lines("", ':', &kv), "renderDistance:6\nparticles:2\n");
        assert!(merge_lines("a:1\r\n", ':', &kv).contains("\r\n"));
        // une clé qui n'est qu'un préfixe d'une autre n'est pas touchée
        assert_eq!(merge_lines("renderDistanceX:1\n", ':', &kv[..1]), "renderDistanceX:1\nrenderDistance:6\n");
    }

    #[test]
    fn table_matches_spec() {
        let l = settings(PerfMode::Low).unwrap();
        assert_eq!((l.shaders, l.render_distance, l.particles, l.render_clouds, l.graphics_preset), (false, 6, 2, "false", "fast"));
        let q = options_pairs(&settings(PerfMode::Quality).unwrap());
        assert!(q.contains(&("graphicsPreset", "\"fabulous\"".into())) && q.contains(&("renderClouds", "\"true\"".into())));
        assert!(settings(PerfMode::Custom).is_none());
    }

    #[test]
    fn memory_is_capped_for_quality_only() {
        assert_eq!(memory_mb(PerfMode::Quality, Some(8192)), Some(5734));
        assert_eq!(memory_mb(PerfMode::Quality, Some(32768)), Some(6144));
        assert_eq!(memory_mb(PerfMode::Quality, None), Some(6144));
        assert_eq!(memory_mb(PerfMode::Balanced, Some(4096)), Some(2867)); // PC 4 Go
        assert_eq!(memory_mb(PerfMode::Balanced, Some(2048)), Some(2048)); // plancher
        assert_eq!(memory_mb(PerfMode::Custom, Some(8192)), None);
    }

    #[test]
    fn iris_pairs_depend_on_shaders() {
        assert_eq!(iris_pairs(&settings(PerfMode::Low).unwrap(), Some("a.zip")), [("enableShaders", "false".to_string())]);
        assert_eq!(iris_pairs(&settings(PerfMode::Balanced).unwrap(), Some("a.zip")).len(), 2);
    }

    #[test]
    fn apply_writes_files_and_custom_touches_nothing() {
        let d = std::env::temp_dir().join(format!("crl-perf-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        assert_eq!(apply(&d, PerfMode::Custom, None, None).unwrap(), None);
        assert!(!d.exists());
        assert_eq!(apply(&d, PerfMode::Balanced, Some("C.zip"), None).unwrap(), Some(4096));
        let iris = std::fs::read_to_string(d.join("config/iris.properties")).unwrap();
        assert!(iris.contains("enableShaders=true") && iris.contains("shaderPack=C.zip"));
        assert!(std::fs::read_to_string(d.join("options.txt")).unwrap().contains("renderClouds:\"fast\""));
        std::fs::remove_dir_all(&d).unwrap();
    }
}
