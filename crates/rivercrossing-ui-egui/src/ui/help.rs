// SPDX-License-Identifier: GPL-3.0-only

//! The bundled user guide: the embedded page, its per-user copy and the browser launch.
//!
//! The guide ships inside the binary and is written to the per-user config directory on demand, so
//! Help ▸ User Guide never depends on a file the installer forgot. The browser launch sits behind
//! [`BrowserLauncher`] so a test can assert the written path without starting a real browser.

use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests;

/// The guide page's file name inside the per-user config directory.
pub(crate) const USER_GUIDE_FILE: &str = "user-guide.html";

/// The bundled guide page.
pub(crate) const USER_GUIDE: &str = include_str!("../../../../docs/user-guide.html");

/// Open *path* with the platform's default handler.
///
/// # Errors
///
/// Return the opener's own sentence when it cannot start.
pub(crate) fn open_in_browser(path: &Path) -> Result<(), String> {
    let (program, leading) = opener_for(std::env::consts::OS);
    std::process::Command::new(program)
        .args(leading)
        .arg(path)
        .spawn()
        .map(|_child| ())
        .map_err(|error| format!("{program}: {error}"))
}

/// Return the opener program and its leading arguments for one OS name.
///
/// `open` is macOS, `xdg-open` is the freedesktop opener, and Windows routes `start` through the
/// command interpreter because `start` is a shell builtin.
pub(crate) fn opener_for(os: &str) -> (&'static str, &'static [&'static str]) {
    match os {
        "macos" => ("open", &[]),
        "windows" => ("cmd", &["/C", "start", ""]),
        _ => ("xdg-open", &[]),
    }
}

/// Write the bundled guide into *directory* and return the written path.
///
/// The app passes the directory that holds its settings file, which is the per-user config
/// directory (`default_settings_path`'s parent, `config_dir_for(...)`, in production).
///
/// # Errors
///
/// Return the directory-creation or write failure.
pub(crate) fn write_user_guide(directory: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(directory)
        .map_err(|error| format!("cannot create {}: {error}", directory.display()))?;
    let path = directory.join(USER_GUIDE_FILE);
    std::fs::write(&path, USER_GUIDE)
        .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    Ok(path)
}

/// The browser that opens the written guide.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait BrowserLauncher: std::fmt::Debug {
    /// Open *path* with the user's default handler.
    ///
    /// # Errors
    ///
    /// Return the platform opener's own sentence when it cannot start.
    fn open(&self, path: &Path) -> Result<(), String>;
}

/// The real launcher: the platform opener over [`open_in_browser`].
#[derive(Debug)]
pub(crate) struct SystemLauncher;

impl BrowserLauncher for SystemLauncher {
    fn open(&self, path: &Path) -> Result<(), String> {
        open_in_browser(path)
    }
}
