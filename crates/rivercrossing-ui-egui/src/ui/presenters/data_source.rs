// SPDX-License-Identifier: GPL-3.0-only

//! The console's read-only view models and display helpers.
//!
//! Every function here is pure over the ride engine, its roster and the
//! engine's event log, so the console's feed, counters, review rows and
//! labels are unit-testable without a window. The rules follow the Python
//! oracle's `ui/presenters/data_source.py` and `ui/presenters/console.py`.

use std::collections::{HashMap, HashSet};

use rivercrossing_ride::{
    Crossing, EntryId, EntrySnapshot, Event, EventAction, RideEngine, RideError, RideStatus,
    RosterView, SystemClock, Timestamp,
};
use rivercrossing_roster::roster::Roster;

#[cfg(test)]
mod tests;

/// The one column separator inside the review rows' identity cells.
const MISS_PLATE: &str = "-";

/// The name cell of a pending miss.
const MISS_NAME: &str = "missed";

/// The miss symbols a scorer types when a rider's number was not caught.
const MISS_SYMBOLS: &str = "=/+-.";

/// The seconds in one hour.
const SECONDS_PER_HOUR: u64 = 3600;

/// The seconds in one minute.
const SECONDS_PER_MINUTE: u64 = 60;

/// One row of the console crossings feed.
///
/// A miss row carries plate [`MISS_PLATE`], name [`MISS_NAME`], lap zero and
/// no card. `flagged` is the short-lap review channel under both card
/// policies; `held` is the hold-mode card disposition alone, so the review
/// routing can tell a credited short lap from a held one.
#[derive(Debug, Clone, PartialEq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is an independent feed channel that the view renders separately"
)]
pub(crate) struct FeedRow {
    /// The rider's typed plate, the entry's plate, or [`MISS_PLATE`].
    pub(crate) plate: String,
    /// The rider's name, the entry's display name, or [`MISS_NAME`].
    pub(crate) name: String,
    /// The per-entry lap number; zero for a miss.
    pub(crate) lap: u32,
    /// The derived lap time in seconds; zero for a miss.
    pub(crate) lap_time: f64,
    /// The running total from the gun to this crossing, in seconds; zero for a miss.
    pub(crate) total_time: f64,
    /// The dealt card code, or `None` for a miss.
    pub(crate) card: Option<String>,
    /// Whether the lap came in under the ride's minimum lap.
    pub(crate) flagged: bool,
    /// Whether a correction touched the crossing.
    pub(crate) edited: bool,
    /// Whether the crossing's entry is out of the results.
    pub(crate) dnf: bool,
    /// Whether the lap's card waits in the hold queue.
    pub(crate) held: bool,
}

/// The console's counter chips.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Counters {
    /// The recorded crossings.
    pub(crate) crossings: usize,
    /// The credited cards: the crossings minus the held cards.
    pub(crate) cards_dealt: usize,
    /// The active entries whose lap count is odd.
    pub(crate) on_course: usize,
    /// The cards left in the shoe's current cycle.
    pub(crate) shoe_remaining: usize,
    /// The cards in the shoe's current cycle.
    pub(crate) shoe_total: usize,
    /// The registered riders, summed over the live entries.
    pub(crate) riders: usize,
    /// The live team entries.
    pub(crate) teams: usize,
}

/// One row of the console's "Needs Review" tab.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ReviewRow {
    /// Why the row entered review: a short lap, a team overlap, or a duplicate.
    pub(crate) issue: String,
    /// The card's disposition word: `Held`, `Credited` or `Void`.
    pub(crate) card: String,
    /// The crossing's plate.
    pub(crate) plate: String,
    /// The per-entry lap number.
    pub(crate) lap: u32,
    /// The derived lap time in seconds.
    pub(crate) lap_time: f64,
    /// The rider's name, or the entry's display name.
    pub(crate) rider: String,
}

/// The ride-status stop light's mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StopLight {
    /// No ride is open, so no circle lights.
    Off,
    /// A live running ride.
    Green,
    /// A stopped running ride, or a ride reopened for corrections.
    Yellow,
    /// A draft ride, or a finished one.
    Red,
}

/// Return *seconds* as the console's `h:mm:ss` clock text.
///
/// A negative value clamps to zero. The value truncates toward zero, so a
/// sub-second remainder never rounds a reading up.
#[must_use]
pub(crate) fn format_duration(seconds: f64) -> String {
    let total = whole_seconds(seconds);
    let hours = total / SECONDS_PER_HOUR;
    let minutes = total % SECONDS_PER_HOUR / SECONDS_PER_MINUTE;
    let remainder = total % SECONDS_PER_MINUTE;
    format!("{hours}:{minutes:02}:{remainder:02}")
}

/// Return *seconds* truncated to a whole count, with a negative clamped to zero.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is clamped to zero first, and a ride duration is far below u64::MAX seconds"
)]
fn whole_seconds(seconds: f64) -> u64 {
    seconds.max(0.0).trunc() as u64
}

/// Return the seconds that the console's clocks show for *engine*.
///
/// A draft ride shows zero, a live running ride reads the wall clock, a stopped ride keeps the
/// reading captured when it stopped, and a closed ride shows the recorded finish instant. The
/// `frozen` reading is the console's captured value; `None` leaves a stopped ride at zero.
#[must_use]
pub(crate) fn clock_seconds(engine: &RideEngine<Roster, SystemClock>, frozen: Option<f64>) -> f64 {
    match engine.state() {
        RideStatus::Draft => 0.0,
        RideStatus::Running if engine.stopped() => frozen.unwrap_or(0.0),
        RideStatus::Running => engine.elapsed().unwrap_or(0.0),
        RideStatus::Finished | RideStatus::Reopened => engine.closed_elapsed(),
    }
}

/// Return *seconds* as a 0.0..=1.0 dial fraction of *total*.
///
/// The fraction clamps at both ends, so a ride past its planned duration shows
/// a full elapsed dial rather than an overflowing one.
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    reason = "the clamped fraction is a bounded 0..=1 value that f32 represents closely enough for a dial"
)]
pub(crate) fn clock_fraction(seconds: f64, total: f64) -> f32 {
    (seconds / total).clamp(0.0, 1.0) as f32
}

/// Return whether *text* is one or more miss symbols, not a plate.
///
/// A blank text is not a miss: it only refocuses the entry field.
#[must_use]
pub(crate) fn is_miss(text: &str) -> bool {
    let trimmed = text.trim();
    !trimmed.is_empty()
        && trimmed
            .chars()
            .all(|character| MISS_SYMBOLS.contains(character))
}

/// Return a ride error's bare message, as the oracle's exceptions carry it.
///
/// The port keeps the error's kind in its `Display` prefix, while the Python
/// console and setup copy quote the message alone.
#[must_use]
pub(crate) fn refusal_message(error: &RideError) -> String {
    match error {
        RideError::IllegalState { message }
        | RideError::Config { message }
        | RideError::Roster { message } => message.clone(),
        other => other.to_string(),
    }
}

/// Return the console's status label for *status*.
///
/// A stopped running ride reads `STOPPED`, so the label and the amber lamp
/// agree. Every other state is its own name in upper case.
#[must_use]
pub(crate) fn status_text(status: RideStatus, stopped: bool) -> String {
    if stopped && status == RideStatus::Running {
        return "STOPPED".to_owned();
    }
    status.as_str().to_uppercase()
}

/// Return the stop light's mode for *status*.
///
/// `None` is the no-ride console. A stopped running ride joins a reopened one
/// on amber, so the light never carries meaning by colour alone.
#[must_use]
pub(crate) fn stop_light_mode(status: Option<RideStatus>, stopped: bool) -> StopLight {
    match status {
        None => StopLight::Off,
        Some(RideStatus::Running) if stopped => StopLight::Yellow,
        Some(RideStatus::Running) => StopLight::Green,
        Some(RideStatus::Reopened) => StopLight::Yellow,
        Some(RideStatus::Draft | RideStatus::Finished) => StopLight::Red,
    }
}

/// Return the highest per-entry lap number that any crossing recorded.
///
/// The reading is zero before the ride's first crossing.
#[must_use]
pub(crate) fn current_lap(crossings: &[Crossing]) -> u32 {
    crossings
        .iter()
        .map(|crossing| crossing.seq)
        .max()
        .unwrap_or(0)
}

/// Return the console's counter chips.
#[must_use]
pub(crate) fn counters(engine: &RideEngine<Roster, SystemClock>, roster: &Roster) -> Counters {
    let entries = RosterView::entries(roster);
    let crossings = engine.crossings().len();
    let held = engine.held_crossings().len();
    Counters {
        crossings,
        cards_dealt: crossings.saturating_sub(held),
        on_course: engine.on_course(),
        shoe_remaining: engine.shoe_remaining(),
        shoe_total: engine.shoe_total(),
        riders: entries.iter().map(|entry| entry.riders.len()).sum(),
        teams: entries.iter().filter(|entry| entry.is_team).count(),
    }
}

/// Return the crossings feed, newest first, with the pending misses merged in.
#[must_use]
pub(crate) fn feed_rows(engine: &RideEngine<Roster, SystemClock>, roster: &Roster) -> Vec<FeedRow> {
    let context = FeedContext::new(engine, roster);
    let mut rows: Vec<(Timestamp, FeedRow)> = engine
        .crossings()
        .iter()
        .map(|crossing| (crossing.crossed_at, crossing_row(&context, crossing)))
        .collect();
    rows.reverse();
    for miss in engine.pending_misses().iter().rev() {
        insert_newest_first(&mut rows, (miss.crossed_at, miss_row()));
    }
    rows.into_iter().map(|(_instant, row)| row).collect()
}

/// Return the rows that match *search* on their plate, name or card cell.
///
/// The match is a case-insensitive substring search, and a blank search keeps
/// every row. The survivors keep the feed's newest-first order.
#[must_use]
pub(crate) fn filter_rows(rows: &[FeedRow], search: &str) -> Vec<FeedRow> {
    let needle = search.trim().to_lowercase();
    if needle.is_empty() {
        return rows.to_vec();
    }
    rows.iter()
        .filter(|row| {
            row.plate.to_lowercase().contains(&needle)
                || row.name.to_lowercase().contains(&needle)
                || row
                    .card
                    .as_deref()
                    .unwrap_or_default()
                    .to_lowercase()
                    .contains(&needle)
        })
        .cloned()
        .collect()
}

/// Return the "Needs Review" rows, newest first.
///
/// A row enters review when it is a short lap or half of a live duplicate
/// pair. `held_only` keeps only the rows whose card waits in the hold queue,
/// plus every duplicate half, which is the tab's own checkbox rule.
#[must_use]
pub(crate) fn review_rows(
    engine: &RideEngine<Roster, SystemClock>,
    roster: &Roster,
    held_only: bool,
) -> Vec<ReviewRow> {
    let context = FeedContext::new(engine, roster);
    engine
        .crossings()
        .iter()
        .rev()
        .filter_map(|crossing| review_row(&context, crossing, held_only))
        .collect()
}

/// Insert *candidate* into *rows* (newest first) at its instant.
///
/// A candidate slots in before the first row at or older than its own instant,
/// so a miss merges into the crossings without re-ordering them.
fn insert_newest_first(rows: &mut Vec<(Timestamp, FeedRow)>, candidate: (Timestamp, FeedRow)) {
    let position = rows
        .iter()
        .position(|(instant, _row)| *instant <= candidate.0)
        .unwrap_or(rows.len());
    rows.insert(position, candidate);
}

/// Return the miss row: a blank plate, no lap, no card and no flags.
fn miss_row() -> FeedRow {
    FeedRow {
        plate: MISS_PLATE.to_owned(),
        name: MISS_NAME.to_owned(),
        lap: 0,
        lap_time: 0.0,
        total_time: 0.0,
        card: None,
        flagged: false,
        edited: false,
        dnf: false,
        held: false,
    }
}

/// Everything one feed render reads beyond the crossing itself.
struct FeedContext<'a> {
    /// The ride engine.
    engine: &'a RideEngine<Roster, SystemClock>,
    /// The roster that carries the crossings' entries.
    roster: &'a Roster,
    /// Each entry's derived lap times, keyed by the entry's stable identity.
    times: HashMap<EntryId, Vec<f64>>,
    /// The deal numbers of the held crossings.
    held: HashSet<u64>,
    /// The crossings that a correction touched, keyed by entry and lap.
    edited: HashSet<(String, u32)>,
    /// The deal numbers of the live duplicate halves.
    duplicates: HashSet<u64>,
}

impl<'a> FeedContext<'a> {
    /// Build the per-render lookups over *engine* and *roster*.
    fn new(engine: &'a RideEngine<Roster, SystemClock>, roster: &'a Roster) -> Self {
        let mut times: HashMap<EntryId, Vec<f64>> = HashMap::new();
        for crossing in engine.crossings() {
            times
                .entry(crossing.entry_id.clone())
                .or_insert_with(|| engine.lap_times(&crossing.entry_id));
        }
        let held = engine
            .held_crossings()
            .into_iter()
            .map(|item| item.crossing.deal_seq)
            .collect();
        let edited = corrected_crossing_keys(engine.events(), engine.crossings());
        let duplicates = engine
            .duplicate_crossings()
            .into_iter()
            .flat_map(|(older, newer)| [older.deal_seq, newer.deal_seq])
            .collect();
        Self {
            engine,
            roster,
            times,
            held,
            edited,
            duplicates,
        }
    }

    /// Return the entry that a crossing was filed under, live or retired.
    fn entry(&self, id: &EntryId) -> Option<EntrySnapshot> {
        self.roster.entry(id)
    }

    /// Return the crossing's derived lap time, or zero when it is unknown.
    fn lap_time(&self, crossing: &Crossing) -> f64 {
        let index = usize::try_from(crossing.seq.saturating_sub(1)).ok();
        index
            .and_then(|index| {
                self.times
                    .get(&crossing.entry_id)
                    .and_then(|times| times.get(index))
                    .copied()
            })
            .unwrap_or(0.0)
    }

    /// Return the running total from the gun to the crossing, or zero when it is unknown.
    fn total_time(&self, crossing: &Crossing) -> f64 {
        usize::try_from(crossing.seq)
            .ok()
            .and_then(|seq| {
                self.times
                    .get(&crossing.entry_id)
                    .map(|times| times.iter().take(seq).sum())
            })
            .unwrap_or(0.0)
    }

    /// Return whether the crossing's lap is under the ride's minimum lap.
    fn is_short_lap(&self, crossing: &Crossing, lap_time: f64) -> bool {
        let known = usize::try_from(crossing.seq).is_ok_and(|seq| {
            self.times
                .get(&crossing.entry_id)
                .is_some_and(|times| seq <= times.len())
        });
        known && lap_time < f64::from(self.engine.config().min_lap_s)
    }
}

/// Return one crossing's feed row.
fn crossing_row(context: &FeedContext<'_>, crossing: &Crossing) -> FeedRow {
    let entry = context.entry(&crossing.entry_id);
    let lap_time = context.lap_time(crossing);
    FeedRow {
        plate: plate_text(entry.as_ref(), crossing),
        name: name_text(entry.as_ref(), crossing),
        lap: crossing.seq,
        lap_time,
        total_time: context.total_time(crossing),
        card: card_code(context.engine, crossing),
        flagged: context.is_short_lap(crossing, lap_time),
        edited: context.edited.contains(&crossing_key(crossing)),
        dnf: entry
            .as_ref()
            .is_some_and(|entry| context.engine.entry_is_dnf(entry)),
        held: context.held.contains(&crossing.deal_seq),
    }
}

/// Return the review row for *crossing*, or `None` when it is not in review.
fn review_row(
    context: &FeedContext<'_>,
    crossing: &Crossing,
    held_only: bool,
) -> Option<ReviewRow> {
    let row = crossing_row(context, crossing);
    let duplicate = context.duplicates.contains(&crossing.deal_seq);
    if !(row.flagged || duplicate) {
        return None;
    }
    if held_only && !(row.held || duplicate) {
        return None;
    }
    let team_overlap = row.flagged
        && context
            .entry(&crossing.entry_id)
            .is_some_and(|entry| entry.is_team);
    let issue = if duplicate {
        "Duplicate crossing"
    } else if team_overlap {
        "Team overlap"
    } else {
        "Short lap"
    };
    Some(ReviewRow {
        issue: issue.to_owned(),
        card: card_status_text(card_status(context.engine, crossing)).to_owned(),
        plate: row.plate,
        lap: row.lap,
        lap_time: row.lap_time,
        rider: row.name,
    })
}

/// Return the plate cell: the typed plate, else the entry's plate.
fn plate_text(entry: Option<&EntrySnapshot>, crossing: &Crossing) -> String {
    crossing
        .rider_plate
        .clone()
        .filter(|plate| !plate.is_empty())
        .or_else(|| entry.map(|entry| entry.plate.clone()))
        .unwrap_or_default()
}

/// Return the name cell: the crossing rider's name, else the entry's name.
fn name_text(entry: Option<&EntrySnapshot>, crossing: &Crossing) -> String {
    let rider_name = crossing
        .rider_plate
        .as_deref()
        .and_then(|plate| {
            entry.and_then(|entry| {
                entry
                    .riders
                    .iter()
                    .find(|rider| rider.plate.as_deref() == Some(plate))
                    .map(|rider| rider.full_name.clone())
            })
        })
        .filter(|name| !name.is_empty());
    let entry_name = entry.map_or_else(
        || crossing.rider_plate.clone().unwrap_or_default(),
        |entry| entry.display_name.clone(),
    );
    rider_name.unwrap_or(entry_name)
}

/// Return the card cell: the held card's code, else the dealt card's code.
fn card_code(engine: &RideEngine<Roster, SystemClock>, crossing: &Crossing) -> Option<String> {
    engine
        .held_card_for(crossing)
        .or_else(|| engine.card_for(crossing))
        .map(|card| card.code())
}

/// Return the crossing's card disposition token.
fn card_status(engine: &RideEngine<Roster, SystemClock>, crossing: &Crossing) -> &'static str {
    if engine.held_card_for(crossing).is_some() {
        return "held";
    }
    if engine.is_card_voided(crossing) {
        return "voided";
    }
    let credited = engine
        .card_for(crossing)
        .is_some_and(|card| engine.credited_cards(&crossing.entry_id).contains(&card));
    if credited { "credited" } else { "voided" }
}

/// Return the Card column's title-cased word for a disposition token.
fn card_status_text(status: &str) -> &'static str {
    match status {
        "held" => "Held",
        "credited" => "Credited",
        "voided" => "Void",
        _ => "",
    }
}

/// Return the (entry, lap) key that a feed row matches a correction by.
fn crossing_key(crossing: &Crossing) -> (String, u32) {
    (crossing.entry_id.0.clone(), crossing.seq)
}

/// Return the (entry, lap) keys of every live crossing a correction touched.
fn corrected_crossing_keys(events: &[Event], crossings: &[Crossing]) -> HashSet<(String, u32)> {
    let mut keys = HashSet::new();
    for event in events {
        let resolved = match event.action {
            EventAction::EditCrossing => edit_key(&event.payload),
            EventAction::AddCrossingAt | EventAction::AssignPlateToMiss => {
                key_at_instant(crossings, &event.payload)
            }
            EventAction::Reassign => latest_key(crossings, &event.payload),
            _ => None,
        };
        keys.extend(resolved);
    }
    keys
}

/// Return the key that an edit event names: its entry and lap.
fn edit_key(payload: &serde_json::Value) -> Option<(String, u32)> {
    let entry_id = payload.get("entry_id")?.as_str()?;
    let seq = u32::try_from(payload.get("seq")?.as_u64()?).ok()?;
    Some((entry_id.to_owned(), seq))
}

/// Return the key of the live crossing at an event's (entry, instant).
fn key_at_instant(crossings: &[Crossing], payload: &serde_json::Value) -> Option<(String, u32)> {
    let entry_id = payload.get("entry_id")?.as_str()?;
    let crossed_at = payload.get("crossed_at")?.as_str()?;
    crossings
        .iter()
        .find(|crossing| {
            crossing.entry_id.0 == entry_id && format_instant(crossing.crossed_at) == crossed_at
        })
        .map(crossing_key)
}

/// Return the key of the entry's highest-lap live crossing.
fn latest_key(crossings: &[Crossing], payload: &serde_json::Value) -> Option<(String, u32)> {
    let entry_id = payload.get("new_entry_id")?.as_str()?;
    crossings
        .iter()
        .rev()
        .find(|crossing| crossing.entry_id.0 == entry_id)
        .map(crossing_key)
}

/// Return *at* rendered as the engine's audit payload renders it.
///
/// The engine writes every payload instant in UTC with no offset, so the
/// correction resolver compares against the same shape.
fn format_instant(at: Timestamp) -> String {
    let utc = Timestamp::from_unix_timestamp(at.unix_timestamp()).unwrap_or(at);
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
