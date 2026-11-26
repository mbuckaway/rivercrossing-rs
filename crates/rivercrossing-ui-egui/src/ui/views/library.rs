// SPDX-License-Identifier: GPL-3.0-only

//! The ride library's list and the typed delete confirmation.

use craft::egui::{self, Ui};

use super::dialogs::stack_text_field;
use super::text;
use crate::ui::state::AppState;

/// Show the library's rows.
pub(crate) fn body(ui: &mut Ui, state: &mut AppState) {
    let mut clicked = None;
    ui.push_id("rides_list", |ui| {
        egui::ScrollArea::vertical()
            .id_salt("rides_list_scroll")
            .max_height(260.0)
            .show(ui, |ui| {
                egui::Grid::new("rides_grid")
                    .num_columns(4)
                    .striped(true)
                    .show(ui, |ui| {
                        for heading in ["Ride", "Date", "Status", "Entries"] {
                            text(ui, heading);
                        }
                        ui.end_row();
                        for (index, row) in state.library.rows.iter().enumerate() {
                            let selected = state.library.selected == Some(index);
                            if super::cell(ui, selected, &row.name) {
                                clicked = Some(index);
                            }
                            text(ui, &row.date);
                            text(ui, &row.status);
                            text(ui, &row.entries.to_string());
                            ui.end_row();
                        }
                    });
            });
    });
    if let Some(index) = clicked {
        state.library.selected = Some(index);
    }
}

/// Show the typed delete confirmation.
pub(crate) fn delete_body(ui: &mut Ui, state: &mut AppState, focused: Option<&str>) {
    let message = state
        .library
        .delete_message
        .clone()
        .unwrap_or_else(|| "Type the ride's name to confirm.".to_owned());
    ui.push_id("message_lbl", |ui| text(ui, &message));
    stack_text_field(ui, state, "confirm_name_input", "Ride name", focused);
}
