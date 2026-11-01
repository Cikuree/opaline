//! Réglage automatique au premier lancement (aucun `config.json` encore enregistré) : mode de performance et mémoire d'après la RAM
//! et, si on le trouve, le nom du GPU. Le choix est ensuite modifiable librement dans les réglages.
use crate::config::{Config, Dirs};
use crate::perf::{self, PerfMode};
use anyhow::Result;

/// GPU intégré ou de bas niveau (noms WMI / lspci). Intel Arc (discret, ou Arc Graphics récent) n'en fait pas partie.
pub fn is_integrated(gpu: &str) -> bool {
    let g = gpu.to_ascii_lowercase();
    let intel = g.contains("intel") && !g.contains("arc");
    intel || ["vega", "radeon(tm) graphics", "radeon graphics", "basic render", "llvmpipe", "swrast", "svga", "virtualbox", "vmware"].iter().any(|k| g.contains(k))
}

/// Table : RAM < 6 Go -> Faible ; < 12 Go -> Équilibré ; sinon Qualité. GPU intégré : jamais plus que Équilibré, et Faible sous 8 Go.
/// RAM inconnue : Équilibré. La mémoire suit `perf::memory_mb` (plafonnée à 70 % de la RAM).
pub fn choose(ram_mb: Option<u64>, gpu: Option<&str>) -> (PerfMode, u32) {
    let mut mode = match ram_mb {
        None => PerfMode::Balanced,
        Some(r) if r < 6000 => PerfMode::Low,
        Some(r) if r < 12000 => PerfMode::Balanced,
        Some(_) => PerfMode::Quality,
    };
    if gpu.is_some_and(is_integrated) {
        mode = match (mode, ram_mb) {
            (PerfMode::Quality, _) => PerfMode::Balanced,
            (PerfMode::Balanced, Some(r)) if r < 8000 => PerfMode::Low,
            (m, _) => m,
        };
    }
    (mode, perf::memory_mb(mode, ram_mb).unwrap_or(4096))
}

/// Noms de GPU d'une sortie `Get-CimInstance Win32_VideoController` (un nom par ligne).
pub fn parse_wmi(text: &str) -> Vec<String> {
    text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.eq_ignore_ascii_case("name")).map(str::to_string).collect()
}

/// Noms de GPU d'une sortie `lspci` (lignes VGA / 3D / Display controller).
pub fn parse_lspci(text: &str) -> Vec<String> {
    text.lines()
        .filter(|l| ["VGA compatible controller", "3D controller", "Display controller"].iter().any(|k| l.contains(k)))
        .filter_map(|l| l.split_once(": ").map(|(_, n)| n.trim().to_string()))
        .collect()
}

/// Le GPU qui compte : un GPU dédié s'il y en a un, sinon le premier.
pub fn pick_gpu(names: &[String]) -> Option<String> {
    names.iter().find(|n| !is_integrated(n)).or(names.first()).cloned()
}

#[cfg(windows)]
pub fn detect_gpu() -> Option<String> {
    use std::os::windows::process::CommandExt;
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", "(Get-CimInstance Win32_VideoController).Name"])
        .creation_flags(0x0800_0000)
        .output()
        .ok()?;
    pick_gpu(&parse_wmi(&String::from_utf8_lossy(&out.stdout)))
}

#[cfg(target_os = "linux")]
pub fn detect_gpu() -> Option<String> {
    let out = std::process::Command::new("lspci").output().ok()?; // absent : ignoré
    pick_gpu(&parse_lspci(&String::from_utf8_lossy(&out.stdout)))
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn detect_gpu() -> Option<String> {
    None
}

/// Détection du GPU avec délai maximal : au-delà, `None` (réglage d'après la RAM seule). Le processus lancé peut finir seul.
pub fn detect_gpu_timeout(max: std::time::Duration) -> Option<String> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(detect_gpu());
    });
    rx.recv_timeout(max).ok().flatten()
}

/// Affine le réglage automatique avec le GPU, seulement si l'utilisateur n'y a pas touché entre-temps (`auto_tuned` toujours vrai).
/// Renvoie vrai si le réglage a changé.
pub fn refine(root: &Dirs, ram_mb: Option<u64>, gpu: Option<String>) -> Result<bool> {
    let mut c = Config::load(root);
    let (mode, mem) = choose(ram_mb, gpu.as_deref());
    if !c.auto_tuned || (c.perf_mode, c.memory_mb) == (mode, mem) {
        return Ok(false);
    }
    c.perf_mode = mode;
    c.memory_mb = mem;
    c.save(root)?;
    Ok(true)
}

/// Premier lancement seulement : sans `config.json`, enregistre le mode et la mémoire choisis (`auto_tuned = true`).
/// Renvoie vrai si un réglage a été écrit ; un `config.json` existant n'est jamais modifié.
pub fn first_run(root: &Dirs, ram_mb: Option<u64>, gpu: impl FnOnce() -> Option<String>) -> Result<bool> {
    if root.config_file().exists() {
        return Ok(false);
    }
    let (perf_mode, memory_mb) = choose(ram_mb, gpu().as_deref());
    Config { perf_mode, memory_mb, auto_tuned: true, ..Config::default() }.save(root)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ram_table() {
        assert_eq!(choose(Some(4096), None), (PerfMode::Low, 2048));
        assert_eq!(choose(Some(8192), None), (PerfMode::Balanced, 4096));
        assert_eq!(choose(Some(16384), None), (PerfMode::Quality, 6144));
        assert_eq!(choose(Some(32768), None), (PerfMode::Quality, 6144));
        assert_eq!(choose(None, None), (PerfMode::Balanced, 4096));
    }

    #[test]
    fn integrated_gpu_caps_the_mode() {
        let igpu = Some("Intel(R) UHD Graphics 630");
        assert_eq!(choose(Some(16384), igpu).0, PerfMode::Balanced);
        assert_eq!(choose(Some(6144), igpu).0, PerfMode::Low);
        assert_eq!(choose(Some(8192), igpu).0, PerfMode::Balanced);
        assert_eq!(choose(Some(16384), Some("NVIDIA GeForce RTX 4070")).0, PerfMode::Quality);
        assert_eq!(choose(Some(16384), Some("Intel(R) Arc(TM) A770 Graphics")).0, PerfMode::Quality);
        assert!(is_integrated("AMD Radeon(TM) Graphics") && is_integrated("Microsoft Basic Render Driver") && !is_integrated("AMD Radeon RX 7800 XT"));
    }

    #[test]
    fn parses_gpu_listings_and_prefers_the_discrete_one() {
        let w = parse_wmi("Name\r\nIntel(R) UHD Graphics 630\r\nNVIDIA GeForce GTX 1650\r\n\r\n");
        assert_eq!(w, ["Intel(R) UHD Graphics 630", "NVIDIA GeForce GTX 1650"]);
        assert_eq!(pick_gpu(&w).as_deref(), Some("NVIDIA GeForce GTX 1650"));
        let l = parse_lspci("00:02.0 VGA compatible controller: Intel Corporation HD Graphics 620 (rev 02)\n01:00.0 3D controller: NVIDIA Corporation GP107M [GeForce GTX 1050 Mobile]\n00:1f.3 Audio device: Intel Corporation x\n");
        assert_eq!(l.len(), 2);
        assert_eq!(pick_gpu(&l).unwrap(), "NVIDIA Corporation GP107M [GeForce GTX 1050 Mobile]");
        assert_eq!(pick_gpu(&[]), None);
    }

    #[test]
    fn first_run_only_writes_when_no_config_exists() {
        let d = Dirs::new(std::env::temp_dir().join(format!("opaline-auto-{}", std::process::id())));
        let _ = std::fs::remove_dir_all(&d.root);
        assert!(first_run(&d, Some(16384), || Some("NVIDIA GeForce RTX 3060".into())).unwrap());
        let c = Config::load(&d);
        assert!(c.auto_tuned && c.perf_mode == PerfMode::Quality && c.memory_mb == 6144);
        let mine = Config { perf_mode: PerfMode::Low, auto_tuned: false, ..c };
        mine.save(&d).unwrap();
        assert!(!first_run(&d, Some(32768), || panic!("GPU non recherché hors premier lancement")).unwrap());
        assert_eq!(Config::load(&d).perf_mode, PerfMode::Low);
        std::fs::remove_dir_all(&d.root).unwrap();
    }

    #[test]
    fn refine_applies_the_gpu_only_while_the_choice_is_still_automatic() {
        let d = Dirs::new(std::env::temp_dir().join(format!("opaline-refine-{}", std::process::id())));
        let _ = std::fs::remove_dir_all(&d.root);
        assert!(first_run(&d, Some(16384), || None).unwrap());
        assert_eq!(Config::load(&d).perf_mode, PerfMode::Quality);
        assert!(refine(&d, Some(16384), Some("Intel(R) UHD Graphics".into())).unwrap());
        assert_eq!(Config::load(&d).perf_mode, PerfMode::Balanced);
        let c = Config { perf_mode: PerfMode::Low, auto_tuned: false, ..Config::load(&d) };
        c.save(&d).unwrap();
        assert!(!refine(&d, Some(16384), Some("NVIDIA GeForce RTX 4090".into())).unwrap());
        assert_eq!(Config::load(&d).perf_mode, PerfMode::Low);
        std::fs::remove_dir_all(&d.root).unwrap();
    }
}
