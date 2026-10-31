//! Backend Tauri d'Opaline : une commande par entrée du contrat (docs/OPALINE_CONTRACT.md §4), événements §6.
//! Tout le travail bloquant (réseau, disque, processus) passe par `blocking` : l'interface ne se fige jamais.
//! Les erreurs renvoyées à l'interface sont nettoyées (`sanitize`) : ni jeton, ni adresse de serveur, ni chemin personnel.
use anyhow::{Context, anyhow, bail};
use base64::Engine;
use opaline_core::config::{Config, Dirs};
use opaline_core::download::Progress;
use opaline_core::perf::PerfMode;
use opaline_core::{auth, friends, head, service, skins};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

type CmdResult<T> = Result<T, String>;

struct Inner {
    root: Dirs,
    auth: auth::AuthManager,
    /// Une seule installation à la fois : les dossiers partagés (Java, bibliothèques) ne supportent pas deux écritures concurrentes.
    install_lock: tokio::sync::Mutex<()>,
    installing: Mutex<HashSet<String>>,
    running: Mutex<HashSet<String>>,
    login_cancel: Mutex<Option<Arc<AtomicBool>>>,
    skins_synced: AtomicBool,
}

#[derive(Clone)]
struct S(Arc<Inner>);

impl S {
    fn sanitize(&self, e: anyhow::Error) -> String {
        service::sanitize_error(&format!("{e}"), &self.0.root)
    }

    /// Exécute `f` hors du fil de l'interface ; l'erreur est nettoyée.
    async fn blocking<T: Send + 'static>(&self, f: impl FnOnce(&Dirs) -> anyhow::Result<T> + Send + 'static) -> CmdResult<T> {
        let s = self.clone();
        let r = tauri::async_runtime::spawn_blocking(move || {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&s.0.root))).unwrap_or_else(|_| Err(anyhow!("erreur interne")))
        })
        .await
        .map_err(|_| "erreur interne".to_string())?;
        r.map_err(|e| self.sanitize(e))
    }

    fn config(&self) -> Config {
        Config::load(&self.0.root)
    }

    /// Jeton Minecraft valide (rafraîchi si besoin). Jamais en mode hors ligne de dev.
    fn token(&self) -> anyhow::Result<String> {
        if auth::dev_offline() {
            bail!("connexion Microsoft requise (mode hors ligne de développement)");
        }
        let session = service::session_for_play(&self.config(), &self.0.auth)?;
        Ok(session.access_token().to_string())
    }

    fn sync_skins_once(&self) {
        if self.0.skins_synced.swap(true, Ordering::SeqCst) {
            return;
        }
        let s = self.clone();
        std::thread::spawn(move || {
            let (Ok(token), Ok(c)) = (s.token(), auth::http_client()) else { return };
            let _ = skins::sync_current(&c, &skins::Endpoints::default(), &token, &skins::Wardrobe::new(s.0.root.skins()));
        });
    }
}

#[derive(Serialize, Clone)]
struct Profile {
    name: String,
    uuid: String,
}

// ---------- Application ----------

#[tauri::command]
async fn app_info(app: AppHandle, state: State<'_, S>) -> CmdResult<Value> {
    let version = app.package_info().version.to_string();
    let v = version.clone();
    let update = state.blocking(move |_| Ok(service::app_update(&v))).await?;
    Ok(json!({ "version": version, "update": update.map(|u| json!({ "version": u.version, "url": u.url })) }))
}

#[tauri::command]
async fn settings_get(state: State<'_, S>) -> CmdResult<service::SettingsView> {
    state.blocking(|r| Ok(service::settings_get(r))).await
}

#[tauri::command]
async fn settings_set(state: State<'_, S>, perf_mode: Option<PerfMode>, memory_mb: Option<u32>, no_remember: Option<bool>) -> CmdResult<service::SettingsView> {
    state.blocking(move |r| service::settings_set(r, perf_mode, memory_mb, no_remember)).await
}

// ---------- Authentification ----------

fn profile_json(p: Option<Profile>) -> Value {
    p.map_or(Value::Null, |p| json!({ "name": p.name, "uuid": p.uuid }))
}

#[tauri::command]
async fn auth_status(state: State<'_, S>) -> CmdResult<Value> {
    let s = state.inner().clone();
    let s2 = s.clone();
    let out = s
        .blocking(move |_| {
            let cfg = s2.config();
            if auth::dev_offline() {
                let p = Profile { name: cfg.nickname.clone(), uuid: auth::offline_uuid(&cfg.nickname) };
                return Ok(json!({ "state": "signedIn", "profile": profile_json(Some(p)), "warning": null }));
            }
            if !auth::configured() {
                return Ok(json!({ "state": "notConfigured", "profile": null, "warning": null }));
            }
            let c = auth::http_client()?;
            let session = s2.0.auth.ensure_session(&c, &auth::Endpoints::default(), auth::CLIENT_ID, !cfg.no_remember).ok().flatten();
            let warning = s2.0.auth.take_warning();
            if session.is_some() {
                s2.sync_skins_once();
            }
            let p = session.map(|s| Profile { name: s.username, uuid: s.uuid });
            Ok(json!({ "state": if p.is_some() { "signedIn" } else { "signedOut" }, "profile": profile_json(p), "warning": warning }))
        })
        .await?;
    Ok(out)
}

#[tauri::command]
async fn auth_start(app: AppHandle, state: State<'_, S>) -> CmdResult<Value> {
    if auth::dev_offline() || !auth::configured() {
        return Err("Connexion Microsoft non configurée".into());
    }
    let s = state.inner().clone();
    let cancel = Arc::new(AtomicBool::new(false));
    if let Some(old) = s.0.login_cancel.lock().unwrap().replace(cancel.clone()) {
        old.store(true, Ordering::SeqCst);
    }
    let (tx, rx) = std::sync::mpsc::channel::<Value>();
    let s2 = s.clone();
    std::thread::spawn(move || {
        let cfg = s2.config();
        let result = auth::http_client().and_then(|c| {
            s2.0.auth.login(
                &c,
                &auth::Endpoints::default(),
                auth::CLIENT_ID,
                !cfg.no_remember,
                |dc| {
                    let _ = tx.send(json!({ "userCode": dc.user_code, "verificationUri": dc.verification_uri, "expiresIn": dc.expires_in }));
                },
                &cancel,
            )
        });
        let payload = match result {
            Ok(session) => {
                s2.0.skins_synced.store(false, Ordering::SeqCst);
                s2.sync_skins_once();
                json!({ "ok": true, "profile": { "name": session.username, "uuid": session.uuid }, "error": null })
            }
            Err(e) => json!({ "ok": false, "profile": null, "error": s2.sanitize(e) }),
        };
        let _ = app.emit("auth://done", payload);
    });
    tauri::async_runtime::spawn_blocking(move || rx.recv_timeout(Duration::from_secs(30)))
        .await
        .map_err(|_| "erreur interne".to_string())?
        .map_err(|_| "Connexion Microsoft impossible : vérifiez votre réseau".to_string())
}

#[tauri::command]
async fn auth_cancel(state: State<'_, S>) -> CmdResult<()> {
    if let Some(c) = state.0.login_cancel.lock().unwrap().take() {
        c.store(true, Ordering::SeqCst);
    }
    Ok(())
}

#[tauri::command]
async fn auth_sign_out(state: State<'_, S>) -> CmdResult<Value> {
    let s = state.inner().clone();
    if let Some(c) = s.0.login_cancel.lock().unwrap().take() {
        c.store(true, Ordering::SeqCst);
    }
    state
        .blocking(move |_| {
            let r = s.0.auth.sign_out();
            s.0.skins_synced.store(false, Ordering::SeqCst);
            let warning = r.err().map(|e| format!("{e}")).or_else(|| s.0.auth.take_warning());
            Ok(json!({ "warning": warning }))
        })
        .await
}

// ---------- Packs ----------

#[tauri::command]
async fn packs_list(state: State<'_, S>) -> CmdResult<Vec<opaline_core::catalog::PackView>> {
    state.blocking(service::packs_list).await
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct InstallDone {
    pack_id: String,
    ok: bool,
    error: Option<String>,
}

/// Lance l'installation en tâche de fond ; progression par `install://progress`, fin par `install://done`.
async fn start_install(app: AppHandle, s: S, id: String) -> CmdResult<()> {
    if !s.0.installing.lock().unwrap().insert(id.clone()) {
        return Err("Une installation de ce pack est déjà en cours".into());
    }
    let progress = Arc::new(Progress::default());
    let finished = Arc::new(AtomicBool::new(false));
    {
        let (app, progress, finished, id) = (app.clone(), progress.clone(), finished.clone(), id.clone());
        tauri::async_runtime::spawn(async move {
            while !finished.load(Ordering::Relaxed) {
                let stage = progress.stage.lock().unwrap().clone();
                if !stage.is_empty() {
                    let _ = app.emit(
                        "install://progress",
                        json!({ "packId": id, "stage": stage, "done": progress.done.load(Ordering::Relaxed), "total": progress.total.load(Ordering::Relaxed) }),
                    );
                }
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
        });
    }
    tauri::async_runtime::spawn(async move {
        let _guard = s.0.install_lock.lock().await;
        let (s2, id2) = (s.clone(), id.clone());
        let r = s.blocking(move |root| service::install_pack(root, &id2, &progress, None)).await;
        finished.store(true, Ordering::Relaxed);
        s2.0.installing.lock().unwrap().remove(&id);
        let done = InstallDone { pack_id: id, ok: r.is_ok(), error: r.err() };
        let _ = app.emit("install://done", done);
    });
    Ok(())
}

#[tauri::command]
async fn pack_install(app: AppHandle, state: State<'_, S>, id: String) -> CmdResult<()> {
    start_install(app, state.inner().clone(), id).await
}

/// Réparation : l'installation revérifie chaque fichier (hash) et retélécharge ce qui manque ou est altéré.
#[tauri::command]
async fn pack_repair(app: AppHandle, state: State<'_, S>, id: String) -> CmdResult<()> {
    start_install(app, state.inner().clone(), id).await
}

#[tauri::command]
async fn pack_changelog(state: State<'_, S>, id: String) -> CmdResult<opaline_core::changelog::Changelog> {
    state.blocking(move |r| service::changelog(r, &id)).await
}

#[tauri::command]
async fn pack_mods(state: State<'_, S>, id: String) -> CmdResult<Vec<opaline_core::changelog::ModView>> {
    state.blocking(move |r| service::pack_mods(r, &id)).await
}

#[tauri::command]
async fn open_folder(app: AppHandle, state: State<'_, S>, id: String, kind: String) -> CmdResult<()> {
    let path = state
        .blocking(move |r| {
            let inst = r.for_pack(&id)?.instance();
            if !inst.is_dir() {
                bail!("ce pack n'est pas installé");
            }
            let dir = match kind.as_str() {
                "instance" => inst,
                "mods" | "shaderpacks" | "screenshots" | "logs" => inst.join(&kind),
                _ => bail!("dossier inconnu"),
            };
            std::fs::create_dir_all(&dir)?;
            Ok(dir)
        })
        .await?;
    app.opener().open_path(path.to_string_lossy(), None::<&str>).map_err(|_| "Impossible d'ouvrir le dossier".to_string())
}

#[tauri::command]
async fn textures_dir(state: State<'_, S>) -> CmdResult<Value> {
    state.blocking(|r| Ok(json!({ "dir": service::textures_dir(r).map(|d| d.to_string_lossy().into_owned()) }))).await
}

// ---------- Jeu ----------

#[tauri::command]
async fn play(app: AppHandle, state: State<'_, S>, id: String) -> CmdResult<Value> {
    let s = state.inner().clone();
    if !s.0.running.lock().unwrap().insert(id.clone()) {
        return Ok(json!({ "started": false, "reason": "Le jeu est déjà lancé pour ce pack" }));
    }
    let (s2, id2) = (s.clone(), id.clone());
    let launched = s
        .blocking(move |root| {
            let cfg = s2.config();
            let session = service::session_for_play(&cfg, &s2.0.auth)?;
            let e = service::entry(root, &id2)?;
            let l = service::launch_pack(root, &e, &cfg, &session)?;
            Ok((l, session.access_token().to_string()))
        })
        .await;
    match launched {
        Err(reason) => {
            s.0.running.lock().unwrap().remove(&id);
            Ok(json!({ "started": false, "reason": reason }))
        }
        Ok((mut l, token)) => {
            std::thread::spawn(move || {
                let code = l.child.wait().ok().and_then(|st| st.code());
                let tail = service::log_tail(&l.log, &token, &l.server, &s.0.root, 40);
                s.0.running.lock().unwrap().remove(&id);
                let _ = app.emit("game://exited", json!({ "packId": id, "code": code, "logTail": tail }));
            });
            Ok(json!({ "started": true, "reason": null }))
        }
    }
}

#[tauri::command]
async fn server_status(state: State<'_, S>, id: String) -> CmdResult<opaline_core::serverview::ServerView> {
    state.blocking(move |r| service::server_status(r, &id)).await
}

#[tauri::command]
async fn run_stats(state: State<'_, S>, id: String) -> CmdResult<Value> {
    Ok(state.blocking(move |r| service::run_stats(r, &id)).await?.unwrap_or(Value::Null))
}

#[tauri::command]
async fn run_history(state: State<'_, S>, id: String, run: Option<u32>) -> CmdResult<Value> {
    Ok(state.blocking(move |r| service::run_history(r, &id, run)).await?.unwrap_or(Value::Null))
}

// ---------- Skins, capes, têtes, amis ----------

#[tauri::command]
async fn skins_list(state: State<'_, S>) -> CmdResult<Vec<skins::SkinView>> {
    state.blocking(|r| Ok(skins::Wardrobe::new(r.skins()).list())).await
}

#[tauri::command]
async fn skin_import(state: State<'_, S>, name: String, model: String, png_base64: String) -> CmdResult<skins::SkinView> {
    state
        .blocking(move |r| {
            if png_base64.len() > skins::MAX_SKIN_BYTES * 4 / 3 + 16 {
                bail!("image trop lourde (256 Ko maximum)");
            }
            let png = base64::engine::general_purpose::STANDARD.decode(png_base64.trim()).map_err(|_| anyhow!("image illisible"))?;
            skins::Wardrobe::new(r.skins()).import(&name, &model, &png)
        })
        .await
}

#[tauri::command]
async fn skin_apply(state: State<'_, S>, id: String) -> CmdResult<()> {
    let s = state.inner().clone();
    state
        .blocking(move |r| {
            let w = skins::Wardrobe::new(r.skins());
            let (e, png) = w.get(&id)?;
            skins::apply_skin(&auth::http_client()?, &skins::Endpoints::default(), &s.token()?, &e.model, &png)?;
            w.set_active(Some(&id))
        })
        .await
}

#[tauri::command]
async fn skin_rename(state: State<'_, S>, id: String, name: String) -> CmdResult<()> {
    state.blocking(move |r| skins::Wardrobe::new(r.skins()).rename(&id, &name)).await
}

#[tauri::command]
async fn skin_delete(state: State<'_, S>, id: String) -> CmdResult<()> {
    state.blocking(move |r| skins::Wardrobe::new(r.skins()).delete(&id)).await
}

#[tauri::command]
async fn capes_list(state: State<'_, S>) -> CmdResult<Vec<skins::CapeView>> {
    let s = state.inner().clone();
    state.blocking(move |_| Ok(skins::parse_capes(&skins::get_profile(&auth::http_client()?, &skins::Endpoints::default(), &s.token()?)?))).await
}

#[tauri::command]
async fn cape_set(state: State<'_, S>, id: Option<String>) -> CmdResult<()> {
    let s = state.inner().clone();
    state.blocking(move |_| skins::set_cape(&auth::http_client()?, &skins::Endpoints::default(), &s.token()?, id.as_deref())).await
}

#[tauri::command]
async fn friends(state: State<'_, S>) -> CmdResult<Vec<friends::Friend>> {
    state.blocking(service::friends_list).await
}

#[tauri::command]
async fn player_head(state: State<'_, S>, uuid: String) -> CmdResult<Value> {
    state
        .blocking(move |r| {
            let c = auth::http_client().context("réseau indisponible")?;
            Ok(json!({ "dataUrl": head::player_head(r, &c, &uuid, head::SESSION_SERVER)? }))
        })
        .await
}

pub fn run() {
    let root = Dirs::new(Dirs::default_root());
    let state = S(Arc::new(Inner {
        root,
        auth: auth::AuthManager::default(),
        install_lock: tokio::sync::Mutex::new(()),
        installing: Mutex::default(),
        running: Mutex::default(),
        login_cancel: Mutex::default(),
        skins_synced: AtomicBool::new(false),
    }));
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state.clone())
        .setup(move |app| {
            // Le dossier de données peut être surchargé (OPALINE_HOME) : l'asset protocol y est alors autorisé à l'exécution.
            let tex = state.0.root.textures();
            std::fs::create_dir_all(&tex)?;
            app.asset_protocol_scope().allow_directory(&tex, true)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            auth_status,
            auth_start,
            auth_cancel,
            auth_sign_out,
            packs_list,
            pack_install,
            pack_changelog,
            pack_mods,
            pack_repair,
            play,
            server_status,
            run_stats,
            run_history,
            settings_get,
            settings_set,
            open_folder,
            textures_dir,
            skins_list,
            skin_import,
            skin_apply,
            skin_rename,
            skin_delete,
            capes_list,
            cape_set,
            friends,
            player_head
        ])
        .run(tauri::generate_context!())
        .expect("erreur au démarrage d'Opaline");
}
