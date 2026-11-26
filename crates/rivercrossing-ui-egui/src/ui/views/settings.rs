// SPDX-License-Identifier: GPL-3.0-only

//! The Settings dialog: the appearance radios, the four checks and the average speed spin.

use craft::egui::{DragValue, Ui};

use super::text;
use crate::ui::presenters::settings_form;
use crate::ui::state::AppState;

/// Show the settings form.
pub(crate) fn body(ui: &mut Ui, state: &mut AppState) {
    let Some(mut form) = state.settings_form.clone() else {
        return;
    };
    ui.heading("Appearance").accessible_name("Appearance");
    for (id, label, spelling) in [
        ("appearance_system_radio", "System", "system"),
        ("appearance_light_radio", "Light", "light"),
        ("appearance_dark_radio", "Dark", "dark"),
    ] {
        let selected = form.appearance == spelling;
        if ui
            .push_id(id, |ui| ui.radio(selected, label))
            .inner
            .clicked()
        {
            spelling.clone_into(&mut form.appearance);
        }
    }
    for (id, label, value) in [
        ("sound_chk", "Sound on crossing", &mut form.sound_on),
        (
            "show_total_times_chk",
            "Show Total Times on Crossings Panel",
            &mut form.show_total_times,
        ),
        (
            "show_lap_time_chk",
            "Show Lap Time in Crossings Panel",
            &mut form.show_lap_time,
        ),
        (
            "verbose_log_chk",
            "Verbose logging",
            &mut form.verbose_logging,
        ),
    ] {
        ui.push_id(id, |ui| {
            ui.checkbox(value, label);
        });
    }
    ui.push_id("avg_speed_spin", |ui| {
        ui.horizontal(|ui| {
            text(ui, "Avg Lap Time (kmh)");
            ui.add(
                DragValue::new(&mut form.avg_speed_kmh)
                    .range(settings_form::MIN_AVG_SPEED..=200.0)
                    .speed(0.1)
                    .max_decimals(1),
            )
            .accessible_name("Avg Lap Time (kmh)");
        });
    });
    if let Some(errors) = &state.settings_errors {
        ui.push_id("settings_infobar", |ui| text(ui, &errors.join("; ")));
    }
    state.settings_form = Some(form);
}
