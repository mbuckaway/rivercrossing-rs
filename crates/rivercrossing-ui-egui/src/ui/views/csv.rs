// SPDX-License-Identifier: GPL-3.0-only

//! The CSV import preview: the summary line, the blocking conflicts, the two import options and
//! the dialog's refusal.
//!
//! The dialog opens only with a previewed file in this release; the OS file picker that starts it
//! arrives with PR D's `craft_dialogs`. Import stays disabled while a conflict remains.

use craft::egui::Ui;
use craft::theme::variant::Tone;
use craft::ui::contract::Show as _;
use craft::ui::display::Text;

use super::dialogs::{NAME, TableData, TableRow, columns, notice, stack_flag, table};
use crate::ui::state::AppState;

/// The conflict rows' visible height before the table scrolls.
const VISIBLE_ROWS: u16 = 4;

/// Show the CSV preview body.
pub(crate) fn body(ui: &mut Ui, state: &mut AppState) {
    let summary = state.csv_preview.model.as_ref().map_or_else(
        || "Choose a CSV file to preview.".to_owned(),
        |model| model.summary.clone(),
    );
    ui.push_id("summary_lbl", |ui| {
        let _ = Text::new(summary).show(ui);
    });
    let conflicts = state
        .csv_preview
        .model
        .as_ref()
        .map(|model| model.conflicts.clone())
        .unwrap_or_default();
    let data = TableData {
        columns: columns(&[(NAME, "Conflict")]),
        rows: conflicts
            .iter()
            .map(|conflict| TableRow {
                label: conflict.clone(),
                cells: vec![(NAME, conflict.clone())],
            })
            .collect(),
    };
    let _ = table(
        ui,
        "conflicts_list",
        &data,
        VISIBLE_ROWS,
        "No conflicts",
        None,
    );
    option(ui, state, "map_unknown_sex_chk", "Map unknown sex to M");
    option(
        ui,
        state,
        "convert_teams_of_one_chk",
        "Convert teams of one to solo",
    );
    if let Some(errors) = &state.csv_preview.errors {
        notice(ui, "csv_infobar", Tone::Bad, errors.clone());
    }
}

/// Show one import option and preview the chosen file again when it changes.
fn option(ui: &mut Ui, state: &mut AppState, id: &'static str, label: &'static str) {
    if let Some(on) = stack_flag(ui, state, id, label) {
        state.set_csv_option(id, on);
    }
}
