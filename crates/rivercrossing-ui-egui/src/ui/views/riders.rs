// SPDX-License-Identifier: GPL-3.0-only

//! The rider editor and the Add/Edit rider form.
//!
//! The editor shows the roster's rows, a search and a read-only record panel; its Add, Edit and
//! Delete buttons raise the Add/Edit form and the delete confirmation. The form is the one window
//! in two modes: Add prefills the next free plate, Edit preloads the selected record, and its Save
//! commits through the presenter before the dialog closes.

use craft::egui::Ui;
use craft::theme::Variant;
use craft::theme::variant::Tone;
use craft::ui::buttons::Button;
use craft::ui::contract::{Bind, Props as _, Show as _};
use craft::ui::text::TextField;

use super::dialogs::{
    NAME, PLATE, TEAM, TableData, TableRow, columns, notice, select_owned, stack_select,
    stack_text_field, table, value,
};
use crate::ui::state::AppState;
use crate::ui::state::RiderSelection;

/// The rider rows' visible height before the table scrolls.
const VISIBLE_ROWS: u16 = 8;

/// The team choice's `solo` item.
const SOLO: &str = "solo";

/// The sex choices: blank, `M` or `F`.
const SEX_CHOICES: [(&str, &str); 3] = [("—", ""), ("M", "M"), ("F", "F")];

/// Show the rider editor's list, record panel and action buttons.
pub(crate) fn editor_body(ui: &mut Ui, state: &mut AppState) {
    ui.push_id("rider_search", |ui| {
        let _ = TextField::search("Search riders", Bind::bound(&mut state.rider_editor.search))
            .show(ui);
    });
    let visible = state.rider_editor.visible();
    let rows = roster_rows(state, &visible);
    let selected = visible.iter().position(|index| selection_at(state, *index));
    if let Some(key) = table(
        ui,
        "riders_list",
        &rows,
        VISIBLE_ROWS,
        "No riders",
        selected,
    ) && let Some(index) = visible.get(key)
        && let Some(row) = state.rider_editor.rows.get(*index)
    {
        state.rider_editor.selected = Some(RiderSelection {
            entry: row.entry.clone(),
            rider_index: row.rider_index,
        });
    }
    record_panel(ui, state);
    buttons(ui, state);
    if let Some(errors) = &state.rider_editor.errors {
        notice(ui, "roster_infobar", Tone::Bad, errors.clone());
    }
}

/// Return the visible roster rows as a table.
fn roster_rows(state: &AppState, visible: &[usize]) -> TableData {
    let mut rows = Vec::with_capacity(visible.len());
    for index in visible {
        let Some(row) = state.rider_editor.rows.get(*index) else {
            continue;
        };
        rows.push(TableRow {
            label: row.row.name.clone(),
            cells: vec![
                (PLATE, row.row.plate.clone()),
                (NAME, row.row.name.clone()),
                (TEAM, row.row.team.clone()),
            ],
        });
    }
    TableData {
        columns: columns(&[(PLATE, "Plate"), (NAME, "Name"), (TEAM, "Team")]),
        rows,
    }
}

/// Return whether the row at *index* is the selected record.
fn selection_at(state: &AppState, index: usize) -> bool {
    let Some(selection) = state.rider_editor.selected.as_ref() else {
        return false;
    };
    state
        .rider_editor
        .rows
        .get(index)
        .is_some_and(|row| selection.entry == row.entry && selection.rider_index == row.rider_index)
}

/// Show the selected record as its read-only fields.
fn record_panel(ui: &mut Ui, state: &mut AppState) {
    let record = state
        .rider_editor
        .selected
        .as_ref()
        .and_then(|selection| state.rider_record(selection));
    // Mirror the record into the frozen field map so `ui.inspect` reads what the panel displays.
    for (id, text) in [
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
        let _ = state.dialogs.set_field(id, text);
    }
    ui.horizontal(|ui| {
        value(
            ui,
            "plate_input",
            "Plate",
            record.as_ref().map_or("", |r| r.plate.as_str()),
        );
        value(
            ui,
            "first_name_input",
            "First name",
            record.as_ref().map_or("", |r| r.first.as_str()),
        );
        value(
            ui,
            "last_name_input",
            "Last name",
            record.as_ref().map_or("", |r| r.last.as_str()),
        );
        value(
            ui,
            "team_choice",
            "Team",
            record.as_ref().map_or("", |r| r.team.as_str()),
        );
    });
}

/// Show the Add, Edit and Delete buttons.
fn buttons(ui: &mut Ui, state: &mut AppState) {
    let selected = state.rider_editor.selected.is_some();
    ui.horizontal(|ui| {
        ui.push_id("add_btn", |ui| {
            if Button::new("Add rider…")
                .variant(Variant::PRIMARY)
                .show(ui)
                .clicked()
            {
                state.open_add_rider();
            }
        });
        ui.push_id("edit_btn", |ui| {
            let clicked = Button::new("Edit Rider…")
                .variant(Variant::SECONDARY)
                .enabled(selected)
                .show(ui)
                .clicked();
            if clicked {
                state.open_edit_rider();
            }
        });
        ui.push_id("delete_btn", |ui| {
            let clicked = Button::new("Delete")
                .variant(Variant::DANGER)
                .enabled(selected)
                .show(ui)
                .clicked();
            if clicked {
                let _ = state.raise_delete_rider();
            }
        });
    });
}

/// Show the Add/Edit rider form.
pub(crate) fn form_body(ui: &mut Ui, state: &mut AppState) {
    stack_text_field(ui, state, "plate_input", "Plate");
    stack_text_field(ui, state, "first_name_input", "First name");
    stack_text_field(ui, state, "last_name_input", "Last name");
    team_choice(ui, state);
    stack_select(ui, state, "sex_choice", "Sex", &SEX_CHOICES);
    if let Some(errors) = &state.rider_editor.errors {
        notice(ui, "add_rider_infobar", Tone::Bad, errors.clone());
    }
}

/// Show the team choice: `solo` or one live team by name.
fn team_choice(ui: &mut Ui, state: &mut AppState) {
    let mut choices: Vec<(String, String)> = vec![(SOLO.to_owned(), SOLO.to_owned())];
    if let Some(ride) = state.ride.as_ref() {
        for entry in ride.roster.entries() {
            if entry.kind == rivercrossing_roster::enums::EntryType::Team {
                choices.push((entry.display_name.clone(), entry.display_name.clone()));
            }
        }
    }
    let stored = state
        .dialogs
        .field("team_choice")
        .unwrap_or(SOLO)
        .to_owned();
    if let Some(picked) = select_owned(ui, "team_choice", "Team", &stored, &choices) {
        let _ = state.dialogs.set_field("team_choice", picked);
    }
}
