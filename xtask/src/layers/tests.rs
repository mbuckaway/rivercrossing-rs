// SPDX-License-Identifier: GPL-3.0-only

use super::{LayersError, violations};

/// A workspace of domain crates that declare no forbidden dependency.
const CLEAN: &str = r#"{
  "packages": [
    {
      "name": "rivercrossing-cards",
      "manifest_path": "/work/crates/rivercrossing-cards/Cargo.toml",
      "dependencies": [
        { "name": "serde", "kind": null },
        { "name": "serde_json", "kind": "dev" }
      ]
    }
  ]
}"#;

/// A domain crate that declares `eframe` as a normal dependency.
const DOMAIN_WITH_EFRAME: &str = r#"{
  "packages": [
    {
      "name": "cards",
      "manifest_path": "/work/crates/rivercrossing-cards/Cargo.toml",
      "dependencies": [{ "name": "eframe", "kind": null }]
    }
  ]
}"#;

/// The UI crate, which owns the egui dependency.
const UI_WITH_EGUI: &str = r#"{
  "packages": [
    {
      "name": "rivercrossing-ui-egui",
      "manifest_path": "/work/crates/rivercrossing-ui-egui/Cargo.toml",
      "dependencies": [{ "name": "egui", "kind": null }]
    }
  ]
}"#;

/// The app crate, which sits under the workspace `apps/` directory.
const APP_WITH_EFRAME: &str = r#"{
  "packages": [
    {
      "name": "rivercrossing",
      "manifest_path": "/work/apps/rivercrossing/Cargo.toml",
      "dependencies": [{ "name": "eframe", "kind": null }]
    }
  ]
}"#;

/// The app crate with Windows path separators in its manifest path.
const APP_WITH_BACKSLASH_PATH: &str = r#"{
  "packages": [
    {
      "name": "rivercrossing",
      "manifest_path": "C:\\work\\apps\\rivercrossing\\Cargo.toml",
      "dependencies": [{ "name": "eframe", "kind": null }]
    }
  ]
}"#;

/// A domain crate that declares `winit` as a dev dependency.
const DOMAIN_WITH_DEV_WINIT: &str = r#"{
  "packages": [
    {
      "name": "cards",
      "manifest_path": "/work/crates/rivercrossing-cards/Cargo.toml",
      "dependencies": [{ "name": "winit", "kind": "dev" }]
    }
  ]
}"#;

/// A domain crate that declares `rfd` as a build dependency.
const DOMAIN_WITH_BUILD_RFD: &str = r#"{
  "packages": [
    {
      "name": "cards",
      "manifest_path": "/work/crates/rivercrossing-cards/Cargo.toml",
      "dependencies": [{ "name": "rfd", "kind": "build" }]
    }
  ]
}"#;

/// Two domain crates that declare forbidden dependencies out of name order.
const UNSORTED_VIOLATIONS: &str = r#"{
  "packages": [
    {
      "name": "zebra",
      "manifest_path": "/work/crates/zebra/Cargo.toml",
      "dependencies": [{ "name": "eframe", "kind": null }]
    },
    {
      "name": "alpha",
      "manifest_path": "/work/crates/alpha/Cargo.toml",
      "dependencies": [
        { "name": "egui", "kind": null },
        { "name": "winit", "kind": null }
      ]
    }
  ]
}"#;

/// A domain crate whose dependencies only resemble the forbidden names.
const SIMILAR_NAMES: &str = r#"{
  "packages": [
    {
      "name": "cards",
      "manifest_path": "/work/crates/rivercrossing-cards/Cargo.toml",
      "dependencies": [
        { "name": "egui_extras", "kind": null },
        { "name": "egui_kittest", "kind": "dev" },
        { "name": "egui-winit", "kind": null }
      ]
    }
  ]
}"#;

/// A domain crate with no `dependencies` key at all.
const WITHOUT_DEPENDENCIES: &str = r#"{
  "packages": [
    {
      "name": "rivercrossing-wordpress",
      "manifest_path": "/work/crates/rivercrossing-wordpress/Cargo.toml"
    }
  ]
}"#;

/// A workspace whose member declares `eframe` beside a third-party package that also does.
const MEMBER_AND_THIRD_PARTY: &str = r#"{
  "workspace_members": ["path+file:///work/crates/rivercrossing-cards#0.1.0"],
  "packages": [
    {
      "id": "path+file:///work/crates/rivercrossing-cards#0.1.0",
      "name": "rivercrossing-cards",
      "manifest_path": "/work/crates/rivercrossing-cards/Cargo.toml",
      "dependencies": [{ "name": "eframe", "kind": null }]
    },
    {
      "id": "git+https://github.com/storytold/egui.git?rev=7e1c30d#eframe@0.37.0",
      "name": "eframe",
      "manifest_path": "/home/user/.cargo/git/checkouts/egui/crates/eframe/Cargo.toml",
      "dependencies": [
        { "name": "egui", "kind": null },
        { "name": "winit", "kind": null }
      ]
    }
  ]
}"#;

#[test]
fn test_layers_clean_workspace_returns_no_violations() {
    // Arrange
    let metadata = CLEAN;

    // Act
    let result = violations(metadata);

    // Assert
    assert_eq!(result, Ok(Vec::new()));
}

#[test]
fn test_layers_domain_dependency_on_eframe_is_a_violation() {
    // Arrange
    let metadata = DOMAIN_WITH_EFRAME;

    // Act
    let result = violations(metadata);

    // Assert
    assert_eq!(result, Ok(vec!["cards -> eframe".to_owned()]));
}

#[test]
fn test_layers_ui_package_dependency_on_egui_is_allowed() {
    // Arrange
    let metadata = UI_WITH_EGUI;

    // Act
    let result = violations(metadata);

    // Assert
    assert_eq!(result, Ok(Vec::new()));
}

#[test]
fn test_layers_app_package_dependency_on_eframe_is_allowed() {
    // Arrange
    let metadata = APP_WITH_EFRAME;

    // Act
    let result = violations(metadata);

    // Assert
    assert_eq!(result, Ok(Vec::new()));
}

#[test]
fn test_layers_app_package_with_backslash_path_is_allowed() {
    // Arrange
    let metadata = APP_WITH_BACKSLASH_PATH;

    // Act
    let result = violations(metadata);

    // Assert
    assert_eq!(result, Ok(Vec::new()));
}

#[test]
fn test_layers_dev_dependency_on_winit_is_a_violation() {
    // Arrange
    let metadata = DOMAIN_WITH_DEV_WINIT;

    // Act
    let result = violations(metadata);

    // Assert
    assert_eq!(result, Ok(vec!["cards -> winit".to_owned()]));
}

#[test]
fn test_layers_build_dependency_on_rfd_is_a_violation() {
    // Arrange
    let metadata = DOMAIN_WITH_BUILD_RFD;

    // Act
    let result = violations(metadata);

    // Assert
    assert_eq!(result, Ok(vec!["cards -> rfd".to_owned()]));
}

#[test]
fn test_layers_violations_are_sorted() {
    // Arrange
    let metadata = UNSORTED_VIOLATIONS;

    // Act
    let result = violations(metadata);

    // Assert
    assert_eq!(
        result,
        Ok(vec![
            "alpha -> egui".to_owned(),
            "alpha -> winit".to_owned(),
            "zebra -> eframe".to_owned(),
        ])
    );
}

#[test]
fn test_layers_similar_dependency_names_are_not_flagged() {
    // Arrange
    let metadata = SIMILAR_NAMES;

    // Act
    let result = violations(metadata);

    // Assert
    assert_eq!(result, Ok(Vec::new()));
}

#[test]
fn test_layers_package_without_dependencies_is_allowed() {
    // Arrange
    let metadata = WITHOUT_DEPENDENCIES;

    // Act
    let result = violations(metadata);

    // Assert
    assert_eq!(result, Ok(Vec::new()));
}

#[test]
fn test_layers_non_workspace_package_is_ignored() {
    // Arrange
    let metadata = MEMBER_AND_THIRD_PARTY;

    // Act
    let result = violations(metadata);

    // Assert
    assert_eq!(result, Ok(vec!["rivercrossing-cards -> eframe".to_owned()]));
}

#[test]
fn test_layers_malformed_json_returns_invalid_json() {
    // Arrange
    let metadata = r#"{ "packages": [ }"#;

    // Act
    let result = violations(metadata);

    // Assert
    assert!(matches!(result, Err(LayersError::InvalidJson(_))));
}

#[test]
fn test_layers_metadata_without_packages_returns_missing_packages() {
    // Arrange
    let metadata = r#"{ "workspace_root": "/work" }"#;

    // Act
    let result = violations(metadata);

    // Assert
    assert_eq!(result, Err(LayersError::MissingPackages));
}
