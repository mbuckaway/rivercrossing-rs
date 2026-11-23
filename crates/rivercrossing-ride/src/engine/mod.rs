// SPDX-License-Identifier: GPL-3.0-only

//! The ride engine: construction, the lifecycle state machine, the crossing
//! ledger, undo, the held-card lifecycle, the pending-miss queue, the timing
//! reads, and the setup-config update.
//!
//! The engine is pure. It reads the current instant from a [`Clock`], reaches
//! the roster through a [`RosterView`], and appends one [`Event`] per mutation.

use std::collections::{BTreeMap, HashMap, HashSet};

use rivercrossing_cards::{Card, JokersMode, Shoe};

use crate::clock::{Clock, Timestamp};
use crate::config::RideConfig;
use crate::error::RideError;
use crate::event::Event;
use crate::roster_view::{EntryId, RosterView};
use crate::status::RideStatus;

mod corrections;
mod crossings;
mod lifecycle;
mod moves;
mod replay;
mod results;

#[cfg(test)]
mod tests;

pub use results::{SnapshotEntry, SnapshotKind};

/// One recorded lap crossing.
///
/// `entry_id` is the entry's stable identity, so a mid-ride re-plating never
/// moves a recorded lap. `seq` is 1-based per entry and follows record order.
/// `deal_seq` names the card this crossing dealt, which keeps the deal ledger
/// independent of the card's value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Crossing {
    /// The stable identity of the entry that crossed.
    pub entry_id: EntryId,
    /// The entry's 1-based lap number in record order.
    pub seq: u32,
    /// The instant the entry crossed the line.
    pub crossed_at: Timestamp,
    /// The plate that the operator typed for this crossing.
    pub rider_plate: Option<String>,
    /// The monotonic deal number of the card this crossing dealt.
    pub deal_seq: u64,
}

/// The outcome of one `record_crossing` call.
///
/// A refusal carries `accepted: false` and a `reason`, and never an error. A
/// success carries the resolved entry, the derived lap time, the dealt card,
/// and the short-lap flag.
#[derive(Debug, Clone, PartialEq)]
pub struct CrossingResult {
    /// True when the engine recorded the crossing.
    pub accepted: bool,
    /// The plate that the caller supplied.
    pub plate: String,
    /// The resolved entry's plate.
    pub entry_plate: Option<String>,
    /// The resolved entry's display name.
    pub entry_name: Option<String>,
    /// The credited lap number.
    pub lap: u32,
    /// The derived lap time in seconds.
    pub lap_time: f64,
    /// The refusal reason, when the crossing was refused.
    pub reason: Option<String>,
    /// The card dealt for this crossing.
    pub card: Option<Card>,
    /// True when the lap is shorter than the ride's minimum lap.
    pub flagged: bool,
}

/// One short-lap crossing whose card waits for review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldCrossing {
    /// The crossing that dealt the held card.
    pub crossing: Crossing,
    /// The held card.
    pub card: Card,
}

/// One voided card, keyed by the deal number of the crossing that dealt it.
///
/// A crossing that [`RideEngine::void_crossing`] retired keeps its own
/// record here, so a later rider move can restore the lap. A card that
/// [`RideEngine::void_card`] or [`RideEngine::void_held`] retired has no
/// crossing of its own, and its `crossing` stays `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct VoidedCard {
    /// The retired card.
    card: Card,
    /// The crossing that `void_crossing` removed, when one did.
    crossing: Option<Crossing>,
}

/// One recorded miss whose plate was not captured.
///
/// A miss is not a crossing: it stays outside the crossing ledger, deals no
/// card, and never enters the per-entry lap index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingMiss {
    /// The ride-wide 1-based miss number in record order.
    pub miss_seq: u32,
    /// The instant the operator signalled the miss.
    pub crossed_at: Timestamp,
}

/// The ride state machine over a roster and a wall clock.
///
/// The engine owns the setup config, the seeded shoe, the clock seam, and the
/// roster seam. Every mutation appends the [`Event`] it returns, so a store can
/// persist the log and replay it.
#[derive(Debug)]
pub struct RideEngine<V: RosterView, C: Clock> {
    config: RideConfig,
    shoe: Shoe,
    clock: C,
    roster: V,
    state: RideStatus,
    actual_start: Option<Timestamp>,
    stopped: bool,
    stopped_at: Option<Timestamp>,
    finished_at: Option<Timestamp>,
    crossings: Vec<Crossing>,
    laps: HashMap<EntryId, Vec<Crossing>>,
    pending_misses: Vec<PendingMiss>,
    miss_counter: u32,
    dealt: BTreeMap<u64, Card>,
    held: BTreeMap<u64, Card>,
    hand: HashMap<EntryId, Vec<(u64, Option<String>)>>,
    dnf_riders: HashSet<String>,
    tiebreak: HashMap<EntryId, Card>,
    events: Vec<Event>,
    /// Every voided card, keyed by the monotonic deal number of its crossing.
    ///
    /// The deal number is the physical card's identity, so a value-equal card
    /// on another deal stays live.
    voided: BTreeMap<u64, VoidedCard>,
    self_test_failed_checks: Vec<String>,
    next_deal_seq: u64,
}

impl<V: RosterView, C: Clock> RideEngine<V, C> {
    /// Build a DRAFT engine over the given collaborators.
    ///
    /// The engine starts with no events, no crossings, and no start instant.
    #[expect(
        clippy::too_many_arguments,
        reason = "the frozen constructor takes the config, shoe, clock and roster"
    )]
    pub fn new(config: RideConfig, shoe: Shoe, clock: C, roster: V) -> Self {
        Self {
            config,
            shoe,
            clock,
            roster,
            state: RideStatus::Draft,
            actual_start: None,
            stopped: false,
            stopped_at: None,
            finished_at: None,
            crossings: Vec::new(),
            laps: HashMap::new(),
            pending_misses: Vec::new(),
            miss_counter: 0,
            dealt: BTreeMap::new(),
            held: BTreeMap::new(),
            hand: HashMap::new(),
            dnf_riders: HashSet::new(),
            tiebreak: HashMap::new(),
            events: Vec::new(),
            voided: BTreeMap::new(),
            self_test_failed_checks: Vec::new(),
            next_deal_seq: 0,
        }
    }

    /// Append one event to the log and return it.
    fn append(&mut self, event: Event) -> Event {
        self.events.push(event.clone());
        event
    }

    /// Return the gun instant, or the not-started refusal.
    ///
    /// # Errors
    ///
    /// Return [`RideError::IllegalState`] when the ride has never started.
    fn require_actual_start(&self) -> Result<Timestamp, RideError> {
        self.actual_start
            .ok_or_else(|| illegal("ride has not started"))
    }
}

/// Return the illegal-state error for one message.
fn illegal(message: impl Into<String>) -> RideError {
    RideError::IllegalState {
        message: message.into(),
    }
}

/// Return the start-blocked error for one message and its reasons.
fn start_blocked(message: String, reasons: Vec<String>) -> RideError {
    RideError::StartBlocked { message, reasons }
}

/// Render *at* as the oracle's naive ISO-8601 text.
///
/// The port keeps instants on a UTC offset, so the offset is dropped from the
/// audit payload and the ride table's epoch seconds carry it instead.
fn format_timestamp(at: Timestamp) -> String {
    let utc = at.to_offset(time::UtcOffset::UTC);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        utc.year(),
        u8::from(utc.month()),
        utc.day(),
        utc.hour(),
        utc.minute(),
        utc.second()
    )
}

/// Return the three shoe fields that the setup config compares.
fn shoe_structure(config: &RideConfig) -> (u32, u32, JokersMode) {
    (
        config.deck_count,
        config.jokers_per_deck,
        config.jokers_mode,
    )
}
