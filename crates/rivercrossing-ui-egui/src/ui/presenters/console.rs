// SPDX-License-Identifier: GPL-3.0-only

//! The console's plate-entry and lifecycle decisions, with no rendering.
//!
//! Each function returns the notice, cue and field action that the console
//! view applies. The rules follow the Python oracle's
//! `ui/presenters/console.py`, and every notice reproduces its copy.

use rivercrossing_ride::{Clock, RideEngine, RideError, RideStatus, SystemClock};
use rivercrossing_roster::roster::Roster;

use super::data_source::{is_miss, refusal_message};

#[cfg(test)]
mod tests;

/// The notice that a successful start posts.
pub(crate) const RIDE_STARTED: &str = "Ride started";

/// The sound cue that the console plays for an entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Cue {
    /// A lap recorded above the minimum.
    Recorded,
    /// A lap recorded but flagged for review.
    Flagged,
    /// A refusal or a miss.
    Error,
}

/// What the console does after one entry-field submission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntryOutcome {
    /// The status notice to post, when one is posted.
    pub(crate) notice: Option<String>,
    /// The sound cue to play, when one is played.
    pub(crate) cue: Option<Cue>,
    /// Whether the entry field keeps its text.
    pub(crate) keep_field: bool,
    /// Whether the console clears the entry field.
    pub(crate) cleared: bool,
}

impl EntryOutcome {
    /// Return the outcome of a blank submission: focus only.
    fn silent() -> Self {
        Self {
            notice: None,
            cue: None,
            keep_field: false,
            cleared: false,
        }
    }

    /// Return the outcome of a refusal: buzz and keep the text.
    fn refused(notice: String) -> Self {
        Self {
            notice: Some(notice),
            cue: Some(Cue::Error),
            keep_field: true,
            cleared: false,
        }
    }

    /// Return the outcome of an accepted entry: post the notice, play the cue
    /// and clear the text.
    fn accepted(notice: String, cue: Cue) -> Self {
        Self {
            notice: Some(notice),
            cue: Some(cue),
            keep_field: false,
            cleared: true,
        }
    }

    /// Return the outcome of an action that only posts a notice.
    fn notice_only(notice: String) -> Self {
        Self {
            notice: Some(notice),
            cue: None,
            keep_field: false,
            cleared: false,
        }
    }
}

/// The console's three button gates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ConsoleGates {
    /// Whether the Start button is enabled.
    pub(crate) start: bool,
    /// Whether the Stop button is enabled.
    pub(crate) stop: bool,
    /// Whether the Undo button is enabled.
    pub(crate) undo: bool,
}

/// Handle Enter or Record with the plate entry's text.
///
/// A blank text only refocuses the field. A text drawn only from the miss
/// symbols records a miss. Any other text goes to the engine: an accepted
/// crossing clears the field and plays its cue, and a refusal keeps the text
/// and buzzes.
pub(crate) fn plate_entered(
    engine: &mut RideEngine<Roster, SystemClock>,
    text: &str,
) -> EntryOutcome {
    let plate = text.trim();
    if plate.is_empty() {
        return EntryOutcome::silent();
    }
    if is_miss(plate) {
        return miss_entered(engine, plate);
    }
    let result = engine.record_crossing(plate, None);
    if !result.accepted {
        let reason = result.reason.as_deref().unwrap_or_default();
        return EntryOutcome::refused(rejection_notice(plate, reason));
    }
    if result.flagged {
        return EntryOutcome::accepted(
            format!("Recorded plate {plate} — short lap flagged for review"),
            Cue::Flagged,
        );
    }
    EntryOutcome::accepted(format!("Recorded plate {plate}"), Cue::Recorded)
}

/// Record the typed miss and re-render the console.
///
/// The caller has already classified *text* as a miss. A recorded miss clears
/// the field and buzzes, because the symbol is a deliberate signal rather
/// than a typo. A refused miss keeps the text and names the refusal.
pub(crate) fn miss_entered(
    engine: &mut RideEngine<Roster, SystemClock>,
    _text: &str,
) -> EntryOutcome {
    let at = SystemClock.now();
    match engine.record_miss(at, "missed number") {
        Ok(_event) => EntryOutcome::accepted(
            "Plate missed — logged for later entry".to_owned(),
            Cue::Error,
        ),
        Err(error) => {
            EntryOutcome::refused(format!("Miss not recorded: {}", refusal_message(&error)))
        }
    }
}

/// Return the three console gates for *engine*.
///
/// Start is enabled in DRAFT, in REOPENED, and in a stopped RUNNING ride,
/// because continue-after-stop is the resume mechanism. Stop is enabled only
/// in a live RUNNING ride. Undo needs a RUNNING ride with one crossing.
#[must_use]
pub(crate) fn console_gates(engine: &RideEngine<Roster, SystemClock>) -> ConsoleGates {
    let status = engine.state();
    let running = status == RideStatus::Running;
    let stopped = running && engine.stopped();
    ConsoleGates {
        start: status == RideStatus::Draft || status == RideStatus::Reopened || stopped,
        stop: running && !stopped,
        undo: running && !engine.crossings().is_empty(),
    }
}

/// Undo the newest crossing, or post why it cannot run.
pub(crate) fn undo(engine: &mut RideEngine<Roster, SystemClock>) -> EntryOutcome {
    let status = engine.state();
    let live = status == RideStatus::Running || status == RideStatus::Reopened;
    if !live || engine.crossings().is_empty() {
        return EntryOutcome::notice_only("Undo unavailable: nothing to undo".to_owned());
    }
    match engine.undo_last() {
        Ok(_event) => EntryOutcome::notice_only("Last crossing undone".to_owned()),
        Err(error) => {
            EntryOutcome::notice_only(format!("Undo unavailable: {}", refusal_message(&error)))
        }
    }
}

/// Start a DRAFT ride, or continue a stopped or reopened one.
///
/// # Errors
///
/// Return the refusal notice when the state machine refuses the start. A
/// blocked DRAFT start is not an error: its reasons come back as
/// `Ok(Some(reasons))` for the blocked-start dialog.
pub(crate) fn start(
    engine: &mut RideEngine<Roster, SystemClock>,
) -> Result<Option<Vec<String>>, String> {
    match engine.start(None) {
        Ok(_event) => Ok(None),
        Err(RideError::StartBlocked { reasons, .. }) => Ok(Some(reasons)),
        Err(error) => Err(format!("Cannot start: {}", refusal_message(&error))),
    }
}

/// Stop a RUNNING ride and return the notice to post.
///
/// # Errors
///
/// Return the refusal notice when the ride is not RUNNING or is stopped.
pub(crate) fn stop(engine: &mut RideEngine<Roster, SystemClock>) -> Result<String, String> {
    match engine.stop() {
        Ok(_event) => Ok("Ride stopped — continue to resume".to_owned()),
        Err(error) => Err(format!("Cannot stop: {}", refusal_message(&error))),
    }
}

/// Finish a RUNNING or REOPENED ride and return the notice to post.
///
/// `failed_checks` names the evaluator checks that the operator overrode, so
/// the finish event records them and the results read as self-test unverified.
///
/// # Errors
///
/// Return the refusal notice when the ride is DRAFT or FINISHED.
pub(crate) fn finish(
    engine: &mut RideEngine<Roster, SystemClock>,
    failed_checks: &[String],
) -> Result<String, String> {
    match engine.finish(failed_checks) {
        Ok(_event) => Ok("Ride is finished. Results are available".to_owned()),
        Err(error) => Err(format!("Cannot finish: {}", refusal_message(&error))),
    }
}

/// Reopen a FINISHED ride for corrections and return the notice to post.
///
/// # Errors
///
/// Return the refusal notice when the ride is not FINISHED.
pub(crate) fn reopen(engine: &mut RideEngine<Roster, SystemClock>) -> Result<String, String> {
    match engine.reopen() {
        Ok(_event) => Ok("Ride reopened for corrections".to_owned()),
        Err(error) => Err(format!("Cannot reopen: {}", refusal_message(&error))),
    }
}

/// Return the console's notice for a refused crossing.
///
/// The engine's three machine-readable refusal reasons map to their own copy.
/// Any other reason still names the plate and the reason, so a refusal never
/// reads as a success.
fn rejection_notice(plate: &str, reason: &str) -> String {
    match reason {
        "unknown_plate" => format!("Unknown plate {plate}"),
        "ride is not running" => "The ride is not running".to_owned(),
        "ride is stopped" => "The ride is stopped".to_owned(),
        other => format!("Plate rejected: {other}"),
    }
}
