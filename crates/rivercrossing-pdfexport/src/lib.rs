// SPDX-License-Identifier: GPL-3.0-only

//! The print-ready PDF results exporter and the one-page podium poster (R-62).
//!
//! Built on `craft-pdf` (krilla). The exporter is UI-agnostic and deterministic:
//! the caller supplies the `created_at` stamp, so the same input yields the same
//! bytes on every host. Layout lives in a pure, unit-testable model; the writer
//! stays a thin adapter over the engine.

#![forbid(unsafe_code)]

pub mod error;
#[cfg(test)]
pub(crate) mod fixtures;
pub mod fonts;
pub mod geometry;
pub mod layout;
#[cfg(test)]
pub(crate) mod pdf_read;
pub mod poster;
pub mod report;
pub mod writer;
