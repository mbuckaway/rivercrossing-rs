// SPDX-License-Identifier: GPL-3.0-only

//! The simulator form and its progress window.

use craft::egui::{self, ComboBox, DragValue, Ui};

use super::text;
use crate::ui::state::AppState;
use crate::ui::state::exports::{count_label, count_value};

/// The highest count a behaviour choice offers.
const MAX_CHOICE: i64 = 9;

/// Show the simulator form.
pub(crate) fn form_body(ui: &mut Ui, state: &mut AppState) {
    for (field, label, low, high) in [
        ("riders_spin", "Number of riders", 1, 2_000),
        ("teams_spin", "Number of teams", 0, 200),
        ("solo_spin", "Solo riders", 0, 2_000),
        ("laps_spin", "Laps", 1, 20),
        ("interval_spin", "Interval (minutes)", 1, 240),
    ] {
        spin(ui, state, field, label, low, high);
    }
    for (field, label) in [
        ("short_lap_choice", "Short-lap riders"),
        ("lapped_choice", "Lapped riders"),
        ("team_stop_choice", "Team riders who stop"),
    ] {
        choices(ui, state, field, label);
    }
    if let Some(message) = state.sim_message.clone() {
        ui.push_id("sim_infobar", |ui| text(ui, &message));
    }
}

/// Show one spinner bound to a dialog field.
fn spin(ui: &mut Ui, state: &mut AppState, id: &str, label: &str, low: i64, high: i64) {
    let mut value = state
        .dialogs
        .field(id)
        .map_or(0, |stored| stored.trim().parse().unwrap_or(0));
    ui.push_id(id, |ui| {
        ui.horizontal(|ui| {
            ui.add(egui::Label::new(label).selectable(false))
                .accessible_name(label);
            ui.add(DragValue::new(&mut value).range(low..=high).speed(1.0))
                .accessible_name(label);
        });
    });
    let text = value.to_string();
    if state.dialogs.field(id) != Some(text.as_str()) {
        let _ = state.dialogs.set_field(id, text);
    }
}

/// Show one count choice bound to a dialog field.
fn choices(ui: &mut Ui, state: &mut AppState, id: &str, label: &str) {
    let stored = state.dialogs.field(id).unwrap_or("0").to_owned();
    let selected = count_value(&stored);
    let mut picked = selected;
    ui.push_id(id, |ui| {
        ui.horizontal(|ui| {
            ui.add(egui::Label::new(label).selectable(false))
                .accessible_name(label);
            ComboBox::from_id_salt(format!("{id}_combo"))
                .selected_text(count_label(selected))
                .show_ui(ui, |ui| {
                    for value in 0..=MAX_CHOICE {
                        ui.selectable_value(&mut picked, value, count_label(value));
                    }
                })
                .response
                .accessible_name(label);
        });
    });
    if picked != selected {
        let _ = state.dialogs.set_field(id, count_label(picked));
    }
}

/// Show the simulation progress window.
pub(crate) fn running_body(ui: &mut Ui, state: &AppState) {
    ui.push_id("sim_status_lbl", |ui| text(ui, &state.sim_status));
    ui.push_id("progress_gauge", |ui| {
        ui.add(egui::ProgressBar::new(state.sim_progress))
            .accessible_name("Simulation progress");
    });
}
