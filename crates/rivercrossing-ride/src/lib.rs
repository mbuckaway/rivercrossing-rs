// SPDX-License-Identifier: GPL-3.0-only

//! The ride engine for `RiverCrossing`: the lifecycle state machine, the
//! crossing and correction events, the card-sufficiency advisory, and the
//! [`RosterView`] seam that breaks the ride-to-roster dependency cycle.
//!
//! The engine is pure. It reads the current instant from a [`Clock`] and the
//! roster through a [`RosterView`], so a test drives both.

#![forbid(unsafe_code)]

mod clock;
mod config;
mod engine;
mod error;
mod event;
mod format;
mod roster_view;
mod status;
mod sufficiency;

pub use clock::{Clock, SystemClock, Timestamp};
pub use config::{
    EntryMode, PlateModel, RideConfig, RideConfigParams, TieBreak, setup_minimum_violations,
};
pub use engine::{
    Crossing, CrossingResult, HeldCrossing, PendingMiss, RideEngine, SnapshotEntry, SnapshotKind,
};
pub use error::RideError;
pub use event::{Event, EventAction};
pub use format::format_elapsed;
pub use roster_view::{EntryId, EntrySnapshot, RiderSnapshot, RosterView, StartViolation};
pub use status::RideStatus;
pub use sufficiency::{CardCheck, Sufficiency, check_card_sufficiency, estimate_cards_needed};
