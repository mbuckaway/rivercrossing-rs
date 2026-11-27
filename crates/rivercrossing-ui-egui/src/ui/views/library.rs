// SPDX-License-Identifier: GPL-3.0-only

//! The ride library's list and the typed delete confirmation.

use craft::behavior::ColumnWidth;
use craft::egui::Ui;
use craft::theme::variant::Tone;
use craft::ui::contract::{ColumnSpec, Show as _};
use craft::ui::display::Text;

use super::dialogs::{
    BadgeCell, COUNT, DATE, NAME, STATUS, TableData, TableRow, stack_text_field, table,
};
use crate::ui::state::AppState;

#[cfg(test)]
mod tests;

/// The ride rows' visible height before the table scrolls.
const VISIBLE_ROWS: u16 = 6;

/// Show the library's rows.
pub(crate) fn body(ui: &mut Ui, state: &mut AppState) {
    let data = TableData {
        columns: vec![
            ColumnSpec::new(NAME, "Ride", ColumnWidth::Content),
            ColumnSpec::new(DATE, "Date", ColumnWidth::Content),
            // The badge must fit "FINISHED"; a content column measures the badge short and clips it.
            ColumnSpec::new(STATUS, "Status", ColumnWidth::Fixed(110.0)),
            ColumnSpec::new(COUNT, "Entries", ColumnWidth::Content),
        ],
        rows: state
            .library
            .rows
            .iter()
            .map(|row| TableRow {
                label: row.name.clone(),
                // The label carries the ride's name into the first column.
                cells: vec![(DATE, row.date.clone()), (COUNT, row.entries.to_string())],
                // The design draws the lifecycle status as a badge, never as plain text.
                badges: vec![(
                    STATUS,
                    BadgeCell {
                        text: row.status.clone(),
                        tone: status_tone(&row.status),
                    },
                )],
            })
            .collect(),
    };
    if let Some(index) = table(
        ui,
        "rides_list",
        &data,
        VISIBLE_ROWS,
        "No rides",
        state.library.selected,
    ) {
        state.library.selected = Some(index);
    }
}

/// Return the badge tone of a lifecycle status, as the design pairs each state with a colour.
fn status_tone(status: &str) -> Tone {
    match status {
        "RUNNING" => Tone::Good,
        "FINISHED" => Tone::Info,
        "REOPENED" => Tone::Warn,
        _ => Tone::Neutral,
    }
}

/// Show the typed delete confirmation.
pub(crate) fn delete_body(ui: &mut Ui, state: &mut AppState) {
    let message = state
        .library
        .delete_message
        .clone()
        .unwrap_or_else(|| "Type the ride's name to confirm.".to_owned());
    ui.push_id("message_lbl", |ui| {
        let _ = Text::new(message).show(ui);
    });
    stack_text_field(
        ui,
        state,
        "confirm_name_input",
        "Type the ride name to confirm",
    );
}
