// SPDX-License-Identifier: GPL-3.0-only

//! CSV import and export for `RiverCrossing`.
//!
//! The crate owns the unified, header-mapped roster CSV (R-21, R-24): a pure
//! preview, an atomic commit through the roster's audited mutators, and the two
//! exporters — the roster CSV and the §15 standings CSV (R-65). It is
//! UI-agnostic and never touches the store; the caller persists the mutated
//! roster.

#![forbid(unsafe_code)]

pub mod error;
pub mod model;
pub mod preview;

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the export path calls this module in a later task"
    )
)]
mod format;
mod header;
mod normalize;
mod wire;

pub use error::CsvIoError;
pub use model::{
    ImportOptions, ImportPreview, ImportProblem, ImportReport, ParsedEntry, ParsedRider,
    ProblemCode,
};
pub use preview::preview;
