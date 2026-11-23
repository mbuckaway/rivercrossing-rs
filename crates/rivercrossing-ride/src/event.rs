// SPDX-License-Identifier: GPL-3.0-only

//! The ride-level audit event and its action set.

use serde::{Deserialize, Serialize};

/// One action that a ride audit event records.
///
/// The serde spelling of each variant is the string that the Python oracle
/// stores in the audit row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventAction {
    /// Start the ride.
    Start,
    /// Continue a ride that stopped.
    Continue,
    /// Set the actual start instant.
    SetStartTime,
    /// Record one crossing.
    RecordCrossing,
    /// Confirm a held short-lap card.
    ConfirmHeld,
    /// Void a held short-lap card.
    VoidHeld,
    /// Return a held crossing to the held list.
    ReturnToHeld,
    /// Undo the last crossing.
    Undo,
    /// Deal a card by hand.
    DealManual,
    /// Edit a recorded crossing.
    EditCrossing,
    /// Void a recorded crossing.
    VoidCrossing,
    /// Add a crossing at a given instant.
    AddCrossingAt,
    /// Record a miss whose plate was not captured.
    RecordMiss,
    /// Assign a plate to a recorded miss.
    AssignPlateToMiss,
    /// Reassign a crossing to another entry.
    Reassign,
    /// Mark an entry or a rider as DNF.
    Dnf,
    /// Void a card from an entry's hand.
    VoidCard,
    /// Move a rider into another team.
    MoveRider,
    /// Extract a rider from a team into a solo entry.
    ExtractRiderToSolo,
    /// Stop the clock.
    Stop,
    /// Finish the ride.
    Finish,
    /// Reopen a finished ride for corrections.
    Reopen,
    /// Draw the tie-break cards.
    TiebreakDraw,
    /// Reshuffle the shoe.
    ShoeReshuffle,
}

/// One ride-level audit event.
///
/// The payload is JSON-ready, so the store persists it and the engine replays
/// it. Every engine mutation appends the event that it returns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    /// The action that the event records.
    pub action: EventAction,
    /// The action's JSON payload.
    pub payload: serde_json::Value,
}
