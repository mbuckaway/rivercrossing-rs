// SPDX-License-Identifier: GPL-3.0-only

//! The atomic swap that both CSV exporters land their bytes through.
//!
//! The writer stages the payload in a same-directory `.tmp` sibling and renames
//! it over the destination, so a failed export never truncates a previous file
//! (R-21).

use std::path::{Path, PathBuf};

use crate::error::CsvIoError;

/// Write *bytes* to *path* through a same-directory temporary file.
///
/// # Errors
///
/// Return [`CsvIoError::Io`] when *path* has no file name, when the temporary
/// file cannot be written, or when the rename fails.
pub(crate) fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> Result<(), CsvIoError> {
    write_bytes_atomic_with(path, bytes, |from, to| std::fs::rename(from, to))
}

/// Write *bytes* to a sibling temporary file, then replace *path* with *rename*.
///
/// The rename is the seam: a test injects a failing one and proves that a
/// pre-existing file survives untouched. Production passes the real rename, so
/// one implementation serves both. A write or a rename failure removes the
/// temporary file (best effort) and leaves the destination alone.
///
/// # Errors
///
/// Return [`CsvIoError::Io`] when *path* has no file name, when the temporary
/// file cannot be written, or when *rename* fails.
pub(crate) fn write_bytes_atomic_with(
    path: &Path,
    bytes: &[u8],
    rename: impl Fn(&Path, &Path) -> std::io::Result<()>,
) -> Result<(), CsvIoError> {
    let temporary = temp_path(path)?;
    if let Err(error) = std::fs::write(&temporary, bytes) {
        remove_temporary(&temporary);
        return Err(CsvIoError::Io(error));
    }
    if let Err(error) = rename(&temporary, path) {
        remove_temporary(&temporary);
        return Err(CsvIoError::Io(error));
    }
    Ok(())
}

/// Return *path*'s same-directory temporary sibling: its name plus `.tmp`.
fn temp_path(path: &Path) -> Result<PathBuf, CsvIoError> {
    let Some(name) = path.file_name() else {
        return Err(CsvIoError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "the export path has no file name",
        )));
    };
    let mut temporary = name.to_os_string();
    temporary.push(".tmp");
    Ok(path.with_file_name(temporary))
}

/// Remove the staged temporary file after a failed write or rename.
fn remove_temporary(temporary: &Path) {
    // Best effort: the caller already reports the write or rename failure.
    std::fs::remove_file(temporary).ok();
}

#[cfg(test)]
mod tests;
