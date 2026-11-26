// SPDX-License-Identifier: GPL-3.0-only

//! The rider editor and the Add/Edit rider form.
//!
//! The editor shows the roster's rows, a search and a read-only record panel; its Add, Edit and
//! Delete buttons raise the Add/Edit form and the delete confirmation. The form is the one window
//! in two modes: Add prefills the next free plate, Edit preloads the selected record, and its Save
//! commits through the presenter before the dialog closes.

use craft::egui::{self, ComboBox, TextEdit, Ui};

use super::dialogs::stack_text_field;
use super::text;
use crate::ui::state::AppState;

/// Show the rider editor's list, record panel and action buttons.
pub(crate) fn editor_body(ui: &mut Ui, state: &mut AppState) {
    ui.push_id("rider_search", |ui| {
        ui.horizontal(|ui| {
            text(ui, "Search");
            ui.add(
                TextEdit::singleline(&mut state.rider_editor.search)
                    .hint_text("Search riders")
                    .desired_width(240.0),
            )
            .accessible_name("Search riders");
        });
    });
    let visible = state.rider_editor.visible();
    let mut clicked = None;
    ui.push_id("riders_list", |ui| {
        egui::ScrollArea::vertical()
            .id_salt("riders_list_scroll")
            .max_height(240.0)
            .show(ui, |ui| {
                egui::Grid::new("riders_grid")
                    .num_columns(3)
                    .striped(true)
                    .show(ui, |ui| {
                        for heading in ["Plate", "Name", "Team"] {
                            text(ui, heading);
                        }
                        ui.end_row();
                        for index in visible {
                            let Some(row) = state.rider_editor.rows.get(index) else {
                                continue;
                            };
                            let selected =
                                state
                                    .rider_editor
                                    .selected
                                    .as_ref()
                                    .is_some_and(|selection| {
                                        selection.entry == row.entry
                                            && selection.rider_index == row.rider_index
                                    });
                            for cell in [&row.row.plate, &row.row.name, &row.row.team] {
                                if super::cell(ui, selected, cell) {
                                    clicked = Some(index);
                                }
                            }
                            ui.end_row();
                        }
                    });
            });
    });
    if let Some(index) = clicked
        && let Some(row) = state.rider_editor.rows.get(index)
    {
        state.rider_editor.selected = Some(crate::ui::state::RiderSelection {
            entry: row.entry.clone(),
            rider_index: row.rider_index,
        });
    }
    record_panel(ui, state);
    ui.horizontal(|ui| {
        if ui
            .push_id("add_btn", |ui| ui.button("Add rider…"))
            .inner
            .clicked()
        {
            state.open_add_rider();
        }
        let selected = state.rider_editor.selected.is_some();
        if ui
            .push_id("edit_btn", |ui| {
                ui.add_enabled(selected, egui::Button::new("Edit Rider…"))
            })
            .inner
            .clicked()
        {
            state.open_edit_rider();
        }
        if ui
            .push_id("delete_btn", |ui| {
                ui.add_enabled(selected, egui::Button::new("Delete"))
            })
            .inner
            .clicked()
        {
            let _ = state.raise_delete_rider();
        }
    });
    if let Some(errors) = &state.rider_editor.errors {
        ui.push_id("roster_infobar", |ui| text(ui, errors));
    }
}

/// Show the selected record as its read-only fields.
fn record_panel(ui: &mut Ui, state: &mut AppState) {
    let record = state
        .rider_editor
        .selected
        .as_ref()
        .and_then(|selection| state.rider_record(selection));
    // Mirror the record into the frozen field map so `ui.inspect` reads what the panel displays.
    for (id, value) in [
        (
            "plate_input",
            record.as_ref().map_or("", |r| r.plate.as_str()),
        ),
        (
            "first_name_input",
            record.as_ref().map_or("", |r| r.first.as_str()),
        ),
        (
            "last_name_input",
            record.as_ref().map_or("", |r| r.last.as_str()),
        ),
        (
            "team_choice",
            record.as_ref().map_or("", |r| r.team.as_str()),
        ),
    ] {
        let _ = state.dialogs.set_field(id, value);
    }
    ui.horizontal(|ui| {
        record_value(
            ui,
            "plate_input",
            "Plate",
            record.as_ref().map(|r| r.plate.as_str()),
        );
        record_value(
            ui,
            "first_name_input",
            "First name",
            record.as_ref().map(|r| r.first.as_str()),
        );
        record_value(
            ui,
            "last_name_input",
            "Last name",
            record.as_ref().map(|r| r.last.as_str()),
        );
        record_value(
            ui,
            "team_choice",
            "Team",
            record.as_ref().map(|r| r.team.as_str()),
        );
    });
}

/// Show one read-only record value under its caption.
fn record_value(ui: &mut Ui, id: &str, caption: &str, value: Option<&str>) {
    ui.push_id(id, |ui| {
        ui.vertical(|ui| {
            text(ui, caption);
            text(ui, value.unwrap_or(""));
        });
    });
}

/// Show the Add/Edit rider form.
pub(crate) fn form_body(ui: &mut Ui, state: &mut AppState, focused: Option<&str>) {
    stack_text_field(ui, state, "plate_input", "Plate", focused);
    stack_text_field(ui, state, "first_name_input", "First name", focused);
    stack_text_field(ui, state, "last_name_input", "Last name", focused);
    team_choice(ui, state);
    sex_choice(ui, state);
    if let Some(errors) = &state.rider_editor.errors {
        ui.push_id("add_rider_infobar", |ui| text(ui, errors));
    }
}

/// Show the team choice: `solo` or one live team by name.
fn team_choice(ui: &mut Ui, state: &mut AppState) {
    let current = state
        .dialogs
        .field("team_choice")
        .unwrap_or("solo")
        .to_owned();
    let mut selected = current.clone();
    let mut choices = vec!["solo".to_owned()];
    if let Some(ride) = state.ride.as_ref() {
        choices.extend(
            ride.roster
                .entries()
                .iter()
                .filter(|entry| entry.kind == rivercrossing_roster::enums::EntryType::Team)
                .map(|entry| entry.display_name.clone()),
        );
    }
    ui.push_id("team_choice", |ui| {
        ui.horizontal(|ui| {
            text(ui, "Team");
            ComboBox::from_id_salt("team_choice_combo")
                .selected_text(&selected)
                .show_ui(ui, |ui| {
                    for choice in &choices {
                        ui.selectable_value(&mut selected, choice.clone(), choice.clone());
                    }
                })
                .response
                .accessible_name("Team");
        });
    });
    if selected != current {
        let _ = state.dialogs.set_field("team_choice", selected);
    }
}

/// Show the sex choice: blank, `M` or `F`.
fn sex_choice(ui: &mut Ui, state: &mut AppState) {
    let current = state.dialogs.field("sex_choice").unwrap_or("").to_owned();
    let mut selected = current.clone();
    ui.push_id("sex_choice", |ui| {
        ui.horizontal(|ui| {
            text(ui, "Sex");
            ComboBox::from_id_salt("sex_choice_combo")
                .selected_text(if selected.is_empty() {
                    "—"
                } else {
                    selected.as_str()
                })
                .show_ui(ui, |ui| {
                    for choice in ["", "M", "F"] {
                        ui.selectable_value(&mut selected, choice.to_owned(), choice);
                    }
                })
                .response
                .accessible_name("Sex");
        });
    });
    if selected != current {
        let _ = state.dialogs.set_field("sex_choice", selected);
    }
}
