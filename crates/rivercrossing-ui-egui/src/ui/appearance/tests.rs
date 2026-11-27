// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the settings-to-`RootEnv` appearance mapping.

use craft::theme::{ContrastChoice, ModeChoice, Size, TextSize};
use pretty_assertions::assert_eq;
use rivercrossing_store::settings::{AppSettings, default_settings};

use super::{contrast, density, mode, root_env, text_size, theme};

// ----------------------------------------------------------------- mode

#[test]
fn test_appearance_mode_given_light_returns_light() {
    // Arrange, Act
    let choice = mode("light");

    // Assert
    assert_eq!(choice, ModeChoice::Light);
}

#[test]
fn test_appearance_mode_given_dark_returns_dark() {
    // Arrange, Act
    let choice = mode("dark");

    // Assert
    assert_eq!(choice, ModeChoice::Dark);
}

#[test]
fn test_appearance_mode_given_system_returns_system() {
    // Arrange, Act
    let choice = mode("system");

    // Assert
    assert_eq!(choice, ModeChoice::System);
}

#[test]
fn test_appearance_mode_given_an_unknown_spelling_returns_system() {
    // Arrange, Act
    let choice = mode("pastel");

    // Assert
    assert_eq!(choice, ModeChoice::System);
}

// ------------------------------------------------------------- contrast

#[test]
fn test_appearance_contrast_given_standard_returns_standard() {
    // Arrange, Act
    let choice = contrast("standard");

    // Assert
    assert_eq!(choice, ContrastChoice::Standard);
}

#[test]
fn test_appearance_contrast_given_high_returns_high() {
    // Arrange, Act
    let choice = contrast("high");

    // Assert
    assert_eq!(choice, ContrastChoice::High);
}

#[test]
fn test_appearance_contrast_given_system_returns_system() {
    // Arrange, Act
    let choice = contrast("system");

    // Assert
    assert_eq!(choice, ContrastChoice::System);
}

#[test]
fn test_appearance_contrast_given_an_unknown_spelling_returns_system() {
    // Arrange, Act
    let choice = contrast("max");

    // Assert
    assert_eq!(choice, ContrastChoice::System);
}

// -------------------------------------------------------------- density

#[test]
fn test_appearance_density_given_compact_returns_compact() {
    // Arrange, Act
    let choice = density("compact");

    // Assert
    assert_eq!(choice, Size::Compact);
}

#[test]
fn test_appearance_density_given_comfortable_returns_comfortable() {
    // Arrange, Act
    let choice = density("comfortable");

    // Assert
    assert_eq!(choice, Size::Comfortable);
}

#[test]
fn test_appearance_density_given_an_unknown_spelling_returns_compact() {
    // Arrange, Act
    let choice = density("wide");

    // Assert
    assert_eq!(choice, Size::Compact);
}

// ------------------------------------------------------------ text size

#[test]
fn test_appearance_text_size_given_tiny_returns_tiny() {
    // Arrange, Act
    let choice = text_size("tiny");

    // Assert
    assert_eq!(choice, TextSize::Tiny);
}

#[test]
fn test_appearance_text_size_given_small_returns_small() {
    // Arrange, Act
    let choice = text_size("small");

    // Assert
    assert_eq!(choice, TextSize::Small);
}

#[test]
fn test_appearance_text_size_given_medium_returns_medium() {
    // Arrange, Act
    let choice = text_size("medium");

    // Assert
    assert_eq!(choice, TextSize::Medium);
}

#[test]
fn test_appearance_text_size_given_large_returns_large() {
    // Arrange, Act
    let choice = text_size("large");

    // Assert
    assert_eq!(choice, TextSize::Large);
}

#[test]
fn test_appearance_text_size_given_an_unknown_spelling_returns_medium() {
    // Arrange, Act
    let choice = text_size("huge");

    // Assert
    assert_eq!(choice, TextSize::Medium);
}

// ---------------------------------------------------------------- theme

#[test]
fn test_appearance_theme_given_a_built_in_id_returns_that_id() {
    // Arrange, Act
    let id = theme("craft.studio");

    // Assert
    assert_eq!(id.as_str(), "craft.studio");
}

#[test]
fn test_appearance_theme_given_a_malformed_id_returns_the_suite_default() {
    // Arrange, Act
    let id = theme("Craft Artcraft");

    // Assert
    assert_eq!(id.as_str(), "craft.artcraft");
}

#[test]
fn test_appearance_theme_given_an_empty_id_returns_the_suite_default() {
    // Arrange, Act
    let id = theme("");

    // Assert
    assert_eq!(id.as_str(), "craft.artcraft");
}

// ------------------------------------------------------------- root_env

#[test]
fn test_appearance_root_env_reads_every_axis_from_the_settings() {
    // Arrange
    let settings = AppSettings {
        appearance: "dark".to_owned(),
        contrast: "high".to_owned(),
        density: "comfortable".to_owned(),
        text_size: "large".to_owned(),
        theme: "craft.studio".to_owned(),
        ..default_settings()
    };

    // Act
    let root = root_env(&settings);

    // Assert
    assert_eq!(
        (root.mode, root.contrast, root.density, root.text_size),
        (
            ModeChoice::Dark,
            ContrastChoice::High,
            Size::Comfortable,
            TextSize::Large,
        )
    );
    assert_eq!(root.theme_id().as_str(), "craft.studio");
}

#[test]
fn test_appearance_root_env_given_the_defaults_uses_system_compact_medium_artcraft() {
    // Arrange
    let settings = default_settings();

    // Act
    let root = root_env(&settings);

    // Assert
    assert_eq!(
        (root.mode, root.contrast, root.density, root.text_size),
        (
            ModeChoice::System,
            ContrastChoice::System,
            Size::Compact,
            TextSize::Medium,
        )
    );
    assert_eq!(root.theme_id().as_str(), "craft.artcraft");
}
