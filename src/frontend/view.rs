//! Disegno della libreria: barra in alto, griglia, pannello di dettaglio e finestre sopra.
//! Le scelte dell'utente diventano `Action`, eseguite dopo il disegno.

use super::theme::{self, ACCENT, BAR, ERROR, RADIUS, SIDEBAR, SURFACE_HOVER, TEXT, TEXT_WEAK};
use super::widgets::{self, ButtonKind, TileView, BAR_ITEM_HEIGHT, COVER_ASPECT, TILE_TEXT_HEIGHT};
use super::{Action, DbState, Entry, Filter, Library, Sort};
use crate::catalog;
use crate::images;
use eframe::egui::{self, Color32, CornerRadius, Key, RichText, Sense, Stroke, StrokeKind, Vec2};

pub const SEARCH_ID: &str = "search";
const MIN_TILE_WIDTH: f32 = 260.0;
const GAP: f32 = 22.0;
/// Margine attorno alle tiles dentro l'area che scorre: senza, il bordo, l'ombra e il
/// sollevamento delle tiles sul margine verrebbero tagliati.
const GRID_PADDING: f32 = 12.0;
const DETAIL_WIDTH: f32 = 420.0;
/// Quanto resta visibile un avviso in fondo alla finestra.
const NOTICE_SECONDS: u64 = 8;

impl Library {
    pub(super) fn draw(&mut self, ctx: &egui::Context) {
        self.menu_open = false;
        egui::TopBottomPanel::top("top")
            .frame(egui::Frame::new().fill(BAR).inner_margin(egui::Margin::symmetric(20, 12)))
            .show_separator_line(false)
            .show(ctx, |ui| self.top_bar(ui));
        // La ricerca può essere appena cambiata nella barra.
        self.refresh_view();
        self.notice_bar(ctx);
        if let Some(index) = self.selected.as_deref().and_then(|s| self.index_of(s)) {
            egui::SidePanel::right("detail")
                .exact_width(DETAIL_WIDTH)
                .resizable(false)
                .show_separator_line(false)
                .frame(egui::Frame::new().fill(SIDEBAR).inner_margin(20))
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                        detail(ui, &self.entries[index], &mut self.images, &mut self.actions);
                    });
                });
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::BACKGROUND).inner_margin(egui::Margin { left: 12, right: 8, top: 8, bottom: 0 }))
            .show(ctx, |ui| {
                if self.view.is_empty() {
                    self.empty(ui);
                } else {
                    self.grid(ui);
                }
            });
        drop_overlay(ctx);
        self.matcher_window(ctx);
        self.progress_window(ctx);
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().interact_size.y = BAR_ITEM_HEIGHT;
        ui.horizontal(|ui| {
            ui.add(egui::Image::new(&self.icon).fit_to_exact_size(Vec2::splat(34.0)));
            let mut logo = egui::text::LayoutJob::default();
            logo.append("PINBALL ", 0.0, egui::TextFormat::simple(theme::display(32.0), TEXT));
            logo.append("AIO", 0.0, egui::TextFormat::simple(theme::display(32.0), ACCENT));
            ui.label(logo);
            ui.add_space(20.0);
            self.search_box(ui);
            ui.add_space(12.0);
            let to_fix = self.entries.iter().filter(|e| e.needs_fixing()).count();
            let mut filters = vec![(Filter::All, "Tutti".to_string()), (Filter::Favorites, "★  Preferiti".to_string())];
            if to_fix > 0 || self.filter == Filter::ToFix {
                filters.push((Filter::ToFix, format!("⚠  Da sistemare ({to_fix})")));
            }
            for (filter, label) in filters {
                if widgets::pill(ui, self.filter == filter, &label).clicked() {
                    self.filter = filter;
                }
            }
            ui.add_space(12.0);
            egui::ComboBox::from_id_salt("sort")
                .selected_text(format!("Ordina: {}", self.sort.label()))
                .width(200.0)
                .show_ui(ui, |ui| {
                    for sort in Sort::ALL {
                        ui.selectable_value(&mut self.sort, sort, sort.label());
                    }
                });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.menu_button(RichText::new("☰").size(20.0), |ui| self.menu(ui));
                let add = widgets::button(ui, "+  Aggiungi tavoli", theme::bold(16.0), ButtonKind::Primary, Vec2::new(0.0, BAR_ITEM_HEIGHT));
                let hint = "Tavoli .vpx, backglass .directb2s e ROM .zip. Puoi anche trascinarli nella finestra.";
                if add.on_hover_text(hint).clicked() {
                    self.actions.push(Action::PickFiles);
                }
                ui.add_space(6.0);
                ui.label(RichText::new(widgets::count(self.view.len(), "tavolo", "tavoli")).color(TEXT_WEAK));
            });
        });
    }

    fn search_box(&mut self, ui: &mut egui::Ui) {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(280.0, BAR_ITEM_HEIGHT), Sense::hover());
        ui.painter().rect_filled(rect, CornerRadius::same(255), theme::SURFACE);
        ui.painter().text(rect.left_center() + Vec2::new(18.0, 0.0), egui::Align2::CENTER_CENTER, "🔍", theme::body(15.0), TEXT_WEAK);
        let field = egui::Rect::from_min_max(rect.left_top() + Vec2::new(34.0, 0.0), rect.right_bottom() - Vec2::new(14.0, 0.0));
        let edit = egui::TextEdit::singleline(&mut self.search)
            .id(egui::Id::new(SEARCH_ID))
            .hint_text("Cerca un tavolo")
            .frame(false)
            .vertical_align(egui::Align::Center);
        let response = ui.put(field, edit);
        // Freccia giù o Invio: dalla ricerca si passa al primo risultato.
        let down = response.has_focus() && ui.input(|i| i.key_pressed(Key::ArrowDown));
        let enter = response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
        if down || enter {
            response.surrender_focus();
            self.refresh_view();
            if let Some(&first) = self.view.first() {
                self.selected = Some(self.entries[first].table.folder.clone());
                self.reveal = true;
            }
        }
        ui.add_space(14.0);
    }

    fn menu(&mut self, ui: &mut egui::Ui) {
        self.menu_open = true;
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            ui.close_menu();
        }
        let full = ui.input(|i| i.viewport().fullscreen.unwrap_or(false));
        let groups = [
            vec![
                ("Apri la cartella dei tavoli", Action::OpenFolder(self.paths.tables())),
                ("Apri la cartella delle ROM", Action::OpenFolder(self.paths.pinmame().join("roms"))),
            ],
            vec![
                ("Cerca di nuovo i tavoli non riconosciuti", Action::RetryUnrecognized),
                ("Ricarica la libreria", Action::Reload),
            ],
            vec![
                (if full { "Esci dallo schermo intero (F11)" } else { "Schermo intero (F11)" }, Action::ToggleFullscreen),
                ("Esci", Action::Quit),
            ],
        ];
        for (n, group) in groups.into_iter().enumerate() {
            if n > 0 {
                ui.separator();
            }
            for (label, action) in group {
                if ui.button(label).clicked() {
                    self.actions.push(action);
                    ui.close_menu();
                }
            }
        }
    }

    fn notice_bar(&mut self, ctx: &egui::Context) {
        let Some((text, at)) = &self.notice else { return };
        if at.elapsed().as_secs() >= NOTICE_SECONDS {
            self.notice = None;
            return;
        }
        let mut close = false;
        egui::TopBottomPanel::bottom("notice")
            .frame(egui::Frame::new().fill(SURFACE_HOVER).inner_margin(egui::Margin::symmetric(20, 10)))
            .show_separator_line(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        close = ui.add(egui::Button::new("🗙").frame(false)).on_hover_text("Chiudi").clicked();
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.add(egui::Label::new(RichText::new(text.as_str()).color(TEXT)).wrap());
                        });
                    });
                });
            });
        if close {
            self.notice = None;
        }
        ctx.request_repaint_after(std::time::Duration::from_secs(1));
    }

    fn grid(&mut self, ui: &mut egui::Ui) {
        let width = ui.available_width() - 2.0 * GRID_PADDING;
        let columns = (((width + GAP) / (MIN_TILE_WIDTH + GAP)).floor() as usize).max(1);
        let tile_width = ((width - GAP * (columns - 1) as f32) / columns as f32).floor();
        let row_height = tile_width / COVER_ASPECT + TILE_TEXT_HEIGHT + GAP;
        self.columns = columns;
        let rows = self.view.len().div_ceil(columns);

        let mut area = egui::ScrollArea::vertical().auto_shrink(false);
        if std::mem::take(&mut self.reveal) {
            let selected = self.selected.as_deref();
            let row = self.view.iter().position(|&i| Some(self.entries[i].table.folder.as_str()) == selected);
            if let Some(row) = row.map(|at| at / columns) {
                let (offset, height) = self.viewport;
                let (top, bottom) = (row as f32 * row_height, (row + 1) as f32 * row_height);
                if top < offset {
                    area = area.vertical_scroll_offset(top);
                } else if bottom > offset + height {
                    area = area.vertical_scroll_offset(bottom - height);
                }
            }
        }
        let (view, entries, images, actions) = (&self.view, &self.entries, &mut self.images, &mut self.actions);
        let selected = self.selected.as_deref();
        let output = area.show_rows(ui, row_height, rows, |ui, range| {
            for row in range {
                // Ogni riga ha spazio sopra: lì sale la tile sotto il mouse.
                ui.add_space(GRID_PADDING - ui.spacing().item_spacing.y);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.add_space(GRID_PADDING);
                    for (n, &index) in view.iter().skip(row * columns).take(columns).enumerate() {
                        if n > 0 {
                            ui.add_space(GAP);
                        }
                        let entry = &entries[index];
                        let table = &entry.table;
                        let texture = table.cover.as_deref().and_then(|cover| images.get(cover, images::TILE));
                        let tile = TileView {
                            key: &table.folder,
                            title: &table.title,
                            subtitle: &entry.subtitle,
                            favorite: table.favorite,
                            selected: selected == Some(table.folder.as_str()),
                            badge: entry.badge(),
                        };
                        let response = widgets::tile(ui, &tile, texture.as_ref(), tile_width);
                        if response.double_clicked() {
                            actions.push(Action::Play(table.folder.clone()));
                        } else if response.clicked() {
                            actions.push(Action::Select(table.folder.clone()));
                        }
                    }
                });
                ui.add_space(GAP - GRID_PADDING - ui.spacing().item_spacing.y);
            }
        });
        self.viewport = (output.state.offset.y, output.inner_rect.height());
    }

    fn empty(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() / 4.0);
            if self.entries.is_empty() {
                ui.label(RichText::new("Non ci sono ancora tavoli").font(theme::display(48.0)));
                ui.add_space(4.0);
                ui.label("Aggiungi i file dei tavoli (.vpx) e le loro ROM (.zip), oppure trascinali qui.");
                ui.label(RichText::new("Schede e copertine vengono scaricate da sole.").color(TEXT_WEAK));
                ui.add_space(20.0);
                if widgets::button(ui, "+  Aggiungi tavoli", theme::bold(19.0), ButtonKind::Primary, Vec2::new(240.0, 48.0)).clicked() {
                    self.actions.push(Action::PickFiles);
                }
            } else {
                ui.label(RichText::new("Nessun tavolo corrisponde").font(theme::display(36.0)));
                ui.add_space(8.0);
                if widgets::button(ui, "Mostra tutti", theme::bold(16.0), ButtonKind::Secondary, Vec2::new(160.0, 38.0)).clicked() {
                    self.actions.push(Action::ShowAll);
                }
            }
        });
    }

    fn matcher_window(&mut self, ctx: &egui::Context) {
        let Some(matcher) = &mut self.matcher else { return };
        let database = self.database.clone();
        let actions = &mut self.actions;
        egui::Modal::new(egui::Id::new("matcher")).show(ctx, |ui| {
            ui.set_width(600.0);
            ui.label(RichText::new("Quale tavolo è?").font(theme::display(34.0)));
            ui.label(RichText::new(&matcher.folder).color(TEXT_WEAK));
            ui.add_space(8.0);
            let edit = egui::TextEdit::singleline(&mut matcher.query).hint_text("Nome, marca o anno").desired_width(f32::INFINITY);
            let edit = ui.add(edit);
            if std::mem::take(&mut matcher.focus) {
                edit.request_focus();
            }
            ui.add_space(6.0);
            let state = database.as_ref().map(|d| d.lock().unwrap());
            match state.as_deref() {
                Some(DbState::Ready(db)) => {
                    if matcher.results_for.as_deref() != Some(matcher.query.as_str()) {
                        matcher.results = db.search(&matcher.query, 60).into_iter().cloned().collect();
                        matcher.results_for = Some(matcher.query.clone());
                    }
                    egui::ScrollArea::vertical().max_height(360.0).auto_shrink([false, true]).show(ui, |ui| {
                        if matcher.results.is_empty() {
                            ui.label(RichText::new("Nessun tavolo trovato: prova con meno parole.").color(TEXT_WEAK));
                        }
                        for game in &matcher.results {
                            let detail = format!("{} {}", game.manufacturer, game.year_text());
                            if result_row(ui, &game.name, &detail).clicked() {
                                actions.push(Action::Assign(matcher.folder.clone(), game.clone()));
                            }
                        }
                    });
                }
                Some(DbState::Failed(error)) => {
                    ui.colored_label(ERROR, format!("Elenco dei tavoli non disponibile: {error}"));
                }
                _ => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Scarico l'elenco dei tavoli…");
                    });
                }
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Annulla").clicked() {
                    actions.push(Action::CloseMatcher);
                }
                ui.label(RichText::new("Dati dalla Virtual Pinball Spreadsheet").small().color(TEXT_WEAK));
            });
        });
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.actions.push(Action::CloseMatcher);
        }
    }

    /// Finestra sopra la libreria mentre si gioca o si lavora in background.
    fn progress_window(&self, ctx: &egui::Context) {
        if let Some(playing) = &self.playing {
            // Niente animazioni: mentre il tavolo gira questa finestra non deve ridisegnarsi.
            egui::Modal::new(egui::Id::new("playing")).show(ctx, |ui| {
                ui.set_width(420.0);
                ui.label(RichText::new("In gioco").font(theme::display(22.0)).color(ACCENT));
                ui.label(RichText::new(&playing.title).font(theme::display(36.0)));
                ui.label(RichText::new("Esci dal tavolo con Esc per tornare alla libreria.").color(TEXT_WEAK));
            });
            return;
        }
        let Some(task) = &self.task else { return };
        let state = task.lock().unwrap();
        egui::Modal::new(egui::Id::new("task")).show(ctx, |ui| {
            ui.set_width(460.0);
            ui.label(&state.text);
            ui.add_space(8.0);
            match state.fraction {
                Some(f) => ui.add(egui::ProgressBar::new(f).show_percentage()),
                None => ui.add(egui::Spinner::new().size(24.0)),
            };
        });
    }
}

/// Riga cliccabile di un elenco: nome in evidenza e dettaglio tenue.
fn result_row(ui: &mut egui::Ui, name: &str, detail: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.0), Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
    if response.hovered() {
        ui.painter().rect_filled(rect, RADIUS, SURFACE_HOVER);
    }
    let left = rect.left_center() + Vec2::new(12.0, 0.0);
    let name = ui.painter().text(left, egui::Align2::LEFT_CENTER, name, theme::bold(16.0), TEXT);
    let after = egui::pos2(name.right() + 12.0, rect.center().y);
    ui.painter().text(after, egui::Align2::LEFT_CENTER, detail, theme::body(15.0), TEXT_WEAK);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Velo sopra la finestra mentre si trascinano file.
fn drop_overlay(ctx: &egui::Context) {
    if ctx.input(|i| i.raw.hovered_files.is_empty()) {
        return;
    }
    egui::Area::new(egui::Id::new("drop")).order(egui::Order::Foreground).fixed_pos(egui::Pos2::ZERO).show(ctx, |ui| {
        let screen = ctx.screen_rect();
        let painter = ui.painter();
        painter.rect_filled(screen, 0, Color32::from_black_alpha(210));
        painter.rect_stroke(screen.shrink(28.0), 18, Stroke::new(3.0_f32, ACCENT), StrokeKind::Inside);
        painter.text(screen.center(), egui::Align2::CENTER_CENTER, "Rilascia per aggiungere tavoli e ROM", theme::display(44.0), TEXT);
    });
}

/// Pannello a destra con la scheda del tavolo scelto.
fn detail(ui: &mut egui::Ui, entry: &Entry, images: &mut images::Images, actions: &mut Vec<Action>) {
    let table = &entry.table;
    let width = ui.available_width();
    let (cover, _) = ui.allocate_exact_size(Vec2::new(width, width / COVER_ASPECT), Sense::hover());
    ui.painter().add(egui::Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(140) }.as_shape(cover, RADIUS));
    let texture = table.cover.as_deref().and_then(|c| images.get(c, images::LARGE));
    widgets::paint_cover(ui, cover, CornerRadius::same(RADIUS), texture.as_ref(), &table.title);
    if widgets::close_button(ui, cover.right_top() + Vec2::new(-22.0, 22.0)).on_hover_text("Chiudi (Esc)").clicked() {
        actions.push(Action::Deselect);
    }

    ui.add_space(10.0);
    ui.add(egui::Label::new(RichText::new(&table.title).font(theme::display(42.0))).wrap());
    if !entry.subtitle.is_empty() {
        ui.label(RichText::new(&entry.subtitle).size(18.0).color(TEXT_WEAK));
    }
    ui.add_space(10.0);
    let play = widgets::button(ui, "▶  GIOCA", theme::display(30.0), ButtonKind::Primary, Vec2::new(width, 54.0));
    if play.on_hover_text("Invio. Durante la partita: Esc esce dal tavolo, F12 apre il menu del motore").clicked() {
        actions.push(Action::Play(table.folder.clone()));
    }
    if table.has_info {
        let label = if table.favorite { "★  Nei preferiti" } else { "☆  Aggiungi ai preferiti" };
        if widgets::button(ui, label, theme::bold(16.0), ButtonKind::Secondary, Vec2::new(width, 40.0)).clicked() {
            actions.push(Action::ToggleFavorite(table.folder.clone()));
        }
    }
    ui.add_space(8.0);
    if !table.has_info {
        widgets::warning(ui, "Tavolo non riconosciuto: niente scheda né copertine. Cercalo nell'elenco dei tavoli per scaricarle.");
        if ui.button("🔍  Cerca nell'elenco dei tavoli…").clicked() {
            actions.push(Action::OpenMatcher(table.folder.clone()));
        }
    }
    if let Some(rom) = &entry.missing_rom {
        widgets::warning(ui, &format!("Manca la ROM {rom}: senza, il tavolo non parte. Aggiungila con «Aggiungi tavoli»."));
    }

    ui.add_space(8.0);
    let mut rows: Vec<(&str, String)> = vec![
        ("AUTORI", table.authors.join(", ")),
        ("ROM", table.rom.clone()),
        ("TEMI", table.themes.join(", ")),
    ];
    if table.start_count > 0 {
        rows.push(("PARTITE", table.start_count.to_string()));
        rows.push(("TEMPO DI GIOCO", widgets::duration(table.run_time)));
    }
    if let Some(time) = table.last_run {
        rows.push(("ULTIMA PARTITA", widgets::when(time, catalog::unix_now())));
    }
    rows.push(("CARTELLA", table.folder.clone()));
    egui::Grid::new("details").num_columns(2).spacing([14.0, 8.0]).show(ui, |ui| {
        for (name, value) in rows.into_iter().filter(|(_, v)| !v.is_empty()) {
            widgets::caption(ui, name);
            ui.add(egui::Label::new(value).wrap());
            ui.end_row();
        }
    });

    ui.add_space(12.0);
    ui.horizontal_wrapped(|ui| {
        if !table.ipdb_id.is_empty() {
            ui.hyperlink_to("Scheda IPDB", format!("https://www.ipdb.org/machine.cgi?id={}", table.ipdb_id));
            ui.add_space(8.0);
        }
        if let Some(tutorial) = &table.tutorial {
            ui.hyperlink_to("Guida al tavolo", tutorial);
        }
    });
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        if ui.button("🗁  Apri la cartella").clicked() {
            actions.push(Action::OpenFolder(table.dir.clone()));
        }
        if table.has_info {
            let rematch = ui.button("Cambia abbinamento…").on_hover_text("Se scheda o copertine sono di un altro tavolo");
            if rematch.clicked() {
                actions.push(Action::OpenMatcher(table.folder.clone()));
            }
        }
    });
}
