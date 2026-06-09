//! Helper for attaching MIDI Learn to any egui widget response.
//!
//! The Knob widget handles MIDI Learn internally, but other widgets (DragValue,
//! selectable labels, etc.) get MIDI Learn by calling [`attach_learn_menu`]
//! on the response they receive from `ui.add()`.

use eframe::egui;

/// Attaches a "MIDI Learn" right-click context menu to `response`.
///
/// When the menu item is clicked the `(key, range_start, range_end)` triple
/// is deposited in egui memory so that `tick_midi_learn` picks it up on the
/// next frame and enters learn mode.
///
/// Call this immediately after `ui.add(...)` has returned:
///
/// ```no_run
/// # use synth_ui::midi_learn_ext::attach_learn_menu;
/// # let mut val = 0i32;
/// # let mut ui: eframe::egui::Ui = unimplemented!();
/// let mut resp = ui.add(eframe::egui::DragValue::new(&mut val).range(1..=15));
/// attach_learn_menu(&mut resp, &mut ui, "my_param", 1.0, 15.0);
/// ```
pub fn attach_learn_menu(
    response: &mut egui::Response,
    ui: &mut egui::Ui,
    key: &str,
    range_start: f32,
    range_end: f32,
) {
    let mut clicked = false;
    response.context_menu(|ui| {
        if ui.button("MIDI Learn").clicked() {
            clicked = true;
            ui.close_menu();
        }
    });
    if clicked {
        let key = key.to_string();
        ui.memory_mut(|m| {
            m.data.insert_temp(egui::Id::new("ts_ml_key"), key);
            m.data.insert_temp(egui::Id::new("ts_ml_start"), range_start);
            m.data.insert_temp(egui::Id::new("ts_ml_end"), range_end);
        });
    }
}
