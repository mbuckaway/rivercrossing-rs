// SPDX-License-Identifier: GPL-3.0-only

//! The default database location.

use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests;

/// Return the database path to open.
///
/// An override is returned verbatim. Otherwise the path follows the platform
/// data directory: `$HOME/Library/Application Support/RiverCrossing/rides.db`
/// on macOS, `%LOCALAPPDATA%\RiverCrossing\rides.db` on Windows, and
/// `$XDG_DATA_HOME/RiverCrossing/rides.db` or
/// `$HOME/.local/share/RiverCrossing/rides.db` elsewhere. With no home to
/// resolve, the fallback is `rides.db` in the working directory.
#[must_use]
pub fn default_db_path(override_path: Option<&Path>) -> PathBuf {
    if let Some(path) = override_path {
        return path.to_path_buf();
    }
    default_db_path_for(std::env::consts::OS, &|name| std::env::var(name).ok())
}

/// Return the default database path for one OS name and one environment lookup.
///
/// The OS name is [`std::env::consts::OS`] in production. The lookup hides
/// [`std::env::var`] so a test drives every platform without touching the
/// process environment.
pub(crate) fn default_db_path_for(os: &str, env: &dyn Fn(&str) -> Option<String>) -> PathBuf {
    let directory = match os {
        "macos" => env("HOME").map(|home| {
            PathBuf::from(home)
                .join("Library")
                .join("Application Support")
        }),
        "windows" => env("LOCALAPPDATA").map(PathBuf::from),
        _ => env("XDG_DATA_HOME")
            .filter(|directory| !directory.is_empty())
            .map(PathBuf::from)
            .or_else(|| env("HOME").map(|home| PathBuf::from(home).join(".local").join("share"))),
    };
    directory.map_or_else(
        || PathBuf::from("rides.db"),
        |directory| directory.join("RiverCrossing").join("rides.db"),
    )
}
