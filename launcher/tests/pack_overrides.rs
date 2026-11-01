//! Overrides d'un pack : téléchargement vérifié, extraction dans l'instance, mise à jour (fichiers retirés), archive inchangée.
#![allow(dead_code)]
mod opaline_common;
mod support;

use opaline_common::*;
use opaline_core::catalog::{self, Overrides};
use opaline_core::config::Dirs;
use opaline_core::download::Progress;
use opaline_core::install::{self, InstallParams};
use std::fs;
use std::sync::Arc;
use support::Server;

fn overrides_for(srv: &Server, route: &str, zip: Vec<u8>) -> Overrides {
    let o = Overrides { url: srv.url(route), sha256: sha256_hex(&zip), size: zip.len() as u64 };
    srv.put(route, zip);
    o
}

fn run(srv: &Server, dirs: &Dirs, pack: &std::path::Path, o: Option<Overrides>, p: &Arc<Progress>) -> anyhow::Result<()> {
    let params = InstallParams { dirs: dirs.clone(), pack_dir: pack.to_path_buf(), mod_jar: None, endpoints: install_endpoints(srv), overrides: o };
    install::install(&http(), &params, p).map(|_| ())
}

#[test]
fn overrides_install_update_and_unchanged_archive() {
    let _g = env_lock();
    let srv = Server::start();
    let t = Tmp::new("ovr-int");
    let pack = t.0.join("pack");
    let index = write_packwiz(&pack, &srv, "/packs/a", "1.0.0", &[]);
    let dirs = Dirs::new(t.0.join("data")).for_pack("alpha").unwrap();
    let inst = dirs.instance();

    let a1 = overrides_for(&srv, "/o1.zip", make_zip(&[("resourcepacks/r.zip", b"r1"), ("datapacks/d.zip", b"d1"), ("options.txt", b"theirs")]));
    let progress = Arc::new(Progress::default());
    run(&srv, &dirs, &pack, Some(a1.clone()), &progress).unwrap();
    let mut cursor = 0;
    assert!(progress.pending(&mut cursor).iter().any(|(s, _, _)| s == "overrides"), "étape overrides émise");
    assert_eq!(fs::read(inst.join("resourcepacks/r.zip")).unwrap(), b"r1");
    assert!(!inst.join("options.txt").exists());
    assert_eq!(install::installed_hash(&dirs).unwrap(), catalog::pack_version_hash(&index, Some(&a1)));
    fs::create_dir_all(inst.join("saves/w")).unwrap();
    fs::write(inst.join("saves/w/level.dat"), b"save").unwrap();
    assert_eq!(srv.count("/o1.zip"), 1);

    // archive inchangée : aucun téléchargement, les fichiers (et le ménage des .zip de resourcepacks) restent
    run(&srv, &dirs, &pack, Some(a1.clone()), &Arc::new(Progress::default())).unwrap();
    assert_eq!(srv.count("/o1.zip"), 1);
    assert_eq!(fs::read(inst.join("resourcepacks/r.zip")).unwrap(), b"r1");

    // archive modifiée : r.zip remplacé, d.zip retiré, saves intactes
    let a2 = overrides_for(&srv, "/o2.zip", make_zip(&[("resourcepacks/r.zip", b"r2"), ("shaderpacks/s.zip", b"s2")]));
    run(&srv, &dirs, &pack, Some(a2.clone()), &Arc::new(Progress::default())).unwrap();
    assert_eq!(fs::read(inst.join("resourcepacks/r.zip")).unwrap(), b"r2");
    assert!(inst.join("shaderpacks/s.zip").is_file() && !inst.join("datapacks/d.zip").exists());
    assert_eq!(fs::read(inst.join("saves/w/level.dat")).unwrap(), b"save");
    assert_ne!(install::installed_hash(&dirs), Some(catalog::pack_version_hash(&index, Some(&a1))));

    // le pack perd ses overrides : ses fichiers disparaissent, la sauvegarde reste
    run(&srv, &dirs, &pack, None, &Arc::new(Progress::default())).unwrap();
    assert!(!inst.join("resourcepacks/r.zip").exists() && inst.join("saves/w/level.dat").is_file());
    assert_eq!(install::installed_hash(&dirs).unwrap(), index);
}

#[test]
fn wrong_sha256_or_oversized_archive_is_refused_and_leaves_no_marker() {
    let _g = env_lock();
    let srv = Server::start();
    let t = Tmp::new("ovr-bad");
    let pack = t.0.join("pack");
    write_packwiz(&pack, &srv, "/packs/a", "1.0.0", &[]);
    let dirs = Dirs::new(t.0.join("data")).for_pack("alpha").unwrap();
    let good = overrides_for(&srv, "/o.zip", make_zip(&[("resourcepacks/r.zip", b"r")]));
    let wrong = Overrides { sha256: "0".repeat(64), ..good.clone() };
    assert!(run(&srv, &dirs, &pack, Some(wrong), &Arc::new(Progress::default())).is_err());
    let small = Overrides { size: 10, ..good };
    assert!(run(&srv, &dirs, &pack, Some(small), &Arc::new(Progress::default())).is_err());
    assert!(!dirs.install_marker().exists() && !dirs.instance().join("resourcepacks").exists());
}
