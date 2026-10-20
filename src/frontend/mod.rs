//! La libreria: i tavoli come tiles con la loro copertina, una scheda con i dettagli e il
//! pulsante per giocare. Prende il posto di VPinFE.
//!
//! Ogni fotogramma la finestra si disegna (`view.rs`) raccogliendo le azioni dell'utente,
//! poi le esegue (`apply`): così il disegno legge lo stato senza modificarlo a metà.

pub mod theme;
mod view;
pub mod widgets;
#[cfg(test)]
mod tests;

use crate::catalog::{self, RomIndex, Table};
use crate::images::Images;
use crate::library;
use crate::metadata;
use crate::paths::Paths;
use crate::play;
use crate::update::Progress;
use crate::vps::{self, Database, Game};
use eframe::egui::{self, Key};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Filter {
    All,
    Favorites,
    /// Non riconosciuti o senza ROM.
    ToFix,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Sort {
    Name,
    Year,
    Manufacturer,
    Recent,
    MostPlayed,
}

impl Sort {
    const ALL: [Sort; 5] = [Sort::Name, Sort::Year, Sort::Manufacturer, Sort::Recent, Sort::MostPlayed];

    fn label(self) -> &'static str {
        match self {
            Sort::Name => "Nome",
            Sort::Year => "Anno",
            Sort::Manufacturer => "Marca",
            Sort::Recent => "Giocati di recente",
            Sort::MostPlayed => "Più giocati",
        }
    }
}

/// Un tavolo con ciò che serve per mostrarlo, calcolato quando lo si carica e non a ogni
/// fotogramma.
struct Entry {
    table: Table,
    subtitle: String,
    /// Titolo, marca, anno e cartella in minuscolo, per la ricerca.
    search_text: String,
    /// File della ROM che manca, se il tavolo ne usa una.
    missing_rom: Option<String>,
}

impl Entry {
    fn new(table: Table, roms: &RomIndex) -> Self {
        let search_text = format!("{} {} {} {}", table.title, table.manufacturer, table.year, table.folder).to_lowercase();
        Self { subtitle: table.subtitle(), missing_rom: roms.missing(&table), search_text, table }
    }

    fn needs_fixing(&self) -> bool {
        !self.table.has_info || self.missing_rom.is_some()
    }

    fn badge(&self) -> Option<&'static str> {
        if !self.table.has_info {
            Some("Non riconosciuto")
        } else if self.missing_rom.is_some() {
            Some("ROM mancante")
        } else {
            None
        }
    }
}

/// Richieste dell'utente raccolte mentre si disegna.
enum Action {
    Select(String),
    Deselect,
    Play(String),
    ToggleFavorite(String),
    OpenMatcher(String),
    Assign(String, Game),
    CloseMatcher,
    OpenFolder(PathBuf),
    PickFiles,
    RetryUnrecognized,
    Reload,
    ShowAll,
    ToggleFullscreen,
    Quit,
}

/// Lavoro in background (importazione, abbinamento) con il suo avanzamento.
#[derive(Default)]
struct TaskState {
    text: String,
    fraction: Option<f32>,
    /// Messaggio finale: quando c'è, il lavoro è finito.
    done: Option<String>,
}

/// Database dei tavoli per l'abbinamento a mano, caricato alla prima richiesta.
enum DbState {
    Loading,
    Ready(Arc<Database>),
    Failed(String),
}

struct Matcher {
    folder: String,
    query: String,
    focus: bool,
    /// Risultati di `query`, rifatti solo quando la ricerca cambia.
    results: Vec<Game>,
    results_for: Option<String>,
}

struct Playing {
    title: String,
    folder: String,
    done: Arc<AtomicBool>,
}

pub struct Library {
    icon: egui::TextureHandle,
    paths: Arc<Paths>,
    engine: PathBuf,
    entries: Vec<Entry>,
    roms: RomIndex,
    /// Indici di `entries` da mostrare, nell'ordine scelto.
    view: Vec<usize>,
    /// Ricerca, filtro e ordinamento con cui è stato calcolato `view`; `None` se va rifatto.
    view_key: Option<(String, Filter, Sort)>,
    images: Images,
    search: String,
    filter: Filter,
    sort: Sort,
    selected: Option<String>,
    /// La selezione è cambiata da tastiera: la griglia scorre fino al tavolo.
    reveal: bool,
    columns: usize,
    /// Scorrimento e altezza visibile della griglia nel fotogramma precedente.
    viewport: (f32, f32),
    playing: Option<Playing>,
    task: Option<Arc<Mutex<TaskState>>>,
    matcher: Option<Matcher>,
    database: Option<Arc<Mutex<DbState>>>,
    notice: Option<(String, Instant)>,
    /// Il menu ☰ era aperto nel fotogramma precedente: i tasti vanno a lui.
    menu_open: bool,
    actions: Vec<Action>,
}

/// Apre una cartella nel gestore dei file del sistema.
fn open_folder(path: &Path) {
    #[cfg(windows)]
    let result = Command::new("explorer").arg(path).spawn();
    #[cfg(not(windows))]
    let result = Command::new("xdg-open").arg(path).spawn();
    if let Err(e) = result {
        eprintln!("impossibile aprire {}: {e}", path.display());
    }
}

fn icon(ctx: &egui::Context) -> egui::TextureHandle {
    let image = image::load_from_memory(include_bytes!("../../assets/vpinball.png"))
        .map(|i| i.to_rgba8())
        .map(|i| egui::ColorImage::from_rgba_unmultiplied([i.width() as usize, i.height() as usize], i.as_raw()))
        .unwrap_or_default();
    ctx.load_texture("icon", image, egui::TextureOptions::LINEAR)
}

impl Library {
    pub fn new(ctx: &egui::Context, paths: Arc<Paths>, engine: PathBuf) -> Self {
        let mut library = Self {
            icon: icon(ctx),
            images: Images::new(ctx, &paths.thumbs()),
            roms: RomIndex::read(&paths),
            paths,
            engine,
            entries: Vec::new(),
            view: Vec::new(),
            view_key: None,
            search: String::new(),
            filter: Filter::All,
            sort: Sort::Name,
            selected: None,
            reveal: true,
            columns: 1,
            viewport: (0.0, 0.0),
            playing: None,
            task: None,
            matcher: None,
            database: None,
            notice: None,
            menu_open: false,
            actions: Vec::new(),
        };
        library.reload();
        // Si riparte dall'ultimo tavolo giocato.
        library.selected = library
            .entries
            .iter()
            .filter(|e| e.table.last_run.is_some())
            .max_by_key(|e| e.table.last_run)
            .map(|e| e.table.folder.clone());
        library
    }

    /// Rilegge tutti i tavoli e le ROM dal disco.
    fn reload(&mut self) {
        self.roms = RomIndex::read(&self.paths);
        self.entries = catalog::load_all(&self.paths).into_iter().map(|t| Entry::new(t, &self.roms)).collect();
        self.view_key = None;
        if self.selected.as_deref().is_some_and(|s| self.index_of(s).is_none()) {
            self.selected = None;
        }
    }

    /// Rilegge un tavolo solo, dopo che è cambiata la sua scheda.
    fn reload_one(&mut self, folder: &str) {
        let Some(index) = self.index_of(folder) else { return };
        match catalog::load(&self.entries[index].table.dir) {
            Some(table) => self.entries[index] = Entry::new(table, &self.roms),
            None => return self.reload(),
        }
        self.view_key = None;
    }

    fn index_of(&self, folder: &str) -> Option<usize> {
        self.entries.iter().position(|e| e.table.folder == folder)
    }

    fn entry(&self, folder: &str) -> Option<&Entry> {
        self.index_of(folder).map(|i| &self.entries[i])
    }

    /// Ricalcola i tavoli da mostrare se sono cambiati ricerca, filtro, ordinamento o dati.
    fn refresh_view(&mut self) {
        let key = (self.search.clone(), self.filter, self.sort);
        if self.view_key.as_ref() == Some(&key) {
            return;
        }
        let words: Vec<String> = self.search.to_lowercase().split_whitespace().map(str::to_string).collect();
        let entries = &self.entries;
        let mut view: Vec<usize> = (0..entries.len())
            .filter(|&i| {
                let entry = &entries[i];
                let keep = match self.filter {
                    Filter::All => true,
                    Filter::Favorites => entry.table.favorite,
                    Filter::ToFix => entry.needs_fixing(),
                };
                keep && words.iter().all(|w| entry.search_text.contains(w.as_str()))
            })
            .collect();
        let table = |i: &usize| &entries[*i].table;
        let name = |i: &usize| table(i).title.to_lowercase();
        match self.sort {
            Sort::Name => view.sort_by_cached_key(name),
            Sort::Year => view.sort_by_cached_key(|i| (table(i).year.clone(), name(i))),
            Sort::Manufacturer => view.sort_by_cached_key(|i| (table(i).manufacturer.to_lowercase(), name(i))),
            Sort::Recent => view.sort_by_cached_key(|i| (std::cmp::Reverse(table(i).last_run), name(i))),
            Sort::MostPlayed => view.sort_by_cached_key(|i| (std::cmp::Reverse(table(i).start_count), name(i))),
        }
        self.view = view;
        self.view_key = Some(key);
    }

    fn notify(&mut self, text: impl Into<String>) {
        self.notice = Some((text.into(), Instant::now()));
    }

    fn busy(&self) -> bool {
        self.playing.is_some() || self.task.is_some()
    }

    fn play(&mut self, ctx: &egui::Context, folder: &str) {
        if self.busy() {
            return;
        }
        let Some(table) = self.entry(folder).map(|e| e.table.clone()) else { return };
        let mut child = match play::start(&self.paths, &self.engine, &table.vpx) {
            Ok(child) => child,
            Err(e) => return self.notify(format!("{e:#}")),
        };
        if let Err(e) = catalog::record_start(&table) {
            eprintln!("statistiche non salvate: {e}");
        }
        let done = Arc::new(AtomicBool::new(false));
        let (flag, ctx) = (done.clone(), ctx.clone());
        let started = Instant::now();
        let finished = table.clone();
        std::thread::spawn(move || {
            let status = child.wait();
            if let Err(e) = catalog::record_time(&finished, started.elapsed().as_secs()) {
                eprintln!("statistiche non salvate: {e}");
            }
            match status {
                Ok(status) if !status.success() => eprintln!("{}: il motore è uscito con {status}", finished.folder),
                Err(e) => eprintln!("{}: {e}", finished.folder),
                Ok(_) => {}
            }
            flag.store(true, Ordering::Relaxed);
            ctx.request_repaint();
        });
        self.playing = Some(Playing { title: table.title, folder: table.folder, done });
    }

    /// Esegue `work` in background mostrando l'avanzamento; alla fine la libreria si ricarica.
    fn run_task(&mut self, ctx: &egui::Context, text: &str, work: impl FnOnce(&dyn Fn(Progress)) -> String + Send + 'static) {
        let state = Arc::new(Mutex::new(TaskState { text: text.into(), ..TaskState::default() }));
        let (shared, ctx) = (state.clone(), ctx.clone());
        std::thread::spawn(move || {
            let report = |p: Progress| {
                let mut s = shared.lock().unwrap();
                s.text = p.text;
                s.fraction = p.fraction;
                ctx.request_repaint();
            };
            let message = work(&report);
            shared.lock().unwrap().done = Some(message);
            ctx.request_repaint();
        });
        self.task = Some(state);
    }

    fn import(&mut self, ctx: &egui::Context, files: Vec<PathBuf>) {
        if files.is_empty() {
            return;
        }
        if self.busy() {
            return self.notify("File non aggiunti: aspetta che finisca il lavoro in corso e riprova");
        }
        let paths = self.paths.clone();
        self.run_task(ctx, "Copio i file…", move |report| {
            let imported = match library::import(&paths, &files) {
                Ok(imported) => imported,
                Err(e) => return format!("Copia non riuscita: {e}"),
            };
            let found = metadata::prepare_new(&paths, report);
            let mut parts = Vec::new();
            if imported.tables > 0 {
                parts.push(widgets::count(imported.tables, "tavolo aggiunto", "tavoli aggiunti"));
            }
            if found < imported.tables {
                parts.push(format!("{} da abbinare a mano", imported.tables - found));
            }
            if imported.roms > 0 {
                parts.push(widgets::count(imported.roms, "ROM aggiunta", "ROM aggiunte"));
            }
            if !imported.skipped.is_empty() {
                parts.push(format!("non copiati: {}", imported.skipped.join(", ")));
            }
            if parts.is_empty() {
                "Nessun file aggiunto".into()
            } else {
                parts.join(" · ")
            }
        });
    }

    fn pick_files(&mut self, ctx: &egui::Context) {
        let files = rfd::FileDialog::new()
            .set_title("Aggiungi tavoli e ROM")
            .add_filter("Tavoli, backglass e ROM", &[&library::TABLE_FILES[..], &["zip"]].concat())
            .pick_files()
            .unwrap_or_default();
        self.import(ctx, files);
    }

    fn retry_unrecognized(&mut self, ctx: &egui::Context) {
        let paths = self.paths.clone();
        self.run_task(ctx, "Cerco i tavoli non riconosciuti…", move |report| {
            if let Err(e) = library::set_tried(&paths, &[]) {
                return format!("{e}");
            }
            match metadata::pending(&paths).len() {
                0 => "Tutti i tavoli sono già riconosciuti".into(),
                pending => format!("Riconosciuti {} tavoli su {pending}", metadata::prepare_new(&paths, report)),
            }
        });
    }

    fn toggle_favorite(&mut self, folder: &str) {
        let Some(table) = self.entry(folder).map(|e| e.table.clone()) else { return };
        match catalog::set_favorite(&table, !table.favorite) {
            Ok(()) => self.reload_one(folder),
            Err(e) => self.notify(format!("Preferito non salvato: {e}")),
        }
    }

    fn open_matcher(&mut self, ctx: &egui::Context, folder: String) {
        let query = vps::parse_folder(&folder).name;
        self.matcher = Some(Matcher { folder, query, focus: true, results: Vec::new(), results_for: None });
        let loaded = self.database.as_ref().is_some_and(|d| !matches!(*d.lock().unwrap(), DbState::Failed(_)));
        if loaded {
            return;
        }
        let state = Arc::new(Mutex::new(DbState::Loading));
        let (shared, paths, ctx) = (state.clone(), self.paths.clone(), ctx.clone());
        std::thread::spawn(move || {
            let loaded = match Database::load(&paths) {
                Ok(db) => DbState::Ready(Arc::new(db)),
                Err(e) => DbState::Failed(format!("{e:#}")),
            };
            *shared.lock().unwrap() = loaded;
            ctx.request_repaint();
        });
        self.database = Some(state);
    }

    fn assign(&mut self, ctx: &egui::Context, folder: &str, game: Game) {
        let Some(table) = self.entry(folder).map(|e| e.table.clone()) else { return };
        self.images.forget(&table.dir);
        let paths = self.paths.clone();
        self.run_task(ctx, &format!("Scarico scheda e copertine di {}…", game.name), move |_| {
            match metadata::assign(&paths, &table, &game) {
                Ok(()) => format!("{} abbinato a {} ({} {})", table.folder, game.name, game.manufacturer, game.year_text()),
                Err(e) => format!("Abbinamento non riuscito: {e:#}"),
            }
        });
    }

    /// Frecce, Invio, Esc e F11 sulla griglia; scrivere porta alla ricerca.
    fn keyboard(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() || self.menu_open || ctx.memory(|m| m.any_popup_open()) {
            return;
        }
        let columns = self.columns.max(1) as isize;
        for (key, step) in [(Key::ArrowRight, 1), (Key::ArrowLeft, -1), (Key::ArrowDown, columns), (Key::ArrowUp, -columns)] {
            let times = ctx.input_mut(|i| i.count_and_consume_key(egui::Modifiers::NONE, key));
            for _ in 0..times {
                self.move_selection(step);
            }
        }
        let pressed = |key| ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key));
        if pressed(Key::Enter) {
            if let Some(folder) = self.selected.clone() {
                self.actions.push(Action::Play(folder));
            }
        }
        if pressed(Key::Escape) {
            if self.search.is_empty() {
                self.actions.push(Action::Deselect);
            } else {
                self.search.clear();
            }
        }
        if pressed(Key::F11) {
            self.actions.push(Action::ToggleFullscreen);
        }
        // Scrivere senza aver cliccato la ricerca cerca lo stesso: il campo prende il fuoco
        // e riceve il testo in questo stesso fotogramma.
        let typed = ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Text(t) if !t.trim().is_empty())));
        if typed {
            ctx.memory_mut(|m| m.request_focus(egui::Id::new(view::SEARCH_ID)));
        }
    }

    /// Sposta la selezione di `step` posti nella griglia, restando nei suoi limiti.
    fn move_selection(&mut self, step: isize) {
        if self.view.is_empty() {
            return;
        }
        let current = self.selected.as_deref().and_then(|s| self.view.iter().position(|&i| self.entries[i].table.folder == s));
        let next = match current {
            Some(at) => (at as isize + step).clamp(0, self.view.len() as isize - 1) as usize,
            None => 0,
        };
        self.selected = Some(self.entries[self.view[next]].table.folder.clone());
        self.reveal = true;
    }

    fn apply(&mut self, ctx: &egui::Context) {
        for action in std::mem::take(&mut self.actions) {
            match action {
                Action::Select(folder) => self.selected = Some(folder),
                Action::Deselect => self.selected = None,
                Action::Play(folder) => {
                    self.selected = Some(folder.clone());
                    self.play(ctx, &folder);
                }
                Action::ToggleFavorite(folder) => self.toggle_favorite(&folder),
                Action::OpenMatcher(folder) => self.open_matcher(ctx, folder),
                Action::Assign(folder, game) => {
                    self.matcher = None;
                    self.assign(ctx, &folder, game);
                }
                Action::CloseMatcher => self.matcher = None,
                Action::OpenFolder(path) => open_folder(&path),
                Action::PickFiles => self.pick_files(ctx),
                Action::RetryUnrecognized => self.retry_unrecognized(ctx),
                Action::Reload => self.reload(),
                Action::ShowAll => {
                    self.search.clear();
                    self.filter = Filter::All;
                }
                Action::ToggleFullscreen => {
                    let full = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!full));
                }
                Action::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            }
        }
    }

    /// Fine di una partita o di un lavoro in background: si aggiorna la libreria.
    fn collect_finished(&mut self) {
        if let Some(playing) = self.playing.take_if(|p| p.done.load(Ordering::Relaxed)) {
            self.reload_one(&playing.folder);
            self.selected = Some(playing.folder);
            self.reveal = true;
        }
        let finished = self.task.as_ref().and_then(|task| task.lock().unwrap().done.clone());
        if let Some(message) = finished {
            self.task = None;
            self.reload();
            self.notify(message);
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        self.images.receive(ctx);
        self.collect_finished();
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        if !dropped.is_empty() {
            self.import(ctx, dropped);
        }
        self.refresh_view();
        if !self.busy() && self.matcher.is_none() {
            self.keyboard(ctx);
        }
        self.draw(ctx);
        self.apply(ctx);
    }
}
