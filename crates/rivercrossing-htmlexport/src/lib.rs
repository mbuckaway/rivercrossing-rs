// SPDX-License-Identifier: GPL-3.0-only

//! The self-contained HTML results exporter: shared payload model, the results
//! page, the podium poster, and the two `WordPress` content fragments (R-61, R-63,
//! R-65, R-86).
//!
//! The exporter is UI-agnostic: callers build an [`ExportInput`] from the ride,
//! roster and standings and receive a rendered `String`. Every byte-level rule —
//! `Jinja2` whitespace, `MarkupSafe` escaping, the canonical `race-data` record — is
//! reproduced from the Python oracle in `design/docs-md/spec.md` §8.

#![forbid(unsafe_code)]

pub mod env;
pub mod error;
pub mod format;
pub mod payload;
pub mod record;
pub mod render;
pub mod sections;
pub mod types;
