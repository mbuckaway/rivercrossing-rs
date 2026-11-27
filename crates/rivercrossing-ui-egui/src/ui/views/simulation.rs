// SPDX-License-Identifier: GPL-3.0-only

//! The simulator form and its progress window.

use craft::egui::Ui;
use craft::theme::variant::Tone;
use craft::ui::contract::{Props as _, Show as _};
use craft::ui::display::{Progress, Text};

use super::dialogs::{notice, select_owned, stack_whole_field};
use crate::ui::state::AppState;
use crate::ui::state::exports::{count_label, count_value};

/// The highest count a behaviour choice offers.
const MAX_CHOICE: i64 = 9;

/// Show the simulator form.
pub(crate) fn form_body(ui: &mut Ui, state: &mut AppState) {
    for (field, label, low, high) in [
        ("riders_spin", "Number of riders", 1.0, 2_000.0),
        ("teams_spin", "Number of teams", 0.0, 200.0),
        ("solo_spin", "Solo riders", 0.0, 2_000.0),
        ("laps_spin", "Laps", 1.0, 20.0),
        ("interval_spin", "Interval (minutes)", 1.0, 240.0),
    ] {
        stack_whole_field(ui, state, field, label, low, low..=high);
    }
    for (field, label) in [
        ("short_lap_choice", "Short-lap riders"),
        ("lapped_choice", "Lapped riders"),
        ("team_stop_choice", "Team riders who stop"),
    ] {
        choices(ui, state, field, label);
    }
    if let Some(message) = state.sim_message.clone() {
        notice(ui, "sim_infobar", Tone::Bad, message);
    }
}

/// Show one count choice bound to a dialog field.
fn choices(ui: &mut Ui, state: &mut AppState, id: &'static str, label: &'static str) {
    let stored = state.dialogs.field(id).unwrap_or("0").to_owned();
    let selected = count_label(count_value(&stored));
    let options: Vec<(String, String)> = (0..=MAX_CHOICE)
        .map(|value| (count_label(value), count_label(value)))
        .collect();
    crate::ui::views::dialogs::field_caption(ui, label);
    if let Some(picked) = select_owned(ui, id, label, &selected, &options) {
        let _ = state.dialogs.set_field(id, picked);
    }
}

/// Show the simulation progress window: a determinate bar and its status line.
pub(crate) fn running_body(ui: &mut Ui, state: &AppState) {
    ui.push_id("sim_status_lbl", |ui| {
        let _ = Text::new(state.sim_status.clone()).show(ui);
    });
    ui.push_id("progress_gauge", |ui| {
        let _ = Progress::new(state.sim_progress)
            .label("Simulation progress")
            .show(ui);
    });
}
