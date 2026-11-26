// SPDX-License-Identifier: GPL-3.0-only

//! The results window's standings model and its stale-export flag.
//!
//! The model ranks the engine's live snapshot with the ride's stored tie-break
//! order and splits the field into its teams and solo sections, each numbered
//! from one. A live ride ignores the stored order and ranks by most laps then
//! shortest total time, so a hand tie never leaves the live board as an
//! unresolved draw, while a finished ride applies the stored order and its
//! zero-lap mode.
//!
//! The stale flag follows the Python oracle's `ui/presenters/results.py`: the
//! window holds the engine's event count captured at the last export, and a
//! correction event at or after that watermark makes the published results
//! stale.
//!
//! The engine's snapshot already carries every row's name, kind, sex and hand,
//! so the standings model takes no roster parameter.

#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the results window and the export composition read every field of this model"
    )
)]

use rivercrossing_ride::{
    Event, EventAction, RideEngine, RideStatus, SnapshotKind, SystemClock, TieBreak,
};
use rivercrossing_roster::roster::Roster;
use rivercrossing_standings::hand_name::hand_name;
use rivercrossing_standings::rank::rank_by_kind;
use rivercrossing_standings::{LIVE_TIEBREAK_ORDER, Placed, ZeroLapMode};

#[cfg(test)]
mod tests;

/// The eight audit actions that correct a recorded ride.
///
/// A recorded crossing, a miss, an undo and a start-time change are live-ride
/// activity, not corrections, so they never trip the stale flag.
pub(crate) const CORRECTION_ACTIONS: [EventAction; 8] = [
    EventAction::AddCrossingAt,
    EventAction::AssignPlateToMiss,
    EventAction::EditCrossing,
    EventAction::Reassign,
    EventAction::DealManual,
    EventAction::Dnf,
    EventAction::VoidCard,
    EventAction::VoidCrossing,
];

/// One rendered standings row.
///
/// `place` is `None` for a did-not-start row, which no ranker emits; only the
/// team/solo split appends one. `hand` is the human prose name, empty for an
/// entry that never credited a card. `draw` is the entry's drawn tie-break card
/// as its card code, empty while no draw happened and on a DNS row.
/// `total_time` is the last crossing minus the gun and `best_lap` is the
/// quickest derived lap, both in whole seconds.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StandingsRowModel {
    /// The 1-based place inside the row's own section, or `None` for a DNS row.
    pub(crate) place: Option<u32>,
    /// The plate that the entry currently carries.
    pub(crate) plate: String,
    /// The name that the entry list shows.
    pub(crate) name: String,
    /// Whether the entry is a solo rider or a team.
    pub(crate) kind: SnapshotKind,
    /// A solo rider's sex spelling; `None` for a team.
    pub(crate) sex: Option<String>,
    /// The entry's completed laps.
    pub(crate) laps: u32,
    /// The hand's prose name, empty when the entry holds no card.
    pub(crate) hand: String,
    /// The drawn tie-break card's code, empty when the entry drew none.
    pub(crate) draw: String,
    /// The last crossing minus the gun, in seconds.
    pub(crate) total_time: f64,
    /// The quickest derived lap, in seconds.
    pub(crate) best_lap: f64,
}

/// The results window's whole model.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StandingsModel {
    /// The team section, ranked from one.
    pub(crate) teams: Vec<StandingsRowModel>,
    /// The solo section, ranked from one.
    pub(crate) solo: Vec<StandingsRowModel>,
    /// Whether a correction landed after the last export.
    pub(crate) stale: bool,
    /// Whether the ride was finished over a failed evaluator self-test.
    pub(crate) self_test_unverified: bool,
}

/// The export watermark that an open results window holds.
///
/// The watermark is the engine's event count at the last successful export;
/// `None` until one happens, because nothing is published to go stale.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ExportWatermark(Option<usize>);

impl ExportWatermark {
    /// Return the event count captured at the last export, or `None`.
    #[must_use]
    pub(crate) fn value(self) -> Option<usize> {
        self.0
    }

    /// Record a fresh export's watermark.
    ///
    /// The next [`standings_model`] call compares against the advanced count,
    /// so the banner clears immediately on a successful export.
    pub(crate) fn mark_exported(&mut self, watermark: usize) {
        self.0 = Some(watermark);
    }
}

/// Return the results window's model for *engine*.
///
/// The rows come from the engine's live snapshot: a finished ride applies
/// `tiebreak_order` and the `show_dns_riders` zero-lap mode, while a live or
/// reopened ride ranks its whole field by the live order.
///
/// # Errors
///
/// Return the standings refusal when a drawn tie reaches a joker draw card. The
/// venue draw deals natural cards only, so a legal ride never reports one.
#[expect(
    clippy::too_many_arguments,
    reason = "the model takes the engine, the ride's order, the DNS toggle and the export watermark"
)]
pub(crate) fn standings_model(
    engine: &RideEngine<Roster, SystemClock>,
    tiebreak_order: [TieBreak; 3],
    show_dns_riders: bool,
    export_watermark: Option<usize>,
) -> Result<StandingsModel, String> {
    let (teams, solo) = ranked_sections(engine, tiebreak_order, show_dns_riders)?;
    Ok(StandingsModel {
        teams: teams.iter().map(row_model).collect(),
        solo: solo.iter().map(row_model).collect(),
        stale: has_correction_since(engine.events(), export_watermark),
        self_test_unverified: engine.self_test_unverified(),
    })
}

/// Return the ranked teams and solo sections for *engine*.
///
/// The results window and the export composition share this seam, so the two
/// surfaces can never disagree about a place or a DNS row.
///
/// # Errors
///
/// Return the standings refusal when a drawn tie reaches a joker draw card.
pub(crate) fn ranked_sections(
    engine: &RideEngine<Roster, SystemClock>,
    tiebreak_order: [TieBreak; 3],
    show_dns_riders: bool,
) -> Result<(Vec<Placed>, Vec<Placed>), String> {
    let (order, zero_laps) = ranking_rule(engine.state(), tiebreak_order, show_dns_riders);
    let entries: Vec<rivercrossing_standings::EntryResult> =
        engine.snapshot().iter().map(Into::into).collect();
    rank_by_kind(&entries, &order, zero_laps).map_err(|error| error.to_string())
}

/// Return the order and zero-lap mode that *status* ranks under.
///
/// Only a finished ride applies the stored order and the DNS toggle; every
/// other state keeps the whole field on the live board.
fn ranking_rule(
    status: RideStatus,
    tiebreak_order: [TieBreak; 3],
    show_dns_riders: bool,
) -> (Vec<TieBreak>, ZeroLapMode) {
    if status == RideStatus::Finished {
        let zero_laps = if show_dns_riders {
            ZeroLapMode::Dns
        } else {
            ZeroLapMode::Hide
        };
        return (tiebreak_order.to_vec(), zero_laps);
    }
    (LIVE_TIEBREAK_ORDER.to_vec(), ZeroLapMode::Rank)
}

/// Map one ranked result onto the window's row model.
fn row_model(placed: &Placed) -> StandingsRowModel {
    let result = &placed.result;
    StandingsRowModel {
        place: placed.place,
        plate: result.plate.clone(),
        name: result.name.clone(),
        kind: result.kind,
        sex: result.sex.clone(),
        laps: result.laps,
        hand: hand_name(&result.hand).unwrap_or_default(),
        draw: draw_text(placed),
        total_time: result.total_time,
        best_lap: result.best_lap,
    }
}

/// Return the Draw cell: the drawn card's code, blank for a DNS row.
fn draw_text(placed: &Placed) -> String {
    if placed.is_dns() {
        return String::new();
    }
    placed
        .result
        .tiebreak_card
        .as_ref()
        .map(rivercrossing_cards::Card::code)
        .unwrap_or_default()
}

/// Return whether a correction landed at or after `export_watermark`.
///
/// The watermark is the engine event count captured at export time, so a
/// correction event at an index at or after it happened after the export.
/// `None` means nothing was published, which is never stale.
#[must_use]
pub(crate) fn has_correction_since(events: &[Event], export_watermark: Option<usize>) -> bool {
    let Some(watermark) = export_watermark else {
        return false;
    };
    events
        .iter()
        .skip(watermark)
        .any(|event| CORRECTION_ACTIONS.contains(&event.action))
}
