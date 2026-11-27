// SPDX-License-Identifier: GPL-3.0-only

//! The Settings dialog: the appearance radios and axes, the four checks and the average speed spin.

use craft::egui::Ui;
use craft::theme::variant::Tone;
use craft::ui::choice::Checkbox;
use craft::ui::contract::{Bind, Props as _, Show as _, ShowContainer as _};
use craft::ui::sections::Section;

use super::dialogs::{decimal_value, field_caption, notice, radio_group, select_value};
use crate::ui::presenters::settings_form;
use crate::ui::state::AppState;

/// The appearance spellings, in radio order.
const APPEARANCES: [&str; 3] = ["system", "light", "dark"];

/// The average speed's highest offered value.
const MAX_AVG_SPEED: f64 = 200.0;

/// Show the settings form.
pub(crate) fn body(ui: &mut Ui, state: &mut AppState) {
    let Some(mut form) = state.settings_form.clone() else {
        return;
    };
    let _ = Section::fixed("Appearance").show(ui, |ui, _| appearance(ui, &mut form));
    let _ = Section::fixed("Console").show(ui, |ui, _| console_group(ui, &mut form));
    if let Some(errors) = &state.settings_errors {
        notice(ui, "settings_infobar", Tone::Bad, errors.join("; "));
    }
    state.settings_form = Some(form);
}

/// Show the mode radios and the four `ArtCraft` appearance axes.
fn appearance(ui: &mut Ui, form: &mut settings_form::FormValues) {
    let current = APPEARANCES
        .iter()
        .position(|stored| *stored == form.appearance)
        .unwrap_or(0);
    let picked = radio_group(
        ui,
        "Mode",
        u8::try_from(current).unwrap_or(0),
        &[
            (0, "appearance_system_radio", "System"),
            (1, "appearance_light_radio", "Light"),
            (2, "appearance_dark_radio", "Dark"),
        ],
    );
    if usize::from(picked) != current
        && let Some(stored) = APPEARANCES.get(usize::from(picked))
    {
        (*stored).clone_into(&mut form.appearance);
    }
    axis(
        ui,
        "appearance_contrast_choice",
        "Contrast",
        &mut form.contrast,
        &settings_form::CONTRAST_CHOICES,
    );
    axis(
        ui,
        "appearance_density_choice",
        "Density",
        &mut form.density,
        &settings_form::DENSITY_CHOICES,
    );
    axis(
        ui,
        "appearance_text_size_choice",
        "Text size",
        &mut form.text_size,
        &settings_form::TEXT_SIZE_CHOICES,
    );
    let themes: Vec<(&str, &str)> = craft::theme::builtins()
        .iter()
        .map(|theme| (theme.name.as_str(), theme.id.as_str()))
        .collect();
    axis(
        ui,
        "appearance_theme_choice",
        "Theme",
        &mut form.theme,
        &themes,
    );
}

/// Show one stored appearance axis as a labelled select.
fn axis(
    ui: &mut Ui,
    id: &'static str,
    label: &'static str,
    value: &mut String,
    choices: &[(&'static str, &'static str)],
) {
    field_caption(ui, label);
    if let Some(picked) = select_value(ui, id, label, value, choices) {
        picked.clone_into(value);
    }
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
    decimal_value(
        ui,
        "avg_speed_spin",
        "Avg Lap Time (kmh)",
        &mut form.avg_speed_kmh,
        settings_form::MIN_AVG_SPEED..=MAX_AVG_SPEED,
    );
}

/// Show one check box under its frozen id.
fn check(ui: &mut Ui, id: &'static str, label: &'static str, value: &mut bool) {
    ui.push_id(id, |ui| {
        let _ = Checkbox::new(Bind::bound(value)).label(label).show(ui);
    });
}
