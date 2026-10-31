//! CLI de développement : `opaline-cli install <id> [--pack <dossier packwiz>]`, `opaline-cli play <id>`, `opaline-cli official <id>` (profil dans le Launcher Minecraft),
//! options `--root <dossier de données>` et `--nick <pseudo hors ligne>`.
use anyhow::{Context, Result, bail};
use opaline_core::config::{Config, Dirs};
use opaline_core::{auth, download::Progress, service};
use std::path::PathBuf;
use std::sync::Arc;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (mut root, mut pack, mut nick) = (Dirs::default_root(), None::<PathBuf>, None::<String>);
    let mut pos = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--root" => root = PathBuf::from(it.next().context("--root attend une valeur")?),
            "--pack" => pack = Some(PathBuf::from(it.next().context("--pack attend une valeur")?)),
            "--nick" => nick = Some(it.next().context("--nick attend une valeur")?.clone()),
            _ => pos.push(a.as_str()),
        }
    }
    let dirs = Dirs::new(root);
    let (Some(cmd), Some(id)) = (pos.first().copied(), pos.get(1).copied()) else {
        bail!("usage : opaline-cli install|play <id> [--pack dir] [--root dir] [--nick pseudo]")
    };
    match cmd {
        "install" => {
            let progress = Arc::new(Progress::default());
            let p = progress.clone();
            let t = std::thread::spawn(move || {
                let mut last = String::new();
                while Arc::strong_count(&p) > 1 {
                    let s = format!(
                        "{} {}/{}",
                        p.stage.lock().unwrap(),
                        p.done.load(std::sync::atomic::Ordering::Relaxed),
                        p.total.load(std::sync::atomic::Ordering::Relaxed)
                    );
                    if s != last {
                        eprintln!("{s}");
                        last = s;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(500));
                }
            });
            let r = service::install_pack(&dirs, id, &progress, pack.as_deref());
            drop(progress);
            let _ = t.join();
            r
        }
        "play" => {
            let mut cfg = Config::load(&dirs);
            if let Some(n) = nick {
                cfg.nickname = n;
            }
            let mgr = auth::AuthManager::default();
            let session = service::session_for_play(&cfg, &mgr)?;
            let e = service::entry(&dirs, id)?;
            let mut l = service::launch_pack(&dirs, &e, &cfg, &session)?;
            eprintln!("jeu lancé (pid {}), journal : {}", l.child.id(), l.log.display());
            l.child.wait()?;
            Ok(())
        }
        "official" => {
            let mc = opaline_core::official::minecraft_dir().context("dossier .minecraft introuvable")?;
            let e = service::entry(&dirs, id)?;
            let l = service::launch_official(&dirs, &e, &Config::load(&dirs), &mc, true)?;
            eprintln!("profil « {} » écrit ; Launcher ouvert : {}", l.profile_name, l.opened);
            Ok(())
        }
        other => bail!("commande inconnue : {other} (install | play | official)"),
    }
}
