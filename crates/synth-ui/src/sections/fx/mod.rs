mod chorus;
mod delay;
mod drive;
mod eq;
mod reverb;

use eframe::egui;

use crate::app::ToneSmithyApp;
use crate::theme;

impl ToneSmithyApp {
    pub(crate) fn fx_tab(&mut self, ui: &mut egui::Ui) {
        ui.add_space(theme::PANEL_PADDING);
        theme::section_label(ui, "FX CHAIN");
        ui.add_space(4.0);

        ui.columns(5, |cols| {
            cols[0].vertical(|ui| self.eq_section(ui));
            cols[1].vertical(|ui| self.drive_section(ui));
            cols[2].vertical(|ui| self.chorus_section(ui));
            cols[3].vertical(|ui| self.delay_section(ui));
            cols[4].vertical(|ui| self.reverb_section(ui));
        });
    }
}
