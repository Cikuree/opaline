//! Cas 1 : catalogue `packs.json` (valide, pack non fabric, packHash, JSON invalide, 404, hors ligne avec cache).
#![allow(dead_code)]
mod opaline_common;
mod support;

use opaline_common::*;
use opaline_core::config::Dirs;
use opaline_core::download::Progress;
use opaline_core::{catalog, service};
use std::fs;
use std::sync::Arc;
use support::{Server, dead_base};

const HASH: &str = "abcdef0123456789";

fn write_catalog(path: &std::path::Path, entries: &[String]) {
    fs::write(path, catalog_json(entries)).unwrap();
}

#[test]
fn valid_catalog_gives_supported_fabric_and_refused_unknown_loader() {
    let _g = env_lock();
    let t = Tmp::new("cat-valid");
    let f = t.0.join("packs.json");
    write_catalog(
        &f,
        &[
            catalog_entry("classroom-run", "fabric", "https://github.com/x/pack.zip", HASH),
            catalog_entry("neo", "bedrock", "https://github.com/x/neo.zip", HASH),
        ],
    );
    set_env("OPALINE_PACKS_URL", f.to_str().unwrap());
    let dirs = Dirs::new(t.0.join("data"));

    let packs = catalog::load(&dirs, &http()).expect("catalogue valide");
    assert_eq!(packs.len(), 2);
    let views = service::packs_list(&dirs).unwrap();
    let cr = views.iter().find(|v| v.id == "classroom-run").unwrap();
    assert!(cr.supported && !cr.installed && !cr.update_available && cr.installed_version_hash.is_none());
    let neo = views.iter().find(|v| v.id == "neo").unwrap();
    assert!(!neo.supported, "un loader inconnu doit être non pris en charge");
    let j = serde_json::to_value(cr).unwrap();
    assert!(j.get("updateAvailable").is_some() && j.get("installedVersionHash").is_some());
    unset_env("OPALINE_PACKS_URL");
}

#[test]
fn unknown_loader_pack_install_is_refused_before_any_download() {
    let _g = env_lock();
    let srv = Server::start();
    let t = Tmp::new("cat-neo");
    let f = t.0.join("packs.json");
    write_catalog(&f, &[catalog_entry("neo", "bedrock", &srv.url("/pack.zip"), HASH)]);
    set_env("OPALINE_PACKS_URL", f.to_str().unwrap());
    let dirs = Dirs::new(t.0.join("data"));

    let err = service::install_pack(&dirs, "neo", &Arc::new(Progress::default()), None).unwrap_err().to_string();
    assert!(err.contains("pas pris en charge") && err.contains("bedrock"), "{err}");
    assert_eq!(srv.count("/pack.zip"), 0, "aucun téléchargement pour un loader refusé");
    assert!(!dirs.for_pack("neo").unwrap().install_marker().exists());
    unset_env("OPALINE_PACKS_URL");
}

#[test]
fn pack_hash_mismatch_refuses_install_and_leaves_no_marker() {
    let _g = env_lock();
    let srv = Server::start();
    let t = Tmp::new("cat-hash");
    let src = t.0.join("src");
    let real = write_packwiz(&src, &srv, "/packs/alpha", "1.0.0", &[]);
    srv.put("/pack.zip", pack_zip(&src));
    // Le catalogue annonce un autre index que celui du zip publié.
    let wrong = "0".repeat(64);
    assert_ne!(real, wrong);
    let f = t.0.join("packs.json");
    write_catalog(&f, &[catalog_entry("alpha", "fabric", &srv.url("/pack.zip"), &wrong)]);
    set_env("OPALINE_PACKS_URL", f.to_str().unwrap());
    let dirs = Dirs::new(t.0.join("data"));

    let err = service::install_pack(&dirs, "alpha", &Arc::new(Progress::default()), None).unwrap_err().to_string();
    assert!(err.contains("ne correspond pas au catalogue"), "{err}");
    let inst = dirs.for_pack("alpha").unwrap();
    assert!(!inst.install_marker().exists() && !inst.instance().join("mods").exists());
    unset_env("OPALINE_PACKS_URL");
}

#[test]
fn invalid_json_file_falls_back_to_cache_then_fails_cleanly_without_cache() {
    let _g = env_lock();
    let t = Tmp::new("cat-bad");
    let f = t.0.join("packs.json");
    write_catalog(&f, &[catalog_entry("classroom-run", "fabric", "https://github.com/x/pack.zip", HASH)]);
    set_env("OPALINE_PACKS_URL", f.to_str().unwrap());
    let dirs = Dirs::new(t.0.join("data"));
    assert_eq!(catalog::load(&dirs, &http()).unwrap().len(), 1, "une première lecture remplit le cache");

    fs::write(&f, "{pas du json").unwrap();
    let cached = catalog::load(&dirs, &http()).expect("repli sur le cache");
    assert_eq!(cached[0].id, "classroom-run");

    let fresh = Dirs::new(t.0.join("vide"));
    let err = catalog::load(&fresh, &http()).unwrap_err().to_string();
    assert!(err.contains("indisponible"), "{err}");
    unset_env("OPALINE_PACKS_URL");
}

#[test]
fn http_invalid_json_and_404_use_cache_offline_without_cache_error() {
    let _g = env_lock();
    let srv = Server::start();
    let t = Tmp::new("cat-http");
    let f = t.0.join("packs.json");
    write_catalog(&f, &[catalog_entry("classroom-run", "fabric", "https://github.com/x/pack.zip", HASH)]);
    set_env("OPALINE_PACKS_URL", f.to_str().unwrap());
    let dirs = Dirs::new(t.0.join("data"));
    catalog::load(&dirs, &http()).unwrap();

    // 200 mais corps non JSON
    srv.put("/packs.json", "<html>panne</html>");
    set_env("OPALINE_PACKS_URL", &srv.url("/packs.json"));
    assert_eq!(catalog::load(&dirs, &http()).expect("cache après JSON invalide").len(), 1);
    assert_eq!(srv.count("/packs.json"), 1);

    // 404 : aucune route
    srv.put_mode("/packs.json", "", support::Mode::Status(404));
    assert_eq!(catalog::load(&dirs, &http()).expect("cache après 404").len(), 1);

    // hors ligne : port fermé
    set_env("OPALINE_PACKS_URL", &format!("{}/packs.json", dead_base()));
    assert_eq!(catalog::load(&dirs, &http()).expect("cache hors ligne").len(), 1);
    unset_env("OPALINE_PACKS_URL");
}

#[test]
fn offline_without_cache_is_a_clean_error_not_a_panic() {
    let _g = env_lock();
    let t = Tmp::new("cat-off");
    set_env("OPALINE_PACKS_URL", &format!("{}/packs.json", dead_base()));
    let dirs = Dirs::new(t.0.join("data"));
    let err = service::packs_list(&dirs).unwrap_err().to_string();
    assert!(err.contains("catalogue des packs indisponible"), "{err}");
    unset_env("OPALINE_PACKS_URL");
}
