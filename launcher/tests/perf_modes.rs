//! Cas 6 : modes de performance (fusion d'options.txt, Personnalisé sans écriture, iris.properties, mémoire).
#![allow(dead_code)]

#[path = "../src/perf.rs"]
mod perf;

use perf::{PerfMode, apply, memory_mb};
use std::path::{Path, PathBuf};

fn dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("crl-it-perf-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap()
}

#[test]
fn existing_options_keep_user_lines_and_mode_keys_are_replaced_once() {
    let d = dir("merge");
    let user = "version:3465\nrenderDistance:12\nlang:fr_fr\nkey_key.jump:key.keyboard.space\nparticles:0\nfov:0.5\n";
    std::fs::write(d.join("options.txt"), user).unwrap();
    assert_eq!(apply(&d, PerfMode::Balanced, Some("C.zip"), None).unwrap(), Some(4096));
    let out = read(&d.join("options.txt"));
    assert_eq!(
        out,
        "version:3465\nrenderDistance:10\nlang:fr_fr\nkey_key.jump:key.keyboard.space\nparticles:1\nfov:0.5\ngraphicsPreset:\"fancy\"\nsimulationDistance:8\nrenderClouds:\"fast\"\n"
    );
    assert_eq!(out.matches("particles:").count(), 1);
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn options_crlf_line_endings_are_kept() {
    let d = dir("crlf");
    std::fs::write(d.join("options.txt"), "lang:fr_fr\r\nrenderDistance:12\r\n").unwrap();
    apply(&d, PerfMode::Low, None, None).unwrap();
    let out = read(&d.join("options.txt"));
    assert!(out.lines().count() >= 3);
    assert!(out.contains("lang:fr_fr\r\n") && out.contains("renderDistance:6\r\n"));
    assert!(!out.replace("\r\n", "").contains('\n'), "fin de ligne LF mélangée : {out:?}");
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn apply_is_idempotent() {
    let d = dir("idem");
    apply(&d, PerfMode::Quality, Some("C.zip"), Some(8192)).unwrap();
    let first = (read(&d.join("options.txt")), read(&d.join("config/iris.properties")));
    apply(&d, PerfMode::Quality, Some("C.zip"), Some(8192)).unwrap();
    let second = (read(&d.join("options.txt")), read(&d.join("config/iris.properties")));
    assert_eq!(first, second);
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn custom_mode_writes_nothing_and_keeps_existing_files() {
    let d = dir("custom");
    std::fs::write(d.join("options.txt"), "renderDistance:12\n").unwrap();
    std::fs::create_dir_all(d.join("config")).unwrap();
    std::fs::write(d.join("config/iris.properties"), "enableShaders=false\n").unwrap();
    let before = (read(&d.join("options.txt")), read(&d.join("config/iris.properties")));
    assert_eq!(apply(&d, PerfMode::Custom, Some("C.zip"), Some(8192)).unwrap(), None, "pas de mémoire imposée");
    assert_eq!((read(&d.join("options.txt")), read(&d.join("config/iris.properties"))), before);
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn custom_mode_creates_no_files_in_empty_instance() {
    let d = dir("custom-empty").join("instance");
    assert_eq!(apply(&d, PerfMode::Custom, None, None).unwrap(), None);
    assert!(!d.exists(), "Personnalisé a créé le dossier d'instance");
    let _ = std::fs::remove_dir_all(d.parent().unwrap());
}

#[test]
fn iris_properties_merge_and_shader_pack_rules() {
    let d = dir("iris");
    std::fs::create_dir_all(d.join("config")).unwrap();
    std::fs::write(d.join("config/iris.properties"), "maxShadowDistance=16\nenableShaders=false\nshaderPack=Old.zip\n").unwrap();
    apply(&d, PerfMode::Balanced, Some("New.zip"), None).unwrap();
    let iris = read(&d.join("config/iris.properties"));
    assert_eq!(iris, "maxShadowDistance=16\nenableShaders=true\nshaderPack=New.zip\n");
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn low_mode_disables_shaders_and_drops_shader_pack_line() {
    let d = dir("iris-low");
    apply(&d, PerfMode::Low, Some("New.zip"), None).unwrap();
    let iris = read(&d.join("config/iris.properties"));
    assert_eq!(iris, "enableShaders=false\n");
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn balanced_without_shader_pack_only_sets_enable() {
    let d = dir("iris-none");
    apply(&d, PerfMode::Balanced, None, None).unwrap();
    assert_eq!(read(&d.join("config/iris.properties")), "enableShaders=true\n");
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn memory_per_mode_and_quality_cap() {
    assert_eq!(memory_mb(PerfMode::Low, None), Some(2048));
    assert_eq!(memory_mb(PerfMode::Balanced, Some(2048)), Some(2048), "plafond 70 % de la RAM pour tous les modes, plancher 2048");
    assert_eq!(memory_mb(PerfMode::Balanced, Some(4096)), Some(2867), "PC 4 Go");
    assert_eq!(memory_mb(PerfMode::Quality, None), Some(6144));
    assert_eq!(memory_mb(PerfMode::Quality, Some(8192)), Some(5734), "70 % de 8192");
    assert_eq!(memory_mb(PerfMode::Quality, Some(4096)), Some(2867), "70 % de 4096");
    assert_eq!(memory_mb(PerfMode::Quality, Some(2000)), Some(2048), "plancher 2048");
    assert_eq!(memory_mb(PerfMode::Custom, Some(8192)), None);
}

#[test]
fn bug_non_utf8_options_lose_user_lines() {
    let d = dir("latin1");
    let mut bytes = b"lang:fr_fr\nrenderDistance:12\nfoo:".to_vec();
    bytes.extend_from_slice(&[0xE9, 0x74, 0xE9, b'\n']); // « été » en Latin-1
    std::fs::write(d.join("options.txt"), &bytes).unwrap();
    apply(&d, PerfMode::Balanced, None, None).unwrap();
    let out = std::fs::read(d.join("options.txt")).unwrap();
    assert!(out.starts_with(b"lang:fr_fr\n"), "ligne utilisateur perdue : {:?}", String::from_utf8_lossy(&out));
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn bug_quality_memory_cast_before_clamp() {
    // 70 % de 6_135_668_995 = 4_294_968_296 : tronqué en u32 à 1000, puis plancher 2048 au lieu de 6144.
    assert_eq!(memory_mb(PerfMode::Quality, Some(6_135_668_995)), Some(6144));
}
