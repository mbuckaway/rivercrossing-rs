// SPDX-License-Identifier: GPL-3.0-only

use craft::egui;
use craft::ui::shell::app::native_options;

use super::spec;

#[test]
fn spec_title_is_the_product_name() {
    // Act
    let title = spec().title;

    // Assert
    assert_eq!(title, "RiverCrossing");
}

#[test]
fn spec_app_id_is_the_binary_name() {
    // Act
    let app_id = spec().app_id;

    // Assert
    assert_eq!(app_id, "rivercrossing");
}

#[test]
fn spec_default_size_is_the_documented_console_minimum() {
    // Act
    let size = spec().defaults.size;

    // Assert
    assert_eq!(size, [1100.0, 780.0]);
}

#[test]
fn spec_min_size_is_the_documented_console_minimum() {
    // Act
    let min = spec().defaults.min;

    // Assert
    assert_eq!(min, [1100.0, 780.0]);
}

#[test]
fn native_options_carry_title_inner_and_min_size() {
    // Act
    let viewport = native_options(&spec()).viewport;

    // Assert
    assert_eq!(viewport.title, Some("RiverCrossing".to_owned()));
    assert_eq!(viewport.inner_size, Some(egui::vec2(1100.0, 780.0)));
    assert_eq!(viewport.min_inner_size, Some(egui::vec2(1100.0, 780.0)));
}
