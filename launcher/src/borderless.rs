//! « Plein écran fenêtré » : Opaline installe pour chaque pack le mod Modrinth Cubes Without Borders (client seulement), via le
//! mécanisme de Mes mods mais marqué `managed: "borderless"` dans l'index (invisible dans la liste, jamais retiré à la main),
//! et écrit sa configuration. Vérifié dans les jars 5.0.0 (1.21.x Fabric et 26.3) : `CWBConfig` lit `config/cwb.json` (Gson,
//! `fullscreenMode` / `preferredFullscreenMode` valent `OFF`, `EXCLUSIVE`, `BORDERLESS` ou `WINDOWED`) ; le mode s'applique quand
//! Minecraft passe en plein écran, d'où aussi `fullscreen:true` dans `options.txt` (fusion habituelle).
use crate::config::Dirs;
use crate::extramods::{self, Index, Resolved};
use crate::modrinth::Api;
use crate::perf;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Projet Modrinth du mod.
pub const SLUG: &str = "cubes-without-borders";
/// Valeur de `managed` dans l'index de Mes mods.
pub const TAG: &str = "borderless";

/// Issue pour un pack, mémorisée dans `opaline-borderless.json` de l'instance.
pub const INSTALLED: &str = "installed";
pub const PROVIDED: &str = "provided";
pub const NO_VERSION: &str = "noVersion";
pub const INCOMPATIBLE: &str = "incompatible";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct State {
    pub state: String,
    /// `<minecraft>/<loader>` pour lequel l'issue a été décidée : changer de version du pack relance la résolution.
    pub target: String,
}

fn state_path(dirs: &Dirs) -> PathBuf {
    dirs.instance().join("opaline-borderless.json")
}

pub fn state(dirs: &Dirs) -> Option<State> {
    serde_json::from_str(&std::fs::read_to_string(state_path(dirs)).ok()?).ok()
}

fn save_state(dirs: &Dirs, target: &str, state: &str) -> Result<()> {
    Ok(std::fs::write(state_path(dirs), serde_json::to_vec(&State { state: state.into(), target: target.into() })?)?)
}

fn managed_files(idx: &Index) -> Vec<String> {
    idx.mods.iter().filter(|e| e.managed.as_deref() == Some(TAG)).map(|e| e.file.clone()).collect()
}

/// Le pack contient déjà le mod (nom de fichier ou titre du manifeste).
fn provided_by_pack(dirs: &Dirs) -> bool {
    let p = extramods::pack_info(dirs);
    let hit = |s: &str| {
        let s = s.to_ascii_lowercase().replace([' ', '_', '-'], "");
        s.contains("cubeswithoutborders") || s.starts_with("cwb")
    };
    p.names.iter().any(|n| hit(n)) || p.files.iter().any(|f| hit(f))
}

/// Écrit `fullscreenMode` / `preferredFullscreenMode` = `BORDERLESS` dans `config/cwb.json` (les autres clés sont conservées ; un
/// fichier illisible n'est pas touché) et `fullscreen:true` dans `options.txt`.
fn write_config(dirs: &Dirs) -> Result<()> {
    let inst = dirs.instance();
    let path = inst.join("config").join("cwb.json");
    let mut v: serde_json::Value = match std::fs::read_to_string(&path) {
        Ok(t) => serde_json::from_str(&t).ok().filter(serde_json::Value::is_object).ok_or_else(|| anyhow::anyhow!("cwb.json illisible : non modifié"))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => serde_json::json!({}),
        Err(e) => return Err(e.into()),
    };
    v["fullscreenMode"] = "BORDERLESS".into();
    v["preferredFullscreenMode"] = "BORDERLESS".into();
    std::fs::create_dir_all(inst.join("config"))?;
    std::fs::write(&path, serde_json::to_vec_pretty(&v)?)?;
    perf::merge_file(&inst.join("options.txt"), ':', &[("fullscreen", "true".to_string())])
}

fn set_managed(dirs: &Dirs, files: &[String]) -> Result<()> {
    let inst = dirs.instance();
    let mut idx = Index::load(&inst);
    for e in idx.mods.iter_mut().filter(|e| files.contains(&e.file)) {
        e.managed = Some(TAG.to_string());
    }
    idx.save(&inst)
}

/// Retire le mod géré (et ses dépendances gérées) de Mes mods et de `mods/`.
fn remove_managed(dirs: &Dirs) -> Result<()> {
    let inst = dirs.instance();
    let mut idx = Index::load(&inst);
    let files = managed_files(&idx);
    idx.mods.retain(|e| e.managed.as_deref() != Some(TAG));
    idx.save(&inst)?;
    let pack = extramods::pack_info(dirs);
    for f in files {
        let _ = std::fs::remove_file(extramods::extra_dir(&inst).join(&f));
        if !pack.files.contains(&f) {
            let _ = std::fs::remove_file(inst.join("mods").join(&f));
        }
    }
    Ok(())
}

/// Active pour ce pack et renvoie l'issue (`INSTALLED`, `PROVIDED`, `NO_VERSION`, `INCOMPATIBLE`). Erreur (réseau...) : rien n'est
/// décidé, l'appelant lance quand même le jeu. Sans réseau si l'issue est déjà connue pour ce Minecraft / loader.
pub fn enable(dirs: &Dirs, api: &dyn Api, mc: &str, loader: &str, fetch: &dyn Fn(&Resolved, &Path) -> Result<()>) -> Result<String> {
    if !dirs.install_marker().is_file() {
        bail!("pack non installé");
    }
    let target = format!("{mc}/{loader}");
    let idx = Index::load(&dirs.instance());
    let known = state(dirs).filter(|s| s.target == target);
    if let Some(s) = &known {
        if s.state == INSTALLED && !managed_files(&idx).is_empty() {
            write_config(dirs)?; // idempotent : le joueur a pu réécrire le fichier
            return Ok(INSTALLED.into());
        }
        if s.state != INSTALLED {
            return Ok(s.state.clone());
        }
    }
    remove_managed(dirs)?; // ancienne version pour un autre Minecraft ou jar disparu : on repart propre
    if provided_by_pack(dirs) {
        save_state(dirs, &target, PROVIDED)?;
        return Ok(PROVIDED.into());
    }
    let idx = Index::load(&dirs.instance());
    let plan = match extramods::plan(api, &extramods::pack_info(dirs), &idx, SLUG, mc, loader) {
        Ok(p) => p,
        Err(e) => {
            let m = format!("{e:#}");
            let outcome = if m.contains("aucune version compatible") {
                NO_VERSION
            } else if m.contains("est incompatible avec") {
                INCOMPATIBLE
            } else if m.contains("déjà fourni par le pack") {
                PROVIDED
            } else {
                return Err(e); // réseau ou réponse illisible : pas de décision mémorisée
            };
            save_state(dirs, &target, outcome)?;
            return Ok(outcome.into());
        }
    };
    let res = extramods::install_plan(dirs, plan, fetch)?;
    let files: Vec<String> = std::iter::once(res.added.file).chain(res.deps.into_iter().map(|d| d.file)).collect();
    set_managed(dirs, &files)?;
    write_config(dirs)?;
    save_state(dirs, &target, INSTALLED)?;
    Ok(INSTALLED.into())
}

/// Désactive : retire le mod géré, remet `fullscreen:false` (si `options.txt` existe) et oublie l'issue.
pub fn disable(dirs: &Dirs) -> Result<()> {
    if state(dirs).is_none() && managed_files(&Index::load(&dirs.instance())).is_empty() {
        return Ok(()); // rien à défaire : aucune écriture à chaque lancement
    }
    remove_managed(dirs)?;
    let opts = dirs.instance().join("options.txt");
    if opts.is_file() && state(dirs).is_some_and(|s| s.state == INSTALLED) {
        perf::merge_file(&opts, ':', &[("fullscreen", "false".to_string())])?;
    }
    let _ = std::fs::remove_file(state_path(dirs));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modrinth::{Dependency, Hit, Version, VersionFile};
    use std::cell::Cell;
    use std::fs;

    struct Fake {
        has_version: bool,
        incompatible: Option<&'static str>,
        calls: Cell<u32>,
    }

    impl Api for Fake {
        fn search(&self, _: &str, _: &str, _: &str) -> Result<Vec<Hit>> {
            Ok(vec![])
        }
        fn versions(&self, _: &str, _: &str, _: &str) -> Result<Vec<Version>> {
            self.calls.set(self.calls.get() + 1);
            if !self.has_version {
                return Ok(vec![]);
            }
            Ok(vec![Version {
                id: "v".into(),
                version_number: "5.0.0".into(),
                version_type: "release".into(),
                files: vec![VersionFile {
                    url: format!("{}data/x/cwb.jar", crate::modrinth::CDN),
                    filename: "cwb-5.0.0.jar".into(),
                    primary: true,
                    size: Some(10),
                    hashes: [("sha512".to_string(), "h".to_string())].into(),
                }],
                dependencies: self
                    .incompatible
                    .map(|p| Dependency { project_id: Some(p.into()), dependency_type: "incompatible".into() })
                    .into_iter()
                    .collect(),
            }])
        }
        fn title(&self, p: &str) -> Option<String> {
            Some(if p == SLUG { "Cubes Without Borders".into() } else { p.to_string() })
        }
    }

    fn fake(has_version: bool) -> Fake {
        Fake { has_version, incompatible: None, calls: Cell::new(0) }
    }

    fn put(_: &Resolved, dest: &Path) -> Result<()> {
        fs::write(dest, b"PK\x03\x04jar")?;
        Ok(())
    }

    fn pack(tag: &str) -> Dirs {
        let d = Dirs::new(std::env::temp_dir().join(format!("opaline-bl-{tag}-{}", std::process::id())));
        let _ = fs::remove_dir_all(&d.root);
        let d = d.for_pack("p").unwrap();
        fs::create_dir_all(d.instance()).unwrap();
        fs::write(d.install_marker(), "ok").unwrap();
        d
    }

    #[test]
    fn enable_installs_a_hidden_managed_mod_and_writes_the_config_then_disable_undoes_it() {
        let d = pack("on");
        fs::write(d.instance().join("options.txt"), "version:4000\nfullscreen:false\nlang:fr_fr\n").unwrap();
        fs::create_dir_all(d.instance().join("config")).unwrap();
        fs::write(d.instance().join("config/cwb.json"), r#"{"autoHideDock":false}"#).unwrap();
        let api = fake(true);
        assert_eq!(enable(&d, &api, "1.21.1", "fabric", &put).unwrap(), INSTALLED);
        let idx = Index::load(&d.instance());
        assert_eq!(idx.mods.len(), 1);
        assert_eq!(idx.mods[0].managed.as_deref(), Some(TAG));
        assert!(extramods::list(&d).is_empty(), "invisible dans Mes mods");
        assert!(extramods::remove(&d, "cwb-5.0.0.jar").is_err(), "jamais retiré à la main");
        let cfg: serde_json::Value = serde_json::from_str(&fs::read_to_string(d.instance().join("config/cwb.json")).unwrap()).unwrap();
        assert_eq!(
            (cfg["fullscreenMode"].as_str(), cfg["preferredFullscreenMode"].as_str(), cfg["autoHideDock"].as_bool()),
            (Some("BORDERLESS"), Some("BORDERLESS"), Some(false))
        );
        assert_eq!(fs::read_to_string(d.instance().join("options.txt")).unwrap(), "version:4000\nfullscreen:true\nlang:fr_fr\n");
        // il est recopié dans mods/ au lancement, comme Mes mods
        extramods::sync(&d.instance(), &Default::default()).unwrap();
        assert!(d.instance().join("mods/cwb-5.0.0.jar").is_file());
        // même Minecraft : plus aucun appel réseau
        let calls = api.calls.get();
        assert_eq!(enable(&d, &api, "1.21.1", "fabric", &put).unwrap(), INSTALLED);
        assert_eq!(api.calls.get(), calls);
        disable(&d).unwrap();
        assert!(
            Index::load(&d.instance()).mods.is_empty()
                && !d.instance().join("mods/cwb-5.0.0.jar").exists()
                && !extramods::extra_dir(&d.instance()).join("cwb-5.0.0.jar").exists()
        );
        assert_eq!(fs::read_to_string(d.instance().join("options.txt")).unwrap(), "version:4000\nfullscreen:false\nlang:fr_fr\n");
        assert!(state(&d).is_none());
        fs::remove_dir_all(&d.root).unwrap();
    }

    #[test]
    fn user_jars_survive_disable_and_a_new_minecraft_reinstalls() {
        let d = pack("mine");
        extramods::add_bytes(&d, "zoomify.jar", b"PK\x03\x04z").unwrap();
        let api = fake(true);
        enable(&d, &api, "1.21.1", "fabric", &put).unwrap();
        assert_eq!(extramods::list(&d).len(), 1, "seul le jar de l'utilisateur est listé");
        enable(&d, &api, "26.3", "fabric", &put).unwrap(); // pack passé en 26.3 : on repart propre, un seul exemplaire
        assert_eq!(Index::load(&d.instance()).mods.len(), 1);
        assert_eq!(state(&d).unwrap().target, "26.3/fabric");
        disable(&d).unwrap();
        assert_eq!(extramods::list(&d).len(), 1);
        assert!(extramods::extra_dir(&d.instance()).join("zoomify.jar").is_file());
        fs::remove_dir_all(&d.root).unwrap();
    }

    #[test]
    fn pack_that_already_ships_the_mod_gets_nothing() {
        let d = pack("prov");
        crate::changelog::save_snapshot(
            &d,
            &crate::changelog::Snapshot {
                mods: vec![crate::changelog::SnapMod {
                    name: "Cubes Without Borders".into(),
                    version: "5".into(),
                    side: "client".into(),
                    dir: "mods".into(),
                    file: "cubeswithoutborders-5.jar".into(),
                    hash_format: "sha1".into(),
                    hash: "0".into(),
                    project_id: None,
                }],
            },
        )
        .unwrap();
        let api = fake(true);
        assert_eq!(enable(&d, &api, "1.21.1", "fabric", &put).unwrap(), PROVIDED);
        assert_eq!((api.calls.get(), Index::load(&d.instance()).mods.len()), (0, 0));
        assert!(!d.instance().join("config/cwb.json").exists());
        fs::remove_dir_all(&d.root).unwrap();
    }

    #[test]
    fn no_compatible_version_or_an_incompatible_mod_ignores_the_pack_and_remembers_it() {
        let d = pack("nov");
        let api = fake(false);
        assert_eq!(enable(&d, &api, "1.21.1", "fabric", &put).unwrap(), NO_VERSION);
        assert!(Index::load(&d.instance()).mods.is_empty() && !d.instance().join("config/cwb.json").exists());
        let calls = api.calls.get();
        assert_eq!(enable(&d, &api, "1.21.1", "fabric", &put).unwrap(), NO_VERSION); // mémorisé : pas de nouvel appel
        assert_eq!(api.calls.get(), calls);
        let d2 = pack("inc");
        crate::changelog::save_snapshot(
            &d2,
            &crate::changelog::Snapshot {
                mods: vec![crate::changelog::SnapMod {
                    name: "Autre".into(),
                    version: "1".into(),
                    side: "client".into(),
                    dir: "mods".into(),
                    file: "autre.jar".into(),
                    hash_format: "sha1".into(),
                    hash: "0".into(),
                    project_id: Some("XtOSFWzb".into()),
                }],
            },
        )
        .unwrap();
        let api2 = Fake { has_version: true, incompatible: Some("XtOSFWzb"), calls: Cell::new(0) };
        assert_eq!(enable(&d2, &api2, "1.21.1", "fabric", &put).unwrap(), INCOMPATIBLE);
        fs::remove_dir_all(&d.root).unwrap();
        fs::remove_dir_all(&d2.root).unwrap();
    }

    #[test]
    fn offline_is_an_error_without_a_memorised_outcome_and_a_broken_config_is_left_alone() {
        struct Down;
        impl Api for Down {
            fn search(&self, _: &str, _: &str, _: &str) -> Result<Vec<Hit>> {
                bail!("Modrinth injoignable")
            }
            fn versions(&self, _: &str, _: &str, _: &str) -> Result<Vec<Version>> {
                bail!("Modrinth injoignable")
            }
            fn title(&self, _: &str) -> Option<String> {
                None
            }
        }
        let d = pack("off");
        assert!(enable(&d, &Down, "1.21.1", "fabric", &put).is_err());
        assert!(state(&d).is_none() && Index::load(&d.instance()).mods.is_empty());
        fs::create_dir_all(d.instance().join("config")).unwrap();
        fs::write(d.instance().join("config/cwb.json"), "pas du json").unwrap();
        let api = fake(true);
        assert!(enable(&d, &api, "1.21.1", "fabric", &put).is_err());
        assert_eq!(fs::read_to_string(d.instance().join("config/cwb.json")).unwrap(), "pas du json");
        fs::remove_dir_all(&d.root).unwrap();
    }

    /// Essai réseau réel (Modrinth) sur une instance temporaire : `cargo test -- --ignored borderless_real`.
    #[test]
    #[ignore = "réseau : Modrinth"]
    fn borderless_real_network_fabric_1_21_1_and_26_3() {
        for mc in ["1.21.1", "26.3"] {
            let d = pack(&format!("real-{}", mc.replace('.', "")));
            let api = crate::modrinth::Http::default_api().unwrap();
            let client = crate::download::http_client().unwrap();
            let out = enable(&d, &api, mc, "fabric", &|r, dest| extramods::download_job(&client, r, dest)).unwrap();
            assert_eq!(out, INSTALLED, "{mc}");
            let idx = Index::load(&d.instance());
            assert!(idx.mods.iter().all(|e| e.managed.as_deref() == Some(TAG)) && !idx.mods.is_empty());
            let jar = extramods::extra_dir(&d.instance()).join(&idx.mods[0].file);
            assert_eq!(extramods::jar_mod_id(&jar).as_deref(), Some("cwb"), "{mc}");
            assert!(fs::read_to_string(d.instance().join("config/cwb.json")).unwrap().contains("BORDERLESS"));
            disable(&d).unwrap();
            fs::remove_dir_all(&d.root).unwrap();
        }
    }
}
