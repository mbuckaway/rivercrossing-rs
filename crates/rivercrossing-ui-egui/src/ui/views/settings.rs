// SPDX-License-Identifier: GPL-3.0-only

//! The Settings dialog: the combined appearance picker and axes, the four checks and the average
//! speed spin.

use craft::egui::Ui;
use craft::theme::variant::Tone;
use craft::ui::choice::Checkbox;
use craft::ui::contract::{Bind, Props as _, Show as _, ShowContainer as _};
use craft::ui::sections::{HeaderStrength, Section};

use super::dialogs::{decimal_value, field_caption, notice, pair, select_owned, select_value};
use crate::ui::presenters::settings_form;
use crate::ui::state::AppState;

#[cfg(test)]
mod tests;

/// The average speed's highest offered value.
const MAX_AVG_SPEED: f64 = 200.0;

/// The average-speed field's width: the mockup caps it at 220 points inside the console group.
const AVG_SPEED_WIDTH: f32 = 220.0;

/// The strength of the Appearance and Console headings: a quiet caption, with no rule.
const HEADING_STRENGTH: HeaderStrength = HeaderStrength::Caption;

/// Show the settings form.
pub(crate) fn body(ui: &mut Ui, state: &mut AppState) {
    let Some(mut form) = state.settings_form.clone() else {
        return;
    };
    let _ = heading("Appearance").show(ui, |ui, _| appearance(ui, &mut form));
    let _ = heading("Console").show(ui, |ui, _| console_group(ui, &mut form));
    if let Some(errors) = &state.settings_errors {
        notice(ui, "settings_infobar", Tone::Bad, errors.join("; "));
    }
    state.settings_form = Some(form);
}

/// Return the group heading: a quiet caption with no rule, as the design's `.section-label` is.
fn heading(title: &'static str) -> Section<'static> {
    Section::fixed(title).strength(HEADING_STRENGTH)
}

/// Show the combined theme picker and the three `ArtCraft` appearance axes.
fn appearance(ui: &mut Ui, form: &mut settings_form::FormValues) {
    let entries = settings_form::appearance_entries();
    let current = settings_form::appearance_choice(&entries, &form.theme, &form.appearance)
        .unwrap_or_default();
    ui.horizontal(|ui| {
        field_caption(ui, "Theme");
        if let Some(value) =
            select_owned(ui, "appearance_theme_choice", "Theme", &current, &entries)
        {
            settings_form::set_appearance(form, &value);
        }
    });
    pair(ui, |left, right| {
        axis(
            left,
            "appearance_contrast_choice",
            "Contrast",
            &mut form.contrast,
            &settings_form::CONTRAST_CHOICES,
        );
        axis(
            right,
            "appearance_density_choice",
            "Density",
            &mut form.density,
            &settings_form::DENSITY_CHOICES,
        );
    });
    axis(
        ui,
        "appearance_text_size_choice",
        "Text size",
        &mut form.text_size,
        &settings_form::TEXT_SIZE_CHOICES,
    );
}

/// Show one stored appearance axis as a labelled select, its caption beside the control.
fn axis(
    ui: &mut Ui,
    id: &'static str,
    label: &'static str,
    value: &mut String,
    choices: &[(&'static str, &'static str)],
) {
    ui.horizontal(|ui| {
        field_caption(ui, label);
        if let Some(picked) = select_value(ui, id, label, value, choices) {
            picked.clone_into(value);
        }
    });
}

/// Show the four console checks and the average speed field.
fn console_group(ui: &mut Ui, form: &mut settings_form::FormValues) {
    check(ui, "sound_chk", "Sound on crossing", &mut form.sound_on);
    check(
        ui,
        "show_total_times_chk",
        "Show Total Times on Crossings Panel",
        &mut form.show_total_times,
    );
    check(
        ui,
        "show_lap_time_chk",
        "Show Lap Time in Crossings Panel",
        &mut form.show_lap_time,
    );
    check(
        ui,
        "verbose_log_chk",
        "Verbose logging",
        &mut form.verbose_logging,
    );
    field_caption(ui, "Avg Lap Time (kmh)");
    // Craft's fill-width leaf measures against the form column, not the inner cell, so the field
    // takes the row's left half — the design's narrow companion cell — instead of the whole width.
    pair(ui, |left, _right| {
        decimal_value(
            left,
            "avg_speed_spin",
            "Avg Lap Time (kmh)",
            &mut form.avg_speed_kmh,
            AVG_SPEED_WIDTH,
            settings_form::MIN_AVG_SPEED..=MAX_AVG_SPEED,
        );
    });
}

/// Show one check box under its frozen id.
fn check(ui: &mut Ui, id: &'static str, label: &'static str, value: &mut bool) {
    ui.push_id(id, |ui| {
        let _ = Checkbox::new(Bind::bound(value)).label(label).show(ui);
    });
}
