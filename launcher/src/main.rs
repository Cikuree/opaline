mod app;
mod auth;
mod config;
mod download;
mod fabric;
mod install;
mod java;
mod launch;
mod mojang;
mod packwiz;
mod safe;
mod stats;

use anyhow::{Context, Result, bail};
use config::{Config, Dirs};
use download::Progress;
use std::path::PathBuf;
use std::sync::Arc;

/// Installe (si `do_install`) puis, si `do_play`, lance le jeu. Partagé par l'interface et la CLI.
pub fn run_install(dirs: &Dirs, cfg: &Config, progress: &Arc<Progress>) -> Result<install::Installed> {
    let client = download::http_client()?;
    let params = install::InstallParams {
        dirs: dirs.clone(),
        pack_dir: PathBuf::from(&cfg.pack_dir),
        mod_jar: (!cfg.mod_jar.trim().is_empty()).then(|| PathBuf::from(&cfg.mod_jar)),
        endpoints: install::Endpoints::default(),
    };
    install::install(&client, &params, progress)
}

pub fn run_play(dirs: &Dirs, cfg: &Config, with_server: bool) -> Result<std::process::Child> {
    let pack = packwiz::load_pack(&PathBuf::from(&cfg.pack_dir))?;
    let inst = install::load_installed(dirs, &pack)?;
    let session = auth::Session::offline(cfg.nickname.trim()); // étape 3 : session Microsoft ici
    let env = mojang::Env::current(&[("is_quick_play_multiplayer", with_server)]);
    let server = cfg.server_host.trim();
    let cmd = launch::build_command(&launch::LaunchInput {
        java: &inst.java,
        version: &inst.version,
        fabric: &inst.fabric,
        dirs,
        session: &session,
        server: (with_server && !server.is_empty()).then_some(server),
        memory_mb: cfg.memory_mb,
        env: &env,
    })?;
    eprintln!("{}", launch::redact(&cmd));
    launch::spawn(&cmd, dirs)
}

fn cli(args: Vec<String>) -> Result<()> {
    let mut root = Dirs::default_root();
    let mut overrides: Vec<(String, String)> = Vec::new();
    let mut no_server = false;
    let mut it = args.iter().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--no-server" => no_server = true,
            "--root" | "--pack" | "--mod-jar" | "--nick" | "--host" => {
                let v = it.next().with_context(|| format!("{a} attend une valeur"))?.clone();
                if a == "--root" {
                    root = PathBuf::from(v);
                } else {
                    overrides.push((a.clone(), v));
                }
            }
            _ => {}
        }
    }
    let dirs = Dirs::new(root);
    let mut cfg = Config::load(&dirs);
    for (k, v) in overrides {
        match k.as_str() {
            "--pack" => cfg.pack_dir = v,
            "--mod-jar" => cfg.mod_jar = v,
            "--nick" => cfg.nickname = v,
            _ => cfg.server_host = v,
        }
    }
    match args[1].as_str() {
        "install" => {
            let progress = Arc::new(Progress::default());
            let p = progress.clone();
            let t = std::thread::spawn(move || {
                let mut last = String::new();
                while Arc::strong_count(&p) > 1 {
                    let s = format!("{} {}/{}", p.stage.lock().unwrap(), p.done.load(std::sync::atomic::Ordering::Relaxed), p.total.load(std::sync::atomic::Ordering::Relaxed));
                    if s != last {
                        eprintln!("{s}");
                        last = s;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(500));
                }
            });
            let r = run_install(&dirs, &cfg, &progress);
            drop(progress);
            let _ = t.join();
            r.map(|_| ())
        }
        "play" => {
            let mut child = run_play(&dirs, &cfg, !no_server)?;
            eprintln!("jeu lancé (pid {}), log : {}", child.id(), dirs.instance().join("logs/launcher-game.log").display());
            child.wait()?;
            Ok(())
        }
        other => bail!("commande inconnue : {other} (install | play)"),
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        return cli(args);
    }
    app::run().map_err(|e| anyhow::anyhow!("{e}"))
}
