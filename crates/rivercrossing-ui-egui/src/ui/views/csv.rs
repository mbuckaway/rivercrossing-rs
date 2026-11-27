// SPDX-License-Identifier: GPL-3.0-only

//! The CSV import preview: the summary line, the blocking conflicts, the two import options and
//! the dialog's refusal.
//!
//! The dialog opens only with a previewed file in this release; the OS file picker that starts it
//! arrives with PR D's `craft_dialogs`. Import stays disabled while a conflict remains.

use craft::behavior::{ColumnKey, ColumnWidth};
use craft::egui::Ui;
use craft::theme::variant::Tone;
use craft::ui::contract::{ColumnSpec, Show as _};
use craft::ui::display::Text;

use super::dialogs::{TableData, TableRow, notice, stack_flag, table};
use crate::ui::presenters::riders::CsvConflict;
use crate::ui::state::AppState;

#[cfg(test)]
mod tests;

/// The conflict rows' visible height before the table scrolls.
const VISIBLE_ROWS: u16 = 4;

/// The row-number column's width, as the design pins a number column.
const ROW_WIDTH: f32 = 60.0;

/// The conflict table's row-number column.
const ROW: ColumnKey = ColumnKey::new("row");

/// The conflict table's problem column.
const PROBLEM: ColumnKey = ColumnKey::new("problem");

/// The warning under the conflict table.
const WARNING: &str = "Import is disabled while conflicts remain.";

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
    if !conflicts.is_empty() {
        // The design puts the warning above the table, closest to the reason Import is disabled.
        notice(ui, "csv_conflicts_warn", Tone::Warn, WARNING);
    }
    let data = conflict_table(&conflicts);
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

/// Return the conflict table: the file row beside the problem that blocks the import, the problem
/// filling the dialog.
fn conflict_table(conflicts: &[CsvConflict]) -> TableData {
    TableData {
        columns: vec![
            ColumnSpec::new(ROW, "Row", ColumnWidth::Fixed(ROW_WIDTH)).numeric(),
            ColumnSpec::new(PROBLEM, "Problem", ColumnWidth::fill(1.0)),
        ],
        rows: conflicts
            .iter()
            .map(|conflict| TableRow {
                // The problem takes the label column, so assistive technology names the row by it.
                label: conflict.message.clone(),
                cells: vec![(ROW, conflict.row.to_string())],
                ..TableRow::default()
            })
            .collect(),
    }
}

/// Show one import option and preview the chosen file again when it changes.
fn option(ui: &mut Ui, state: &mut AppState, id: &'static str, label: &'static str) {
    if let Some(on) = stack_flag(ui, state, id, label) {
        state.set_csv_option(id, on);
    }
}
