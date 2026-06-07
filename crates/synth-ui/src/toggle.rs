//! Pill-shaped on/off toggle widget.
//!
//! A cleaner visual alternative to `egui::Checkbox` for enable/disable
//! parameters. The pill is accent-coloured when on, muted when off.
//!
//! Usage:
//! ```no_run
//! # use synth_ui::toggle::Toggle;
//! # let mut enabled = false;
//! # let mut ui: eframe::egui::Ui = unimplemented!();
//! if ui.add(Toggle::new(&mut enabled, "Reverb")).changed() {
//!     // send ParameterChange event
//! }
//! ```

use eframe::egui;

use crate::theme;

/// Width of the pill track.
const PILL_W: f32 = 28.0;
/// Height of the pill track.
const PILL_H: f32 = 14.0;
/// Radius of the sliding thumb circle.
const THUMB_R: f32 = 5.0;
/// Gap between the pill and the label text.
const LABEL_GAP: f32 = 4.0;

/// Pill-shaped on/off toggle.
pub struct Toggle<'a> {
    value: &'a mut bool,
    label: &'a str,
}

impl<'a> Toggle<'a> {
    /// Creates a toggle bound to `value` with a text label to the right.
    #[must_use]
    pub fn new(value: &'a mut bool, label: &'a str) -> Self {
        Self { value, label }
    }
}

impl egui::Widget for Toggle<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let label_galley = ui
            .painter()
            .layout_no_wrap(self.label.to_string(), theme::font_body(), theme::FG1);
        let total_width = PILL_W + LABEL_GAP + label_galley.size().x;
        let total_height = PILL_H.max(label_galley.size().y);
        let desired_size = egui::vec2(total_width, total_height);

        let (rect, mut response) = ui.allocate_exact_size(desired_size, egui::Sense::click());

        if response.clicked() {
            *self.value = !*self.value;
            response.mark_changed();
        }

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();

            // Pill track
            let pill_rect = egui::Rect::from_min_size(
                egui::pos2(rect.left(), rect.center().y - PILL_H / 2.0),
                egui::vec2(PILL_W, PILL_H),
            );
            let pill_rounding = egui::Rounding::same(PILL_H / 2.0);

            let track_color = if *self.value {
                theme::ACCENT.gamma_multiply(0.7)
            } else {
                theme::BG2
            };
            painter.rect_filled(pill_rect, pill_rounding, track_color);

            // Outline on hover
            if response.hovered() {
                painter.rect_stroke(
                    pill_rect,
                    pill_rounding,
                    egui::Stroke::new(1.0, theme::ACCENT.gamma_multiply(0.4)),
                );
            }

            // Thumb
            let thumb_x = if *self.value {
                pill_rect.right() - THUMB_R - 2.0
            } else {
                pill_rect.left() + THUMB_R + 2.0
            };
            let thumb_center = egui::pos2(thumb_x, pill_rect.center().y);
            let thumb_color = if *self.value { theme::ACCENT } else { theme::FG2 };
            painter.circle_filled(thumb_center, THUMB_R, thumb_color);

            // Label
            let label_pos = egui::pos2(
                pill_rect.right() + LABEL_GAP,
                rect.center().y - label_galley.size().y / 2.0,
            );
            painter.galley(label_pos, label_galley, theme::FG1);
        }

        response
    }
}
