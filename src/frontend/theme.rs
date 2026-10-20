//! Aspetto della finestra: colori, font e spaziature.
//!
//! Font inclusi nell'eseguibile (licenza OFL, vedi `assets/README.md`): Barlow per il testo,
//! Barlow SemiBold per le parti in evidenza, Bebas Neue per i titoli. I simboli che mancano
//! (★, ▶, ⚠…) arrivano dai font di egui, tenuti come riserva.

use eframe::egui::{self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, Vec2};
use std::sync::Arc;

pub const ACCENT: Color32 = Color32::from_rgb(255, 122, 36);
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(255, 146, 72);
/// Testo sopra il colore d'accento.
pub const ON_ACCENT: Color32 = Color32::from_rgb(24, 16, 10);
pub const BACKGROUND: Color32 = Color32::from_rgb(19, 21, 26);
pub const BAR: Color32 = Color32::from_rgb(13, 14, 18);
pub const SIDEBAR: Color32 = Color32::from_rgb(25, 27, 33);
pub const SURFACE: Color32 = Color32::from_rgb(31, 34, 41);
pub const SURFACE_HOVER: Color32 = Color32::from_rgb(42, 46, 55);
pub const TEXT: Color32 = Color32::from_gray(236);
pub const TEXT_WEAK: Color32 = Color32::from_gray(150);
pub const WARNING: Color32 = Color32::from_rgb(255, 196, 64);
pub const WARNING_FILL: Color32 = Color32::from_rgb(56, 45, 20);
pub const ERROR: Color32 = Color32::from_rgb(255, 128, 116);

pub const RADIUS: u8 = 10;

pub fn body(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}

pub fn display(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("display".into()))
}

fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    let fallback = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    for (name, bytes) in [
        ("barlow", &include_bytes!("../../assets/fonts/Barlow-Regular.ttf")[..]),
        ("barlow-semibold", &include_bytes!("../../assets/fonts/Barlow-SemiBold.ttf")[..]),
        ("bebas", &include_bytes!("../../assets/fonts/BebasNeue-Regular.ttf")[..]),
    ] {
        fonts.font_data.insert(name.into(), Arc::new(FontData::from_static(bytes)));
    }
    let family = |first: &[&str]| first.iter().map(ToString::to_string).chain(fallback.iter().cloned()).collect();
    fonts.families.insert(FontFamily::Proportional, family(&["barlow"]));
    fonts.families.insert(FontFamily::Name("bold".into()), family(&["barlow-semibold"]));
    fonts.families.insert(FontFamily::Name("display".into()), family(&["bebas", "barlow-semibold"]));
    fonts
}

pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(fonts());
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BACKGROUND;
    visuals.window_fill = SIDEBAR;
    visuals.window_stroke = Stroke::new(1.0_f32, SURFACE_HOVER);
    visuals.window_corner_radius = CornerRadius::same(14);
    visuals.menu_corner_radius = CornerRadius::same(RADIUS);
    visuals.extreme_bg_color = BAR;
    visuals.faint_bg_color = SURFACE;
    visuals.selection.bg_fill = ACCENT.linear_multiply(0.8);
    visuals.selection.stroke = Stroke::new(1.0_f32, TEXT);
    visuals.hyperlink_color = ACCENT;
    visuals.warn_fg_color = WARNING;
    visuals.error_fg_color = ERROR;
    for (widget, fill) in [
        (&mut visuals.widgets.inactive, SURFACE),
        (&mut visuals.widgets.hovered, SURFACE_HOVER),
        (&mut visuals.widgets.active, SURFACE_HOVER),
        (&mut visuals.widgets.open, SURFACE_HOVER),
    ] {
        widget.bg_fill = fill;
        widget.weak_bg_fill = fill;
        widget.corner_radius = CornerRadius::same(RADIUS);
        widget.fg_stroke.color = TEXT;
    }
    visuals.widgets.inactive.bg_stroke = Stroke::NONE;
    visuals.widgets.noninteractive.fg_stroke.color = TEXT;
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(RADIUS);
    ctx.set_visuals(visuals);
    ctx.style_mut(|style| {
        use egui::TextStyle;
        style.text_styles = [
            (TextStyle::Heading, display(32.0)),
            (TextStyle::Body, body(16.0)),
            (TextStyle::Button, body(16.0)),
            (TextStyle::Small, body(13.5)),
            (TextStyle::Monospace, FontId::monospace(14.0)),
        ]
        .into();
        style.spacing.button_padding = Vec2::new(12.0, 6.0);
        style.spacing.item_spacing = Vec2::new(8.0, 8.0);
        style.spacing.interact_size.y = 30.0;
    });
}
