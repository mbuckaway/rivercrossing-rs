// SPDX-License-Identifier: GPL-3.0-only

//! The audit viewer's search, action filter and rows.

use craft::egui::{self, ComboBox, TextEdit, Ui};

use super::text;
use crate::ui::presenters::audit;
use crate::ui::state::AppState;

/// Show the audit viewer.
pub(crate) fn body(ui: &mut Ui, state: &mut AppState) {
    ui.horizontal(|ui| {
        ui.push_id("audit_search", |ui| {
            ui.add(
                TextEdit::singleline(&mut state.audit.search)
                    .hint_text("Search entry")
                    .desired_width(200.0),
            )
            .accessible_name("Search entry");
        });
        ui.push_id("action_choice", |ui| {
            let current = state.audit.action.clone();
            let label = label_for(&current);
            let mut selected = current.clone();
            ComboBox::from_id_salt("action_choice_combo")
                .selected_text(label)
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut selected,
                        audit::ALL_ACTIONS.to_owned(),
                        audit::ALL_ACTIONS,
                    );
                    for (label, action) in audit::ACTION_CHOICES {
                        ui.selectable_value(&mut selected, (*action).to_owned(), *label);
                    }
                })
                .response
                .accessible_name("Action filter");
            if selected != current {
                state.audit.action = selected;
            }
        });
    });
    let rows = state.filtered_audit();
    ui.push_id("audit_list", |ui| {
        egui::ScrollArea::vertical()
            .id_salt("audit_list_scroll")
            .max_height(360.0)
            .show(ui, |ui| {
                egui::Grid::new("audit_grid")
                    .num_columns(4)
                    .striped(true)
                    .show(ui, |ui| {
                        for heading in ["When", "Action", "Entry", "Reason"] {
                            text(ui, heading);
                        }
                        ui.end_row();
                        for row in &rows {
                            text(ui, &row.at);
                            text(ui, &row.action);
                            text(ui, &row.entry);
                            text(ui, &row.reason);
                            ui.end_row();
                        }
                    });
            });
    });
}

/// Return the label that the action choice shows for *action*.
fn label_for(action: &str) -> &str {
    if action == audit::ALL_ACTIONS {
        return audit::ALL_ACTIONS;
    }
    audit::ACTION_CHOICES
        .iter()
        .find(|(_, stored)| *stored == action)
        .map_or(action, |(label, _)| label)
}
