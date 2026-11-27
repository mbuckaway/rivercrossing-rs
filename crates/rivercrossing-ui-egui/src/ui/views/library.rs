// SPDX-License-Identifier: GPL-3.0-only

//! The ride library's list and the typed delete confirmation.

use craft::egui::Ui;
use craft::ui::contract::Show as _;
use craft::ui::display::Text;

use super::dialogs::{
    COUNT, DATE, NAME, STATUS, TableData, TableRow, columns, stack_text_field, table,
};
use crate::ui::state::AppState;

/// The ride rows' visible height before the table scrolls.
const VISIBLE_ROWS: u16 = 6;

/// Show the library's rows.
pub(crate) fn body(ui: &mut Ui, state: &mut AppState) {
    let data = TableData {
        columns: columns(&[
            (NAME, "Ride"),
            (DATE, "Date"),
            (STATUS, "Status"),
            (COUNT, "Entries"),
        ]),
        rows: state
            .library
            .rows
            .iter()
            .map(|row| TableRow {
                label: row.name.clone(),
                // The label carries the ride's name into the first column.
                cells: vec![
                    (DATE, row.date.clone()),
                    (STATUS, row.status.clone()),
                    (COUNT, row.entries.to_string()),
                ],
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
    stack_text_field(ui, state, "confirm_name_input", "Ride name");
}
