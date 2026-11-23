// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the default database location. Every per-OS case drives the
//! pure helper, so no test touches the process environment.

use std::path::{Path, PathBuf};

use pretty_assertions::assert_eq;
use rstest::rstest;

use super::{default_db_path, default_db_path_for};

/// Build an environment lookup over a fixed table of pairs.
fn env_from<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |name: &str| {
        pairs
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| (*value).to_owned())
    }
}

#[test]
fn test_paths_an_override_is_returned_verbatim() {
    let override_path = Path::new("/tmp/custom/rides.db");

    assert_eq!(
        default_db_path(Some(override_path)),
        PathBuf::from("/tmp/custom/rides.db")
    );
}

#[test]
fn test_paths_no_override_ends_at_the_database_file_name() {
    let path = default_db_path(None);

    assert_eq!(path.file_name(), Some(Path::new("rides.db").as_os_str()));
}

#[rstest]
#[case::macos_home(
    "macos",
    &[("HOME", "/Users/ada")],
    "/Users/ada/Library/Application Support/RiverCrossing/rides.db"
)]
#[case::macos_without_a_home("macos", &[], "rides.db")]
#[case::windows_local_app_data(
    "windows",
    &[("LOCALAPPDATA", "C:/Users/ada/AppData/Local")],
    "C:/Users/ada/AppData/Local/RiverCrossing/rides.db"
)]
#[case::windows_without_local_app_data("windows", &[], "rides.db")]
#[case::other_xdg_data_home(
    "linux",
    &[("XDG_DATA_HOME", "/home/ada/.xdg"), ("HOME", "/home/ada")],
    "/home/ada/.xdg/RiverCrossing/rides.db"
)]
#[case::other_an_empty_xdg_data_home_falls_back_to_the_home(
    "linux",
    &[("XDG_DATA_HOME", ""), ("HOME", "/home/ada")],
    "/home/ada/.local/share/RiverCrossing/rides.db"
)]
#[case::other_home_only(
    "freebsd",
    &[("HOME", "/home/ada")],
    "/home/ada/.local/share/RiverCrossing/rides.db"
)]
#[case::other_without_a_home("linux", &[], "rides.db")]
fn test_paths_default_db_path_for_os_resolves_the_data_directory(
    #[case] os: &str,
    #[case] environment: &[(&str, &str)],
    #[case] expected: &str,
) {
    let path = default_db_path_for(os, &env_from(environment));

    assert_eq!(path, PathBuf::from(expected));
}
