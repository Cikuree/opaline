//! Backend Tauri d'Opaline : une commande par entrée du contrat (docs/OPALINE_CONTRACT.md §4), événements §6.
//! Tout le travail bloquant (réseau, disque, processus) passe par `blocking` : l'interface ne se fige jamais.
//! Les erreurs renvoyées à l'interface sont nettoyées (`sanitize`) : ni jeton, ni adresse de serveur, ni chemin personnel.
use anyhow::{Context, anyhow, bail};
use base64::Engine;
use opaline_core::config::{Config, Dirs, LaunchVia};
use opaline_core::download::Progress;
use opaline_core::perf::PerfMode;
use opaline_core::{auth, auth_browser, friends, head, identity, official, service, skins};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

type CmdResult<T> = Result<T, String>;

/// Services externes (injectables pour des tests sur fixtures ; production : valeurs par défaut).
#[derive(Clone)]
pub struct Services {
    pub install: opaline_core::install::Endpoints,
    pub skins: skins::Endpoints,
    pub session_server: String,
    /// API publique Mojang pseudo -> UUID (mode officiel).
    pub profile_api: String,
}

impl Default for Services {
    fn default() -> Services {
        Services {
            install: Default::default(),
            skins: Default::default(),
            session_server: head::SESSION_SERVER.to_string(),
            profile_api: identity::PROFILE_API.to_string(),
        }
    }
}

/// Packs en cours d'installation ou de jeu : une seule structure, un seul verrou (test-et-insertion atomiques).
#[derive(Default)]
struct Activity {
    installing: HashSet<String>,
    running: HashSet<String>,
}

#[derive(Default)]
struct Login {
    attempt: u64,
    cancel: Option<Arc<AtomicBool>>,
}

struct Inner {
    root: Dirs,
    svc: Services,
    auth: auth::AuthManager,
    /// Une seule installation à la fois : les dossiers partagés (Java, bibliothèques) ne supportent pas deux écritures concurrentes.
    install_lock: tokio::sync::Mutex<()>,
    activity: Mutex<Activity>,
    login: Mutex<Login>,
    /// Dernière adresse de serveur résolue (à masquer des erreurs).
    last_server: Mutex<Option<String>>,
    skins_synced: AtomicBool,
    /// Dernière connexion refusée par Mojang (app Azure non approuvée) : l'interface l'explique (mémoire seulement : le prochain essai tranche).
    mojang_pending: AtomicBool,
}

#[derive(Clone)]
struct S(Arc<Inner>);

impl S {
    fn sanitize(&self, e: anyhow::Error) -> String {
        let token = self.0.auth.current_token();
        let server = self.0.last_server.lock().unwrap().clone();
        let secrets: Vec<&str> = token.iter().chain(server.iter()).map(String::as_str).collect();
        service::sanitize_error_with(&format!("{e}"), &self.0.root, &secrets)
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
            let _ = skins::sync_current(&c, &s.0.svc.skins, &token, &skins::Wardrobe::new(s.0.root.skins()));
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
    Ok(json!({ "version": version, "update": update.map(|u| json!({ "version": u.version, "url": u.url, "auto": opaline_core::update::can_auto(&u) })) }))
}

/// Clé réservée dans `installing` pendant une mise à jour d'Opaline (aucun id de pack ne peut la prendre).
const UPDATING: &str = "<update>";

/// Télécharge l'installeur (vérifié), le lance, puis quitte Opaline. Progression : `update://progress`, échec : `update://done`.
#[tauri::command]
async fn app_update_install(app: AppHandle, state: State<'_, S>) -> CmdResult<()> {
    {
        let mut a = state.0.activity.lock().unwrap();
        if !a.running.is_empty() {
            return Err("Le jeu est en cours : fermez-le avant de mettre à jour Opaline".into());
        }
        if !a.installing.is_empty() {
            return Err("Une installation est en cours : attendez la fin avant de mettre à jour Opaline".into());
        }
        a.installing.insert(UPDATING.to_string());
    }
    let s = state.inner().clone();
    let version = app.package_info().version.to_string();
    tauri::async_runtime::spawn(async move {
        let emitter = app.clone();
        let r = s
            .blocking(move |root| {
                let path = service::app_update_download(root, &version, |done, total| {
                    let _ = emitter.emit("update://progress", json!({ "done": done, "total": total }));
                })?;
                opaline_core::update::apply(&path)
            })
            .await;
        if r.is_ok() {
            tokio::time::sleep(Duration::from_millis(300)).await;
            app.exit(0); // libère l'exe (Windows) ; la nouvelle version est relancée par l'installeur ou par `apply`
            return;
        }
        s.0.activity.lock().unwrap().installing.remove(UPDATING);
        let _ = app.emit("update://done", json!({ "ok": false, "error": r.err() }));
    });
    Ok(())
}

#[tauri::command]
async fn settings_get(state: State<'_, S>) -> CmdResult<service::SettingsView> {
    state.blocking(|r| Ok(service::settings_get(r))).await
}

#[tauri::command]
async fn settings_set(state: State<'_, S>, perf_mode: Option<PerfMode>, memory_mb: Option<u32>, no_remember: Option<bool>, launch_via: Option<LaunchVia>, join_server: Option<bool>, notify_server: Option<bool>) -> CmdResult<service::SettingsView> {
    state.blocking(move |r| service::settings_set(r, perf_mode, memory_mb, no_remember, launch_via, join_server, notify_server)).await
}

// ---------- Authentification ----------

fn profile_json(p: Option<Profile>) -> Value {
    p.map_or(Value::Null, |p| json!({ "name": p.name, "uuid": p.uuid }))
}

/// `mode` : `direct` (session Opaline et lancement par Opaline choisi) ou `official` (Launcher Minecraft). `identity` : pseudo mémorisé (mode officiel seulement).
fn status_json(s: &S, state: &str, profile: Option<Profile>, warning: Option<String>) -> Value {
    let direct = state == "signedIn";
    let identity = if direct { None } else { service::identity_get(&s.0.root) };
    json!({
        "state": state, "profile": profile_json(profile), "warning": warning,
        "mode": if direct && s.config().launch_via == LaunchVia::Opaline { "direct" } else { "official" },
        "mojangPending": !direct && s.0.mojang_pending.load(Ordering::SeqCst),
        "identity": identity,
    })
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
                return Ok(status_json(&s2, "signedIn", Some(p), None));
            }
            if !auth::configured() {
                return Ok(status_json(&s2, "notConfigured", None, None));
            }
            let c = auth::http_client()?;
            let session = s2.0.auth.ensure_session(&c, &auth::Endpoints::default(), auth::CLIENT_ID, !cfg.no_remember).ok().flatten();
            let warning = s2.0.auth.take_warning().map(|w| s2.sanitize(anyhow!(w)));
            if session.is_some() {
                s2.sync_skins_once();
            }
            let p = session.map(|s| Profile { name: s.username, uuid: s.uuid });
            Ok(status_json(&s2, if p.is_some() { "signedIn" } else { "signedOut" }, p, warning))
        })
        .await?;
    Ok(out)
}

#[tauri::command]
async fn auth_start(app: AppHandle, state: State<'_, S>, mode: Option<String>) -> CmdResult<Value> {
    let browser = match mode.as_deref() {
        None | Some("browser") => true,
        Some("device") => false,
        Some(_) => return Err("Mode de connexion inconnu".into()),
    };
    if auth::dev_offline() || !auth::configured() {
        return Err("Connexion Microsoft non configurée".into());
    }
    let s = state.inner().clone();
    let cancel = Arc::new(AtomicBool::new(false));
    let attempt = {
        let mut l = s.0.login.lock().unwrap();
        if let Some(old) = l.cancel.replace(cancel.clone()) {
            old.store(true, Ordering::SeqCst);
        }
        l.attempt += 1;
        l.attempt
    };
    let (tx, rx) = std::sync::mpsc::channel::<Result<Value, String>>();
    let s2 = s.clone();
    std::thread::spawn(move || {
        let cfg = s2.config();
        let result = auth::http_client().and_then(|c| {
            let (ep, remember) = (auth::Endpoints::default(), !cfg.no_remember);
            if browser {
                let on_url = |url: &str| {
                    // Échec d'ouverture ignoré : l'interface propose « Rouvrir la page ».
                    let _ = app.opener().open_url(url, None::<&str>);
                    let _ = tx.send(Ok(json!({ "attempt": attempt, "mode": "browser", "authorizeUrl": url })));
                };
                s2.0.auth.login_browser(&c, &ep, auth::CLIENT_ID, remember, on_url, &cancel, auth_browser::BROWSER_TIMEOUT)
            } else {
                let on_code = |dc: &auth::DeviceCode| {
                    let _ = tx.send(Ok(json!({ "attempt": attempt, "mode": "device", "userCode": dc.user_code, "verificationUri": dc.verification_uri, "expiresIn": dc.expires_in })));
                };
                s2.0.auth.login(&c, &ep, auth::CLIENT_ID, remember, on_code, &cancel)
            }
        });
        // Échec avant l'affichage (aléa, port local, réseau) : la commande renvoie la vraie cause (sans effet si déjà répondu).
        if let Err(e) = &result {
            let _ = tx.send(Err(s2.sanitize(anyhow!("{e}"))));
        }
        // Une tentative annulée (auth_cancel, nouvelle tentative, déconnexion) n'émet jamais auth://done.
        if cancel.load(Ordering::SeqCst) {
            return;
        }
        let payload = match result {
            Ok(session) => {
                s2.0.mojang_pending.store(false, Ordering::SeqCst);
                s2.0.skins_synced.store(false, Ordering::SeqCst);
                s2.sync_skins_once();
                json!({ "attempt": attempt, "ok": true, "profile": { "name": session.username, "uuid": session.uuid }, "error": null, "mojangPending": false })
            }
            Err(e) => {
                let pending = auth::is_not_approved(&format!("{e:#}"));
                s2.0.mojang_pending.store(pending, Ordering::SeqCst);
                json!({ "attempt": attempt, "ok": false, "profile": null, "error": s2.sanitize(e), "mojangPending": pending })
            }
        };
        let _ = app.emit("auth://done", payload);
    });
    tauri::async_runtime::spawn_blocking(move || rx.recv_timeout(Duration::from_secs(30)))
        .await
        .map_err(|_| "erreur interne".to_string())?
        .map_err(|_| "Connexion Microsoft impossible : vérifiez votre réseau".to_string())
        .and_then(|r| r)
}

/// Pseudo Minecraft (mode officiel) : résolu via l'API publique Mojang, mémorisé dans la config ; vide = l'oublier.
#[tauri::command]
async fn identity_set(state: State<'_, S>, name: String) -> CmdResult<Value> {
    let s = state.inner().clone();
    let api = s.0.svc.profile_api.clone();
    let id = state.blocking(move |r| service::identity_set(r, &name, &api)).await?;
    Ok(json!(id))
}

/// Skin complet du pseudo mémorisé (sessionserver public, sans authentification).
#[tauri::command]
async fn identity_skin(state: State<'_, S>) -> CmdResult<Option<identity::SkinOf>> {
    let ss = state.0.svc.session_server.clone();
    state
        .blocking(move |r| match service::identity_get(r) {
            Some(i) => Ok(Some(identity::skin_of(&auth::http_client()?, &ss, &i.uuid)?)),
            None => Ok(None),
        })
        .await
}

#[tauri::command]
async fn auth_cancel(state: State<'_, S>) -> CmdResult<()> {
    if let Some(c) = state.0.login.lock().unwrap().cancel.take() {
        c.store(true, Ordering::SeqCst);
    }
    Ok(())
}

#[tauri::command]
async fn auth_sign_out(state: State<'_, S>) -> CmdResult<Value> {
    let s = state.inner().clone();
    if let Some(c) = s.0.login.lock().unwrap().cancel.take() {
        c.store(true, Ordering::SeqCst);
    }
    state
        .blocking(move |_| {
            let r = s.0.auth.sign_out();
            s.0.skins_synced.store(false, Ordering::SeqCst);
            let warning = r.err().map(|e| format!("{e}")).or_else(|| s.0.auth.take_warning()).map(|w| s.sanitize(anyhow!(w)));
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
    {
        let mut a = s.0.activity.lock().unwrap();
        if a.running.contains(&id) {
            return Err("Le jeu est en cours pour ce pack : fermez-le avant d'installer ou de réparer".into());
        }
        if a.installing.contains(UPDATING) {
            return Err("Mise à jour d'Opaline en cours : attendez la fin".into());
        }
        if !a.installing.insert(id.clone()) {
            return Err("Une installation de ce pack est déjà en cours".into());
        }
    }
    let progress = Arc::new(Progress::default());
    let finished = Arc::new(AtomicBool::new(false));
    {
        let (app, progress, finished, id) = (app.clone(), progress.clone(), finished.clone(), id.clone());
        tauri::async_runtime::spawn(async move {
            // Étapes terminées d'abord (ordre d'exécution, compteurs finaux), puis l'étape courante si elle a bougé.
            let (mut cursor, mut last) = (0usize, None::<(String, usize, usize)>);
            loop {
                let stop = finished.load(Ordering::Relaxed);
                for ev in progress.pending(&mut cursor) {
                    if last.as_ref() != Some(&ev) {
                        let _ = app.emit("install://progress", json!({ "packId": id, "stage": ev.0, "done": ev.1, "total": ev.2 }));
                        last = Some(ev);
                    }
                }
                if stop {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(150)).await;
            }
        });
    }
    tauri::async_runtime::spawn(async move {
        let _guard = s.0.install_lock.lock().await;
        let (s2, id2) = (s.clone(), id.clone());
        let r = s.blocking(move |root| service::install_pack_with(root, &id2, &progress, None, &s2.0.svc.install)).await;
        finished.store(true, Ordering::Relaxed);
        // `install://done` après la dernière progression : laisse l'émetteur vider les étapes restantes.
        tokio::time::sleep(Duration::from_millis(200)).await;
        s.0.activity.lock().unwrap().installing.remove(&id);
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
async fn backups_list(state: State<'_, S>, id: String) -> CmdResult<Vec<opaline_core::backup::BackupView>> {
    state.blocking(move |r| Ok(opaline_core::backup::list(&r.for_pack(&id)?))).await
}

/// Restaure une sauvegarde de mondes (le `saves/` actuel est d'abord sauvegardé). Refusé pendant le jeu ou une installation du pack.
#[tauri::command]
async fn backup_restore(state: State<'_, S>, id: String, backup_id: String) -> CmdResult<()> {
    {
        let mut a = state.0.activity.lock().unwrap();
        if a.running.contains(&id) || a.installing.contains(&id) || a.installing.contains(UPDATING) {
            return Err("Ferme le jeu et attends la fin des installations avant de restaurer".into());
        }
        a.installing.insert(id.clone()); // réserve le pack : JOUER est refusé pendant la restauration
    }
    let id2 = id.clone();
    let r = state.blocking(move |r| opaline_core::backup::restore(&r.for_pack(&id2)?, &backup_id, friends::now_secs())).await;
    state.0.activity.lock().unwrap().installing.remove(&id);
    r
}

#[tauri::command]
async fn open_folder(app: AppHandle, state: State<'_, S>, id: String, kind: String) -> CmdResult<()> {
    let path = state
        .blocking(move |r| {
            let inst = r.for_pack(&id)?.instance();
            if kind == "backups" {
                let dir = r.for_pack(&id)?.backups();
                std::fs::create_dir_all(&dir)?;
                return Ok(dir);
            }
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

/// Réponse de `play` : `via` et `serverListed` sont toujours présents (contrat §4).
fn refused(reason: impl Into<String>, via: &str) -> Value {
    json!({ "started": false, "reason": reason.into(), "serverListed": false, "via": via })
}

#[tauri::command]
async fn play(app: AppHandle, state: State<'_, S>, id: String) -> CmdResult<Value> {
    let s = state.inner().clone();
    // Session Opaline valide et lancement par Opaline choisi : direct ; sinon Launcher Minecraft officiel.
    let s2 = s.clone();
    let session = if s.config().launch_via == LaunchVia::Official {
        None
    } else {
        match s.blocking(move |_| service::try_session(&s2.config(), &s2.0.auth)).await {
            Ok(v) => v,
            Err(reason) => return Ok(refused(reason, "direct")),
        }
    };
    let via = if session.is_some() { "direct" } else { "official" };
    // Une seule vérification, et l'id est réservé pour toute la préparation (double-clic, installation concurrente).
    {
        let mut a = s.0.activity.lock().unwrap();
        if a.installing.contains(&id) {
            return Ok(refused("Une installation ou une réparation de ce pack est en cours : attendez la fin", via));
        }
        if !a.running.insert(id.clone()) {
            return Ok(refused(if session.is_some() { "Le jeu est déjà lancé pour ce pack" } else { "Le lancement de ce pack est déjà en cours" }, via));
        }
    }
    let Some(session) = session else {
        let r = play_official(s.clone(), id.clone()).await;
        s.0.activity.lock().unwrap().running.remove(&id); // on ne possède pas le processus : pas de verrou de jeu
        return Ok(r);
    };
    let (s2, id2) = (s.clone(), id.clone());
    let launched = s
        .blocking(move |root| {
            let cfg = s2.config();
            let e = service::entry(root, &id2)?;
            let l = service::launch_pack(root, &e, &cfg, &session)?;
            *s2.0.last_server.lock().unwrap() = Some(l.server.clone()).filter(|a| !a.is_empty());
            Ok((l, session.access_token().to_string()))
        })
        .await;
    match launched {
        Err(reason) => {
            s.0.activity.lock().unwrap().running.remove(&id);
            Ok(refused(reason, "direct"))
        }
        Ok((mut l, token)) => {
            let (listed, joining) = (l.server_listed, l.joining);
            let since = std::time::SystemTime::now();
            let started = std::time::Instant::now();
            std::thread::spawn(move || {
                let code = l.child.wait().ok().and_then(|st| st.code());
                let _ = opaline_core::playtime::record(&s.0.root, &id, started.elapsed().as_secs(), friends::now_secs());
                let tail = service::log_tail(&l.log, &token, &l.server, &s.0.root, 40);
                // Code non nul : résumé lisible (latest.log + rapport de crash de cette session), sans jeton ni adresse.
                let crash = (code != Some(0)).then(|| service::crash_view(&s.0.root, &id, code, tail.as_deref(), since, &token, &l.server).ok()).flatten();
                s.0.activity.lock().unwrap().running.remove(&id);
                let _ = app.emit("game://exited", json!({ "packId": id, "code": code, "logTail": tail, "crash": crash }));
            });
            Ok(json!({ "started": true, "reason": null, "serverListed": listed, "via": "direct", "joining": joining }))
        }
    }
}

async fn play_official(s: S, id: String) -> Value {
    let s2 = s.clone();
    let r = s
        .blocking(move |root| {
            let mc = official::minecraft_dir().context("dossier .minecraft introuvable")?;
            let e = service::entry(root, &id)?;
            let l = service::launch_official(root, &e, &s2.config(), &mc, true)?;
            *s2.0.last_server.lock().unwrap() = Some(l.server.clone()).filter(|a| !a.is_empty());
            Ok(l)
        })
        .await;
    match r {
        Err(reason) => refused(reason, "official"),
        Ok(l) => {
            let message = if l.opened {
                "Profil prêt dans le Launcher Minecraft : clique sur Jouer".to_string()
            } else {
                format!("Ouvre le Launcher Minecraft et choisis le profil {}", l.profile_name)
            };
            json!({ "started": true, "reason": null, "serverListed": l.server_listed, "via": "official", "message": message })
        }
    }
}

/// Temps de jeu mesuré par Opaline (lancement direct) : `{<packId>: {totalSecs, lastPlayed}}`, `lastPlayed` en secondes Unix.
#[tauri::command]
async fn playtime(state: State<'_, S>) -> CmdResult<std::collections::BTreeMap<String, opaline_core::playtime::Playtime>> {
    state.blocking(|r| Ok(opaline_core::playtime::load(r))).await
}

#[tauri::command]
async fn server_status(state: State<'_, S>, id: String) -> CmdResult<Option<opaline_core::serverview::ServerView>> {
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
            skins::apply_skin(&auth::http_client()?, &s.0.svc.skins, &s.token()?, &e.model, &png)?;
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
    state.blocking(move |_| Ok(skins::parse_capes(&skins::get_profile(&auth::http_client()?, &s.0.svc.skins, &s.token()?)?))).await
}

#[tauri::command]
async fn cape_set(state: State<'_, S>, id: Option<String>) -> CmdResult<()> {
    let s = state.inner().clone();
    state.blocking(move |_| skins::set_cape(&auth::http_client()?, &s.0.svc.skins, &s.token()?, id.as_deref())).await
}

/// Colonne d'amis : joueurs vus sur les serveurs des packs, en ligne d'abord.
#[tauri::command]
async fn friends(state: State<'_, S>) -> CmdResult<Vec<friends::Friend>> {
    state.blocking(|r| Ok(service::friends_list(r).unwrap_or_else(|_| friends::record(r, &[], friends::now_secs())))).await
}

#[tauri::command]
async fn player_head(state: State<'_, S>, uuid: String) -> CmdResult<Value> {
    let s = state.inner().clone();
    state
        .blocking(move |r| {
            let c = auth::http_client().context("réseau indisponible")?;
            Ok(json!({ "dataUrl": head::player_head(r, &c, &uuid, &s.0.svc.session_server)? }))
        })
        .await
}

pub fn run() {
    run_with(Services::default())
}

/// Démarre l'application avec des services donnés (tests sur fixtures).
pub fn run_with(svc: Services) {
    let root = Dirs::new(Dirs::default_root());
    let state = S(Arc::new(Inner {
        root,
        svc,
        auth: auth::AuthManager::default(),
        install_lock: tokio::sync::Mutex::new(()),
        activity: Mutex::default(),
        login: Mutex::default(),
        last_server: Mutex::default(),
        skins_synced: AtomicBool::new(false),
        mojang_pending: AtomicBool::new(false),
    }));
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state.clone())
        .setup(move |app| {
            // Le dossier de données peut être surchargé (OPALINE_HOME) : l'asset protocol y est alors autorisé à l'exécution.
            let tex = state.0.root.textures();
            std::fs::create_dir_all(&tex)?;
            app.asset_protocol_scope().allow_directory(&tex, true)?;
            // Textures du client.jar : tâche de fond, sous le verrou d'installation (pas de doublon avec une installation en cours).
            let (s, app) = (state.clone(), app.handle().clone());
            tauri::async_runtime::spawn(async move {
                let _guard = s.0.install_lock.lock().await;
                let s2 = s.clone();
                if let Ok(Ok(Some(dir))) = tauri::async_runtime::spawn_blocking(move || service::ensure_textures(&s2.0.root, &s2.0.svc.install)).await {
                    let _ = app.emit("textures://ready", json!({ "dir": dir.to_string_lossy() }));
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            app_update_install,
            auth_status,
            auth_start,
            auth_cancel,
            identity_set,
            identity_skin,
            auth_sign_out,
            packs_list,
            pack_install,
            pack_changelog,
            pack_mods,
            pack_repair,
            play,
            server_status,
            playtime,
            run_stats,
            run_history,
            settings_get,
            settings_set,
            open_folder,
            backups_list,
            backup_restore,
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
