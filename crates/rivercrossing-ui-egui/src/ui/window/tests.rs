// SPDX-License-Identifier: GPL-3.0-only

use std::sync::Arc;
use std::sync::Mutex;

use craft::behavior::placement::Placement;
use craft::egui;
use craft::ui::shell::app::native_options;
use craft::window::restore::{MAIN, Store};
use rivercrossing_store::settings::AppSettings;

use super::{first_run_placement, seeded_store, spec_with_store};

/// Return the spec over an in-memory placement store, so the tests touch no user file.
fn memory_spec() -> craft::ui::shell::app::AppSpec {
    spec_with_store(Arc::new(Mutex::new(Store::in_memory())))
}

#[test]
fn spec_title_is_the_product_name() {
    // Act
    let title = memory_spec().title;

    // Assert
    assert_eq!(title, "RiverCrossing");
}

#[test]
fn spec_app_id_is_the_binary_name() {
    // Act
    let app_id = memory_spec().app_id;

    // Assert
    assert_eq!(app_id, "rivercrossing");
}

#[test]
fn spec_default_size_is_the_documented_console_minimum() {
    // Act
    let size = memory_spec().defaults.size;

    // Assert
    assert_eq!(size, [1100.0, 780.0]);
}

#[test]
fn spec_min_size_is_the_documented_console_minimum() {
    // Act
    let min = memory_spec().defaults.min;

    // Assert
    assert_eq!(min, [1100.0, 780.0]);
}

#[test]
fn spec_carries_the_placement_store() {
    // Arrange
    let store = Arc::new(Mutex::new(Store::in_memory()));

    // Act
    let spec = spec_with_store(Arc::clone(&store));

    // Assert
    assert!(Arc::ptr_eq(&spec.placement, &store));
}

#[test]
fn spec_without_a_store_argument_opens_the_app_placement_file() {
    // Act
    let spec = super::spec();

    // Assert
    assert_eq!(spec.app_id, "rivercrossing");
}

#[test]
fn native_options_carry_title_inner_and_min_size() {
    // Act
    let viewport = native_options(&memory_spec()).viewport;

    // Assert
    assert_eq!(viewport.title, Some("RiverCrossing".to_owned()));
    assert_eq!(viewport.inner_size, Some(egui::vec2(1100.0, 780.0)));
    assert_eq!(viewport.min_inner_size, Some(egui::vec2(1100.0, 780.0)));
}

#[test]
fn test_first_run_placement_uses_the_settings_geometry() {
    // Arrange
    let settings = AppSettings {
        window_geometry: Some([100, 200, 1100, 780]),
        ..AppSettings::default()
    };

    // Act
    let placement = first_run_placement(&settings);

    // Assert
    let placement = placement.expect("a sized rectangle yields a placement");
    assert_eq!(placement.position_px, Some([100, 200]));
    assert_eq!(placement.normal.pos, [100.0, 200.0]);
    assert_eq!(placement.normal.size, [1100.0, 780.0]);
    assert_eq!(placement.scale, 1.0);
}

#[test]
fn test_first_run_placement_given_no_geometry_is_none() {
    // Arrange
    let settings = AppSettings {
        window_geometry: None,
        ..AppSettings::default()
    };

    // Act
    let placement = first_run_placement(&settings);

    // Assert
    assert_eq!(placement, None);
}

#[test]
fn test_first_run_placement_given_a_zero_size_is_none() {
    // Arrange
    let settings = AppSettings {
        window_geometry: Some([10, 10, 0, 0]),
        ..AppSettings::default()
    };

    // Act
    let placement = first_run_placement(&settings);

    // Assert
    assert_eq!(placement, None);
}

#[test]
fn test_seeded_store_sets_main_from_the_settings() {
    // Arrange
    let settings = AppSettings {
        window_geometry: Some([40, 60, 1200, 800]),
        ..AppSettings::default()
    };
    let store = Store::in_memory();

    // Act
    let store = seeded_store(store, &settings);

    // Assert
    assert_eq!(
        store.get(MAIN).map(|placement| placement.normal.size),
        Some([1200.0, 800.0])
    );
}

#[test]
fn test_seeded_store_keeps_a_saved_main() {
    // Arrange
    let settings = AppSettings {
        window_geometry: Some([40, 60, 1200, 800]),
        ..AppSettings::default()
    };
    let mut store = Store::in_memory();
    store.set(MAIN, &Placement::new([900.0, 700.0], 1.0));

    // Act
    let store = seeded_store(store, &settings);

    // Assert
    assert_eq!(
        store.get(MAIN).map(|placement| placement.normal.size),
        Some([900.0, 700.0])
    );
}

#[test]
fn test_seeded_store_given_no_geometry_keeps_main_unset() {
    // Arrange
    let settings = AppSettings {
        window_geometry: None,
        ..AppSettings::default()
    };

    // Act
    let store = seeded_store(Store::in_memory(), &settings);

    // Assert
    assert_eq!(store.get(MAIN), None);
}
