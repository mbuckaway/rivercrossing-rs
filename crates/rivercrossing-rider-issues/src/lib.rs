// SPDX-License-Identifier: GPL-3.0-only

//! The advisory rider-defect report (R-78): a read-only scan of a roster that
//! names the seven kinds of defect an operator should fix before the ride.
//!
//! The report is advisory, not the hard start gate: `Roster::validate_for_start`
//! stays in the roster crate. This crate holds no roster state.

#![forbid(unsafe_code)]

pub mod checks;
pub mod issue;
