// SPDX-License-Identifier: GPL-3.0-only

//! The teams editor and the Add/Edit team form.
//!
//! The editor lists the ride's teams, filters to single-member teams on request and shows the
//! selected team's read-only record beside Add, Edit and Remove. The form is the one window in two
//! modes; its Add/Save commits through the presenter, and Pick card draws a fresh seeded logo.

use craft::egui::{self, Ui};

use super::dialogs::stack_text_field;
use super::text;
use crate::ui::state::AppState;

/// Show the teams editor's list, record panel and buttons.
pub(crate) fn editor_body(ui: &mut Ui, state: &mut AppState) {
    let mut single = state.dialogs.field("single_member_only_chk") == Some("true");
    ui.push_id("single_member_only_chk", |ui| {
        if ui
            .checkbox(&mut single, "Show single-member teams only")
            .changed()
        {
            state.set_team_single_member_only(single);
        }
    });
    let mut clicked = None;
    ui.push_id("teams_list", |ui| {
        egui::ScrollArea::vertical()
            .id_salt("teams_list_scroll")
            .max_height(220.0)
            .show(ui, |ui| {
                egui::Grid::new("teams_grid")
                    .num_columns(3)
                    .striped(true)
                    .show(ui, |ui| {
                        for heading in ["Name", "Plate", "Members"] {
                            text(ui, heading);
                        }
                        ui.end_row();
                        for (index, row) in state.team_editor.rows.iter().enumerate() {
                            let selected = state.team_editor.selected.as_ref() == Some(&row.entry);
                            if super::cell(ui, selected, &row.row.name) {
                                clicked = Some(index);
                            }
                            super::cell(ui, selected, &row.row.plate);
                            text(ui, &row.row.members.to_string());
                            ui.end_row();
                        }
                    });
            });
    });
    if let Some(index) = clicked
        && let Some(row) = state.team_editor.rows.get(index)
    {
        state.team_editor.selected = Some(row.entry.clone());
    }
    record_panel(ui, state);
    ui.horizontal(|ui| {
        if ui
            .push_id("add_btn", |ui| ui.button("Add team…"))
            .inner
            .clicked()
        {
            state.open_add_team();
        }
        let selected = state.team_editor.selected.is_some();
        if ui
            .push_id("edit_btn", |ui| {
                ui.add_enabled(selected, egui::Button::new("Edit Team…"))
            })
            .inner
            .clicked()
        {
            state.open_edit_team();
        }
        if ui
            .push_id("remove_btn", |ui| {
                ui.add_enabled(selected, egui::Button::new("Remove"))
            })
            .inner
            .clicked()
        {
            let _ = state.raise_remove_team();
        }
    });
    if let Some(errors) = &state.team_editor.errors {
        ui.push_id("teams_infobar", |ui| text(ui, errors));
    }
}

/// Show the selected team's read-only record.
fn record_panel(ui: &mut Ui, state: &mut AppState) {
    let record = state
        .team_editor
        .selected
        .as_ref()
        .and_then(|entry| state.team_record(entry));
    // Mirror the record into the frozen field map so `ui.inspect` reads what the panel displays.
    for (id, value) in [
        (
            "name_input",
            record.as_ref().map_or("", |r| r.name.as_str()),
        ),
        (
            "relay_plate_input",
            record.as_ref().map_or("", |r| r.plate.as_str()),
        ),
        (
            "notes_input",
            record.as_ref().map_or("", |r| r.notes.as_str()),
        ),
    ] {
        let _ = state.dialogs.set_field(id, value);
    }
    ui.horizontal(|ui| {
        record_value(
            ui,
            "name_input",
            "Name",
            record.as_ref().map(|r| r.name.as_str()),
        );
        record_value(
            ui,
            "relay_plate_input",
            "Relay plate",
            record.as_ref().map(|r| r.plate.as_str()),
        );
        record_value(
            ui,
            "notes_input",
            "Notes",
            record.as_ref().map(|r| r.notes.as_str()),
        );
    });
    let members = state
        .team_editor
        .selected
        .as_ref()
        .and_then(|entry| state.team_members(entry))
        .unwrap_or_default();
    ui.push_id("members_list", |ui| {
        ui.vertical(|ui| {
            text(ui, "Members");
            for member in &members {
                text(ui, member);
            }
        });
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

/// Show the Add/Edit team form.
pub(crate) fn form_body(ui: &mut Ui, state: &mut AppState, focused: Option<&str>) {
    stack_text_field(ui, state, "name_input", "Team name", focused);
    if state.ride.as_ref().is_some_and(|ride| {
        ride.roster.plate_model() == rivercrossing_roster::enums::PlateModel::TeamRelay
    }) {
        stack_text_field(ui, state, "relay_plate_input", "Relay plate", focused);
    }
    stack_text_field(ui, state, "notes_input", "Notes", focused);
    ui.horizontal(|ui| {
        if ui
            .push_id("pick_card_btn", |ui| ui.button("Pick card…"))
            .inner
            .clicked()
        {
            let _ = state.pick_team_card();
        }
        let preview = state
            .team_editor
            .selected
            .as_ref()
            .or(state.team_editor.editing.as_ref())
            .and_then(|entry| state.team_logo(entry))
            .unwrap_or_else(|| "NO LOGO".to_owned());
        ui.push_id("logo_bmp", |ui| text(ui, &preview));
    });
    if let Some(errors) = &state.team_editor.errors {
        ui.push_id("add_team_infobar", |ui| text(ui, errors));
    }
}
