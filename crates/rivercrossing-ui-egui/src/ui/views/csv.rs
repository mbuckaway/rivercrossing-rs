// SPDX-License-Identifier: GPL-3.0-only

//! The CSV import preview: the summary line, the blocking conflicts, the two import options and
//! the dialog's refusal.
//!
//! The dialog opens only with a previewed file in this release; the OS file picker that starts it
//! arrives with PR D's `craft_dialogs`. Import stays disabled while a conflict remains.

use craft::egui::{self, Ui};

use super::text;
use crate::ui::state::AppState;

/// Show the CSV preview body.
pub(crate) fn body(ui: &mut Ui, state: &mut AppState) {
    let summary = state.csv_preview.model.as_ref().map_or_else(
        || "Choose a CSV file to preview.".to_owned(),
        |model| model.summary.clone(),
    );
    ui.push_id("summary_lbl", |ui| text(ui, &summary));
    let conflicts = state
        .csv_preview
        .model
        .as_ref()
        .map(|model| model.conflicts.clone())
        .unwrap_or_default();
    ui.push_id("conflicts_list", |ui| {
        egui::ScrollArea::vertical()
            .id_salt("conflicts_scroll")
            .max_height(120.0)
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    for conflict in &conflicts {
                        text(ui, conflict);
                    }
                });
            });
    });
    option(ui, state, "map_unknown_sex_chk", "Map unknown sex to M");
    option(
        ui,
        state,
        "convert_teams_of_one_chk",
        "Convert teams of one to solo",
    );
    if let Some(errors) = &state.csv_preview.errors {
        ui.push_id("csv_infobar", |ui| text(ui, errors));
    }
}

/// Show one import option and preview the chosen file again when it changes.
fn option(ui: &mut Ui, state: &mut AppState, id: &str, label: &str) {
    let mut on = state.dialogs.field(id) == Some("true");
    let changed = ui
        .push_id(id, |ui| ui.checkbox(&mut on, label).changed())
        .inner;
    if changed {
        state.set_csv_option(id, on);
    }
}
