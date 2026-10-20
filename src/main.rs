#![cfg_attr(windows, windows_subsystem = "windows")]

mod catalog;
mod config;
mod files;
mod frontend;
mod hash;
mod images;
mod ini;
mod library;
mod media;
mod metadata;
mod net;
mod patches;
mod paths;
mod play;
mod update;
mod vps;
mod vpx;

use anyhow::{anyhow, Context, Result};
use eframe::egui::{self, RichText};
use frontend::theme;
use frontend::widgets::{self, ButtonKind};
use paths::Paths;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use update::{Outcome, Progress, VPINBALL};

/// Aggiorna il motore, scrive la configurazione e prepara i tavoli nuovi. Restituisce
/// l'eseguibile del motore. Se il controllo degli aggiornamenti fallisce ma il motore è
/// già installato si prosegue con quello che c'è.
fn prepare(paths: &Paths, screen: (u32, u32), report: &dyn Fn(Progress)) -> Result<PathBuf> {
    paths.create_all().with_context(|| format!("impossibile creare {}", paths.root.display()))?;
    let vpx_before = update::installed_version(paths, &VPINBALL);
    match update::update(paths, &VPINBALL, report) {
        Ok(Outcome::UpToDate(tag)) => eprintln!("{} {tag}: già aggiornato", VPINBALL.name),
        Ok(Outcome::Updated(tag)) => eprintln!("{} aggiornato a {tag}", VPINBALL.name),
        Err(e) if update::exe_path(paths, &VPINBALL).is_some() => {
            eprintln!("{}: aggiornamento saltato: {e:#}", VPINBALL.name)
        }
        Err(e) => return Err(e.context(format!("installazione di {} non riuscita", VPINBALL.name))),
    }
    let vpx = update::exe_path(paths, &VPINBALL).ok_or_else(|| anyhow!("VPinballX non trovato"))?;
    let vpx_tag = update::installed_version(paths, &VPINBALL).unwrap_or_default();
    if let Some(before) = vpx_before.filter(|before| *before != vpx_tag) {
        config::follow_engine(paths, &before, &vpx_tag).context("impossibile aggiornare VPinballX.ini")?;
    }
    config::write(paths, &vpx_tag, screen).context("impossibile scrivere la configurazione")?;
    update::remove_vpinfe(paths);
    // Un errore qui non blocca l'avvio: i tavoli già pronti restano giocabili.
    if let Err(e) = library::organize(paths) {
        eprintln!("riordino dei tavoli non riuscito: {e}");
    }
    metadata::prepare_new(paths, report);
    metadata::mark_pinmame(paths, report);
    Ok(vpx)
}

const FALLBACK_SCREEN: (u32, u32) = (1920, 1080);

/// Quanto si aspetta che il sistema dica su quale monitor è la finestra.
const SCREEN_WAIT: Duration = Duration::from_secs(2);

/// Risoluzione del monitor in pixel. È nota solo qualche fotogramma dopo che la finestra
/// è comparsa: fino ad allora `None`.
fn screen_size(ctx: &egui::Context) -> Option<(u32, u32)> {
    let scale = ctx.native_pixels_per_point().unwrap_or(1.0);
    let size = ctx.input(|i| i.viewport().monitor_size).filter(|size| size.x >= 640.0)?;
    Some(((size.x * scale).round() as u32, (size.y * scale).round() as u32))
}

#[derive(Default)]
struct State {
    text: String,
    fraction: Option<f32>,
    /// `Some` quando il lavoro in background è finito.
    result: Option<Result<PathBuf, String>>,
}

struct App {
    /// `None` finché l'utente non ha scelto la cartella dati.
    paths: Option<Arc<Paths>>,
    state: Arc<Mutex<State>>,
    folder: String,
    folder_error: Option<String>,
    started: bool,
    library: Option<frontend::Library>,
    opened_at: Instant,
}

impl App {
    fn start(&mut self, ctx: &egui::Context) {
        let Some(paths) = self.paths.clone() else { return };
        self.started = true;
        *self.state.lock().unwrap() = State { text: "Avvio…".into(), ..State::default() };
        let screen = screen_size(ctx).unwrap_or(FALLBACK_SCREEN);
        let (state, ctx) = (self.state.clone(), ctx.clone());
        std::thread::spawn(move || {
            let report = |p: Progress| {
                let mut s = state.lock().unwrap();
                s.text = p.text;
                s.fraction = p.fraction;
                ctx.request_repaint();
            };
            let result = prepare(&paths, screen, &report).map_err(|e| format!("{e:#}"));
            state.lock().unwrap().result = Some(result);
            ctx.request_repaint();
        });
    }

    /// Primo avvio: chiede dove mettere la cartella che conterrà tutto.
    fn choose_folder(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.label(RichText::new("Dove vuoi tenere tavoli, ROM e programmi?").font(theme::bold(19.0)));
        ui.label(RichText::new("Tutto finisce in questa cartella: cancellandola si rimuove ogni cosa.").color(theme::TEXT_WEAK));
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.folder).desired_width(ui.available_width() - 100.0));
            if ui.button("Sfoglia…").clicked() {
                if let Some(picked) = rfd::FileDialog::new().pick_folder() {
                    self.folder = Paths::root_for_picked(picked).display().to_string();
                }
            }
        });
        ui.add_space(14.0);
        let next = widgets::button(ui, "Continua", theme::bold(17.0), ButtonKind::Primary, egui::vec2(160.0, 40.0));
        if next.clicked() {
            match Paths::choose(PathBuf::from(self.folder.trim())) {
                Ok(paths) => {
                    self.paths = Some(Arc::new(paths));
                    self.start(ctx);
                }
                Err(e) => self.folder_error = Some(format!("Cartella non utilizzabile: {e}")),
            }
        }
        if let Some(error) = &self.folder_error {
            ui.colored_label(theme::ERROR, error);
        }
    }

    fn progress(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, paths: &Paths) {
        let state = self.state.lock().unwrap();
        match &state.result {
            None => {
                ui.label(&state.text);
                ui.add_space(10.0);
                match state.fraction {
                    Some(f) => ui.add(egui::ProgressBar::new(f).show_percentage()),
                    None => ui.add(egui::Spinner::new().size(26.0)),
                };
            }
            Some(Ok(_)) => {}
            Some(Err(message)) => {
                ui.colored_label(theme::ERROR, message);
                ui.add_space(10.0);
                drop(state);
                ui.horizontal(|ui| {
                    if ui.button("Riprova").clicked() {
                        self.start(ctx);
                    }
                    if ui.button("Esci").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            }
        }
        ui.add_space(20.0);
        ui.label(RichText::new(format!("Cartella: {}", paths.root.display())).small().color(theme::TEXT_WEAK));
    }

    /// Quando la preparazione è finita la finestra diventa la libreria.
    fn open_library(&mut self, ctx: &egui::Context) {
        let Some(paths) = &self.paths else { return };
        let engine = match &self.state.lock().unwrap().result {
            Some(Ok(engine)) => engine.clone(),
            _ => return,
        };
        self.library = Some(frontend::Library::new(ctx, paths.clone(), engine));
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.library.is_none() {
            self.open_library(ctx);
        }
        if let Some(library) = &mut self.library {
            library.show(ctx);
            return;
        }
        // Si parte quando è nota la risoluzione del monitor, che serve per creare
        // VPinballX.ini; se non arriva si prosegue lo stesso con quella di ripiego.
        if !self.started && self.paths.is_some() {
            if screen_size(ctx).is_some() || self.opened_at.elapsed() >= SCREEN_WAIT {
                self.start(ctx);
            } else {
                ctx.request_repaint_after(Duration::from_millis(50));
            }
        }
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::BACKGROUND)).show(ctx, |ui| {
            let width = 600.0_f32.min(ui.available_width() - 32.0);
            ui.add_space((ui.available_height() / 2.0 - 200.0).max(16.0));
            ui.vertical_centered(|ui| {
                let mut logo = egui::text::LayoutJob::default();
                logo.append("PINBALL ", 0.0, egui::TextFormat::simple(theme::display(72.0), theme::TEXT));
                logo.append("AIO", 0.0, egui::TextFormat::simple(theme::display(72.0), theme::ACCENT));
                ui.label(logo);
                ui.add_space(16.0);
            });
            ui.vertical_centered(|ui| {
                egui::Frame::new().fill(theme::SIDEBAR).corner_radius(16).inner_margin(28).show(ui, |ui| {
                    ui.set_width(width - 56.0);
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| match self.paths.clone() {
                        Some(paths) => self.progress(ctx, ui, &paths),
                        None => self.choose_folder(ctx, ui),
                    });
                });
            });
        });
    }
}

/// `--cli`: aggiorna, configura e prepara i tavoli senza finestra, con l'avanzamento
/// sul terminale.
fn run_cli(paths: &Paths) -> Result<()> {
    prepare(paths, FALLBACK_SCREEN, &|p| eprintln!("{}", p.text))?;
    let tables = catalog::load_all(paths);
    let unrecognized: Vec<&str> = tables.iter().filter(|t| !t.has_info).map(|t| t.folder.as_str()).collect();
    eprintln!("{} tavoli, {} non riconosciuti", tables.len(), unrecognized.len());
    for folder in unrecognized {
        eprintln!("  non riconosciuto: {folder}");
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let paths = Paths::detect();
    if args.iter().any(|a| a == "--cli") {
        let result = match &paths {
            Some(paths) => run_cli(paths),
            None => Err(anyhow!("cartella dati non scelta: imposta PINBALL_AIO_DIR")),
        };
        if let Err(e) = result {
            eprintln!("Errore: {e:#}");
            std::process::exit(1);
        }
        return;
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Pinball AIO")
            .with_app_id("pinball-aio")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([760.0, 480.0])
            .with_maximized(true)
            .with_icon(eframe::icon_data::from_png_bytes(include_bytes!("../assets/vpinball.png")).unwrap_or_default()),
        ..Default::default()
    };
    let app = App {
        paths: paths.map(Arc::new),
        state: Arc::default(),
        folder: Paths::default_root().display().to_string(),
        folder_error: None,
        started: false,
        library: None,
        opened_at: Instant::now(),
    };
    let result = eframe::run_native(
        "Pinball AIO",
        options,
        Box::new(move |cc| {
            theme::install(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    );
    if let Err(e) = result {
        eprintln!("Errore: {e}");
        std::process::exit(1);
    }
}
