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
mod perf;
mod ping;
mod safe;
mod stats;
mod sysmem;
mod tailscale;
mod update;

use anyhow::{Context, Result, bail};
use config::{Config, Dirs};
use download::Progress;
use std::path::PathBuf;
use std::sync::Arc;

/// Installe (si `do_install`) puis, si `do_play`, lance le jeu. Partagé par l'interface et la CLI.
/// Une panique (fil de téléchargement, JSON inattendu...) devient une erreur : l'interface ne reste jamais figée.
pub fn run_install(dirs: &Dirs, cfg: &Config, progress: &Arc<Progress>) -> Result<install::Installed> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_install_inner(dirs, cfg, progress)))
        .unwrap_or_else(|_| bail!("erreur interne pendant l'installation (panique), voir la console"))
}

fn run_install_inner(dirs: &Dirs, cfg: &Config, progress: &Arc<Progress>) -> Result<install::Installed> {
    let client = download::http_client()?;
    // Pack local (option avancée, dev) ou pack.zip de la dernière release.
    let pack_dir = if cfg.pack_dir.trim().is_empty() {
        progress.set_stage("Téléchargement du pack");
        packwiz::fetch_remote_pack(&client, dirs, &config::PackProfile::default().pack_zip_url)?
    } else {
        PathBuf::from(&cfg.pack_dir)
    };
    let params = install::InstallParams {
        dirs: dirs.clone(),
        pack_dir,
        mod_jar: (!cfg.mod_jar.trim().is_empty()).then(|| PathBuf::from(&cfg.mod_jar)),
        endpoints: install::Endpoints::default(),
    };
    install::install(&client, &params, progress)
}

/// Session pour jouer : hors ligne en dev (`CLASSROOM_DEV_OFFLINE=1`), sinon Microsoft (mémoire, puis trousseau).
pub fn session_for_play(cfg: &Config, mgr: &auth::AuthManager) -> Result<auth::Session> {
    if auth::dev_offline() {
        return Ok(auth::Session::offline(cfg.nickname.trim()));
    }
    if !auth::configured() {
        bail!("Connexion Microsoft non configurée");
    }
    let client = auth::http_client()?;
    mgr.ensure_session(&client, &auth::Endpoints::default(), auth::CLIENT_ID, !cfg.no_remember)?
        .context("connexion Microsoft requise : cliquez sur « Se connecter avec Microsoft »")
}

/// Adresse du serveur pour cette exécution (`classroom.<suffixe MagicDNS>` si connu). Jamais loggée.
pub fn resolve_server(suffix: Option<&str>) -> String {
    let p = config::PackProfile::default();
    p.server_addr_for(&tailscale::server_host(&p.server_name, suffix))
}

/// Vérification légère (launcher.json, 5 s) : le pack publié diffère-t-il de l'installé ?
/// Hors ligne, erreur ou pack local : faux (on joue avec l'installé).
pub fn pack_update_available(dirs: &Dirs, cfg: &Config) -> bool {
    if !cfg.pack_dir.trim().is_empty() {
        return false;
    }
    let Ok(inst) = install::load_installed(dirs) else { return false };
    let Ok(client) = reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(5)).build() else { return false };
    update::fetch_info(&client, &config::launcher_json_url()).is_ok_and(|i| update::pack_changed(&i, inst.index_hash.as_deref()))
}

pub fn run_play(dirs: &Dirs, cfg: &Config, session: &auth::Session) -> Result<std::process::Child> {
    let inst = install::load_installed(dirs)?;
    let server = resolve_server(tailscale::status().magic_dns_suffix.as_deref());
    let memory_mb = perf::apply(&dirs.instance(), cfg.perf_mode, inst.shader_pack.as_deref(), sysmem::total_ram_mb())?.unwrap_or(cfg.memory_mb);
    let env = mojang::Env::current(&[("is_quick_play_multiplayer", true)]);
    let cmd = launch::build_command(&launch::LaunchInput {
        java: &inst.java,
        version: &inst.version,
        fabric: &inst.fabric,
        dirs,
        session,
        server: Some(&server),
        memory_mb,
        env: &env,
    })?;
    eprintln!("{}", launch::redact(&cmd, session.access_token(), Some(&server)));
    launch::spawn(&cmd, dirs)
}

fn cli(args: Vec<String>) -> Result<()> {
    let mut root = Dirs::default_root();
    let mut overrides: Vec<(String, String)> = Vec::new();
    let mut it = args.iter().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--root" | "--pack" | "--mod-jar" | "--nick" => {
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
            _ => {}
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
            let mgr = auth::AuthManager::default();
            let session = session_for_play(&cfg, &mgr)?;
            if let Some(w) = mgr.take_warning() {
                eprintln!("avertissement : {w}");
            }
            let mut child = run_play(&dirs, &cfg, &session)?;
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
