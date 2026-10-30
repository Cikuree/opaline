//! Interface egui minimale.
use crate::config::{Config, Dirs};
use crate::download::Progress;
use crate::stats::{self, Stats};
use eframe::egui;
use std::process::Child;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

type InstallState = Option<Option<Result<(), String>>>;
type StatsResult = Option<Result<Stats, String>>;

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
}

pub fn run() -> eframe::Result {
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([560.0, 560.0]).with_title("Classroom Run Launcher"),
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
        spawn_stats_poller(ctx, stats.clone(), stats_url.clone());
        App { dirs, cfg, progress: Arc::default(), install: Arc::default(), game: None, message: String::new(), stats, stats_url }
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

    fn play(&mut self) {
        let _ = self.cfg.save(&self.dirs);
        match crate::run_play(&self.dirs, &self.cfg, true) {
            Ok(c) => {
                self.game = Some((c, Instant::now()));
                self.message = "Jeu lancé.".into();
            }
            Err(e) => self.message = format!("Lancement impossible : {e:#}"),
        }
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
            let r = stats::fetch(&client, &u).map_err(|e| format!("{e:#}"));
            *out.lock().unwrap() = Some(r);
            ctx.request_repaint();
            // 10 s, mais on reprend plus tôt si l'adresse a changé.
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

        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("Classroom Run");
            ui.add_enabled_ui(!installing, |ui| {
                egui::Grid::new("cfg").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                    ui.label("Pseudo (hors ligne)");
                    ui.text_edit_singleline(&mut self.cfg.nickname);
                    ui.end_row();
                    ui.label("Serveur (hôte[:port])");
                    ui.text_edit_singleline(&mut self.cfg.server_host);
                    ui.end_row();
                    ui.label("Dossier du pack");
                    ui.text_edit_singleline(&mut self.cfg.pack_dir);
                    ui.end_row();
                    ui.label("Jar classroomrun");
                    ui.text_edit_singleline(&mut self.cfg.mod_jar).on_hover_text("Fichier ou dossier ; vide = <pack>/../build/libs");
                    ui.end_row();
                });
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.add_enabled(!installing && self.game.is_none(), egui::Button::new("Installer / Mettre à jour")).clicked() {
                    self.start_install(&ctx);
                }
                if ui.add_enabled(!installing && self.game.is_none(), egui::Button::new("Jouer")).clicked() {
                    self.play();
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
                    ui.colored_label(egui::Color32::LIGHT_RED, format!("Stats indisponibles : {e}"));
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
