// SPDX-License-Identifier: GPL-3.0-only

//! The persisted app settings: the Python `settings.json` schema.
//!
//! The module reads and writes the same file the Python app uses, with the
//! same 29 fields and the same per-field coercers. A load never fails: a
//! missing, unreadable, corrupt or non-object file returns the defaults and
//! records one warning. A save creates the parent directory, writes a sibling
//! temporary file and renames it over the target.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use crate::error::StoreError;

#[cfg(test)]
mod tests;

/// The zoom rungs the View menu's radios carry.
const ZOOM_LADDER: [u16; 7] = [90, 100, 110, 120, 130, 140, 150];

/// The appearance spellings the settings dialog carries.
const APPEARANCES: [&str; 3] = ["system", "light", "dark"];

/// The `WordPress` post statuses the publish feature can reach.
const WP_STATUSES: [&str; 4] = ["draft", "publish", "pending", "private"];

/// The publish kinds the publish feature can reach.
const WP_KINDS: [&str; 2] = ["full", "podium"];

/// The documented default zoom.
const DEFAULT_ZOOM_PERCENT: u16 = 100;

/// The stored average-speed default.
const DEFAULT_AVG_SPEED_KMH: f64 = 12.0;

/// The average-speed floor: a lower value makes the lap estimate infinite.
const MIN_AVG_SPEED_KMH: f64 = 1.0;

/// The suffix of the temporary file a save renames over the target.
const TEMP_SUFFIX: &str = ".tmp";

/// The settings dialog's fields, persisted schema-compatibly with Python.
#[expect(
    clippy::struct_excessive_bools,
    reason = "mirrors the Python settings table, one named field per stored preference"
)]
#[derive(Debug, Clone, PartialEq)]
pub struct AppSettings {
    /// The appearance spelling: `system`, `light` or `dark`.
    pub appearance: String,
    /// Whether the sound cues play.
    pub sound_on: bool,
    /// Whether the console shows the total-time column.
    pub show_total_times: bool,
    /// Whether the console shows the lap-time column.
    pub show_lap_time: bool,
    /// The text zoom percentage, one of the zoom ladder rungs.
    pub zoom_percent: u16,
    /// The saved splitter position, when one is saved.
    pub splitter_sash: Option<i32>,
    /// The saved window rectangle `[x, y, width, height]`, when one is saved.
    pub window_geometry: Option<[i32; 4]>,
    /// Whether the verbose log is written.
    pub verbose_logging: bool,
    /// The simulator's rider count.
    pub sim_riders: i32,
    /// The simulator's team count.
    pub sim_teams: i32,
    /// The simulator's solo-rider count.
    pub sim_solo: i32,
    /// The simulator's lap count.
    pub sim_laps: i32,
    /// The simulator's interval in seconds.
    pub sim_interval: i32,
    /// The simulator's short-lap choice, `0` for disabled.
    pub sim_short_laps: i32,
    /// The simulator's lapped-rider choice, `0` for disabled.
    pub sim_lapped: i32,
    /// The simulator's team-stop choice, `0` for disabled.
    pub sim_team_stop: i32,
    /// The average rider speed in km/h that the card estimate uses.
    pub avg_speed_kmh: f64,
    /// Whether the published results show the times.
    pub publish_show_times: bool,
    /// Whether the published results show the laps leaderboard.
    pub publish_laps_board: bool,
    /// Whether the published results show the fastest-time leaderboard.
    pub publish_time_board: bool,
    /// Whether the published results show the full field.
    pub publish_full_field: bool,
    /// Whether the published results show every drawn card.
    pub publish_all_cards: bool,
    /// The `WordPress` site URL.
    pub wp_url: String,
    /// The `WordPress` REST user name.
    pub wp_username: String,
    /// The `WordPress` REST password.
    pub wp_password: String,
    /// The `WordPress` parent page id.
    pub wp_parent: String,
    /// The `WordPress` post status a published result lands as.
    pub wp_status: String,
    /// The publish kind: the full results page or the podium one.
    pub wp_kind: String,
    /// Whether a finished ride shows its did-not-start riders.
    pub show_dns_riders: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        default_settings()
    }
}

/// Return the all-defaults settings.
#[must_use]
pub fn default_settings() -> AppSettings {
    AppSettings {
        appearance: "system".to_owned(),
        sound_on: true,
        show_total_times: false,
        show_lap_time: true,
        zoom_percent: DEFAULT_ZOOM_PERCENT,
        splitter_sash: None,
        window_geometry: None,
        verbose_logging: true,
        sim_riders: 175,
        sim_teams: 40,
        sim_solo: 15,
        sim_laps: 1,
        sim_interval: 45,
        sim_short_laps: 1,
        sim_lapped: 0,
        sim_team_stop: 0,
        avg_speed_kmh: DEFAULT_AVG_SPEED_KMH,
        publish_show_times: false,
        publish_laps_board: true,
        publish_time_board: false,
        publish_full_field: true,
        publish_all_cards: true,
        wp_url: String::new(),
        wp_username: String::new(),
        wp_password: String::new(),
        wp_parent: String::new(),
        wp_status: "draft".to_owned(),
        wp_kind: "full".to_owned(),
        show_dns_riders: true,
    }
}

/// Load the settings at `path`.
///
/// The load never fails. A file that cannot be read returns the defaults with
/// one warning. A file that does not decode, or that decodes to a value other
/// than a JSON object, also returns the defaults with one warning. Otherwise
/// each field takes its stored value when the value has the field's type, and
/// its default when the value is missing or wrong. Unknown keys are ignored.
#[must_use]
pub fn load_settings(path: &Path) -> (AppSettings, Vec<String>) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return (
            default_settings(),
            vec![format!(
                "settings file {} could not be read; using defaults",
                path.display()
            )],
        );
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Object(raw)) => (from_mapping(&raw), Vec::new()),
        Ok(_) => (
            default_settings(),
            vec![format!(
                "settings file {} is not a JSON object; using defaults",
                path.display()
            )],
        ),
        Err(_) => (
            default_settings(),
            vec![format!(
                "settings file {} could not be decoded; using defaults",
                path.display()
            )],
        ),
    }
}

/// Save the settings to `path`.
///
/// Create the parent directory, write a sibling temporary file and rename it
/// over `path`. A failure leaves the previous file in place.
///
/// # Errors
///
/// Return [`StoreError::Io`] when the directory, the temporary file or the
/// rename fails. Return [`StoreError::Json`] when the payload does not encode.
pub fn save_settings(settings: &AppSettings, path: &Path) -> Result<(), StoreError> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;
    let text = serde_json::to_string_pretty(&payload(settings))?;
    let temp_path = temp_path(path);
    std::fs::write(&temp_path, format!("{text}\n"))?;
    std::fs::rename(&temp_path, path)?;
    Ok(())
}

/// Build the JSON payload that [`save_settings`] writes.
fn payload(settings: &AppSettings) -> Value {
    json!({
        "appearance": settings.appearance,
        "sound_on": settings.sound_on,
        "show_total_times": settings.show_total_times,
        "show_lap_time": settings.show_lap_time,
        "zoom_percent": settings.zoom_percent,
        "splitter_sash": settings.splitter_sash,
        "window_geometry": settings.window_geometry,
        "verbose_logging": settings.verbose_logging,
        "sim_riders": settings.sim_riders,
        "sim_teams": settings.sim_teams,
        "sim_solo": settings.sim_solo,
        "sim_laps": settings.sim_laps,
        "sim_interval": settings.sim_interval,
        "sim_short_laps": settings.sim_short_laps,
        "sim_lapped": settings.sim_lapped,
        "sim_team_stop": settings.sim_team_stop,
        "avg_speed_kmh": settings.avg_speed_kmh,
        "publish_show_times": settings.publish_show_times,
        "publish_laps_board": settings.publish_laps_board,
        "publish_time_board": settings.publish_time_board,
        "publish_full_field": settings.publish_full_field,
        "publish_all_cards": settings.publish_all_cards,
        "wp_url": settings.wp_url,
        "wp_username": settings.wp_username,
        "wp_password": settings.wp_password,
        "wp_parent": settings.wp_parent,
        "wp_status": settings.wp_status,
        "wp_kind": settings.wp_kind,
        "show_dns_riders": settings.show_dns_riders,
    })
}

/// Return the sibling temporary path that a save renames over `path`.
fn temp_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(TEMP_SUFFIX);
    PathBuf::from(name)
}

/// Build the settings from a decoded JSON object, field by field.
fn from_mapping(raw: &Map<String, Value>) -> AppSettings {
    let defaults = default_settings();
    AppSettings {
        appearance: whitelist_or(raw.get("appearance"), &APPEARANCES, &defaults.appearance),
        sound_on: bool_or(raw.get("sound_on"), defaults.sound_on),
        show_total_times: bool_or(raw.get("show_total_times"), defaults.show_total_times),
        show_lap_time: bool_or(raw.get("show_lap_time"), defaults.show_lap_time),
        zoom_percent: clamp_zoom(
            raw.get("zoom_percent")
                .and_then(Value::as_i64)
                .unwrap_or(i64::from(DEFAULT_ZOOM_PERCENT)),
        ),
        splitter_sash: optional_i32(raw.get("splitter_sash")),
        window_geometry: geometry_or(raw.get("window_geometry")),
        verbose_logging: bool_or(raw.get("verbose_logging"), defaults.verbose_logging),
        sim_riders: i32_or(raw.get("sim_riders"), defaults.sim_riders),
        sim_teams: i32_or(raw.get("sim_teams"), defaults.sim_teams),
        sim_solo: i32_or(raw.get("sim_solo"), defaults.sim_solo),
        sim_laps: i32_or(raw.get("sim_laps"), defaults.sim_laps),
        sim_interval: i32_or(raw.get("sim_interval"), defaults.sim_interval),
        sim_short_laps: i32_or(raw.get("sim_short_laps"), defaults.sim_short_laps),
        sim_lapped: i32_or(raw.get("sim_lapped"), defaults.sim_lapped),
        sim_team_stop: i32_or(raw.get("sim_team_stop"), defaults.sim_team_stop),
        avg_speed_kmh: avg_speed_or(
            raw.get("avg_speed_kmh").and_then(Value::as_f64),
            defaults.avg_speed_kmh,
        ),
        publish_show_times: bool_or(raw.get("publish_show_times"), defaults.publish_show_times),
        publish_laps_board: bool_or(raw.get("publish_laps_board"), defaults.publish_laps_board),
        publish_time_board: bool_or(raw.get("publish_time_board"), defaults.publish_time_board),
        publish_full_field: bool_or(raw.get("publish_full_field"), defaults.publish_full_field),
        publish_all_cards: bool_or(raw.get("publish_all_cards"), defaults.publish_all_cards),
        wp_url: string_or(raw.get("wp_url"), &defaults.wp_url),
        wp_username: string_or(raw.get("wp_username"), &defaults.wp_username),
        wp_password: string_or(raw.get("wp_password"), &defaults.wp_password),
        wp_parent: string_or(raw.get("wp_parent"), &defaults.wp_parent),
        wp_status: whitelist_or(raw.get("wp_status"), &WP_STATUSES, &defaults.wp_status),
        wp_kind: whitelist_or(raw.get("wp_kind"), &WP_KINDS, &defaults.wp_kind),
        show_dns_riders: bool_or(raw.get("show_dns_riders"), defaults.show_dns_riders),
    }
}

/// Return the whitelisted spelling of `value`, else `default`.
fn whitelist_or(value: Option<&Value>, allowed: &[&str], default: &str) -> String {
    match value.and_then(Value::as_str) {
        Some(text) if allowed.contains(&text) => text.to_owned(),
        _ => default.to_owned(),
    }
}

/// Return the bool stored at `value`, else `default`.
fn bool_or(value: Option<&Value>, default: bool) -> bool {
    value.and_then(Value::as_bool).unwrap_or(default)
}

/// Return the string stored at `value`, else `default`.
fn string_or(value: Option<&Value>, default: &str) -> String {
    value
        .and_then(Value::as_str)
        .map_or_else(|| default.to_owned(), str::to_owned)
}

/// Return the `i32` stored at `value`, else `default`.
fn i32_or(value: Option<&Value>, default: i32) -> i32 {
    value.and_then(int_item).unwrap_or(default)
}

/// Return the optional `i32` stored at `value`.
fn optional_i32(value: Option<&Value>) -> Option<i32> {
    value.and_then(int_item)
}

/// Return the JSON int at `value` when it fits an `i32`. A bool never fits.
fn int_item(value: &Value) -> Option<i32> {
    value.as_i64().and_then(|number| i32::try_from(number).ok())
}

/// Return the average speed, floored and finite, else `default`.
fn avg_speed_or(value: Option<f64>, default: f64) -> f64 {
    match value {
        Some(number) if number.is_finite() => number.max(MIN_AVG_SPEED_KMH),
        _ => default,
    }
}

/// Return the four-item window rectangle stored at `value`.
fn geometry_or(value: Option<&Value>) -> Option<[i32; 4]> {
    let Some(Value::Array(items)) = value else {
        return None;
    };
    let [x, y, width, height] = items.as_slice() else {
        return None;
    };
    Some([
        int_item(x)?,
        int_item(y)?,
        int_item(width)?,
        int_item(height)?,
    ])
}

/// Return `zoom` moved onto the nearest zoom rung, ties to the lower rung.
fn clamp_zoom(zoom: i64) -> u16 {
    if let Some(rung) = ZOOM_LADDER
        .iter()
        .copied()
        .find(|rung| i64::from(*rung) == zoom)
    {
        return rung;
    }
    ZOOM_LADDER
        .iter()
        .copied()
        .min_by_key(|rung| (i64::from(*rung).abs_diff(zoom), *rung))
        .unwrap_or(DEFAULT_ZOOM_PERCENT)
}
