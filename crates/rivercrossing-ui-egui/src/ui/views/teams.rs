// SPDX-License-Identifier: GPL-3.0-only

//! The teams editor and the Add/Edit team form.
//!
//! The editor is the design's two-pane split: the filter and the team list on the left, a 250 pt
//! read-only record panel and the members on the right. The form is the one window in two modes;
//! its Add/Save commits through the presenter, and Browse draws a fresh seeded logo card.

use craft::egui::{Align, Layout, Ui};
use craft::theme::TypeRole;
use craft::theme::Variant;
use craft::theme::variant::Tone;
use craft::ui::buttons::Button;
use craft::ui::contract::{Props as _, Show as _};
use craft::ui::display::{Badge, Shade, Text};

use super::dialogs::{
    COUNT, NAME, TableData, TableRow, columns, field_caption, notice, split, stack_flag,
    stack_text_field, stack_text_field_lines, table, value,
};
use crate::ui::state::AppState;

#[cfg(test)]
mod tests;

/// The team rows' visible height before the table scrolls.
const VISIBLE_ROWS: u16 = 6;

/// The member rows' visible height before the table scrolls.
const MEMBER_ROWS: u16 = 4;

/// The record pane's width (`design/mockups/index.html`, `.panel` `width:250px`).
const RECORD_PANE: f32 = 250.0;

/// The list-and-record area's least height (`design/mockups/index.html`, `.split`
/// `min-height:260px`), so the dialog cannot collapse to the fields' own height.
const LIST_MIN_HEIGHT: f32 = 260.0;

/// The relay plate's help line (`design/mockups/index.html`, `#s-add-team` `.help`).
const RELAY_HELP: &str = "Relay rides only.";

/// The logo card's title and the words it shows with no card picked.
const LOGO_CARD: &str = "Logo card";
const NO_CARD: &str = "No card";
const NO_PREVIEW: &str = "—";

/// The logo row's button label, as the setup dialog's own Browse reads.
const BROWSE_LABEL: &str = "Browse…";

/// The notes field's rows (`design/mockups/index.html`, `#s-add-team` `textarea rows="2"`).
const NOTES_LINES: usize = 2;

/// Show the teams editor's actions row, two-pane split and refusal.
pub(crate) fn editor_body(ui: &mut Ui, state: &mut AppState) {
    actions_row(ui, state);
    let rows = team_rows(state);
    let selected = state.team_editor.selected.as_ref().and_then(|entry| {
        state
            .team_editor
            .rows
            .iter()
            .position(|row| &row.entry == entry)
    });
    split(ui, RECORD_PANE, LIST_MIN_HEIGHT, |list, record| {
        if let Some(key) = table(
            list,
            "teams_list",
            &rows,
            VISIBLE_ROWS,
            "No teams",
            selected,
        ) && let Some(row) = state.team_editor.rows.get(key)
        {
            state.team_editor.selected = Some(row.entry.clone());
        }
        record_panel(record, state);
    });
    if let Some(errors) = &state.team_editor.errors {
        notice(ui, "teams_infobar", Tone::Bad, errors.clone());
    }
}

/// Return the team rows as a table: the mockup's Team | Riders pair (§15b: the Riders cell is the
/// team's rider count).
fn team_rows(state: &AppState) -> TableData {
    let rows = state
        .team_editor
        .rows
        .iter()
        .map(|row| {
            let count = row.row.members.to_string();
            TableRow {
                label: row.row.name.clone(),
                cells: vec![(NAME, row.row.name.clone()), (COUNT, count)],
                ..TableRow::default()
            }
        })
        .collect();
    TableData {
        columns: columns(&[(NAME, "Team"), (COUNT, "Riders")]),
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
    // Mirror the record into the frozen field map so `ui.inspect` reads what the panel displays. A
    // click that opened the Add/Edit form in this same pass has made it the top dialog, so the
    // editor's record must not be written over that form's fields.
    let editing = state
        .dialogs
        .top()
        .is_some_and(|dialog| dialog.id == "team_editor_dlg");
    if editing {
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
    }
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
            ..TableRow::default()
        })
        .collect();
    let data = TableData {
        columns: columns(&[(NAME, "Members")]),
        rows,
    };
    let _ = table(ui, "members_list", &data, MEMBER_ROWS, "No members", None);
}

/// Show the filter and the Add, Edit and Remove buttons, all on one row above the list (§15b).
fn actions_row(ui: &mut Ui, state: &mut AppState) {
    let selected = state.team_editor.selected.is_some();
    ui.horizontal(|ui| {
        if let Some(on) = stack_flag(
            ui,
            state,
            "single_member_only_chk",
            "Single-member teams only",
        ) {
            state.set_team_single_member_only(on);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
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
            ui.push_id("add_btn", |ui| {
                if Button::new("Add team…")
                    .variant(Variant::PRIMARY)
                    .show(ui)
                    .clicked()
                {
                    state.open_add_team();
                }
            });
        });
    });
}

/// Show the Add/Edit team form.
pub(crate) fn form_body(ui: &mut Ui, state: &mut AppState) {
    stack_text_field(ui, state, "name_input", "Team name");
    if relay_ride(state) {
        stack_text_field(ui, state, "relay_plate_input", "Relay plate");
        let _ = Text::new(RELAY_HELP)
            .role(TypeRole::Caption)
            .shade(Shade::Faint)
            .show(ui);
    }
    stack_text_field_lines(ui, state, "notes_input", "Notes", NOTES_LINES);
    logo_row(ui, state);
    if let Some(errors) = &state.team_editor.errors {
        notice(ui, "add_team_infobar", Tone::Bad, errors.clone());
    }
}

/// Return whether the ride gives a team an explicit relay plate (§15b: the row hides on a
/// rider-pooled ride, whose plate derives from the members).
fn relay_ride(state: &AppState) -> bool {
    state.ride.as_ref().is_some_and(|ride| {
        ride.roster.plate_model() == rivercrossing_roster::enums::PlateModel::TeamRelay
    })
}

/// Show the team's logo card row: the preview, the card's chip and Browse.
///
/// The mockup's "Logo card" row (`design/mockups/index.html`, `#s-add-team`); the frozen ids are
/// `logo_bmp` and `pick_card_btn`.
fn logo_row(ui: &mut Ui, state: &mut AppState) {
    let card = state
        .team_editor
        .selected
        .as_ref()
        .or(state.team_editor.editing.as_ref())
        .and_then(|entry| state.team_logo(entry));
    field_caption(ui, LOGO_CARD);
    ui.horizontal(|ui| {
        let gap = craft::ui::runtime::env::Env::of(ui)
            .tokens
            .space
            .pad
            .regular;
        ui.push_id("logo_bmp", |ui| {
            let _ = Text::new(card.clone().unwrap_or_else(|| NO_PREVIEW.to_owned())).show(ui);
        });
        ui.add_space(gap);
        let _ = Badge::new(card.unwrap_or_else(|| NO_CARD.to_owned()))
            .variant(Variant::SECONDARY)
            .show(ui);
        ui.add_space(gap);
        ui.push_id("pick_card_btn", |ui| {
            if Button::new(BROWSE_LABEL)
                .variant(Variant::SECONDARY)
                .show(ui)
                .clicked()
            {
                let _ = state.pick_team_card();
            }
        });
    });
}
