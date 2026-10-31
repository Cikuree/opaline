//! Cas 2 : deux packs dans deux instances, Java et assets partagés, `installed.ok` par instance,
//! différence installé / publié (`pack_changelog`).
#![allow(dead_code)]
mod opaline_common;
mod support;

use opaline_common::*;
use opaline_core::changelog::{self, Changelog, Updated};
use opaline_core::config::Dirs;
use opaline_core::{packwiz, service};
use std::fs;
use support::Server;

const ALPHA_V1: &[ModSpec] = &[
    ModSpec { name: "Sodium", filename: "sodium-0.9.2.jar", body: b"sodium-092" },
    ModSpec { name: "Iris", filename: "iris-1.0.jar", body: b"iris-10" },
    ModSpec { name: "Gone", filename: "gone-1.jar", body: b"gone" },
];

#[test]
fn two_packs_share_java_and_assets_but_not_instances() {
    let srv = Server::start();
    let ep = install_endpoints(&srv);
    let t = Tmp::new("multi");
    let root = Dirs::new(t.0.join("root"));
    let alpha = root.for_pack("alpha").unwrap();
    let beta = root.for_pack("beta").unwrap();

    let a_src = t.0.join("alpha");
    write_packwiz(&a_src, &srv, "/packs/alpha", "1.0.0", ALPHA_V1);
    install_pack_dir(&ep, &alpha, &a_src).expect("installation alpha");

    let b_src = t.0.join("beta");
    write_packwiz(&b_src, &srv, "/packs/beta", "1.0.0", &[ModSpec { name: "Lithium", filename: "lithium-0.14.jar", body: b"lithium" }]);
    let before = srv.requests().len();
    install_pack_dir(&ep, &beta, &b_src).expect("installation beta");
    let during: Vec<String> = srv.requests()[before..].iter().map(|r| r.path.clone()).collect();

    // Java et assets/bibliothèques du dossier partagé : rien n'est retéléchargé pour beta.
    for heavy in ["/adoptium/", "/mc/client.jar", "/lib/", "/assets/"] {
        assert!(!during.iter().any(|p| p.starts_with(heavy)), "retéléchargé pour beta : {heavy} ({during:?})");
    }
    assert!(alpha.java(25).starts_with(root.shared()) && alpha.java(25) == beta.java(25));
    assert!(alpha.assets().join("indexes/26.json").is_file());

    // Instances séparées : mods propres à chaque pack, aucun ménage croisé.
    assert!(alpha.instance().join("mods/sodium-0.9.2.jar").is_file());
    assert!(beta.instance().join("mods/lithium-0.14.jar").is_file());
    assert!(!beta.instance().join("mods/sodium-0.9.2.jar").exists());
    assert!(alpha.instance().join("mods/sodium-0.9.2.jar").is_file(), "le ménage de beta a touché alpha");

    // installed.ok par instance, avec le hash d'index propre à chaque pack.
    assert!(alpha.install_marker().is_file() && beta.install_marker().is_file());
    assert_ne!(installed_hash_of(&alpha), installed_hash_of(&beta));
    assert_eq!(opaline_core::install::installed_hash(&alpha).as_deref(), Some(sha256_of_index(&a_src).as_str()));

    // Supprimer le marqueur d'un pack n'affecte pas l'autre.
    fs::remove_file(beta.install_marker()).unwrap();
    assert!(alpha.install_marker().is_file());
    assert!(opaline_core::install::installed_hash(&beta).is_none());
}

fn installed_hash_of(d: &Dirs) -> Option<String> {
    opaline_core::install::installed_hash(d)
}

fn sha256_of_index(src: &std::path::Path) -> String {
    sha256_hex(&fs::read(src.join("index.toml")).unwrap())
}

#[test]
fn failing_second_install_keeps_first_pack_intact() {
    let srv = Server::start();
    let ep = install_endpoints(&srv);
    let t = Tmp::new("multi-fail");
    let root = Dirs::new(t.0.join("root"));
    let alpha = root.for_pack("alpha").unwrap();
    let beta = root.for_pack("beta").unwrap();
    let a_src = t.0.join("alpha");
    write_packwiz(&a_src, &srv, "/packs/alpha", "1.0.0", ALPHA_V1);
    install_pack_dir(&ep, &alpha, &a_src).unwrap();

    // Beta : manifeste d'un jar dont le hash ne correspond pas.
    let b_src = t.0.join("beta");
    write_packwiz(&b_src, &srv, "/packs/beta", "1.0.0", &[ModSpec { name: "X", filename: "x.jar", body: b"x" }]);
    srv.put("/packs/beta/x.jar", b"altered".to_vec());
    assert!(install_pack_dir(&ep, &beta, &b_src).is_err());
    assert!(alpha.install_marker().is_file(), "l'échec de beta a supprimé le marqueur d'alpha");
    assert!(alpha.instance().join("mods/iris-1.0.jar").is_file());
}

#[test]
fn changelog_reports_added_removed_and_updated_between_installed_and_published() {
    let _g = env_lock();
    let srv = Server::start();
    let ep = install_endpoints(&srv);
    let t = Tmp::new("chlog");
    let root = Dirs::new(t.0.join("root"));
    let alpha = root.for_pack("alpha").unwrap();

    let v1 = t.0.join("v1");
    write_packwiz(&v1, &srv, "/packs/alpha", "1.0.0", ALPHA_V1);
    install_pack_dir(&ep, &alpha, &v1).unwrap();

    // Version publiée : Sodium mis à jour, Iris inchangé, Gone retiré, Fresh ajouté.
    let v2 = t.0.join("v2");
    write_packwiz(
        &v2,
        &srv,
        "/packs/alpha",
        "2.0.0",
        &[
            ModSpec { name: "Sodium", filename: "sodium-0.9.3.jar", body: b"sodium-093" },
            ModSpec { name: "Iris", filename: "iris-1.0.jar", body: b"iris-10" },
            ModSpec { name: "Fresh", filename: "fresh-2.jar", body: b"fresh" },
        ],
    );
    srv.put("/pack.zip", pack_zip(&v2));
    let f = t.0.join("packs.json");
    fs::write(&f, catalog_json(&[catalog_entry("alpha", "fabric", &srv.url("/pack.zip"), &sha256_of_index(&v2))])).unwrap();
    set_env("OPALINE_PACKS_URL", f.to_str().unwrap());

    let c: Changelog = service::changelog(&root, "alpha").expect("changelog");
    unset_env("OPALINE_PACKS_URL");
    assert_eq!(c.added, ["Fresh"]);
    assert_eq!(c.removed, ["Gone"]);
    assert_eq!(c.updated, [Updated { name: "Sodium".into(), from: "0.9.2".into(), to: "0.9.3".into() }]);
    let j = serde_json::to_value(&c).unwrap();
    assert!(j.get("added").is_some() && j.get("removed").is_some() && j.get("updated").is_some());
    // Le dossier de travail du changelog est nettoyé.
    assert!(!alpha.downloads().join("changelog-alpha").exists());
    // Le manifeste installé n'a pas changé.
    assert_eq!(changelog::load_snapshot(&alpha).unwrap().mods.len(), 3);
    let _ = packwiz::load_pack(&v2).unwrap();
}
