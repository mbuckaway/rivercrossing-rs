// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the persisted settings: the Python default table, the
//! per-field coercers, the never-failing loader, the atomic saver and the
//! platform config directory.

use std::ffi::OsStr;
use std::path::PathBuf;

use pretty_assertions::assert_eq;
use rstest::rstest;
use serde_json::Value;
use tempfile::TempDir;

use super::{AppSettings, avg_speed_or, default_settings, load_settings, save_settings};
use crate::error::StoreError;
use crate::paths::{config_dir_for, default_settings_path};

/// Build an environment lookup over a fixed table of pairs.
fn env_from<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |name: &str| {
        pairs
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| (*value).to_owned())
    }
}

/// Build a settings value that differs from the defaults in every field.
fn non_default_settings() -> AppSettings {
    AppSettings {
        appearance: "dark".to_owned(),
        sound_on: false,
        show_total_times: true,
        show_lap_time: false,
        zoom_percent: 130,
        splitter_sash: Some(420),
        window_geometry: Some([10, 20, 800, 600]),
        verbose_logging: false,
        sim_riders: 200,
        sim_teams: 50,
        sim_solo: 20,
        sim_laps: 2,
        sim_interval: 60,
        sim_short_laps: 2,
        sim_lapped: 1,
        sim_team_stop: 1,
        avg_speed_kmh: 15.5,
        publish_show_times: true,
        publish_laps_board: false,
        publish_time_board: true,
        publish_full_field: false,
        publish_all_cards: false,
        wp_url: "https://example.test".to_owned(),
        wp_username: "ada".to_owned(),
        wp_password: "secret".to_owned(),
        wp_parent: "1234".to_owned(),
        wp_status: "publish".to_owned(),
        wp_kind: "podium".to_owned(),
        show_dns_riders: false,
    }
}

/// Build the defaults with one appearance spelling.
fn appearance(value: &str) -> AppSettings {
    AppSettings {
        appearance: value.to_owned(),
        ..default_settings()
    }
}

/// Build the defaults with one sound flag.
fn sound_on(value: bool) -> AppSettings {
    AppSettings {
        sound_on: value,
        ..default_settings()
    }
}

/// Build the defaults with one lap-time column flag.
fn show_lap_time(value: bool) -> AppSettings {
    AppSettings {
        show_lap_time: value,
        ..default_settings()
    }
}

/// Build the defaults with one simulator rider count.
fn sim_riders(value: i32) -> AppSettings {
    AppSettings {
        sim_riders: value,
        ..default_settings()
    }
}

/// Build the defaults with one splitter position.
fn splitter(value: Option<i32>) -> AppSettings {
    AppSettings {
        splitter_sash: value,
        ..default_settings()
    }
}

/// Build the defaults with one `WordPress` site URL.
fn wp_url(value: &str) -> AppSettings {
    AppSettings {
        wp_url: value.to_owned(),
        ..default_settings()
    }
}

/// Build the defaults with one `WordPress` post status.
fn wp_status(value: &str) -> AppSettings {
    AppSettings {
        wp_status: value.to_owned(),
        ..default_settings()
    }
}

/// Build the defaults with one `WordPress` publish kind.
fn wp_kind(value: &str) -> AppSettings {
    AppSettings {
        wp_kind: value.to_owned(),
        ..default_settings()
    }
}

/// Build the defaults with one average rider speed.
fn avg_speed(value: f64) -> AppSettings {
    AppSettings {
        avg_speed_kmh: value,
        ..default_settings()
    }
}

/// Build the defaults with one zoom percentage.
fn zoom(value: u16) -> AppSettings {
    AppSettings {
        zoom_percent: value,
        ..default_settings()
    }
}

/// Build the defaults with one saved window rectangle.
fn geometry(value: Option<[i32; 4]>) -> AppSettings {
    AppSettings {
        window_geometry: value,
        ..default_settings()
    }
}

#[test]
fn test_settings_defaults_match_the_python_table() {
    let expected = AppSettings {
        appearance: "system".to_owned(),
        sound_on: true,
        show_total_times: false,
        show_lap_time: true,
        zoom_percent: 100,
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
        avg_speed_kmh: 12.0,
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
    };

    assert_eq!(default_settings(), expected);
    assert_eq!(AppSettings::default(), expected);
}

#[test]
fn test_settings_round_trip_every_field() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let directory = TempDir::new()?;
    let path = directory.path().join("settings.json");
    let settings = non_default_settings();

    // Act
    save_settings(&settings, &path)?;
    let (loaded, warnings) = load_settings(&path);

    // Assert
    assert_eq!((loaded, warnings), (settings, Vec::new()));
    Ok(())
}

#[test]
fn test_load_settings_given_missing_file_returns_defaults_with_warning()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let directory = TempDir::new()?;
    let path = directory.path().join("settings.json");

    // Act
    let (settings, warnings) = load_settings(&path);

    // Assert
    assert_eq!(
        (settings, warnings),
        (
            default_settings(),
            vec![format!(
                "settings file {} could not be read; using defaults",
                path.display()
            )],
        )
    );
    Ok(())
}

#[test]
fn test_load_settings_given_corrupt_json_returns_defaults_with_warning()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let directory = TempDir::new()?;
    let path = directory.path().join("settings.json");
    std::fs::write(&path, "{not json")?;

    // Act
    let (settings, warnings) = load_settings(&path);

    // Assert
    assert_eq!(
        (settings, warnings),
        (
            default_settings(),
            vec![format!(
                "settings file {} could not be decoded; using defaults",
                path.display()
            )],
        )
    );
    Ok(())
}

#[test]
fn test_load_settings_given_a_non_object_returns_defaults_with_warning()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let directory = TempDir::new()?;
    let path = directory.path().join("settings.json");
    std::fs::write(&path, "[]")?;

    // Act
    let (settings, warnings) = load_settings(&path);

    // Assert
    assert_eq!(
        (settings, warnings),
        (
            default_settings(),
            vec![format!(
                "settings file {} is not a JSON object; using defaults",
                path.display()
            )],
        )
    );
    Ok(())
}

#[rstest]
#[case::an_empty_object_returns_the_defaults("{}", default_settings())]
#[case::an_unknown_key_is_ignored(r#"{"unknown_key": 1}"#, default_settings())]
#[case::appearance_light_is_kept(r#"{"appearance": "light"}"#, appearance("light"))]
#[case::appearance_dark_is_kept(r#"{"appearance": "dark"}"#, appearance("dark"))]
#[case::appearance_unknown_is_system(r#"{"appearance": "pastel"}"#, appearance("system"))]
#[case::appearance_not_a_string_is_system(r#"{"appearance": 7}"#, default_settings())]
#[case::sound_on_false_is_kept(r#"{"sound_on": false}"#, sound_on(false))]
#[case::sound_on_not_a_bool_is_the_default(r#"{"sound_on": 1}"#, default_settings())]
#[case::show_lap_time_false_is_kept(r#"{"show_lap_time": false}"#, show_lap_time(false))]
#[case::show_lap_time_not_a_bool_is_the_default(r#"{"show_lap_time": "yes"}"#, default_settings())]
#[case::sim_riders_an_int_is_kept(r#"{"sim_riders": 200}"#, sim_riders(200))]
#[case::sim_riders_a_bool_is_the_default(r#"{"sim_riders": true}"#, default_settings())]
#[case::sim_riders_above_i32_max_is_the_default(
    r#"{"sim_riders": 2147483648}"#,
    default_settings()
)]
#[case::sim_riders_at_i32_min_is_kept(r#"{"sim_riders": -2147483648}"#, sim_riders(i32::MIN))]
#[case::splitter_sash_an_int_is_kept(r#"{"splitter_sash": 42}"#, splitter(Some(42)))]
#[case::splitter_sash_null_is_none(r#"{"splitter_sash": null}"#, splitter(None))]
#[case::splitter_sash_a_string_is_none(r#"{"splitter_sash": "42"}"#, splitter(None))]
#[case::splitter_sash_a_bool_is_none(r#"{"splitter_sash": true}"#, splitter(None))]
#[case::splitter_sash_above_i32_max_is_none(r#"{"splitter_sash": 2147483648}"#, splitter(None))]
#[case::wp_url_a_string_is_kept(
    r#"{"wp_url": "https://example.test"}"#,
    wp_url("https://example.test")
)]
#[case::wp_url_not_a_string_is_empty(r#"{"wp_url": 5}"#, default_settings())]
#[case::wp_username_a_blank_string_is_kept(r#"{"wp_username": ""}"#, default_settings())]
#[case::avg_speed_a_float_is_kept(r#"{"avg_speed_kmh": 15.5}"#, avg_speed(15.5))]
#[case::avg_speed_an_int_is_kept(r#"{"avg_speed_kmh": 12}"#, avg_speed(12.0))]
#[case::avg_speed_below_the_floor_is_the_floor(r#"{"avg_speed_kmh": 0.5}"#, avg_speed(1.0))]
#[case::avg_speed_negative_is_the_floor(r#"{"avg_speed_kmh": -5}"#, avg_speed(1.0))]
#[case::avg_speed_at_the_floor_is_kept(r#"{"avg_speed_kmh": 1.0}"#, avg_speed(1.0))]
#[case::avg_speed_a_string_is_the_default(r#"{"avg_speed_kmh": "fast"}"#, default_settings())]
#[case::avg_speed_a_bool_is_the_default(r#"{"avg_speed_kmh": true}"#, default_settings())]
#[case::zoom_on_the_ladder_is_kept(r#"{"zoom_percent": 90}"#, zoom(90))]
#[case::zoom_95_ties_to_the_lower_rung(r#"{"zoom_percent": 95}"#, zoom(90))]
#[case::zoom_125_ties_to_the_lower_rung(r#"{"zoom_percent": 125}"#, zoom(120))]
#[case::zoom_96_rounds_up(r#"{"zoom_percent": 96}"#, zoom(100))]
#[case::zoom_85_rounds_up_to_90(r#"{"zoom_percent": 85}"#, zoom(90))]
#[case::zoom_160_rounds_down_to_150(r#"{"zoom_percent": 160}"#, zoom(150))]
#[case::zoom_not_an_int_is_the_default(r#"{"zoom_percent": 100.5}"#, zoom(100))]
#[case::zoom_a_bool_is_the_default(r#"{"zoom_percent": true}"#, zoom(100))]
#[case::zoom_above_the_ladder_is_150(r#"{"zoom_percent": 100000}"#, zoom(150))]
#[case::zoom_below_the_ladder_is_90(r#"{"zoom_percent": -100000}"#, zoom(90))]
#[case::geometry_four_ints_are_kept(
    r#"{"window_geometry": [10, 20, 800, 600]}"#,
    geometry(Some([10, 20, 800, 600]))
)]
#[case::geometry_a_short_array_is_none(r#"{"window_geometry": [1, 2, 3]}"#, geometry(None))]
#[case::geometry_a_long_array_is_none(r#"{"window_geometry": [1, 2, 3, 4, 5]}"#, geometry(None))]
#[case::geometry_a_non_array_is_none(r#"{"window_geometry": "x"}"#, geometry(None))]
#[case::geometry_null_is_none(r#"{"window_geometry": null}"#, geometry(None))]
#[case::geometry_a_non_int_first_item_is_none(
    r#"{"window_geometry": ["x", 2, 3, 4]}"#,
    geometry(None)
)]
#[case::geometry_a_bool_second_item_is_none(
    r#"{"window_geometry": [1, true, 3, 4]}"#,
    geometry(None)
)]
#[case::geometry_a_float_third_item_is_none(
    r#"{"window_geometry": [1, 2, 3.5, 4]}"#,
    geometry(None)
)]
#[case::geometry_a_fourth_item_above_i32_max_is_none(
    r#"{"window_geometry": [1, 2, 3, 2147483648]}"#,
    geometry(None)
)]
#[case::wp_status_publish_is_kept(r#"{"wp_status": "publish"}"#, wp_status("publish"))]
#[case::wp_status_future_is_draft(r#"{"wp_status": "future"}"#, wp_status("draft"))]
#[case::wp_status_not_a_string_is_draft(r#"{"wp_status": 5}"#, default_settings())]
#[case::wp_kind_podium_is_kept(r#"{"wp_kind": "podium"}"#, wp_kind("podium"))]
#[case::wp_kind_a_stranger_is_full(r#"{"wp_kind": "half"}"#, wp_kind("full"))]
#[case::wp_kind_not_a_string_is_full(r#"{"wp_kind": 5}"#, default_settings())]
fn test_load_settings_coerces_each_field(
    #[case] raw: &str,
    #[case] expected: AppSettings,
) -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let directory = TempDir::new()?;
    let path = directory.path().join("settings.json");
    std::fs::write(&path, raw)?;

    // Act
    let (settings, warnings) = load_settings(&path);

    // Assert
    assert_eq!((settings, warnings), (expected, Vec::new()));
    Ok(())
}

#[test]
fn test_settings_avg_speed_given_a_non_finite_number_returns_the_default() {
    assert_eq!(
        (
            avg_speed_or(Some(f64::NAN), 12.0),
            avg_speed_or(Some(f64::INFINITY), 12.0),
            avg_speed_or(Some(f64::NEG_INFINITY), 12.0),
            avg_speed_or(None, 12.0),
        ),
        (12.0, 12.0, 12.0, 12.0)
    );
}

#[test]
fn test_save_settings_writes_atomically_and_creates_parents()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let directory = TempDir::new()?;
    let path = directory.path().join("RiverCrossing").join("settings.json");
    let temp_path = directory
        .path()
        .join("RiverCrossing")
        .join("settings.json.tmp");

    // Act
    save_settings(&non_default_settings(), &path)?;

    // Assert
    let text = std::fs::read_to_string(&path)?;
    let value: Value = serde_json::from_str(&text)?;
    assert_eq!(
        (
            value.as_object().map(serde_json::Map::len),
            text.starts_with("{\n  \""),
            text.ends_with("\n}\n"),
            temp_path.exists(),
        ),
        (Some(29), true, true, false)
    );
    Ok(())
}

#[test]
fn test_save_settings_overwrites_an_existing_file() -> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let directory = TempDir::new()?;
    let path = directory.path().join("settings.json");
    std::fs::write(&path, "old")?;
    let settings = non_default_settings();

    // Act
    save_settings(&settings, &path)?;

    // Assert
    let (loaded, warnings) = load_settings(&path);
    assert_eq!(
        (
            loaded,
            warnings,
            directory.path().join("settings.json.tmp").exists(),
        ),
        (settings, Vec::new(), false)
    );
    Ok(())
}

#[test]
fn test_save_settings_given_a_parent_that_is_a_file_returns_an_io_error()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let directory = TempDir::new()?;
    let blocker = directory.path().join("blocker");
    std::fs::write(&blocker, "not a directory")?;
    let path = blocker.join("settings.json");

    // Act
    let result = save_settings(&default_settings(), &path);

    // Assert
    assert!(matches!(result, Err(StoreError::Io(_))));
    Ok(())
}

#[test]
fn test_save_settings_given_a_temporary_directory_in_the_way_returns_an_io_error()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let directory = TempDir::new()?;
    let path = directory.path().join("settings.json");
    std::fs::create_dir(directory.path().join("settings.json.tmp"))?;

    // Act
    let result = save_settings(&default_settings(), &path);

    // Assert
    assert!(matches!(result, Err(StoreError::Io(_))));
    Ok(())
}

#[test]
fn test_save_settings_given_a_target_directory_returns_an_io_error()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let directory = TempDir::new()?;
    let path = directory.path().join("settings.json");
    std::fs::create_dir(&path)?;

    // Act
    let result = save_settings(&default_settings(), &path);

    // Assert
    assert!(matches!(result, Err(StoreError::Io(_))));
    Ok(())
}

#[rstest]
#[case::macos_home(
    "macos",
    &[("HOME", "/Users/ada")],
    "/Users/ada/Library/Application Support/RiverCrossing"
)]
#[case::macos_without_a_home("macos", &[], "")]
#[case::windows_local_app_data(
    "windows",
    &[("LOCALAPPDATA", "C:/Users/ada/AppData/Local")],
    "C:/Users/ada/AppData/Local/RiverCrossing"
)]
#[case::windows_without_local_app_data("windows", &[], "")]
#[case::other_xdg_config_home(
    "linux",
    &[("XDG_CONFIG_HOME", "/home/ada/.xdg"), ("HOME", "/home/ada")],
    "/home/ada/.xdg/RiverCrossing"
)]
#[case::other_an_empty_xdg_config_home_falls_back_to_the_home(
    "linux",
    &[("XDG_CONFIG_HOME", ""), ("HOME", "/home/ada")],
    "/home/ada/.config/RiverCrossing"
)]
#[case::other_home_only(
    "freebsd",
    &[("HOME", "/home/ada")],
    "/home/ada/.config/RiverCrossing"
)]
#[case::other_without_a_home("linux", &[], "")]
fn test_config_dir_follows_platformdirs(
    #[case] os: &str,
    #[case] environment: &[(&str, &str)],
    #[case] expected: &str,
) {
    let directory = config_dir_for(os, &env_from(environment));

    assert_eq!(directory, PathBuf::from(expected));
}

#[test]
fn test_settings_path_is_the_config_dir_settings_json() {
    let path = default_settings_path();
    let expected = config_dir_for(std::env::consts::OS, &|name| std::env::var(name).ok())
        .join("settings.json");

    assert_eq!(path, expected);
    assert_eq!(path.file_name(), Some(OsStr::new("settings.json")));
}

#[test]
fn test_settings_path_without_a_home_is_settings_json() {
    let path = config_dir_for("linux", &env_from(&[])).join("settings.json");

    assert_eq!(path, PathBuf::from("settings.json"));
}
