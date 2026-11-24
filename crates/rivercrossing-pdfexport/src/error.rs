// SPDX-License-Identifier: GPL-3.0-only

//! The PDF exporter's error type.

use thiserror::Error;

/// One failure the PDF exporter reports.
#[derive(Debug, Error)]
pub enum PdfError {
    /// The organizer logo is not a PNG.
    #[error("logo: {0}")]
    Logo(String),
    /// The page model cannot place the content.
    #[error("layout: {0}")]
    Layout(String),
    /// The document could not be written.
    #[error("write: {0}")]
    Write(String),
    /// A filesystem operation failed.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests;
