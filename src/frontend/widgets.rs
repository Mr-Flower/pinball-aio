//! Elementi disegnati della libreria: tile dei tavoli, pillole dei filtri, avvisi, pulsanti.

use super::theme::{self, ACCENT, ACCENT_HOVER, ON_ACCENT, RADIUS, SURFACE, SURFACE_HOVER, TEXT, TEXT_WEAK, WARNING};
use eframe::egui::{self, Color32, CornerRadius, Rect, RichText, Sense, Stroke, StrokeKind, Vec2};
use std::sync::Arc;

pub const COVER_ASPECT: f32 = 16.0 / 9.0;
/// Altezza della parte di testo sotto la copertina di una tile.
pub const TILE_TEXT_HEIGHT: f32 = 58.0;

/// Parte della texture da mostrare per riempire un riquadro `aspect` senza deformarla: le
/// immagini larghe si tagliano ai lati, quelle alte (flyer, cabinet) si tengono dall'alto.
pub fn cover_uv(texture: &egui::TextureHandle, aspect: f32) -> Rect {
    let [w, h] = texture.size();
    let image = w as f32 / h.max(1) as f32;
    if image > aspect {
        let width = aspect / image;
        Rect::from_min_max(egui::pos2((1.0 - width) / 2.0, 0.0), egui::pos2((1.0 + width) / 2.0, 1.0))
    } else {
        Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, image / aspect))
    }
}

/// Colore di ripiego per un tavolo senza copertina, diverso da tavolo a tavolo.
fn placeholder_color(title: &str) -> Color32 {
    let hash = title.bytes().fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(u32::from(b)));
    egui::ecolor::Hsva::new((hash % 360) as f32 / 360.0, 0.55, 0.24, 1.0).into()
}

/// Copertina in un riquadro 16:9, oppure un riquadro colorato con il titolo.
pub fn paint_cover(ui: &egui::Ui, rect: Rect, radius: CornerRadius, texture: Option<&egui::TextureHandle>, title: &str) {
    match texture {
        Some(texture) => {
            egui::Image::new(texture).uv(cover_uv(texture, COVER_ASPECT)).corner_radius(radius).paint_at(ui, rect);
        }
        None => {
            let painter = ui.painter();
            painter.rect_filled(rect, radius, placeholder_color(title));
            let size = (rect.width() / 9.0).clamp(20.0, 44.0);
            let mut job = egui::text::LayoutJob::simple(title.to_string(), theme::display(size), TEXT, rect.width() - 32.0);
            job.halign = egui::Align::Center;
            let galley = ui.fonts(|f| f.layout_job(job));
            painter.galley(rect.center() - Vec2::new(0.0, galley.size().y / 2.0), galley, TEXT);
        }
    }
}

/// Testo su una riga sola, con i puntini se non ci sta.
fn one_line(ui: &egui::Ui, text: &str, font: egui::FontId, color: Color32, width: f32) -> Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_string(), font, color);
    job.wrap = egui::text::TextWrapping { max_width: width, max_rows: 1, break_anywhere: true, overflow_character: Some('…') };
    ui.fonts(|f| f.layout_job(job))
}

fn shadow(lifted: bool) -> egui::Shadow {
    let (offset, blur, alpha) = if lifted { (10, 24, 150) } else { (4, 12, 110) };
    egui::Shadow { offset: [0, offset], blur, spread: 0, color: Color32::from_black_alpha(alpha) }
}

/// Etichetta colorata sopra una copertina.
fn badge(ui: &egui::Ui, at: egui::Pos2, text: &str) {
    let galley = ui.fonts(|f| f.layout_no_wrap(text.into(), theme::bold(13.0), ON_ACCENT));
    let rect = Rect::from_min_size(at, galley.size() + Vec2::new(16.0, 6.0));
    ui.painter().rect_filled(rect, CornerRadius::same(255), WARNING);
    ui.painter().galley(rect.min + Vec2::new(8.0, 3.0), galley, ON_ACCENT);
}

pub struct TileView<'a> {
    /// Identifica la tile tra un fotogramma e l'altro (la cartella del tavolo).
    pub key: &'a str,
    pub title: &'a str,
    pub subtitle: &'a str,
    pub favorite: bool,
    pub selected: bool,
    /// Problema da segnalare sulla copertina ("ROM mancante"…).
    pub badge: Option<&'static str>,
}

/// Una tile della griglia: copertina, titolo e marca. Passandoci sopra si solleva.
pub fn tile(ui: &mut egui::Ui, view: &TileView, texture: Option<&egui::TextureHandle>, width: f32) -> egui::Response {
    let cover_height = width / COVER_ASPECT;
    let (slot, response) = ui.allocate_exact_size(Vec2::new(width, cover_height + TILE_TEXT_HEIGHT), Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, view.title));
    if !ui.is_rect_visible(slot) {
        return response;
    }
    let hovered = response.hovered();
    let lift = ui.ctx().animate_bool(egui::Id::new(("tile", view.key)), hovered || view.selected) * 4.0;
    let rect = slot.translate(Vec2::new(0.0, -lift));
    let painter = ui.painter();
    painter.add(shadow(hovered).as_shape(rect, RADIUS));
    painter.rect_filled(rect, RADIUS, if hovered || view.selected { SURFACE_HOVER } else { SURFACE });
    let cover = Rect::from_min_size(rect.min, Vec2::new(width, cover_height));
    paint_cover(ui, cover, CornerRadius { nw: RADIUS, ne: RADIUS, sw: 0, se: 0 }, texture, view.title);
    if view.favorite {
        let center = cover.right_top() + Vec2::new(-22.0, 22.0);
        painter.circle_filled(center, 15.0, Color32::from_black_alpha(170));
        painter.text(center, egui::Align2::CENTER_CENTER, "★", theme::body(18.0), WARNING);
    }
    if let Some(text) = view.badge {
        badge(ui, cover.min + Vec2::splat(10.0), text);
    }
    let left = egui::pos2(rect.left() + 14.0, cover.bottom() + 9.0);
    let title = one_line(ui, view.title, theme::bold(18.0), TEXT, width - 28.0);
    painter.galley(left, title, TEXT);
    let subtitle = one_line(ui, view.subtitle, theme::body(14.5), TEXT_WEAK, width - 28.0);
    painter.galley(left + Vec2::new(0.0, 25.0), subtitle, TEXT_WEAK);
    if view.selected {
        painter.rect_stroke(rect, RADIUS, Stroke::new(2.5_f32, ACCENT), StrokeKind::Outside);
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Pulsante a pillola per i filtri: pieno quando è scelto.
pub fn pill(ui: &mut egui::Ui, selected: bool, text: &str) -> egui::Response {
    let galley = ui.fonts(|f| f.layout_no_wrap(text.into(), theme::bold(15.0), TEXT));
    let (rect, response) = ui.allocate_exact_size(Vec2::new(galley.size().x + 32.0, BAR_ITEM_HEIGHT), Sense::click());
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, text));
    let (fill, color) = match (selected, response.hovered()) {
        (true, _) => (ACCENT, ON_ACCENT),
        (false, true) => (SURFACE_HOVER, TEXT),
        (false, false) => (SURFACE, TEXT_WEAK),
    };
    ui.painter().rect_filled(rect, CornerRadius::same(255), fill);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, color);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Pulsante rotondo con la crocetta, sopra un'immagine. Non occupa posto nel layout.
pub fn close_button(ui: &mut egui::Ui, center: egui::Pos2) -> egui::Response {
    let rect = Rect::from_center_size(center, Vec2::splat(30.0));
    let response = ui.interact(rect, ui.id().with("close"), Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Chiudi"));
    let alpha = if response.hovered() { 230 } else { 160 };
    ui.painter().circle_filled(center, 15.0, Color32::from_black_alpha(alpha));
    ui.painter().text(center, egui::Align2::CENTER_CENTER, "🗙", theme::body(15.0), TEXT);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Altezza comune degli elementi della barra in alto.
pub const BAR_ITEM_HEIGHT: f32 = 36.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    /// Nel colore d'accento: l'azione principale della schermata.
    Primary,
    Secondary,
}

/// Pulsante con il testo centrato, largo almeno `min_size` (o quanto il testo).
pub fn button(ui: &mut egui::Ui, text: &str, font: egui::FontId, kind: ButtonKind, min_size: Vec2) -> egui::Response {
    let color = if kind == ButtonKind::Primary { ON_ACCENT } else { TEXT };
    let galley = ui.fonts(|f| f.layout_no_wrap(text.into(), font, color));
    let size = Vec2::new(min_size.x.max(galley.size().x + 32.0), min_size.y.max(galley.size().y + 12.0));
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, text));
    let fill = match (kind, response.hovered()) {
        (ButtonKind::Primary, false) => ACCENT,
        (ButtonKind::Primary, true) => ACCENT_HOVER,
        (ButtonKind::Secondary, false) => SURFACE,
        (ButtonKind::Secondary, true) => SURFACE_HOVER,
    };
    ui.painter().rect_filled(rect, RADIUS, fill);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, color);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Riquadro giallo con un avviso.
pub fn warning(ui: &mut egui::Ui, text: &str) {
    egui::Frame::new().fill(theme::WARNING_FILL).corner_radius(RADIUS).inner_margin(12).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.add(egui::Label::new(RichText::new(text).color(WARNING)).wrap());
    });
}

/// Etichetta piccola e tenue sopra un gruppo di informazioni.
pub fn caption(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).font(theme::bold(13.0)).color(TEXT_WEAK));
}

/// "oggi", "ieri", "5 giorni fa" a partire da un orario in secondi dal 1970.
pub fn when(time: i64, now: i64) -> String {
    match (now - time).max(0) / 86_400 {
        0 => "oggi".into(),
        1 => "ieri".into(),
        days if days < 60 => format!("{days} giorni fa"),
        days => format!("{} mesi fa", days / 30),
    }
}

pub fn duration(minutes: i64) -> String {
    match minutes {
        m if m < 60 => format!("{m} min"),
        m => format!("{} h {} min", m / 60, m % 60),
    }
}

/// "1 tavolo", "3 tavoli".
pub fn count(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_and_counts_read_naturally() {
        let day = 86_400;
        assert_eq!(when(10 * day, 10 * day + 5), "oggi");
        assert_eq!(when(10 * day, 11 * day + 5), "ieri");
        assert_eq!(when(0, 5 * day), "5 giorni fa");
        assert_eq!(when(0, 90 * day), "3 mesi fa");
        assert_eq!(duration(25), "25 min");
        assert_eq!(duration(85), "1 h 25 min");
        assert_eq!(count(1, "tavolo", "tavoli"), "1 tavolo");
        assert_eq!(count(21, "tavolo", "tavoli"), "21 tavoli");
    }
}
