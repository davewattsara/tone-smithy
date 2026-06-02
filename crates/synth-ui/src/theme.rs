//! Visual theme — palette tokens, type scale, spacing, and egui visuals builder.
//!
//! A single `Theme` is constructed at the top of every `update()` call and
//! passed into panels and widgets by reference. All colours and sizes live
//! here; nothing is hardcoded in widget or panel code.

use eframe::egui::{self, Color32, FontId, Rounding, Stroke, Visuals};

// ── Palette ──────────────────────────────────────────────────────────────────

/// Near-black window background.
pub const BG0: Color32 = Color32::from_rgb(0x0E, 0x10, 0x13);
/// Panel background.
pub const BG1: Color32 = Color32::from_rgb(0x17, 0x1A, 0x1F);
/// Control well / inset background.
pub const BG2: Color32 = Color32::from_rgb(0x1F, 0x23, 0x2A);
/// Primary text.
pub const FG0: Color32 = Color32::from_rgb(0xE6, 0xE8, 0xEB);
/// Secondary text / parameter labels.
pub const FG1: Color32 = Color32::from_rgb(0x8A, 0x92, 0x9E);
/// Muted / tertiary text / units.
pub const FG2: Color32 = Color32::from_rgb(0x52, 0x59, 0x64);
/// Single accent colour — knob arcs, active selection, focus rings.
pub const ACCENT: Color32 = Color32::from_rgb(0x5B, 0xC8, 0xDE);
/// Warning / clip indicator / destructive actions.
pub const WARN: Color32 = Color32::from_rgb(0xE0, 0x79, 0x5B);
/// Modulation ring — positive direction.
pub const MOD_POS: Color32 = Color32::from_rgb(0x4D, 0xC9, 0x7A);
/// Modulation ring — negative direction.
pub const MOD_NEG: Color32 = Color32::from_rgb(0xD4, 0x5C, 0xA0);

// ── Sizing ────────────────────────────────────────────────────────────────────

/// Standard diameter for knob circles, in logical pixels.
pub const KNOB_DIAMETER: f32 = 40.0;
/// Padding inside section panels.
pub const PANEL_PADDING: f32 = 8.0;
/// Gap between parameter groups within a panel.
pub const GROUP_GAP: f32 = 12.0;
/// Height of the header bar.
pub const HEADER_HEIGHT: f32 = 40.0;
/// Height of the tab bar.
pub const TAB_BAR_HEIGHT: f32 = 32.0;
/// Height of the virtual keyboard strip.
pub const KEYBOARD_HEIGHT: f32 = 80.0;
/// Height of the status footer.
pub const FOOTER_HEIGHT: f32 = 24.0;

// ── Type scale ────────────────────────────────────────────────────────────────

/// Section / preset name — large heading.
pub fn font_display() -> FontId {
    FontId::proportional(16.0)
}
/// Parameter labels, dropdown items, browser entries.
pub fn font_body() -> FontId {
    FontId::proportional(12.0)
}
/// Units, hints, secondary labels.
pub fn font_small() -> FontId {
    FontId::proportional(11.0)
}
/// Footer, tooltips, micro labels.
pub fn font_micro() -> FontId {
    FontId::proportional(10.0)
}

// ── Visuals builder ───────────────────────────────────────────────────────────

/// Builds an `egui::Visuals` from the theme palette. Apply once per frame
/// with `ctx.set_visuals(theme_visuals())`.
#[must_use]
pub fn make_visuals() -> Visuals {
    let mut v = Visuals::dark();

    v.override_text_color = Some(FG0);
    v.panel_fill = BG1;
    v.window_fill = BG0;
    v.extreme_bg_color = BG2;
    v.faint_bg_color = BG2;
    v.code_bg_color = BG2;

    v.selection.bg_fill = ACCENT.gamma_multiply(0.25);
    v.selection.stroke = Stroke::new(1.0, ACCENT);

    v.hyperlink_color = ACCENT;
    v.warn_fg_color = WARN;
    v.error_fg_color = WARN;

    // Widget states
    let rounding = Rounding::same(4.0);

    v.widgets.noninteractive = {
        let mut w = v.widgets.noninteractive;
        w.bg_fill = BG1;
        w.bg_stroke = Stroke::new(1.0, BG2);
        w.fg_stroke = Stroke::new(1.0, FG1);
        w.rounding = rounding;
        w
    };
    v.widgets.inactive = {
        let mut w = v.widgets.inactive;
        w.bg_fill = BG2;
        w.bg_stroke = Stroke::NONE;
        w.fg_stroke = Stroke::new(1.0, FG1);
        w.rounding = rounding;
        w
    };
    v.widgets.hovered = {
        let mut w = v.widgets.hovered;
        w.bg_fill = Color32::from_rgb(0x2A, 0x30, 0x3A);
        w.bg_stroke = Stroke::new(1.0, ACCENT.gamma_multiply(0.5));
        w.fg_stroke = Stroke::new(1.0, FG0);
        w.rounding = rounding;
        w
    };
    v.widgets.active = {
        let mut w = v.widgets.active;
        w.bg_fill = ACCENT.gamma_multiply(0.15);
        w.bg_stroke = Stroke::new(1.0, ACCENT);
        w.fg_stroke = Stroke::new(1.0, FG0);
        w.rounding = rounding;
        w
    };
    v.widgets.open = {
        let mut w = v.widgets.open;
        w.bg_fill = BG2;
        w.bg_stroke = Stroke::new(1.0, ACCENT.gamma_multiply(0.6));
        w.fg_stroke = Stroke::new(1.0, FG0);
        w.rounding = rounding;
        w
    };

    v.window_rounding = Rounding::same(6.0);
    v.window_stroke = Stroke::new(1.0, BG2);
    v.popup_shadow = egui::epaint::Shadow::NONE;
    v.window_shadow = egui::epaint::Shadow::NONE;

    v
}

/// Draws a section heading label styled with `FG1` at the `font_small` scale.
/// Use at the top of each panel column to identify the section.
pub fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.add_space(2.0);
    ui.label(egui::RichText::new(text).color(FG1).font(font_small()));
    ui.add_space(4.0);
}

/// Renders a subtle horizontal rule in `FG2` colour.
pub fn subtle_separator(ui: &mut egui::Ui) {
    let color = FG2.gamma_multiply(0.4);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
    ui.painter()
        .line_segment([rect.left_center(), rect.right_center()], Stroke::new(1.0, color));
}
