// SPDX-License-Identifier: GPL-3.0-only

//! The About box and the Keyboard Shortcuts list, generated from the action registry.

use craft::egui::{self, Ui};

use super::text;
use crate::ui::action::spec::{ACTIONS, Shortcut};
use crate::ui::state::AppState;

/// The product version the About box names.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The product description line.
const DESCRIPTION: &str = "Timing & poker-hand scoring for poker-run rides.";

/// The author line.
const AUTHOR: &str = "Storytold";

/// The copyright line.
const COPYRIGHT: &str = "Copyright GORBA";

/// The licence line.
const LICENSE: &str = "GPL-3.0-only";

/// Show the About box.
pub(crate) fn about(ui: &mut Ui, _state: &mut AppState) {
    ui.push_id("version_lbl", |ui| {
        text(ui, &format!("RiverCrossing {VERSION}"));
    });
    text(ui, DESCRIPTION);
    ui.push_id("author_lbl", |ui| text(ui, AUTHOR));
    ui.push_id("copyright_lbl", |ui| text(ui, COPYRIGHT));
    ui.push_id("license_lbl", |ui| text(ui, LICENSE));
    ui.push_id("gorba_link", |ui| text(ui, "https://gorba.example"));
}

/// Show the shortcut table, generated from the registry.
pub(crate) fn shortcuts(ui: &mut Ui, _state: &mut AppState) {
    ui.push_id("shortcuts_list", |ui| {
        egui::ScrollArea::vertical()
            .id_salt("shortcuts_scroll")
            .max_height(420.0)
            .show(ui, |ui| {
                egui::Grid::new("shortcuts_grid")
                    .num_columns(2)
                    .striped(true)
                    .show(ui, |ui| {
                        for row in ACTIONS {
                            text(ui, row.label);
                            text(
                                ui,
                                row.shortcut
                                    .map_or_else(String::new, Shortcut::text)
                                    .as_str(),
                            );
                            ui.end_row();
                        }
                    });
            });
    });
}
