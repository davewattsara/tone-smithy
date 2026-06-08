//! Circular knob widget for continuous parameters.
//!
//! - Drag upward/downward to change value (200 px = full range).
//! - Hold **Shift** while dragging for 10× fine resolution.
//! - **Double-click** resets to the default value.
//! - **Right-click** opens a context menu (Reset, Copy value, MIDI Learn stub).
//! - Tooltip shows the formatted value on hover.
//! - Optional modulation ring: pass `mod_offset` to show a coloured arc
//!   indicating the current modulated displacement from the base value.

use std::ops::RangeInclusive;

use eframe::egui;

use crate::theme;

/// Diameter of the knob circle in logical pixels.
pub const KNOB_DIAMETER: f32 = 40.0;

/// Height reserved for the parameter name label below the circle.
const LABEL_HEIGHT: f32 = 14.0;

/// Height reserved for the formatted value line below the label.
const VALUE_HEIGHT: f32 = 14.0;

/// Total vertical drag distance (px) covering the full range at normal sensitivity.
const DRAG_PIXELS_FULL_RANGE: f32 = 200.0;

/// Fine-mode sensitivity multiplier (applied when Shift is held).
const FINE_MULTIPLIER: f32 = 0.1;

/// Circular knob widget for a continuous parameter.
///
/// ```no_run
/// # use synth_ui::knob::Knob;
/// # let mut cutoff = 1000.0_f32;
/// # let mut ui: eframe::egui::Ui = unimplemented!();
/// if ui.add(Knob::new(&mut cutoff, 20.0..=20_000.0, "Cutoff")).changed() {
///     // send ParameterChange event
/// }
/// ```
pub struct Knob<'a> {
    value: &'a mut f32,
    range: RangeInclusive<f32>,
    label: &'a str,
    /// Value restored on double-click or context-menu Reset.
    default_value: Option<f32>,
    /// Custom format function for tooltip and value readout.
    format: Option<Box<dyn Fn(f32) -> String + 'a>>,
    /// Normalised modulation offset (-1..=1) for the modulation ring.
    /// Positive → `MOD_POS` colour; negative → `MOD_NEG` colour.
    mod_offset: Option<f32>,
    /// Preset parameter key for MIDI Learn (e.g. `"filter_cutoff_hz"`).
    param_key: Option<&'a str>,
}

impl<'a> Knob<'a> {
    /// Creates a knob bound to `value` with the given range and label.
    #[must_use]
    pub fn new(value: &'a mut f32, range: RangeInclusive<f32>, label: &'a str) -> Self {
        Self {
            value,
            range,
            label,
            default_value: None,
            format: None,
            mod_offset: None,
            param_key: None,
        }
    }

    /// Sets the preset parameter key for MIDI Learn. When provided, the
    /// right-click "MIDI Learn" menu item becomes active.
    #[must_use]
    pub fn param_key(mut self, key: &'a str) -> Self {
        self.param_key = Some(key);
        self
    }

    /// Sets the value restored on double-click or context-menu Reset.
    #[must_use]
    pub fn default_value(mut self, v: f32) -> Self {
        self.default_value = Some(v);
        self
    }

    /// Overrides the value format used in the tooltip and readout text.
    #[must_use]
    pub fn format(mut self, f: impl Fn(f32) -> String + 'a) -> Self {
        self.format = Some(Box::new(f));
        self
    }

    /// Attaches a modulation ring. `offset` is normalised to -1..=1 over the
    /// full parameter range; the ring shows the displacement from base value.
    #[must_use]
    pub fn mod_offset(mut self, offset: f32) -> Self {
        self.mod_offset = Some(offset);
        self
    }
}

impl egui::Widget for Knob<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let desired_size = egui::vec2(KNOB_DIAMETER, KNOB_DIAMETER + LABEL_HEIGHT + VALUE_HEIGHT);
        let (rect, mut response) = ui.allocate_exact_size(desired_size, egui::Sense::click_and_drag());

        let value_text = match &self.format {
            Some(f) => f(*self.value),
            None => format!("{:.3}", *self.value),
        };

        // Double-click: reset to default.
        if response.double_clicked() {
            if let Some(def) = self.default_value {
                *self.value = def;
                response.mark_changed();
            }
        }

        // Drag: Shift held → fine mode (1/10 sensitivity).
        if response.dragged() {
            let fine = ui.input(|i| i.modifiers.shift);
            let range_span = *self.range.end() - *self.range.start();
            let sensitivity = if fine { FINE_MULTIPLIER } else { 1.0 };
            let delta = -response.drag_delta().y * (range_span / DRAG_PIXELS_FULL_RANGE) * sensitivity;
            *self.value = (*self.value + delta).clamp(*self.range.start(), *self.range.end());
            response.mark_changed();
        }

        // Right-click context menu. Capture intents in locals so the closure
        // does not borrow `response` a second time.
        let mut reset_clicked = false;
        let mut copy_clicked = false;
        let mut paste_clicked = false;
        let mut learn_clicked = false;
        response.context_menu(|ui| {
            ui.set_min_width(140.0);
            if ui
                .add_enabled(self.default_value.is_some(), egui::Button::new("Reset to default"))
                .clicked()
            {
                reset_clicked = true;
                ui.close_menu();
            }
            if ui.button("Copy value").clicked() {
                copy_clicked = true;
                ui.close_menu();
            }
            if ui.button("Paste value").clicked() {
                paste_clicked = true;
                ui.close_menu();
            }
            ui.separator();
            if ui
                .add_enabled(self.param_key.is_some(), egui::Button::new("MIDI Learn"))
                .clicked()
            {
                learn_clicked = true;
                ui.close_menu();
            }
        });
        if reset_clicked {
            if let Some(def) = self.default_value {
                *self.value = def;
                response.mark_changed();
            }
        }
        if copy_clicked {
            // Store the raw value in egui memory so Paste works cross-knob.
            ui.memory_mut(|mem| {
                mem.data.insert_temp(egui::Id::new("ts_knob_clip"), *self.value);
            });
            ui.ctx().copy_text(value_text.clone());
        }
        if paste_clicked {
            let clip: Option<f32> = ui.memory(|mem| mem.data.get_temp(egui::Id::new("ts_knob_clip")));
            if let Some(v) = clip {
                *self.value = v.clamp(*self.range.start(), *self.range.end());
                response.mark_changed();
            }
        }
        if learn_clicked {
            if let Some(key) = self.param_key {
                // Deposit key + range into egui memory. tick_midi_learn reads
                // all three items and enters learn mode with the full context.
                let start = *self.range.start();
                let end = *self.range.end();
                ui.memory_mut(|m| {
                    m.data.insert_temp(egui::Id::new("ts_ml_key"), key.to_string());
                    m.data.insert_temp(egui::Id::new("ts_ml_start"), start);
                    m.data.insert_temp(egui::Id::new("ts_ml_end"), end);
                });
            }
        }

        // Tooltip: label + formatted value, shown while hovered or dragging.
        if response.hovered() || response.dragged() {
            let tooltip = if self.label.is_empty() {
                value_text.clone()
            } else {
                format!("{}: {}", self.label, value_text)
            };
            response = response.on_hover_text(tooltip);
        }

        if ui.is_rect_visible(rect) {
            paint_knob(
                ui,
                rect,
                self.value,
                &self.range,
                self.label,
                &value_text,
                &response,
                self.mod_offset,
            );
        }

        response
    }
}

/// Paints the knob: background circle, groove arc, value arc, modulation ring,
/// indicator line, label, and value readout.
#[allow(clippy::too_many_arguments)]
fn paint_knob(
    ui: &egui::Ui,
    rect: egui::Rect,
    value: &f32,
    range: &RangeInclusive<f32>,
    label: &str,
    value_text: &str,
    response: &egui::Response,
    mod_offset: Option<f32>,
) {
    let painter = ui.painter();

    let radius = KNOB_DIAMETER / 2.0;
    let center = egui::pos2(rect.center().x, rect.top() + radius);

    // 270° arc: 7 o'clock (start) → 5 o'clock (end), clockwise.
    let start_angle: f32 = std::f32::consts::PI * 0.75;
    let end_angle: f32 = std::f32::consts::PI * 2.25;
    let sweep = end_angle - start_angle;

    // Background circle — state-dependent colour from theme visuals.
    let bg_color = if response.dragged() {
        theme::BG2.gamma_multiply(1.4)
    } else if response.hovered() {
        theme::BG2.gamma_multiply(1.2)
    } else {
        theme::BG2
    };
    painter.circle_filled(center, radius, bg_color);

    // Groove arc (full 270° track in muted colour).
    paint_arc(
        painter,
        center,
        radius - 4.0,
        start_angle,
        sweep,
        2.0,
        theme::FG2.gamma_multiply(0.5),
    );

    // Value arc in accent colour.
    let t = (*value - *range.start()) / (*range.end() - *range.start());
    let t = t.clamp(0.0, 1.0);
    paint_arc(
        painter,
        center,
        radius - 4.0,
        start_angle,
        sweep * t,
        2.5,
        theme::ACCENT,
    );

    // Modulation ring: coloured arc offset from the base-value position.
    if let Some(offset) = mod_offset {
        if offset.abs() > 1e-4 {
            let base_angle = start_angle + sweep * t;
            let mod_sweep = offset * sweep;
            let mod_color = if offset > 0.0 { theme::MOD_POS } else { theme::MOD_NEG };
            // Draw at a slightly tighter radius so it sits just inside the value arc.
            paint_arc(painter, center, radius - 6.5, base_angle, mod_sweep, 2.5, mod_color);
        }
    }

    // Indicator line from centre to rim.
    let angle = start_angle + sweep * t;
    let indicator = center + egui::vec2(angle.cos(), angle.sin()) * (radius - 5.0);
    painter.line_segment(
        [center, indicator],
        egui::Stroke::new(2.0, theme::FG0.gamma_multiply(0.9)),
    );

    // Label below the circle.
    let label_center = egui::pos2(rect.center().x, rect.top() + KNOB_DIAMETER + LABEL_HEIGHT * 0.5);
    painter.text(
        label_center,
        egui::Align2::CENTER_CENTER,
        label,
        theme::font_small(),
        theme::FG1,
    );

    // Value readout below the label.
    let value_center = egui::pos2(
        rect.center().x,
        rect.top() + KNOB_DIAMETER + LABEL_HEIGHT + VALUE_HEIGHT * 0.5,
    );
    painter.text(
        value_center,
        egui::Align2::CENTER_CENTER,
        value_text,
        theme::font_micro(),
        theme::FG2,
    );
}

/// Approximates an arc with short line segments (~1 segment per 4°).
fn paint_arc(
    painter: &egui::Painter,
    center: egui::Pos2,
    radius: f32,
    start_angle: f32,
    sweep: f32,
    stroke_width: f32,
    color: egui::Color32,
) {
    if sweep.abs() < 1e-4 {
        return;
    }
    let segments = ((sweep.abs() * (180.0 / std::f32::consts::PI)) / 4.0).ceil() as usize;
    let segments = segments.max(2);
    let step = sweep / segments as f32;
    let stroke = egui::Stroke::new(stroke_width, color);
    let mut prev = center + egui::vec2(start_angle.cos(), start_angle.sin()) * radius;
    for i in 1..=segments {
        let a = start_angle + step * i as f32;
        let next = center + egui::vec2(a.cos(), a.sin()) * radius;
        painter.line_segment([prev, next], stroke);
        prev = next;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn knob_clamps_drag_to_range() {
        let mut value = 1.0_f32;
        let range = 0.0_f32..=1.0_f32;
        let range_span = *range.end() - *range.start();
        let delta = 50.0 * (range_span / super::DRAG_PIXELS_FULL_RANGE);
        value = (value + delta).clamp(*range.start(), *range.end());
        assert_eq!(value, 1.0);
    }

    #[test]
    fn fine_mode_reduces_delta_by_ten() {
        let range_span = 1.0_f32;
        let drag_px = 10.0_f32;
        let normal = drag_px * (range_span / super::DRAG_PIXELS_FULL_RANGE);
        let fine = normal * super::FINE_MULTIPLIER;
        assert!((fine - normal / 10.0).abs() < 1e-6);
    }
}
