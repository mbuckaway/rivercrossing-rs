// SPDX-License-Identifier: GPL-3.0-only

//! The §15 "Enabled when" gates: one pure matrix over the ride state.

use super::id;
use super::id::ActionId;

/// The ride state the §15 gates read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum RidePhase {
    /// No ride is open.
    #[default]
    None,
    /// A new ride that has not started.
    Draft,
    /// A live running ride.
    Running,
    /// A running ride the operator stopped; it can continue.
    RunningStopped,
    /// A finished ride, locked for the results.
    Finished,
    /// A finished ride reopened for corrections.
    Reopened,
}

/// The independent inputs the §15 "Enabled when" rules read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is an independent gate input that the §15 column names separately"
)]
pub(crate) struct GateState {
    /// The ride state.
    pub(crate) phase: RidePhase,
    /// Whether the ride carries teams.
    pub(crate) teams_enabled: bool,
    /// Whether the ride has at least one crossing.
    pub(crate) has_crossing: bool,
    /// Whether the ride has at least one audit row.
    pub(crate) has_audit_row: bool,
    /// Whether an HTML export exists this session.
    pub(crate) html_export: bool,
    /// Whether a podium poster page export exists this session.
    pub(crate) poster_export: bool,
    /// Whether a PDF export exists this session.
    pub(crate) pdf_export: bool,
    /// Whether the "Show lap & total times" publish option is on.
    pub(crate) show_times: bool,
}

/// Return whether the action is enabled in the gate state.
#[must_use]
#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "the frozen registry signature takes the gate state by reference"
)]
pub(crate) fn enabled(action: ActionId, gate: &GateState) -> bool {
    if always_enabled(action) {
        return true;
    }
    match action {
        id::MI_NEW_RIDE => matches!(gate.phase, RidePhase::None),
        id::MI_DUPLICATE_RIDE
        | id::MI_IMPORT_CSV
        | id::MI_EXPORT_CSV
        | id::MI_EDIT_RIDE
        | id::MI_RIDER_EDITOR
        | id::MI_CHECK_RIDER_ISSUES => ride_open(gate.phase),
        id::MI_TEAM_EDITOR => ride_open(gate.phase) && gate.teams_enabled,
        id::MI_AUDIT_TRAIL => ride_open(gate.phase) && gate.has_audit_row,
        id::MI_SIMULATION => can_simulate(gate.phase),
        id::MI_START_RIDE => can_start(gate.phase),
        id::MI_STOP_RIDE => matches!(gate.phase, RidePhase::Running),
        id::MI_SET_START_TIME
        | id::MI_FINISH_RIDE
        | id::MI_MARK_DNF
        | id::MI_ADD_CROSSING_AT
        | id::MI_DEAL_MANUAL => can_correct(gate.phase),
        id::MI_UNDO_CROSSING => matches!(gate.phase, RidePhase::Running) && gate.has_crossing,
        id::MI_CLEAR_RIDE => can_clear(gate.phase),
        id::MI_TIME_BOARD => gate.show_times,
        // A finished ride gates the exports, the publish route and reopening.
        id::MI_EXPORT_HTML
        | id::MI_EXPORT_PDF
        | id::MI_EXPORT_POSTER
        | id::MI_EXPORT_POSTER_HTML
        | id::MI_EXPORT_RESULTS_CSV
        | id::MI_PUBLISH_WORDPRESS
        | id::MI_REOPEN_RIDE => is_finished(gate.phase),
        id::MI_PREVIEW_HTML_BROWSER => is_finished(gate.phase) && gate.html_export,
        id::MI_PREVIEW_POSTER_HTML_BROWSER => is_finished(gate.phase) && gate.poster_export,
        id::MI_PREVIEW_PDF_BROWSER => is_finished(gate.phase) && gate.pdf_export,
        _ => false,
    }
}

/// Return whether the action is available in every ride state.
#[must_use]
fn always_enabled(action: ActionId) -> bool {
    matches!(
        action,
        id::MI_OPEN_LIBRARY
            | id::MI_BACKUP_NOW
            | id::WX_ID_PREFERENCES
            | id::WX_ID_EXIT
            | id::MI_STANDINGS
            | id::MI_SHOW_TIMES
            | id::MI_LAPS_BOARD
            | id::MI_FULL_FIELD
            | id::MI_ALL_CARDS
            | id::MI_SHOW_DNS_RIDERS
            | id::MI_SHOW_TOTAL_TIMES
            | id::MI_SHOW_LAP_TIME
            | id::MI_DENSITY_COMPACT
            | id::MI_DENSITY_COMFORTABLE
            | id::MI_TEXT_SIZE_TINY
            | id::MI_TEXT_SIZE_SMALL
            | id::MI_TEXT_SIZE_MEDIUM
            | id::MI_TEXT_SIZE_LARGE
            | id::MI_USER_GUIDE
            | id::MI_SHORTCUTS
            | id::MI_SELFTEST
            | id::WX_ID_ABOUT
    )
}

/// Return whether a ride is loaded in the phase.
#[must_use]
pub(crate) const fn ride_open(phase: RidePhase) -> bool {
    !matches!(phase, RidePhase::None)
}

/// Return whether the §15 simulation row is enabled: a draft ride, no ride, or a stopped ride.
const fn can_simulate(phase: RidePhase) -> bool {
    matches!(
        phase,
        RidePhase::None | RidePhase::Draft | RidePhase::RunningStopped
    )
}

/// Return whether the §15 start row is enabled: a draft, reopened or stopped ride.
const fn can_start(phase: RidePhase) -> bool {
    matches!(
        phase,
        RidePhase::Draft | RidePhase::Reopened | RidePhase::RunningStopped
    )
}

/// Return whether the §15 correction rows are enabled: a running or reopened ride.
const fn can_correct(phase: RidePhase) -> bool {
    matches!(phase, RidePhase::Running | RidePhase::Reopened)
}

/// Return whether the §15 clear row is enabled: a loaded ride except a live running one.
const fn can_clear(phase: RidePhase) -> bool {
    matches!(
        phase,
        RidePhase::Draft | RidePhase::RunningStopped | RidePhase::Finished | RidePhase::Reopened
    )
}

/// Return whether the ride is finished.
const fn is_finished(phase: RidePhase) -> bool {
    matches!(phase, RidePhase::Finished)
}

#[cfg(test)]
mod tests;
