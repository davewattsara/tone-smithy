//! Circular knob widget for continuous parameters.
//!
//! Drag upward to increase, downward to decrease. A 200 px drag covers
//! the full range. Right-click resets to the default value. Tooltip
//! shows the formatted value while hovered or dragged.

use std::ops::RangeInclusive;

use eframe::egui;

/// Diameter of the knob circle in logical pixels.
const KNOB_DIAMETER: f32 = 40.0;

/// Height reserved for the parameter name label below the circle.
const LABEL_HEIGHT: f32 = 14.0;

/// Height reserved for the formatted value line below the label.
const VALUE_HEIGHT: f32 = 14.0;

/// Total vertical drag distance (px) that covers the full parameter range.
const DRAG_PIXELS_FULL_RANGE: f32 = 200.0;

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
    /// Optional value to restore on right-click.
    default_value: Option<f32>,
    /// Custom format function for the tooltip.
    format: Option<Box<dyn Fn(f32) -> String + 'a>>,
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
        }
    }

    /// Sets the value that right-clicking resets to.
    #[must_use]
    pub fn default_value(mut self, v: f32) -> Self {
        self.default_value = Some(v);
        self
    }

    /// Overrides the tooltip format. The closure receives the current value.
    #[must_use]
    pub fn format(mut self, f: impl Fn(f32) -> String + 'a) -> Self {
        self.format = Some(Box::new(f));
        self
    }
}

impl egui::Widget for Knob<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let desired_size = egui::vec2(KNOB_DIAMETER, KNOB_DIAMETER + LABEL_HEIGHT + VALUE_HEIGHT);
        let (rect, mut response) = ui.allocate_exact_size(desired_size, egui::Sense::click_and_drag());

        // Right-click: reset to default.
        if response.secondary_clicked() {
            if let Some(def) = self.default_value {
                *self.value = def;
                response.mark_changed();
            }
        }

        // Drag: vertical movement maps linearly to the range.
        if response.dragged() {
            let range_span = *self.range.end() - *self.range.start();
            let delta = -response.drag_delta().y * (range_span / DRAG_PIXELS_FULL_RANGE);
            *self.value = (*self.value + delta).clamp(*self.range.start(), *self.range.end());
            response.mark_changed();
        }

        let value_text = match &self.format {
            Some(f) => f(*self.value),
            None => format!("{:.3}", *self.value),
        };

        if ui.is_rect_visible(rect) {
            paint_knob(ui, rect, self.value, &self.range, self.label, &value_text, &response);
        }

        response
    }
}

/// Paints the knob circle, value indicator line, label, and current value text.
fn paint_knob(
    ui: &egui::Ui,
    rect: egui::Rect,
    value: &f32,
    range: &RangeInclusive<f32>,
    label: &str,
    value_text: &str,
    response: &egui::Response,
) {
    let painter = ui.painter();
    let visuals = ui.visuals();

    let radius = KNOB_DIAMETER / 2.0;
    let center = egui::pos2(rect.center().x, rect.top() + radius);

    // Track arc: 270° sweep, from -135° (bottom-left) to +135° (bottom-right).
    // 0° is 12 o'clock. Angles increase clockwise per egui convention.
    let start_angle: f32 = std::f32::consts::PI * 0.75; // 135° = 7 o'clock
    let end_angle: f32 = std::f32::consts::PI * 2.25; // 405° = 5 o'clock
    let sweep = end_angle - start_angle; // 270° = 1.5π

    // Background circle
    let bg_color = if response.dragged() {
        visuals.widgets.active.bg_fill
    } else if response.hovered() {
        visuals.widgets.hovered.bg_fill
    } else {
        visuals.widgets.inactive.bg_fill
    };
    painter.circle_filled(center, radius, bg_color);

    // Groove arc (full 270° track)
    paint_arc(
        painter,
        center,
        radius - 4.0,
        start_angle,
        sweep,
        2.0,
        visuals.faint_bg_color,
    );

    // Value arc (filled portion)
    let t = (*value - *range.start()) / (*range.end() - *range.start());
    let t = t.clamp(0.0, 1.0);
    let accent = visuals.selection.stroke.color;
    paint_arc(painter, center, radius - 4.0, start_angle, sweep * t, 2.0, accent);

    // Indicator line from center to rim
    let angle = start_angle + sweep * t;
    let indicator = center + egui::vec2(angle.cos(), angle.sin()) * (radius - 5.0);
    painter.line_segment([center, indicator], egui::Stroke::new(2.0, egui::Color32::WHITE));

    // Parameter name below the circle.
    let label_center = egui::pos2(rect.center().x, rect.top() + KNOB_DIAMETER + LABEL_HEIGHT * 0.5);
    painter.text(
        label_center,
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(11.0),
        visuals.text_color(),
    );
    // Formatted value below the label.
    let value_center = egui::pos2(
        rect.center().x,
        rect.top() + KNOB_DIAMETER + LABEL_HEIGHT + VALUE_HEIGHT * 0.5,
    );
    painter.text(
        value_center,
        egui::Align2::CENTER_CENTER,
        value_text,
        egui::FontId::proportional(10.0),
        visuals.weak_text_color(),
    );
}

/// Approximates an arc by drawing short line segments.
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
    // Number of segments: ~1 per 4° of arc.
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
        // Verify the clamp logic: a value already at max cannot go above it.
        let mut value = 1.0_f32;
        let range = 0.0_f32..=1.0_f32;
        let range_span = *range.end() - *range.start();
        let delta = 50.0 * (range_span / super::DRAG_PIXELS_FULL_RANGE);
        value = (value + delta).clamp(*range.start(), *range.end());
        assert_eq!(value, 1.0);
    }
}
