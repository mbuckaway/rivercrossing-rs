// SPDX-License-Identifier: GPL-3.0-only

//! The results-report entry point: payload, section plan, pages, bytes, file.
//!
//! The module stays thin. It wires the pure page model to the craft-pdf writer
//! and lands the bytes atomically, so a failed export never truncates a
//! previous report.

use std::path::{Path, PathBuf};

use rivercrossing_htmlexport::payload::build_payload;
use rivercrossing_htmlexport::sections::sections;
use rivercrossing_htmlexport::types::ExportInput;
use time::OffsetDateTime;

use crate::error::PdfError;
use crate::layout::report_pages;
use crate::writer;

/// One report render: the export input, the paper choice, the pinned timestamp
/// and the organizer logo.
#[derive(Debug)]
pub struct PdfInput {
    /// Everything the payload and the section plan need.
    pub export: ExportInput,
    /// Whether the page is Letter instead of A4.
    pub letter: bool,
    /// The caller's pinned creation timestamp.
    pub created_at: OffsetDateTime,
    /// The organizer logo's PNG bytes.
    pub logo_png: Option<Vec<u8>>,
}

/// Render *input* to the PDF at *path*.
///
/// # Errors
///
/// Return [`PdfError::Logo`] when the logo is set but is not a PNG,
/// [`PdfError::Layout`] when the export input cannot become a payload or the
/// engine refuses an operation's geometry, [`PdfError::Write`] when
/// serialization fails, and [`PdfError::Io`] when the file cannot be written.
pub fn render(input: &PdfInput, path: &Path) -> Result<(), PdfError> {
    let payload =
        build_payload(&input.export).map_err(|error| PdfError::Layout(error.to_string()))?;
    let plan = sections(&payload, &input.export.placed);
    let pages = report_pages(&payload, &plan, input.letter, input.logo_png.as_deref())?;
    let bytes = writer::write(&pages, &payload.event.title, input.created_at)?;
    write_bytes_atomic(path, &bytes)?;
    Ok(())
}

/// Write *bytes* to *path* through a same-directory temporary file.
fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    write_bytes_atomic_with(path, bytes, |from, to| std::fs::rename(from, to))
}

/// Write *bytes* to a sibling temporary file, then replace *path* with *rename*.
///
/// The rename is the seam: a test injects a failing one and proves that a
/// pre-existing report survives untouched. It is not a test-only exemption —
/// production passes the real rename — so one implementation serves both.
fn write_bytes_atomic_with(
    path: &Path,
    bytes: &[u8],
    rename: impl Fn(&Path, &Path) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let temporary = temp_path(path)?;
    std::fs::write(&temporary, bytes)?;
    rename(&temporary, path)
}

/// Return *path*'s same-directory temporary sibling: its name plus `.tmp`.
fn temp_path(path: &Path) -> std::io::Result<PathBuf> {
    let name = path.file_name().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "the report path has no file name",
        )
    })?;
    let mut temporary = name.to_os_string();
    temporary.push(".tmp");
    Ok(path.with_file_name(temporary))
}

#[cfg(test)]
mod tests;
