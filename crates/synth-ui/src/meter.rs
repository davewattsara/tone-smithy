//! VU meter widget — vertical peak bar for left/right channels.
//!
//! Renders two thin bars side-by-side. The bar height is proportional to the
//! linear peak level (0..=1 = silence..=0 dBFS). Values above 1.0 mean the
//! signal has clipped; the bar turns [`theme::WARN`] in that case.
//!
//! ```no_run
//! # use synth_ui::meter::VuMeter;
//! # let (peak_l, peak_r) = (0.5_f32, 0.5_f32);
//! # let mut ui: eframe::egui::Ui = unimplemented!();
//! ui.add(VuMeter::new(peak_l, peak_r));
//! ```

use eframe::egui;

use crate::theme;

/// Width of each channel bar in logical pixels.
const BAR_WIDTH: f32 = 8.0;
/// Gap between the two channel bars.
const BAR_GAP: f32 = 2.0;
/// Total height of the meter.
const METER_HEIGHT: f32 = 80.0;

/// Stereo peak VU meter widget.
pub struct VuMeter {
    peak_left: f32,
    peak_right: f32,
}

impl VuMeter {
    /// Creates a meter displaying `peak_left` and `peak_right` (linear, 0..=∞).
    #[must_use]
    pub fn new(peak_left: f32, peak_right: f32) -> Self {
        Self { peak_left, peak_right }
    }
}

impl egui::Widget for VuMeter {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let total_width = BAR_WIDTH * 2.0 + BAR_GAP;
        let desired_size = egui::vec2(total_width, METER_HEIGHT);
        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());

        if ui.is_rect_visible(rect) {
            paint_bar(ui.painter(), rect.min, self.peak_left);
            let right_x = rect.min.x + BAR_WIDTH + BAR_GAP;
            paint_bar(ui.painter(), egui::pos2(right_x, rect.min.y), self.peak_right);
        }

        response
    }
}

fn paint_bar(painter: &egui::Painter, top_left: egui::Pos2, peak: f32) {
    let clipped = peak >= 1.0;
    let fill_frac = peak.min(1.0);
    let fill_height = METER_HEIGHT * fill_frac;

    let track = egui::Rect::from_min_size(top_left, egui::vec2(BAR_WIDTH, METER_HEIGHT));
    painter.rect_filled(track, 2.0, theme::BG2);

    if fill_height > 0.0 {
        let fill_top = top_left.y + METER_HEIGHT - fill_height;
        let fill_rect = egui::Rect::from_min_size(egui::pos2(top_left.x, fill_top), egui::vec2(BAR_WIDTH, fill_height));
        let color = if clipped { theme::WARN } else { theme::ACCENT };
        painter.rect_filled(fill_rect, 2.0, color);
    }
}
