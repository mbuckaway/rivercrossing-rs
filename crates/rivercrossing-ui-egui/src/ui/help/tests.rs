// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the bundled user guide: its content, its write and the platform opener.

use rstest::rstest;

use super::{USER_GUIDE, USER_GUIDE_FILE, opener_for, write_user_guide};

/// The chapter and appendix anchors the guide must carry.
const GUIDE_ANCHORS: [&str; 13] = [
    "getting-started",
    "console",
    "ride-setup",
    "riders-and-teams",
    "recording-crossings",
    "corrections-and-voids",
    "cards-and-dnf",
    "stopping-finishing-reopening",
    "standings-and-results",
    "exports-and-publishing",
    "settings-and-appearance",
    "appendix-keyboard-shortcuts",
    "appendix-troubleshooting",
];

/// Return the anchors in *guide* that are missing as an `id` attribute.
fn missing_anchors(guide: &str, anchors: &[&str]) -> Vec<String> {
    anchors
        .iter()
        .filter(|anchor| !guide.contains(&format!("id=\"{anchor}\"")))
        .map(|anchor| (*anchor).to_owned())
        .collect()
}

/// Return whether *guide* references no external asset.
fn has_external_asset(guide: &str) -> bool {
    guide.contains("http://")
        || guide.contains("https://")
        || guide.contains("<script")
        || guide.contains("src=\"")
}

#[test]
fn test_embedded_guide_is_a_self_contained_page_with_every_anchor() {
    // Assert
    assert_eq!(
        missing_anchors(USER_GUIDE, &GUIDE_ANCHORS),
        Vec::<String>::new()
    );
    assert!(USER_GUIDE.starts_with("<!DOCTYPE html>"));
    assert!(USER_GUIDE.trim_end().ends_with("</html>"));
    assert!(USER_GUIDE.contains("<style>"));
    assert!(USER_GUIDE.contains("prefers-color-scheme: dark"));
    assert!(!has_external_asset(USER_GUIDE));
}

#[test]
fn test_user_guide_file_name_is_the_browser_page_name() {
    // Assert
    assert_eq!(USER_GUIDE_FILE, "user-guide.html");
}

#[test]
fn test_write_user_guide_writes_the_embedded_page_under_the_config_dir() -> Result<(), String> {
    // Arrange: the per-user directory shape that `config_dir_for` builds on macOS.
    let home = tempfile::tempdir().map_err(|error| error.to_string())?;
    let home_text = home.path().display().to_string();
    let config = rivercrossing_store::paths::config_dir_for("macos", &|name| {
        (name == "HOME").then(|| home_text.clone())
    });

    // Act
    let path = write_user_guide(&config)?;

    // Assert
    assert_eq!(
        config,
        home.path()
            .join("Library")
            .join("Application Support")
            .join("RiverCrossing")
    );
    assert_eq!(path, config.join(USER_GUIDE_FILE));
    let written = std::fs::read(&path).map_err(|error| error.to_string())?;
    assert_eq!(written, USER_GUIDE.as_bytes());
    Ok(())
}

#[test]
fn test_write_user_guide_given_a_file_path_refuses() -> Result<(), String> {
    // Arrange: a directory that cannot be created because a file holds its name.
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let blocked = directory.path().join("blocked");
    std::fs::write(&blocked, "not a directory").map_err(|error| error.to_string())?;
    let expected_prefix = format!("cannot create {}", blocked.display());

    // Act
    let Err(message) = write_user_guide(&blocked) else {
        return Err("the guide was written into a file path".to_owned());
    };

    // Assert
    assert!(
        message.starts_with(&expected_prefix),
        "unexpected message: {message}"
    );
    Ok(())
}

#[test]
fn test_write_user_guide_given_the_page_name_taken_by_a_directory_refuses() -> Result<(), String> {
    // Arrange: the directory exists, but the page's own name is held by a directory.
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let page = directory.path().join(USER_GUIDE_FILE);
    std::fs::create_dir(&page).map_err(|error| error.to_string())?;
    let expected_prefix = format!("cannot write {}", page.display());

    // Act
    let Err(message) = write_user_guide(directory.path()) else {
        return Err("the guide was written over a directory".to_owned());
    };

    // Assert
    assert!(
        message.starts_with(&expected_prefix),
        "unexpected message: {message}"
    );
    Ok(())
}

#[rstest]
#[case("macos", "open", &[][..])]
#[case("linux", "xdg-open", &[][..])]
#[case("freebsd", "xdg-open", &[][..])]
#[case("windows", "cmd", &["/C", "start", ""][..])]
fn test_opener_for_names_the_platform_command(
    #[case] os: &str,
    #[case] program: &str,
    #[case] leading: &[&str],
) {
    // Act
    let opener = opener_for(os);

    // Assert
    assert_eq!(opener, (program, leading));
}
