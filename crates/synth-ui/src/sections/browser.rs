//! Preset browser tab — search, filter by category, click to load.

use eframe::egui;
use synth_presets::{
    CATEGORIES, PresetEntry, load, load_factory_preset, map_to_events, map_to_snapshot, save, snapshot_to_map,
    user_presets_dir,
};

use crate::app::ToneSmithyApp;
use crate::theme;

impl ToneSmithyApp {
    pub(crate) fn presets_tab(&mut self, ui: &mut egui::Ui) {
        ui.add_space(theme::PANEL_PADDING);

        // ── Search + category bar ──────────────────────────────────────────
        ui.horizontal(|ui| {
            ui.add_space(theme::PANEL_PADDING);
            ui.label(
                egui::RichText::new("Search:")
                    .color(theme::FG1)
                    .font(theme::font_small()),
            );
            ui.add(
                egui::TextEdit::singleline(&mut self.preset_search)
                    .desired_width(200.0)
                    .hint_text("name, author, or tag")
                    .font(theme::font_body()),
            );
            ui.add_space(16.0);

            // Category chips
            let all_selected = self.preset_category_filter.is_empty();
            if ui
                .selectable_label(all_selected, egui::RichText::new("All").font(theme::font_small()))
                .clicked()
            {
                self.preset_category_filter.clear();
            }
            for &cat in CATEGORIES {
                let selected = self.preset_category_filter == cat;
                if ui
                    .selectable_label(selected, egui::RichText::new(cat).font(theme::font_small()))
                    .clicked()
                {
                    if selected {
                        self.preset_category_filter.clear();
                    } else {
                        self.preset_category_filter = cat.to_string();
                    }
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Refresh").clicked() {
                    self.refresh_preset_list();
                }
            });
        });

        ui.add_space(theme::GROUP_GAP);
        theme::subtle_separator(ui);
        ui.add_space(4.0);

        // Snapshot of current filter state (avoids borrow issues in the loop).
        let search = self.preset_search.clone();
        let cat_filter = self.preset_category_filter.clone();
        let current_name = self.patch_name.clone();

        // Collect filtered entries split into factory / user.
        let (factory_entries, user_entries): (Vec<_>, Vec<_>) = self
            .preset_entries
            .iter()
            .filter(|e| e.matches_search(&search) && e.matches_category(&cat_filter))
            .cloned()
            .partition(|e| e.is_factory);

        // ── Browser columns: list + detail ─────────────────────────────────
        ui.columns(2, |cols| {
            // Left column: scrollable preset list
            egui::ScrollArea::vertical()
                .id_salt("preset_list")
                .show(&mut cols[0], |ui| {
                    if !factory_entries.is_empty() {
                        ui.label(
                            egui::RichText::new(format!("FACTORY ({})", factory_entries.len()))
                                .color(theme::FG2)
                                .font(theme::font_small()),
                        );
                        ui.add_space(4.0);
                        for entry in &factory_entries {
                            preset_row(ui, entry, &current_name, &mut self.load_actions);
                        }
                        ui.add_space(theme::GROUP_GAP);
                        theme::subtle_separator(ui);
                        ui.add_space(4.0);
                    }

                    ui.label(
                        egui::RichText::new(format!("USER ({})", user_entries.len()))
                            .color(theme::FG2)
                            .font(theme::font_small()),
                    );
                    ui.add_space(4.0);
                    if user_entries.is_empty() {
                        ui.label(
                            egui::RichText::new("No user presets found.")
                                .color(theme::FG2)
                                .font(theme::font_small()),
                        );
                        ui.label(
                            egui::RichText::new(
                                user_presets_dir()
                                    .map(|p| format!("Drop .tsmith files into:\n{}", p.display()))
                                    .unwrap_or_default(),
                            )
                            .color(theme::FG2)
                            .font(theme::font_micro()),
                        );
                    } else {
                        for entry in &user_entries {
                            preset_row(ui, entry, &current_name, &mut self.load_actions);
                        }
                    }
                });

            // Right column: description of the hovered/selected preset
            cols[1].add_space(theme::PANEL_PADDING);
            cols[1].label(
                egui::RichText::new("Preset Browser")
                    .color(theme::FG2)
                    .font(theme::font_small()),
            );
            cols[1].add_space(4.0);
            cols[1].label(
                egui::RichText::new(format!("Active patch: {current_name}"))
                    .color(theme::FG1)
                    .font(theme::font_body()),
            );
            cols[1].add_space(theme::GROUP_GAP);
            if let Some(dir) = user_presets_dir() {
                cols[1].label(
                    egui::RichText::new(format!("User preset dir:\n{}", dir.display()))
                        .color(theme::FG2)
                        .font(theme::font_micro()),
                );
            }
        });

        // Apply any load actions collected during the row rendering loop.
        let actions: Vec<_> = self.load_actions.drain(..).collect();
        for action in actions {
            match action {
                LoadAction::LoadFactory(name) => self.load_factory_preset(&name),
                LoadAction::LoadFile(path) => self.load_file_preset(&path),
                LoadAction::DeleteFile(path) => self.delete_user_preset(&path),
                LoadAction::SaveCurrentAs(path) => self.save_current_as(&path),
            }
        }
    }

    fn load_factory_preset(&mut self, name: &str) {
        let Some(preset) = load_factory_preset(name) else {
            return;
        };
        for event in map_to_events(&preset.parameters) {
            self.events.send(event);
        }
        let snap = map_to_snapshot(&preset.parameters);
        self.sync_from_snapshot(&snap);
        self.patch_name = preset.metadata.name.clone();
        self.preset_error = None;
    }

    fn load_file_preset(&mut self, path: &std::path::Path) {
        match load(path) {
            Ok(preset) => {
                for event in map_to_events(&preset.parameters) {
                    self.events.send(event);
                }
                let snap = map_to_snapshot(&preset.parameters);
                self.sync_from_snapshot(&snap);
                self.patch_name = preset.metadata.name.clone();
                self.preset_error = None;
            }
            Err(e) => {
                self.preset_error = Some(format!("Load failed: {e}"));
            }
        }
    }

    fn delete_user_preset(&mut self, path: &std::path::Path) {
        if let Err(e) = std::fs::remove_file(path) {
            self.preset_error = Some(format!("Delete failed: {e}"));
        } else {
            self.refresh_preset_list();
        }
    }

    fn save_current_as(&mut self, path: &std::path::Path) {
        use synth_engine::param_bus::load_snapshot;
        let snapshot = load_snapshot(&self.snapshot_slot);
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_string();
        let mut preset = synth_presets::Preset::new(name.clone());
        preset.parameters = snapshot_to_map(&snapshot);
        if let Err(e) = save(path, &preset) {
            self.preset_error = Some(format!("Save failed: {e}"));
        } else {
            self.patch_name = name;
            self.refresh_preset_list();
        }
    }
}

// ── Deferred action queue ─────────────────────────────────────────────────────
// egui closures can't borrow self mutably, so we collect actions during
// rendering and apply them after the UI loop.

#[derive(Debug)]
pub(crate) enum LoadAction {
    LoadFactory(String),
    LoadFile(std::path::PathBuf),
    DeleteFile(std::path::PathBuf),
    SaveCurrentAs(std::path::PathBuf),
}

// ── Row renderer ──────────────────────────────────────────────────────────────

fn preset_row(ui: &mut egui::Ui, entry: &PresetEntry, current_name: &str, actions: &mut Vec<LoadAction>) {
    let is_current = entry.metadata.name == current_name;
    let name_text = egui::RichText::new(&entry.metadata.name)
        .font(theme::font_body())
        .color(if is_current { theme::ACCENT } else { theme::FG0 });

    let cat_text = egui::RichText::new(&entry.metadata.category)
        .font(theme::font_small())
        .color(theme::FG2);

    ui.horizontal(|ui| {
        let resp = ui.add(egui::Label::new(name_text).sense(egui::Sense::click()));
        ui.label(cat_text);

        // Left-click: load
        if resp.clicked() {
            if entry.is_factory {
                actions.push(LoadAction::LoadFactory(entry.metadata.name.clone()));
            } else if let Some(path) = &entry.path {
                actions.push(LoadAction::LoadFile(path.clone()));
            }
        }

        // Right-click context menu
        resp.context_menu(|ui| {
            if ui.button("Load").clicked() {
                if entry.is_factory {
                    actions.push(LoadAction::LoadFactory(entry.metadata.name.clone()));
                } else if let Some(path) = &entry.path {
                    actions.push(LoadAction::LoadFile(path.clone()));
                }
                ui.close_menu();
            }
            if !entry.is_factory {
                ui.separator();
                if let Some(path) = &entry.path {
                    if ui.button("Save current as this preset").clicked() {
                        actions.push(LoadAction::SaveCurrentAs(path.clone()));
                        ui.close_menu();
                    }
                    if ui.button("Delete").clicked() {
                        actions.push(LoadAction::DeleteFile(path.clone()));
                        ui.close_menu();
                    }
                }
            }
        });
    });
}
