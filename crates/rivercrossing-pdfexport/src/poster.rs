// SPDX-License-Identifier: GPL-3.0-only

//! The one-page podium poster: payload, poster pages, bytes, file.
//!
//! The module stays thin. It wires the pure poster page model to the craft-pdf
//! writer and lands the bytes atomically. The poster reads only `all_cards` and
//! `show_times` from the payload's own options; every other report option is
//! ignored.

use std::path::Path;

use rivercrossing_htmlexport::payload::build_payload;

use crate::error::PdfError;
use crate::layout::poster_pages;
use crate::report::{PdfInput, write_bytes_atomic};
use crate::writer;

/// Render the podium poster for *input* to the PDF at *path*.
///
/// # Errors
///
/// Return [`PdfError::Logo`] when the logo is set but is not a PNG,
/// [`PdfError::Layout`] when the export input cannot become a payload or the
/// engine refuses an operation's geometry, [`PdfError::Write`] when
/// serialization fails, and [`PdfError::Io`] when the file cannot be written.
pub fn podium_poster(input: &PdfInput, path: &Path) -> Result<(), PdfError> {
    let payload =
        build_payload(&input.export).map_err(|error| PdfError::Layout(error.to_string()))?;
    let pages = poster_pages(&payload, input.letter, input.logo_png.as_deref())?;
    let bytes = writer::write(&pages, &payload.event.title, input.created_at)?;
    write_bytes_atomic(path, &bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests;
