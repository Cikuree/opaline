//! Interface egui minimale (sera remplacée par une interface web) : la logique vit dans les autres modules.
use crate::auth;
use crate::config::{Config, Dirs};
use crate::download::Progress;
use crate::perf::PerfMode;
use crate::ping::{self, ServerStatus};
use crate::stats::{self, Stats};
use crate::tailscale::{self, TailscaleState};
use crate::update::{self, Update};
use eframe::egui;
use std::process::Child;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

type InstallState = Option<Option<Result<(), String>>>;
type StatsResult = Option<Result<Stats, String>>;

#[derive(Clone)]
enum AuthUi {
    Checking,
    SignedOut,
    Code { code: String, uri: String },
    SignedIn(String),
    Error(String),
}

#[derive(Clone)]
struct Net {
    tailscale: TailscaleState,
    server: Option<ServerStatus>,
}

pub struct App {
    dirs: Dirs,
    cfg: Config,
    progress: Arc<Progress>,
    /// Some(None) = installation en cours ; Some(Some(r)) = terminée.
    install: Arc<Mutex<InstallState>>,
    game: Option<(Child, Instant)>,
    message: String,
    stats: Arc<Mutex<StatsResult>>,
    stats_url: Arc<Mutex<String>>,
    auth: Arc<Mutex<AuthUi>>,
    cancel_login: Arc<AtomicBool>,
    update: Arc<Mutex<Option<Update>>>,
    net: Arc<Mutex<Option<Net>>>,
    play_slot: Arc<Mutex<Option<Result<Child, String>>>>,
    play_busy: bool,
}

pub fn run() -> eframe::Result {
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([560.0, 620.0]).with_title("Classroom Run Launcher"),
        ..Default::default()
    };
    eframe::run_native("Classroom Run Launcher", opts, Box::new(|cc| Ok(Box::new(App::new(cc.egui_ctx.clone())))))
}

impl App {
    fn new(ctx: egui::Context) -> App {
        let dirs = Dirs::new(Dirs::default_root());
        let cfg = Config::load(&dirs);
        let stats = Arc::new(Mutex::new(None));
        let stats_url = Arc::new(Mutex::new(cfg.stats_url()));
        spawn_stats_poller(ctx.clone(), stats.clone(), stats_url.clone());
        let app = App {
            dirs,
            cfg,
            progress: Arc::default(),
            install: Arc::default(),
            game: None,
            message: String::new(),
            stats,
            stats_url,
            auth: Arc::new(Mutex::new(AuthUi::Checking)),
            cancel_login: Arc::default(),
            update: Arc::default(),
            net: Arc::default(),
            play_slot: Arc::default(),
            play_busy: false,
        };
        app.spawn_background(ctx);
        app
    }

    /// Mise à jour du launcher, état Tailscale + ping, rafraîchissement silencieux de la session.
    fn spawn_background(&self, ctx: egui::Context) {
        let (upd, c) = (self.update.clone(), ctx.clone());
        std::thread::spawn(move || {
            if let Ok(client) = auth::http_client() {
                *upd.lock().unwrap() = update::check(&client, &crate::config::launcher_json_url(), env!("CARGO_PKG_VERSION"));
                c.request_repaint();
            }
        });
        let (net, c) = (self.net.clone(), ctx.clone());
        std::thread::spawn(move || {
            loop {
                let ts = tailscale::tailscale_state();
                let server = if ts == TailscaleState::Connected || crate::config::server_overridden() {
                    ping::ping(&crate::config::server_addr(), Duration::from_secs(3)).ok()
                } else {
                    None
                };
                *net.lock().unwrap() = Some(Net { tailscale: ts, server });
                c.request_repaint();
                std::thread::sleep(Duration::from_secs(10));
            }
        });
        let slot = self.auth.clone();
        if auth::dev_offline() || !auth::configured() {
            *slot.lock().unwrap() = AuthUi::SignedOut;
            return;
        }
        std::thread::spawn(move || {
            let r = auth::http_client().and_then(|c| auth::silent_login(&c, &auth::Endpoints::default(), auth::CLIENT_ID));
            *slot.lock().unwrap() = match r {
                Ok(Some(s)) => AuthUi::SignedIn(s.username.clone()),
                Ok(None) => AuthUi::SignedOut,
                Err(e) => AuthUi::Error(format!("{e:#}")),
            };
            ctx.request_repaint();
        });
    }

    fn start_login(&mut self, ctx: &egui::Context) {
        self.cancel_login.store(false, Ordering::Relaxed);
        let (slot, cancel, ctx) = (self.auth.clone(), self.cancel_login.clone(), ctx.clone());
        std::thread::spawn(move || {
            let (s2, c2) = (slot.clone(), ctx.clone());
            let r = auth::http_client().and_then(|c| {
                auth::login_interactive(
                    &c,
                    &auth::Endpoints::default(),
                    auth::CLIENT_ID,
                    |dc| {
                        *s2.lock().unwrap() = AuthUi::Code { code: dc.user_code.clone(), uri: dc.verification_uri.clone() };
                        c2.request_repaint();
                    },
                    &cancel,
                )
            });
            *slot.lock().unwrap() = match r {
                Ok(s) => AuthUi::SignedIn(s.username.clone()),
                Err(e) => AuthUi::Error(format!("{e:#}")),
            };
            ctx.request_repaint();
        });
    }

    fn start_install(&mut self, ctx: &egui::Context) {
        if let Err(e) = self.cfg.save(&self.dirs) {
            self.message = format!("Configuration non enregistrée : {e}");
        }
        *self.install.lock().unwrap() = Some(None);
        let (dirs, cfg, progress, slot, ctx) = (self.dirs.clone(), self.cfg.clone(), self.progress.clone(), self.install.clone(), ctx.clone());
        std::thread::spawn(move || {
            let r = crate::run_install(&dirs, &cfg, &progress).map(|_| ()).map_err(|e| format!("{e:#}"));
            *slot.lock().unwrap() = Some(Some(r));
            ctx.request_repaint();
        });
    }

    fn play(&mut self, ctx: &egui::Context) {
        let _ = self.cfg.save(&self.dirs);
        self.play_busy = true;
        let (dirs, cfg, slot, ctx) = (self.dirs.clone(), self.cfg.clone(), self.play_slot.clone(), ctx.clone());
        std::thread::spawn(move || {
            let r = crate::session_for_play(&cfg).and_then(|s| crate::run_play(&dirs, &cfg, &s)).map_err(|e| format!("{e:#}"));
            *slot.lock().unwrap() = Some(r);
            ctx.request_repaint();
        });
    }
}

/// Dernières lignes de `instance/logs/launcher-game.log`.
fn log_tail(dirs: &Dirs, n: usize) -> String {
    let text = std::fs::read_to_string(dirs.instance().join("logs").join("launcher-game.log")).unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(n)..].join("\n")
}

fn spawn_stats_poller(ctx: egui::Context, out: Arc<Mutex<StatsResult>>, url: Arc<Mutex<String>>) {
    std::thread::spawn(move || {
        let Ok(client) = reqwest::blocking::Client::builder().timeout(Duration::from_secs(5)).build() else { return };
        loop {
            let u = url.lock().unwrap().clone();
            // Message générique : l'adresse ne doit pas apparaître dans l'interface.
            let r = stats::fetch(&client, &u).map_err(|_| "indisponibles".to_string());
            *out.lock().unwrap() = Some(r);
            ctx.request_repaint();
            for _ in 0..20 {
                std::thread::sleep(Duration::from_millis(500));
                if *url.lock().unwrap() != u {
                    break;
                }
            }
        }
    });
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let installing = matches!(*self.install.lock().unwrap(), Some(None));
        if let Some(Some(r)) = self.install.lock().unwrap().take_if(|s| s.is_some()) {
            self.message = match r {
                Ok(()) => "Installation terminée.".into(),
                Err(e) => format!("Installation échouée : {e}"),
            };
        }
        if let Some(r) = self.play_slot.lock().unwrap().take() {
            self.play_busy = false;
            match r {
                Ok(c) => {
                    self.game = Some((c, Instant::now()));
                    self.message = "Jeu lancé.".into();
                }
                Err(e) => self.message = format!("Lancement impossible : {e}"),
            }
        }
        if let Some((c, started)) = &mut self.game
            && !matches!(c.try_wait(), Ok(None))
        {
            // Mort dans les premières secondes : on montre la fin du log pour comprendre pourquoi.
            self.message = if started.elapsed() < Duration::from_secs(30) {
                format!("Le jeu s'est arrêté au démarrage. Fin du log :\n{}", log_tail(&self.dirs, 15))
            } else {
                "Le jeu est fermé.".into()
            };
            self.game = None;
        }
        *self.stats_url.lock().unwrap() = self.cfg.stats_url();
        let auth_state = self.auth.lock().unwrap().clone();
        let net = self.net.lock().unwrap().clone();
        let ts_ok = crate::config::server_overridden() || net.as_ref().is_some_and(|n| n.tailscale == TailscaleState::Connected);
        let signed_in = matches!(auth_state, AuthUi::SignedIn(_));
        let can_play = !installing && self.game.is_none() && !self.play_busy && ts_ok && (auth::dev_offline() || (auth::configured() && signed_in));

        egui::CentralPanel::default().show(ui, |ui| {
            if let Some(u) = self.update.lock().unwrap().clone() {
                ui.horizontal(|ui| {
                    ui.colored_label(egui::Color32::LIGHT_GREEN, format!("Nouvelle version {}", u.version));
                    if ui.button("Télécharger").clicked()
                        && let Err(e) = update::open_url(&u.url)
                    {
                        self.message = format!("{e:#}");
                    }
                });
            }
            ui.heading("Classroom Run");
            if auth::dev_offline() {
                ui.horizontal(|ui| {
                    ui.label("Pseudo (hors ligne, dev)");
                    ui.text_edit_singleline(&mut self.cfg.nickname);
                });
            } else if !auth::configured() {
                ui.colored_label(egui::Color32::LIGHT_RED, "Connexion Microsoft non configurée");
            } else {
                match &auth_state {
                    AuthUi::Checking => {
                        ui.label("Connexion en cours…");
                    }
                    AuthUi::SignedOut | AuthUi::Error(_) => {
                        if let AuthUi::Error(e) = &auth_state {
                            ui.colored_label(egui::Color32::LIGHT_RED, e);
                        }
                        if ui.button("Se connecter avec Microsoft").clicked() {
                            self.start_login(&ctx);
                        }
                    }
                    AuthUi::Code { code, uri } => {
                        ui.label(format!("Ouvre {uri} et saisis le code : {code}"));
                        ui.horizontal(|ui| {
                            if ui.button("Copier le code").clicked() {
                                ctx.copy_text(code.clone());
                            }
                            if ui.button("Ouvrir le navigateur").clicked() {
                                let _ = update::open_url(uri);
                            }
                            if ui.button("Annuler").clicked() {
                                self.cancel_login.store(true, Ordering::Relaxed);
                            }
                        });
                    }
                    AuthUi::SignedIn(name) => {
                        ui.horizontal(|ui| {
                            ui.label(format!("Connecté : {name}"));
                            if ui.button("Se déconnecter").clicked() {
                                match auth::sign_out() {
                                    Ok(()) => *self.auth.lock().unwrap() = AuthUi::SignedOut,
                                    Err(e) => self.message = format!("{e:#}"),
                                }
                            }
                        });
                    }
                }
            }
            // État Tailscale / serveur : jamais l'adresse.
            match &net {
                None => {
                    ui.label("Vérification du réseau…");
                }
                Some(n) => {
                    if let Some(m) = tailscale::message(n.tailscale, n.server.is_some()) {
                        ui.colored_label(egui::Color32::LIGHT_YELLOW, m);
                    }
                    if let Some(s) = &n.server {
                        ui.label(format!(
                            "Serveur : en ligne - {} ms - {}/{} joueurs - {}",
                            s.latency_ms.map_or("?".into(), |l| l.to_string()),
                            s.online,
                            s.max,
                            s.version
                        ));
                    }
                }
            }
            ui.add_enabled_ui(!installing, |ui| {
                egui::ComboBox::from_label("Mode de performance").selected_text(self.cfg.perf_mode.label()).show_ui(ui, |ui| {
                    for m in PerfMode::ALL {
                        ui.selectable_value(&mut self.cfg.perf_mode, m, m.label());
                    }
                });
                ui.collapsing("Options avancées (dev)", |ui| {
                    ui.label("Dossier du pack local (vide = pack de la release)");
                    ui.text_edit_singleline(&mut self.cfg.pack_dir);
                    ui.label("Jar classroomrun (repli local)");
                    ui.text_edit_singleline(&mut self.cfg.mod_jar);
                });
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.add_enabled(!installing && self.game.is_none(), egui::Button::new("Installer / Mettre à jour")).clicked() {
                    self.start_install(&ctx);
                }
                if ui.add_enabled(can_play, egui::Button::new("Jouer")).clicked() {
                    self.play(&ctx);
                }
            });
            if installing {
                let (d, t) = (self.progress.done.load(Ordering::Relaxed), self.progress.total.load(Ordering::Relaxed));
                let frac = if t == 0 { 0.0 } else { d as f32 / t as f32 };
                ui.label(self.progress.stage.lock().unwrap().clone());
                ui.add(egui::ProgressBar::new(frac).text(format!("{d} / {t}")));
                ctx.request_repaint_after(Duration::from_millis(200));
            }
            if !self.message.is_empty() {
                ui.colored_label(egui::Color32::LIGHT_YELLOW, &self.message);
            }
            ui.separator();
            ui.strong("Run en cours");
            match &*self.stats.lock().unwrap() {
                None => {
                    ui.label("Chargement…");
                }
                Some(Err(e)) => {
                    ui.colored_label(egui::Color32::LIGHT_RED, format!("Stats {e}"));
                }
                Some(Ok(s)) => {
                    ui.label(format!(
                        "Run #{} - {} - {}",
                        s.run_number,
                        if s.running { "en cours" } else { "arrêtée" },
                        stats::format_time(s.total_time_ms)
                    ));
                    ui.label(format!("Morts : {}", s.deaths));
                    ui.label(format!("Split actuel : {}", s.current_split_name.as_deref().unwrap_or("-")));
                    ui.label(format!("Meilleur split : {}", s.best_split.as_deref().unwrap_or("-")));
                    ui.label(format!("Splits atteints : {}", s.reached_splits.join(", ")));
                    ui.label(format!("Joueurs ({}) : {}", s.players.len(), s.players.join(", ")));
                }
            }
        });
    }
}
