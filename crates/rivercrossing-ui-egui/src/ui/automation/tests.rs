// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the pure automation helpers: the selector and the write root.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use craft::egui::{Rect, pos2};

use super::path::{write_root, write_screenshot};
use super::selector::{Candidate, Selector, resolve};

/// The bounds of the start button in these tests.
fn button_bounds() -> Rect {
    Rect::from_min_max(pos2(10.0, 20.0), pos2(110.0, 60.0))
}

/// The bounds of the plate field in these tests.
fn input_bounds() -> Rect {
    Rect::from_min_max(pos2(10.0, 70.0), pos2(210.0, 100.0))
}

/// A candidate node with `role`, `label` and `bounds`.
fn candidate(role: &str, label: Option<&str>, bounds: Rect) -> Candidate {
    Candidate {
        role: role.to_owned(),
        label: label.map(str::to_owned),
        bounds,
    }
}

#[test]
fn test_selector_given_an_action_resolves_the_start_button() {
    // Arrange
    let candidates = [candidate("Button", Some("Start Ride"), button_bounds())];
    let selector = Selector::Action {
        action: "mi_start_ride".to_owned(),
    };

    // Act
    let resolved = resolve(&selector, &candidates);

    // Assert
    assert_eq!(resolved, Ok(button_bounds()));
}

#[test]
fn test_selector_given_an_action_resolves_its_label_not_a_lookalike() {
    // Arrange
    let lookalike = Rect::from_min_max(pos2(0.0, 0.0), pos2(10.0, 10.0));
    let candidates = [
        candidate("Button", Some("Start Ride 2"), lookalike),
        candidate("Button", Some("Start Ride"), button_bounds()),
    ];
    let selector = Selector::Action {
        action: "mi_start_ride".to_owned(),
    };

    // Act
    let resolved = resolve(&selector, &candidates);

    // Assert
    assert_eq!(resolved, Ok(button_bounds()));
}

#[test]
fn test_selector_given_the_stop_action_resolves_the_button() {
    // Arrange
    let candidates = [candidate("Button", Some("Stop Ride"), button_bounds())];
    let selector = Selector::Action {
        action: "mi_stop_ride".to_owned(),
    };

    // Act
    let resolved = resolve(&selector, &candidates);

    // Assert
    assert_eq!(resolved, Ok(button_bounds()));
}

#[test]
fn test_selector_given_an_unknown_action_errs() {
    // Arrange
    let selector = Selector::Action {
        action: "nope".to_owned(),
    };

    // Act
    let resolved = resolve(&selector, &[]);

    // Assert
    assert_eq!(resolved, Err("unknown action `nope`".to_owned()));
}

#[test]
fn test_selector_given_role_and_label_matches_the_role_case_insensitively() {
    // Arrange
    let candidates = [candidate("Button", Some("Plate"), input_bounds())];
    let selector = Selector::RoleAndLabel {
        role: "button".to_owned(),
        label: "Plate".to_owned(),
        window: None,
    };

    // Act
    let resolved = resolve(&selector, &candidates);

    // Assert
    assert_eq!(resolved, Ok(input_bounds()));
}

#[test]
fn test_selector_given_a_label_mismatch_errs() {
    // Arrange
    let candidates = [candidate("Button", Some("Start Ride"), button_bounds())];
    let selector = Selector::RoleAndLabel {
        role: "Button".to_owned(),
        label: "Plate".to_owned(),
        window: None,
    };

    // Act
    let resolved = resolve(&selector, &candidates);

    // Assert
    assert_eq!(
        resolved,
        Err("no node matches role `Button` and label `Plate`".to_owned())
    );
}

#[test]
fn test_selector_given_a_candidate_without_a_label_errs() {
    // Arrange
    let candidates = [candidate("Button", None, button_bounds())];
    let selector = Selector::RoleAndLabel {
        role: "Button".to_owned(),
        label: "Start Ride".to_owned(),
        window: None,
    };

    // Act
    let resolved = resolve(&selector, &candidates);

    // Assert
    assert_eq!(
        resolved,
        Err("no node matches role `Button` and label `Start Ride`".to_owned())
    );
}

#[test]
fn test_selector_given_duplicates_without_a_window_errs() {
    // Arrange
    let candidates = [
        candidate("Button", Some("Start Ride"), button_bounds()),
        candidate("Button", Some("Start Ride"), input_bounds()),
    ];
    let selector = Selector::RoleAndLabel {
        role: "Button".to_owned(),
        label: "Start Ride".to_owned(),
        window: None,
    };

    // Act
    let resolved = resolve(&selector, &candidates);

    // Assert
    assert_eq!(
        resolved,
        Err(
            "2 nodes match role `Button` and label `Start Ride`; a window scope is required"
                .to_owned()
        )
    );
}

#[test]
fn test_selector_given_a_window_scope_errs() {
    // Arrange
    let candidates = [candidate("Button", Some("Start Ride"), button_bounds())];
    let selector = Selector::RoleAndLabel {
        role: "Button".to_owned(),
        label: "Start Ride".to_owned(),
        window: Some("main".to_owned()),
    };

    // Act
    let resolved = resolve(&selector, &candidates);

    // Assert
    assert_eq!(
        resolved,
        Err("window-scoped selectors are not supported at EPIC-10 (window `main`)".to_owned())
    );
}

#[test]
fn test_selector_given_no_candidates_errs() {
    // Arrange
    let selector = Selector::RoleAndLabel {
        role: "TextInput".to_owned(),
        label: "Plate".to_owned(),
        window: None,
    };

    // Act
    let resolved = resolve(&selector, &[]);

    // Assert
    assert_eq!(
        resolved,
        Err("no node matches role `TextInput` and label `Plate`".to_owned())
    );
}

/// A unique base directory under the system temporary directory, removed on drop.
struct TempTree {
    base: PathBuf,
}

impl TempTree {
    /// Create a base directory unique to this process and test.
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let serial = NEXT.fetch_add(1, Ordering::Relaxed);
        let base = std::env::temp_dir().join(format!(
            "rivercrossing-path-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir_all(&base).expect("create the temporary base directory");
        Self { base }
    }

    /// The canonical base directory.
    fn base(&self) -> PathBuf {
        self.base.canonicalize().expect("canonicalise the base")
    }

    /// The canonical write root, `base/root`.
    fn root(&self) -> PathBuf {
        let root = self.base.join("root");
        fs::create_dir_all(&root).expect("create the write root");
        root.canonicalize().expect("canonicalise the write root")
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.base).ok();
    }
}

#[test]
fn test_path_writes_inside_the_root() {
    // Arrange
    let tree = TempTree::new();
    let root = tree.root();

    // Act
    let written = write_screenshot(&root, "shot.png", b"png");

    // Assert
    assert_eq!(written, Ok(root.join("shot.png")));
    assert_eq!(fs::read(root.join("shot.png")).ok(), Some(b"png".to_vec()));
}

#[test]
fn test_path_rejects_parent_escape() {
    // Arrange
    let tree = TempTree::new();
    let root = tree.root();

    // Act
    let written = write_screenshot(&root, "../escape.png", b"png");

    // Assert
    assert_eq!(
        written,
        Err("screenshot path `../escape.png` must stay inside the control directory".to_owned())
    );
    assert!(!tree.base().join("escape.png").exists());
}

#[test]
fn test_path_rejects_absolute() {
    // Arrange
    let tree = TempTree::new();
    let root = tree.root();

    // Act
    let written = write_screenshot(&root, "/tmp/escape.png", b"png");

    // Assert
    assert_eq!(
        written,
        Err("screenshot path `/tmp/escape.png` must be relative".to_owned())
    );
}

#[test]
fn test_path_rejects_a_symlink_escape() {
    // Arrange
    let tree = TempTree::new();
    let root = tree.root();
    std::os::unix::fs::symlink(tree.base(), root.join("link"))
        .expect("create the escaping symlink");

    // Act
    let written = write_screenshot(&root, "link/escape.png", b"png");

    // Assert
    assert_eq!(
        written,
        Err("screenshot path `link/escape.png` escapes the control directory".to_owned())
    );
    assert!(!tree.base().join("escape.png").exists());
}

#[test]
fn test_path_rejects_a_symlinked_file() {
    // Arrange
    let tree = TempTree::new();
    let root = tree.root();
    fs::write(tree.base().join("target.png"), b"png").expect("write the link target");
    std::os::unix::fs::symlink(tree.base().join("target.png"), root.join("evil.png"))
        .expect("create the symlinked file");

    // Act
    let written = write_screenshot(&root, "evil.png", b"png");

    // Assert
    assert_eq!(
        written,
        Err("screenshot path `evil.png` is a symlink".to_owned())
    );
}

#[test]
fn test_path_rejects_an_empty_name() {
    // Arrange
    let tree = TempTree::new();
    let root = tree.root();

    // Act
    let written = write_screenshot(&root, "", b"png");

    // Assert
    assert_eq!(written, Err("screenshot path is empty".to_owned()));
}

#[test]
fn test_path_reports_a_write_failure() {
    // Arrange
    let tree = TempTree::new();
    let root = tree.root();
    fs::create_dir(root.join("dir")).expect("create a directory at the target name");

    // Act
    let written = write_screenshot(&root, "dir", b"png");

    // Assert
    assert!(
        matches!(&written, Err(message) if message.starts_with("cannot write `dir`")),
        "the write failure names the path: {written:?}"
    );
}

#[test]
fn test_path_write_root_is_the_current_directory_target_control() {
    // Act
    let root = write_root();

    // Assert
    let root = root.expect("the write root is created");
    assert!(root.is_absolute(), "the write root is absolute: {root:?}");
    assert!(root.ends_with(Path::new("target").join("control")));
    assert!(root.is_dir(), "the write root exists: {root:?}");
    remove_write_root(&root);
}

/// Remove the temporary write root and its `target` parent when empty.
fn remove_write_root(root: &Path) {
    fs::remove_dir(root).ok();
    if let Some(parent) = root.parent() {
        fs::remove_dir(parent).ok();
    }
}
