// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the settings dialog's form values and their validation.

#![expect(
    clippy::float_cmp,
    reason = "the asserted speeds are exact literals that the form copies unchanged or floors to 1.0"
)]

use pretty_assertions::assert_eq;
use rivercrossing_store::settings::{AppSettings, default_settings};
use rstest::rstest;

use super::{FormValues, appearance_for_radio, apply, from_settings};

// ------------------------------------------------------------- fixtures

/// Return a form holding every valid value.
fn valid_form() -> FormValues {
    FormValues {
        appearance: "dark".to_owned(),
        sound_on: false,
        show_total_times: true,
        show_lap_time: false,
        verbose_logging: false,
        avg_speed_kmh: 17.5,
    }
}

/// Return settings carrying a non-default value in every dialogless field.
fn decorated_settings() -> AppSettings {
    AppSettings {
        zoom_percent: 130,
        splitter_sash: Some(320),
        window_geometry: Some([10, 20, 304, 404]),
        sim_riders: 150,
        sim_teams: 30,
        sim_solo: 10,
        sim_laps: 2,
        sim_interval: 60,
        sim_short_laps: 2,
        sim_lapped: 1,
        sim_team_stop: 3,
        publish_show_times: true,
        publish_laps_board: false,
        publish_time_board: true,
        publish_full_field: false,
        publish_all_cards: false,
        wp_url: "https://blog.example.com".to_owned(),
        wp_username: "race-ops".to_owned(),
        wp_password: "example-value".to_owned(),
        wp_parent: "2026 Results".to_owned(),
        wp_status: "publish".to_owned(),
        wp_kind: "podium".to_owned(),
        show_dns_riders: false,
        ..default_settings()
    }
}

// --------------------------------------------------------- from_settings

#[test]
fn test_settings_form_from_settings_given_a_stored_setting_copies_the_six_fields() {
    // Arrange
    let settings = AppSettings {
        appearance: "light".to_owned(),
        sound_on: false,
        show_total_times: true,
        show_lap_time: true,
        verbose_logging: false,
        avg_speed_kmh: 9.5,
        ..default_settings()
    };

    // Act
    let values = from_settings(&settings);

    // Assert
    assert_eq!(
        values,
        FormValues {
            appearance: "light".to_owned(),
            sound_on: false,
            show_total_times: true,
            show_lap_time: true,
            verbose_logging: false,
            avg_speed_kmh: 9.5,
        }
    );
}

#[test]
fn test_settings_form_from_settings_given_the_defaults_reads_system_and_sound() {
    // Arrange
    let settings = default_settings();

    // Act
    let values = from_settings(&settings);

    // Assert
    assert_eq!(values.appearance, "system");
    assert_eq!(values.sound_on, true);
    assert_eq!(values.show_total_times, false);
    assert_eq!(values.show_lap_time, true);
    assert_eq!(values.verbose_logging, true);
    assert_eq!(values.avg_speed_kmh, 12.0);
}

// ---------------------------------------------------------------- apply

#[test]
fn test_settings_form_apply_given_a_valid_form_copies_the_six_fields() {
    // Arrange
    let mut settings = default_settings();

    // Act
    let messages = apply(&valid_form(), &mut settings);

    // Assert
    assert_eq!(messages, Vec::<String>::new());
    assert_eq!(settings.appearance, "dark");
    assert_eq!(settings.sound_on, false);
    assert_eq!(settings.show_total_times, true);
    assert_eq!(settings.show_lap_time, false);
    assert_eq!(settings.verbose_logging, false);
    assert_eq!(settings.avg_speed_kmh, 17.5);
}

#[test]
fn test_settings_form_apply_given_a_valid_form_leaves_the_dialogless_fields_alone() {
    // Arrange
    let mut settings = decorated_settings();
    let before = decorated_settings();

    // Act
    let messages = apply(&valid_form(), &mut settings);

    // Assert
    assert_eq!(messages, Vec::<String>::new());
    assert_eq!(settings.zoom_percent, before.zoom_percent);
    assert_eq!(settings.splitter_sash, before.splitter_sash);
    assert_eq!(settings.window_geometry, before.window_geometry);
    assert_eq!(settings.sim_riders, before.sim_riders);
    assert_eq!(settings.sim_interval, before.sim_interval);
    assert_eq!(settings.publish_show_times, before.publish_show_times);
    assert_eq!(settings.publish_all_cards, before.publish_all_cards);
    assert_eq!(settings.wp_url, before.wp_url);
    assert_eq!(settings.wp_status, before.wp_status);
    assert_eq!(settings.wp_kind, before.wp_kind);
    assert_eq!(settings.show_dns_riders, before.show_dns_riders);
}

#[rstest]
#[case("system")]
#[case("light")]
#[case("dark")]
fn test_settings_form_apply_given_a_whitelisted_appearance_keeps_it(#[case] appearance: &str) {
    // Arrange
    let mut settings = default_settings();
    let form = FormValues {
        appearance: appearance.to_owned(),
        ..valid_form()
    };

    // Act
    let messages = apply(&form, &mut settings);

    // Assert
    assert_eq!(messages, Vec::<String>::new());
    assert_eq!(settings.appearance, appearance);
}

#[rstest]
#[case("")]
#[case("System")]
#[case("blue")]
fn test_settings_form_apply_given_an_unknown_appearance_refuses_and_keeps_the_stored_one(
    #[case] appearance: &str,
) {
    // Arrange
    let mut settings = default_settings();
    let form = FormValues {
        appearance: appearance.to_owned(),
        ..valid_form()
    };

    // Act
    let messages = apply(&form, &mut settings);

    // Assert
    assert_eq!(
        messages,
        vec!["Appearance must be system, light or dark".to_owned()]
    );
    assert_eq!(settings.appearance, "system");
}

#[test]
fn test_settings_form_apply_given_the_speed_floor_accepts_it() {
    // Arrange
    let mut settings = default_settings();
    let form = FormValues {
        avg_speed_kmh: 1.0,
        ..valid_form()
    };

    // Act
    let messages = apply(&form, &mut settings);

    // Assert
    assert_eq!(messages, Vec::<String>::new());
    assert_eq!(settings.avg_speed_kmh, 1.0);
}

#[test]
fn test_settings_form_apply_given_a_speed_below_the_floor_floors_it_and_refuses() {
    // Arrange
    let mut settings = default_settings();
    let form = FormValues {
        avg_speed_kmh: 0.5,
        ..valid_form()
    };

    // Act
    let messages = apply(&form, &mut settings);

    // Assert
    assert_eq!(
        messages,
        vec!["Average rider speed must be at least 1 km/h".to_owned()]
    );
    assert_eq!(settings.avg_speed_kmh, 1.0);
}

#[test]
fn test_settings_form_apply_given_a_negative_speed_floors_it_and_refuses() {
    // Arrange
    let mut settings = default_settings();
    let form = FormValues {
        avg_speed_kmh: -5.0,
        ..valid_form()
    };

    // Act
    let messages = apply(&form, &mut settings);

    // Assert
    assert_eq!(
        messages,
        vec!["Average rider speed must be at least 1 km/h".to_owned()]
    );
    assert_eq!(settings.avg_speed_kmh, 1.0);
}

#[rstest]
#[case(f64::NAN)]
#[case(f64::INFINITY)]
#[case(f64::NEG_INFINITY)]
fn test_settings_form_apply_given_a_non_finite_speed_refuses_and_keeps_the_stored_one(
    #[case] speed: f64,
) {
    // Arrange
    let mut settings = AppSettings {
        avg_speed_kmh: 12.0,
        ..default_settings()
    };
    let form = FormValues {
        avg_speed_kmh: speed,
        ..valid_form()
    };

    // Act
    let messages = apply(&form, &mut settings);

    // Assert
    assert_eq!(
        messages,
        vec!["Average rider speed must be a number".to_owned()]
    );
    assert_eq!(settings.avg_speed_kmh, 12.0);
}

#[test]
fn test_settings_form_apply_given_two_bad_fields_names_both() {
    // Arrange
    let mut settings = default_settings();
    let form = FormValues {
        appearance: "blue".to_owned(),
        avg_speed_kmh: 0.0,
        ..valid_form()
    };

    // Act
    let messages = apply(&form, &mut settings);

    // Assert
    assert_eq!(
        messages,
        vec![
            "Appearance must be system, light or dark".to_owned(),
            "Average rider speed must be at least 1 km/h".to_owned(),
        ]
    );
    assert_eq!(settings.sound_on, false);
}

// ------------------------------------------------- appearance_for_radio

#[test]
fn test_settings_form_appearance_for_radio_given_light_returns_light() {
    // Arrange, Act
    let appearance = appearance_for_radio(true, false);

    // Assert
    assert_eq!(appearance, "light");
}

#[test]
fn test_settings_form_appearance_for_radio_given_dark_returns_dark() {
    // Arrange, Act
    let appearance = appearance_for_radio(false, true);

    // Assert
    assert_eq!(appearance, "dark");
}

#[test]
fn test_settings_form_appearance_for_radio_given_neither_returns_system() {
    // Arrange, Act
    let appearance = appearance_for_radio(false, false);

    // Assert
    assert_eq!(appearance, "system");
}

#[test]
fn test_settings_form_appearance_for_radio_given_both_returns_light() {
    // Arrange: the dialog checks exactly one radio; Light wins if both read set.
    // Act
    let appearance = appearance_for_radio(true, true);

    // Assert
    assert_eq!(appearance, "light");
}
