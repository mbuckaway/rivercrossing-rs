// SPDX-License-Identifier: GPL-3.0-only

//! CSV import and export for `RiverCrossing`.
//!
//! The crate owns the unified, header-mapped roster CSV (R-21, R-24): a pure
//! preview, an atomic commit through the roster's audited mutators, and the two
//! exporters — the roster CSV and the §15 standings CSV (R-65). It is
//! UI-agnostic and never touches the store; the caller persists the mutated
//! roster.

#![forbid(unsafe_code)]
