// SPDX-License-Identifier: GPL-3.0-only

//! The rider editor and the Add/Edit rider form.
//!
//! The editor is the design's two-pane split: the roster and its search on the left, a 250 pt
//! read-only record panel on the right, and the Add, Edit and Delete buttons at the panes' feet.
//! The form is the one window in two modes: Add prefills the next free plate, Edit preloads the
//! selected record, and its Save commits through the presenter before the dialog closes.

use craft::egui::{Align, Layout, Ui};
use craft::theme::Variant;
use craft::theme::variant::Tone;
use craft::ui::buttons::Button;
use craft::ui::contract::{Bind, Props as _, Show as _, Sizing};
use craft::ui::text::TextField;

use super::dialogs::{
    NAME, PLATE, SEX, TEAM, TableData, TableRow, columns, field_caption, notice, pair,
    select_owned, split, stack_select, stack_text_field, table, value,
};
use crate::ui::state::{AppState, RiderEditorRow, RiderSelection};

#[cfg(test)]
mod tests;

/// The rider rows' visible height before the table scrolls.
const VISIBLE_ROWS: u16 = 8;

/// The record pane's width (`design/mockups/index.html`, `.panel` `width:250px`).
const RECORD_PANE: f32 = 250.0;

/// The list-and-record area's least height (`design/mockups/index.html`, `.split`
/// `min-height:300px`), so the dialog cannot collapse to the fields' own height.
const LIST_MIN_HEIGHT: f32 = 300.0;

/// The team choice's `solo` item.
const SOLO: &str = "solo";

/// The sex choices: blank, `M` or `F`.
const SEX_CHOICES: [(&str, &str); 3] = [("—", ""), ("M", "M"), ("F", "F")];

/// The search field's placeholder (`design/mockups/index.html`, `#s-rider-editor`).
const SEARCH_HINT: &str = "Search plate, name or team";

/// The delete rule under the panes (`design/mockups/index.html`, `#s-rider-editor`).
const DELETE_NOTICE: &str = "Delete is available only for a DRAFT entry with no recorded data.";

/// The plate rule in the Add/Edit form (`design/mockups/index.html`, `#s-add-rider`).
const PLATE_NOTICE: &str = "Plate can be changed only while the ride is DRAFT.";

/// Show the rider editor's split, notices and action buttons.
pub(crate) fn editor_body(ui: &mut Ui, state: &mut AppState) {
    ui.push_id("rider_search", |ui| {
        let _ = TextField::search("Search", Bind::bound(&mut state.rider_editor.search))
            .placeholder(SEARCH_HINT)
            .sizing(Sizing::FillWidth)
            .show(ui);
    });
    let visible = state.rider_editor.visible();
    let rows = roster_rows(state, &visible);
    let selected = visible.iter().position(|index| selection_at(state, *index));
    split(ui, RECORD_PANE, LIST_MIN_HEIGHT, |list, record| {
        let picked = table(
            list,
            "riders_list",
            &rows,
            VISIBLE_ROWS,
            "No riders",
            selected,
        );
        if let Some(key) = picked
            && let Some(index) = visible.get(key)
            && let Some(row) = state.rider_editor.rows.get(*index)
        {
            state.rider_editor.selected = Some(RiderSelection {
                entry: row.entry.clone(),
                rider_index: row.rider_index,
            });
        }
        list_actions(list, state);
        record_panel(record, state);
        form_actions(record, state);
    });
    if let Some(errors) = &state.rider_editor.errors {
        notice(ui, "roster_infobar", Tone::Bad, errors.clone());
    }
    notice(ui, "rider_delete_note", Tone::Info, DELETE_NOTICE);
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
            // The label carries the rider's name into its unclaimed column.
            cells: vec![
                (PLATE, row.row.plate.clone()),
                (TEAM, row.row.team.clone()),
                (SEX, sex_cell(state, row)),
            ],
            ..TableRow::default()
        });
    }
    TableData {
        columns: columns(&[
            (PLATE, "Plate"),
            (NAME, "Name"),
            (TEAM, "Team"),
            (SEX, "Sex"),
        ]),
        rows,
    }
}

/// Return one row's Sex cell: the rider's recorded sex, or the blank choice's text.
fn sex_cell(state: &AppState, row: &RiderEditorRow) -> String {
    state
        .rider_record(&RiderSelection {
            entry: row.entry.clone(),
            rider_index: row.rider_index,
        })
        .and_then(|record| record.sex)
        .unwrap_or_default()
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

/// Show the selected record as its read-only fields, stacked as the design's `.panel .body`.
fn record_panel(ui: &mut Ui, state: &mut AppState) {
    let record = state
        .rider_editor
        .selected
        .as_ref()
        .and_then(|selection| state.rider_record(selection));
    // Mirror the record into the frozen field map so `ui.inspect` reads what the panel displays. A
    // click that opened the Add/Edit form in this same pass has made it the top dialog, so the
    // editor's record must not be written over that form's preloaded fields.
    let editing = state
        .dialogs
        .top()
        .is_some_and(|dialog| dialog.id == "rider_editor_dlg");
    if editing {
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
    }
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
}

/// Show the Add and Delete buttons at the list pane's foot (§15b: `add_btn`, `delete_btn`).
fn list_actions(ui: &mut Ui, state: &mut AppState) {
    let selected = state.rider_editor.selected.is_some();
    ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
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
    });
}

/// Show the Edit button at the form pane's foot (§15b: `edit_btn` acts on the record above it).
fn form_actions(ui: &mut Ui, state: &mut AppState) {
    let selected = state.rider_editor.selected.is_some();
    ui.with_layout(Layout::bottom_up(Align::Max), |ui| {
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
    });
}

/// Show the Add/Edit rider form.
pub(crate) fn form_body(ui: &mut Ui, state: &mut AppState) {
    pair(ui, |left, right| {
        stack_text_field(left, state, "plate_input", "Plate");
        stack_text_field(right, state, "first_name_input", "First name");
    });
    pair(ui, |left, right| {
        stack_text_field(left, state, "last_name_input", "Last name");
        team_choice(right, state);
    });
    pair(ui, |left, _| {
        stack_select(left, state, "sex_choice", "Sex", &SEX_CHOICES);
    });
    notice(ui, "add_rider_plate_note", Tone::Info, PLATE_NOTICE);
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
    field_caption(ui, "Team");
    if let Some(picked) = select_owned(ui, "team_choice", "Team", &stored, &choices) {
        let _ = state.dialogs.set_field("team_choice", picked);
    }
}
