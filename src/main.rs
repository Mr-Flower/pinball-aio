#![cfg_attr(windows, windows_subsystem = "windows")]

mod config;
mod ini;
mod library;
mod paths;
mod update;

use anyhow::{anyhow, Context, Result};
use eframe::egui;
use paths::Paths;
use std::net::{Ipv4Addr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use update::{Outcome, Progress, VPINBALL, VPINFE};

/// Aggiorna i due programmi e scrive la configurazione. Restituisce l'eseguibile di VPinFE.
/// Se un controllo fallisce ma il programma è già installato si prosegue con quello che c'è.
fn prepare(paths: &Paths, screen: (u32, u32), report: &dyn Fn(Progress)) -> Result<PathBuf> {
    paths.create_all().with_context(|| format!("impossibile creare {}", paths.root.display()))?;
    let vpx_before = update::installed_version(paths, &VPINBALL);
    for component in [&VPINBALL, &VPINFE] {
        match update::update(paths, component, report) {
            Ok(Outcome::UpToDate(tag)) => eprintln!("{} {tag}: già aggiornato", component.name),
            Ok(Outcome::Updated(tag)) => eprintln!("{} aggiornato a {tag}", component.name),
            Err(e) if update::exe_path(paths, component).is_some() => {
                eprintln!("{}: aggiornamento saltato: {e:#}", component.name)
            }
            Err(e) => return Err(e.context(format!("installazione di {} non riuscita", component.name))),
        }
    }
    let vpx = update::exe_path(paths, &VPINBALL).ok_or_else(|| anyhow!("VPinballX non trovato"))?;
    let vpx_tag = update::installed_version(paths, &VPINBALL).unwrap_or_default();
    if let Some(before) = vpx_before.filter(|before| *before != vpx_tag) {
        config::follow_engine(paths, &before, &vpx_tag).context("impossibile aggiornare VPinballX.ini")?;
    }
    config::write(paths, &vpx, &vpx_tag, screen).context("impossibile scrivere la configurazione")?;
    let vpinfe = update::exe_path(paths, &VPINFE).ok_or_else(|| anyhow!("VPinFE non trovato"))?;
    prepare_tables(paths, &vpinfe, report);
    Ok(vpinfe)
}

/// Sistema in cartelle i tavoli copiati in `tables/` e, se ce ne sono di nuovi, fa creare a
/// VPinFE le schede con copertine e applicare le patch agli script. Un errore qui non
/// blocca l'avvio: i tavoli già pronti restano giocabili.
fn prepare_tables(paths: &Paths, vpinfe: &Path, report: &dyn Fn(Progress)) {
    let moved = library::organize(paths).unwrap_or_else(|e| {
        eprintln!("riordino dei tavoli non riuscito: {e}");
        0
    });
    let (tables, with_info) = library::count(paths);
    if moved == 0 && (tables == 0 || with_info > 0) {
        return;
    }
    for (flag, text) in [
        ("--buildmeta", format!("Preparo {tables} tavoli: schede e copertine…")),
        ("--vpxpatch", "Applico le patch agli script dei tavoli…".to_string()),
    ] {
        report(Progress { text, fraction: None });
        match vpinfe_command(paths, vpinfe).arg(flag).status() {
            Ok(status) if status.success() => {}
            Ok(status) => eprintln!("vpinfe {flag}: uscito con {status}"),
            Err(e) => eprintln!("vpinfe {flag}: {e}"),
        }
    }
}

fn vpinfe_command(paths: &Paths, vpinfe: &Path) -> Command {
    // Su Windows VPinFE va avviato dal suo .bat, come indica la sua documentazione.
    #[cfg(windows)]
    let mut command = {
        use std::os::windows::process::CommandExt;
        let mut c = Command::new("cmd");
        c.arg("/C").arg(vpinfe).creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        c
    };
    #[cfg(not(windows))]
    let mut command = Command::new(vpinfe);

    command.arg("--configdir").arg(paths.vpinfe_config());
    if let Some(dir) = vpinfe.parent() {
        command.current_dir(dir);
    }
    command
}

/// Avvia VPinFE. Con `headless` partono solo i suoi servizi, compresa la pagina di gestione
/// da cui si importano i tavoli, senza la libreria a tutto schermo.
fn launch(paths: &Paths, vpinfe: &Path, headless: bool) -> Result<Child> {
    let mut command = vpinfe_command(paths, vpinfe);
    if headless {
        command.arg("--headless");
    }
    command.spawn().context("impossibile avviare VPinFE")
}

/// Chiude VPinFE e aspetta che sia uscito, così le sue porte tornano libere.
/// SIGTERM e non kill: VPinFE è un pacchetto PyInstaller con un processo figlio,
/// che con un kill secco resterebbe vivo.
fn stop(mut child: Child) {
    #[cfg(windows)]
    let _ = Command::new("taskkill").args(["/PID", &child.id().to_string(), "/T", "/F"]).status();
    #[cfg(not(windows))]
    let _ = Command::new("kill").args(["-TERM", &child.id().to_string()]).status();
    let _ = child.wait();
}

fn has_tables(paths: &Paths) -> bool {
    library::count(paths).0 > 0
}

fn manager_url(paths: &Paths) -> (u16, String) {
    let port = std::fs::read_to_string(paths.vpinfe_ini())
        .ok()
        .and_then(|text| ini::get(&text, "Network", "manageruiport"))
        .and_then(|p| p.parse().ok())
        .unwrap_or(8001);
    (port, format!("http://localhost:{port}/"))
}

const FALLBACK_SCREEN: (u32, u32) = (1920, 1080);

/// Risoluzione del monitor in pixel; è nota solo dopo che la finestra è comparsa.
fn screen_size(ctx: &egui::Context) -> (u32, u32) {
    let scale = ctx.native_pixels_per_point().unwrap_or(1.0);
    match ctx.input(|i| i.viewport().monitor_size) {
        Some(size) if size.x >= 640.0 => ((size.x * scale).round() as u32, (size.y * scale).round() as u32),
        _ => FALLBACK_SCREEN,
    }
}

#[derive(Default)]
struct State {
    text: String,
    fraction: Option<f32>,
    /// `Some` quando il lavoro in background è finito.
    result: Option<Result<PathBuf, String>>,
}

/// VPinFE avviato senza libreria perché non c'è ancora nessun tavolo da mostrare.
struct Import {
    child: Child,
    vpinfe: PathBuf,
    url: String,
    /// La pagina di gestione risponde.
    ready: Arc<AtomicBool>,
    opened: bool,
}

struct App {
    /// `None` finché l'utente non ha scelto la cartella dati.
    paths: Option<Arc<Paths>>,
    state: Arc<Mutex<State>>,
    folder: String,
    folder_error: Option<String>,
    import: Option<Import>,
    started: bool,
}

impl App {
    fn start(&mut self, ctx: &egui::Context) {
        let Some(paths) = self.paths.clone() else { return };
        self.started = true;
        *self.state.lock().unwrap() = State { text: "Avvio…".into(), ..State::default() };
        let screen = screen_size(ctx);
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

    fn fail(&self, error: anyhow::Error) {
        self.state.lock().unwrap().result = Some(Err(format!("{error:#}")));
    }

    /// Primo avvio: chiede dove mettere la cartella che conterrà tutto.
    fn choose_folder(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.label("Dove vuoi tenere tavoli, ROM e programmi?");
        ui.small("Tutto finisce in questa cartella: cancellandola si rimuove ogni cosa.");
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.folder).desired_width(330.0));
            if ui.button("Sfoglia…").clicked() {
                if let Some(picked) = rfd::FileDialog::new().pick_folder() {
                    self.folder = Paths::root_for_picked(picked).display().to_string();
                }
            }
        });
        ui.add_space(8.0);
        if ui.button("Continua").clicked() {
            match Paths::choose(PathBuf::from(self.folder.trim())) {
                Ok(paths) => {
                    self.paths = Some(Arc::new(paths));
                    self.start(ctx);
                }
                Err(e) => self.folder_error = Some(format!("Cartella non utilizzabile: {e}")),
            }
        }
        if let Some(error) = &self.folder_error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
    }

    /// Con dei tavoli apre la libreria e chiude il launcher. Senza tavoli la libreria
    /// sarebbe una schermata vuota: si apre invece la pagina da cui importarli.
    fn open_vpinfe(&mut self, ctx: &egui::Context, paths: &Paths, vpinfe: PathBuf) {
        if has_tables(paths) {
            match launch(paths, &vpinfe, false) {
                Ok(_) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                Err(e) => self.fail(e),
            }
            return;
        }
        let child = match launch(paths, &vpinfe, true) {
            Ok(child) => child,
            Err(e) => return self.fail(e),
        };
        let (port, url) = manager_url(paths);
        let ready = Arc::new(AtomicBool::new(false));
        let (flag, ctx) = (ready.clone(), ctx.clone());
        std::thread::spawn(move || {
            for _ in 0..120 {
                if TcpStream::connect((Ipv4Addr::LOCALHOST, port)).is_ok() {
                    flag.store(true, Ordering::Relaxed);
                    ctx.request_repaint();
                    return;
                }
                std::thread::sleep(Duration::from_millis(500));
            }
        });
        self.import = Some(Import { child, vpinfe, url, ready, opened: false });
    }

    fn import_screen(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, paths: &Paths) {
        let Some(import) = &mut self.import else { return };
        let ready = import.ready.load(Ordering::Relaxed);
        if ready && !import.opened {
            import.opened = true;
            ctx.open_url(egui::OpenUrl::new_tab(&import.url));
        }
        ui.label("Non ci sono ancora tavoli.");
        if !ready {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Apro la pagina per importarli…");
            });
            return;
        }
        ui.label("Nel browser si è aperta la pagina di VPinFE: trascina lì i file dei tavoli (.vpx, backglass, ROM, PUP pack).");
        ui.add_space(8.0);
        let mut play = false;
        ui.horizontal(|ui| {
            play = ui.button("Ho finito, apri la libreria").clicked();
            if ui.button("Riapri la pagina").clicked() {
                ctx.open_url(egui::OpenUrl::new_tab(&import.url));
            }
        });
        if play {
            if let Some(import) = self.import.take() {
                stop(import.child);
                match launch(paths, &import.vpinfe, false) {
                    Ok(_) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                    Err(e) => self.fail(e),
                }
            }
        }
    }

    fn progress(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, paths: &Paths) {
        let (text, fraction, result) = {
            let s = self.state.lock().unwrap();
            (s.text.clone(), s.fraction, s.result.clone())
        };
        match result {
            None => {
                ui.label(text);
                ui.add_space(8.0);
                match fraction {
                    Some(f) => ui.add(egui::ProgressBar::new(f).show_percentage()),
                    None => ui.add(egui::Spinner::new()),
                };
            }
            Some(Ok(_)) if self.import.is_some() => self.import_screen(ctx, ui, paths),
            Some(Ok(vpinfe)) => self.open_vpinfe(ctx, paths, vpinfe),
            Some(Err(message)) => {
                ui.colored_label(egui::Color32::LIGHT_RED, message);
                ui.add_space(8.0);
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
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            ui.small(format!("Cartella: {}", paths.root.display()));
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Si parte dal primo fotogramma e non prima: serve la risoluzione del monitor.
        if !self.started {
            self.start(ctx);
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(12.0);
            ui.heading("Pinball AIO");
            ui.add_space(12.0);
            match self.paths.clone() {
                Some(paths) => self.progress(ctx, ui, &paths),
                None => self.choose_folder(ctx, ui),
            }
        });
    }

    /// Chiudendo la finestra durante l'import non deve restare VPinFE in background.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(import) = self.import.take() {
            stop(import.child);
        }
    }
}

/// `--cli`: stesso lavoro senza finestra, con l'avanzamento sul terminale.
/// `--no-launch` (solo con `--cli`): aggiorna e configura senza avviare VPinFE.
fn run_cli(paths: &Paths, launch_after: bool) -> Result<()> {
    let vpinfe = prepare(paths, FALLBACK_SCREEN, &|p| eprintln!("{}", p.text))?;
    if launch_after {
        if !has_tables(paths) {
            eprintln!("Nessun tavolo: importali dalla pagina {}", manager_url(paths).1);
        }
        launch(paths, &vpinfe, false)?;
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let paths = Paths::detect();
    if args.iter().any(|a| a == "--cli") {
        let result = match &paths {
            Some(paths) => run_cli(paths, !args.iter().any(|a| a == "--no-launch")),
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
            .with_inner_size([480.0, 220.0])
            .with_resizable(false)
            .with_icon(eframe::icon_data::from_png_bytes(include_bytes!("../assets/vpinball.png")).unwrap_or_default()),
        ..Default::default()
    };
    let app = App {
        paths: paths.map(Arc::new),
        state: Arc::default(),
        folder: Paths::default_root().display().to_string(),
        folder_error: None,
        import: None,
        started: false,
    };
    let result = eframe::run_native(
        "Pinball AIO",
        options,
        Box::new(move |_cc| Ok(Box::new(app))),
    );
    if let Err(e) = result {
        eprintln!("Errore: {e}");
        std::process::exit(1);
    }
}
