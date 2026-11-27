// SPDX-License-Identifier: GPL-3.0-only

//! The settings dialog's form values, their validation and the appearance radios.
//!
//! The rules follow the Python oracle's `ui/presenters/settings.py` and
//! `ui/views/settings.py`: the three appearance radios spell the stored
//! appearance, Light winning over Dark, and the average rider speed keeps its
//! decimal and its 1 km/h floor. The `ArtCraft` contrast, density, text-size and
//! theme axes are additive controls outside the frozen fields. Every field the
//! dialog has no control for carries through unchanged.

#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the EPIC-12c views wire these decisions in a later phase; phase 1 unit-tests them"
    )
)]

use rivercrossing_store::settings::AppSettings;

#[cfg(test)]
mod tests;

/// The appearance spellings the dialog carries.
const APPEARANCES: [&str; 3] = ["system", "light", "dark"];

/// The contrast choices the dialog carries, as (label, stored spelling).
pub(crate) const CONTRAST_CHOICES: [(&str, &str); 3] = [
    ("System", "system"),
    ("Standard", "standard"),
    ("High", "high"),
];

/// The density choices the dialog carries, as (label, stored spelling).
pub(crate) const DENSITY_CHOICES: [(&str, &str); 2] =
    [("Compact", "compact"), ("Comfortable", "comfortable")];

/// The text-size choices the dialog carries, as (label, stored spelling).
pub(crate) const TEXT_SIZE_CHOICES: [(&str, &str); 4] = [
    ("Tiny", "tiny"),
    ("Small", "small"),
    ("Medium", "medium"),
    ("Large", "large"),
];

/// The floor for the stored average rider speed, in km/h.
const MIN_AVG_SPEED_KMH: f64 = 1.0;

/// The floor the settings dialog's spin starts at.
pub(crate) const MIN_AVG_SPEED: f64 = MIN_AVG_SPEED_KMH;

/// The refusal for an appearance outside the three spellings.
const APPEARANCE_UNKNOWN: &str = "Appearance must be system, light or dark";

/// The refusal for a contrast outside the three spellings.
const CONTRAST_UNKNOWN: &str = "Contrast must be system, standard or high";

/// The refusal for a density outside the two spellings.
const DENSITY_UNKNOWN: &str = "Density must be compact or comfortable";

/// The refusal for a text size outside the four spellings.
const TEXT_SIZE_UNKNOWN: &str = "Text size must be tiny, small, medium or large";

/// The refusal for a theme id that names no built-in theme.
const THEME_UNKNOWN: &str = "Theme must be a built-in theme";

/// The refusal for a non-finite average rider speed.
const AVG_SPEED_NOT_A_NUMBER: &str = "Average rider speed must be a number";

/// The refusal for an average rider speed below the floor.
const AVG_SPEED_BELOW_FLOOR: &str = "Average rider speed must be at least 1 km/h";

/// The settings dialog's six control values.
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is one independent dialog control that the view renders separately"
)]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FormValues {
    /// The appearance spelling the checked radio selects.
    pub(crate) appearance: String,
    /// The contrast spelling.
    pub(crate) contrast: String,
    /// The density spelling.
    pub(crate) density: String,
    /// The text-size spelling.
    pub(crate) text_size: String,
    /// The theme id.
    pub(crate) theme: String,
    /// Whether the sound cues play.
    pub(crate) sound_on: bool,
    /// Whether the console shows the total-time column.
    pub(crate) show_total_times: bool,
    /// Whether the console shows the lap-time column.
    pub(crate) show_lap_time: bool,
    /// Whether the verbose log is written.
    pub(crate) verbose_logging: bool,
    /// The average rider speed in km/h.
    pub(crate) avg_speed_kmh: f64,
}

/// Return the form values that *settings* renders.
#[must_use]
pub(crate) fn from_settings(settings: &AppSettings) -> FormValues {
    FormValues {
        appearance: settings.appearance.clone(),
        contrast: settings.contrast.clone(),
        density: settings.density.clone(),
        text_size: settings.text_size.clone(),
        theme: settings.theme.clone(),
        sound_on: settings.sound_on,
        show_total_times: settings.show_total_times,
        show_lap_time: settings.show_lap_time,
        verbose_logging: settings.verbose_logging,
        avg_speed_kmh: settings.avg_speed_kmh,
    }
}

/// Apply *values* to *settings* and return the refusals, if any.
///
/// The ten dialog fields always apply. A spelling outside its axis's choices
/// is refused and leaves the stored value alone; a theme id that names no
/// built-in theme is refused the same way. A non-finite average rider speed is
/// refused and leaves the stored value alone; a value below 1 km/h is floored
/// to 1 km/h and refused. Every other setting carries through unchanged. An
/// empty list means the form was accepted.
#[must_use]
pub(crate) fn apply(values: &FormValues, settings: &mut AppSettings) -> Vec<String> {
    let mut messages = Vec::new();
    if APPEARANCES.contains(&values.appearance.as_str()) {
        settings.appearance.clone_from(&values.appearance);
    } else {
        messages.push(APPEARANCE_UNKNOWN.to_owned());
    }
    if is_choice(&CONTRAST_CHOICES, &values.contrast) {
        settings.contrast.clone_from(&values.contrast);
    } else {
        messages.push(CONTRAST_UNKNOWN.to_owned());
    }
    if is_choice(&DENSITY_CHOICES, &values.density) {
        settings.density.clone_from(&values.density);
    } else {
        messages.push(DENSITY_UNKNOWN.to_owned());
    }
    if is_choice(&TEXT_SIZE_CHOICES, &values.text_size) {
        settings.text_size.clone_from(&values.text_size);
    } else {
        messages.push(TEXT_SIZE_UNKNOWN.to_owned());
    }
    if is_builtin_theme(&values.theme) {
        settings.theme.clone_from(&values.theme);
    } else {
        messages.push(THEME_UNKNOWN.to_owned());
    }
    settings.sound_on = values.sound_on;
    settings.show_total_times = values.show_total_times;
    settings.show_lap_time = values.show_lap_time;
    settings.verbose_logging = values.verbose_logging;
    if values.avg_speed_kmh.is_finite() {
        if values.avg_speed_kmh < MIN_AVG_SPEED_KMH {
            messages.push(AVG_SPEED_BELOW_FLOOR.to_owned());
        }
        settings.avg_speed_kmh = values.avg_speed_kmh.max(MIN_AVG_SPEED_KMH);
    } else {
        messages.push(AVG_SPEED_NOT_A_NUMBER.to_owned());
    }
    messages
}

/// Whether *spelling* is one of the axis's stored choices.
fn is_choice(choices: &[(&str, &str)], spelling: &str) -> bool {
    choices.iter().any(|(_, value)| *value == spelling)
}

/// Whether *id* names a built-in theme.
fn is_builtin_theme(id: &str) -> bool {
    craft::theme::builtins()
        .iter()
        .any(|theme| theme.id.as_str() == id)
}

/// Return the checked appearance radio's spelling.
///
/// Light wins over Dark. Neither checked is the System radio's own state, so a
/// form that carries both false reads `system`.
#[must_use]
pub(crate) fn appearance_for_radio(light: bool, dark: bool) -> &'static str {
    if light {
        return "light";
    }
    if dark {
        return "dark";
    }
    "system"
}
