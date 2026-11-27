// SPDX-License-Identifier: GPL-3.0-only

//! The teams editor and the Add/Edit team form.
//!
//! The editor lists the ride's teams, filters to single-member teams on request and shows the
//! selected team's read-only record beside Add, Edit and Remove. The form is the one window in two
//! modes; its Add/Save commits through the presenter, and Pick card draws a fresh seeded logo.

use craft::egui::Ui;
use craft::theme::Variant;
use craft::theme::variant::Tone;
use craft::ui::buttons::Button;
use craft::ui::contract::{Props as _, Show as _};
use craft::ui::display::Text;

use super::dialogs::{
    COUNT, NAME, PLATE, TableData, TableRow, columns, notice, stack_flag, stack_text_field, table,
    value,
};
use crate::ui::state::AppState;

/// The team rows' visible height before the table scrolls.
const VISIBLE_ROWS: u16 = 6;

/// The member rows' visible height before the table scrolls.
const MEMBER_ROWS: u16 = 4;

/// Show the teams editor's list, record panel and buttons.
pub(crate) fn editor_body(ui: &mut Ui, state: &mut AppState) {
    if let Some(on) = stack_flag(
        ui,
        state,
        "single_member_only_chk",
        "Show single-member teams only",
    ) {
        state.set_team_single_member_only(on);
    }
    let rows = team_rows(state);
    let selected = state.team_editor.selected.as_ref().and_then(|entry| {
        state
            .team_editor
            .rows
            .iter()
            .position(|row| &row.entry == entry)
    });
    if let Some(key) = table(ui, "teams_list", &rows, VISIBLE_ROWS, "No teams", selected)
        && let Some(row) = state.team_editor.rows.get(key)
    {
        state.team_editor.selected = Some(row.entry.clone());
    }
    record_panel(ui, state);
    buttons(ui, state);
    if let Some(errors) = &state.team_editor.errors {
        notice(ui, "teams_infobar", Tone::Bad, errors.clone());
    }
}

/// Return the team rows as a table.
fn team_rows(state: &AppState) -> TableData {
    let rows = state
        .team_editor
        .rows
        .iter()
        .map(|row| TableRow {
            label: row.row.name.clone(),
            cells: vec![
                (NAME, row.row.name.clone()),
                (PLATE, row.row.plate.clone()),
                (COUNT, row.row.members.to_string()),
            ],
        })
        .collect();
    TableData {
        columns: columns(&[(NAME, "Team"), (PLATE, "Plate"), (COUNT, "Members")]),
        rows,
    }
}

/// Show the selected team's read-only record and its members.
fn record_panel(ui: &mut Ui, state: &mut AppState) {
    let record = state
        .team_editor
        .selected
        .as_ref()
        .and_then(|entry| state.team_record(entry));
    // Mirror the record into the frozen field map so `ui.inspect` reads what the panel displays.
    for (id, text) in [
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
        let _ = state.dialogs.set_field(id, text);
    }
    ui.horizontal(|ui| {
        value(
            ui,
            "name_input",
            "Name",
            record.as_ref().map_or("", |r| r.name.as_str()),
        );
        value(
            ui,
            "relay_plate_input",
            "Relay plate",
            record.as_ref().map_or("", |r| r.plate.as_str()),
        );
        value(
            ui,
            "notes_input",
            "Notes",
            record.as_ref().map_or("", |r| r.notes.as_str()),
        );
    });
    let members = state
        .team_editor
        .selected
        .as_ref()
        .and_then(|entry| state.team_members(entry))
        .unwrap_or_default();
    let rows = members
        .iter()
        .map(|member| TableRow {
            label: member.clone(),
            cells: vec![(NAME, member.clone())],
        })
        .collect();
    let data = TableData {
        columns: columns(&[(NAME, "Members")]),
        rows,
    };
    let _ = table(ui, "members_list", &data, MEMBER_ROWS, "No members", None);
}

/// Show the Add, Edit and Remove buttons.
fn buttons(ui: &mut Ui, state: &mut AppState) {
    let selected = state.team_editor.selected.is_some();
    ui.horizontal(|ui| {
        ui.push_id("add_btn", |ui| {
            if Button::new("Add team…")
                .variant(Variant::PRIMARY)
                .show(ui)
                .clicked()
            {
                state.open_add_team();
            }
        });
        ui.push_id("edit_btn", |ui| {
            let clicked = Button::new("Edit Team…")
                .variant(Variant::SECONDARY)
                .enabled(selected)
                .show(ui)
                .clicked();
            if clicked {
                state.open_edit_team();
            }
        });
        ui.push_id("remove_btn", |ui| {
            let clicked = Button::new("Remove")
                .variant(Variant::DANGER)
                .enabled(selected)
                .show(ui)
                .clicked();
            if clicked {
                let _ = state.raise_remove_team();
            }
        });
    });
}

/// Show the Add/Edit team form.
pub(crate) fn form_body(ui: &mut Ui, state: &mut AppState) {
    stack_text_field(ui, state, "name_input", "Team name");
    if state.ride.as_ref().is_some_and(|ride| {
        ride.roster.plate_model() == rivercrossing_roster::enums::PlateModel::TeamRelay
    }) {
        stack_text_field(ui, state, "relay_plate_input", "Relay plate");
    }
    stack_text_field(ui, state, "notes_input", "Notes");
    ui.horizontal(|ui| {
        ui.push_id("pick_card_btn", |ui| {
            if Button::new("Pick card…")
                .variant(Variant::SECONDARY)
                .show(ui)
                .clicked()
            {
                let _ = state.pick_team_card();
            }
        });
        let preview = state
            .team_editor
            .selected
            .as_ref()
            .or(state.team_editor.editing.as_ref())
            .and_then(|entry| state.team_logo(entry))
            .unwrap_or_else(|| "NO LOGO".to_owned());
        ui.push_id("logo_bmp", |ui| {
            let _ = Text::new(preview).show(ui);
        });
    });
    if let Some(errors) = &state.team_editor.errors {
        notice(ui, "add_team_infobar", Tone::Bad, errors.clone());
    }
}
