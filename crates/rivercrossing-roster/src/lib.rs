// SPDX-License-Identifier: GPL-3.0-only

//! The roster of `RiverCrossing`: the entries, riders and teams of one ride,
//! the plate namespace and the lock matrix, the start gate, the audit log of
//! roster mutations, and the `RosterView` implementation that the ride engine
//! calls.
//!
//! The crate holds no ride state of its own. The ride engine drives every
//! change through `RosterView`, so a test drives the roster without the engine.

#![forbid(unsafe_code)]

pub mod audit;
pub mod enums;
pub mod error;
pub mod lock;
pub mod logos;
pub mod moves;
pub mod names;
pub mod plates;
pub mod roster;
pub mod types;
pub mod validate;
pub mod view;
