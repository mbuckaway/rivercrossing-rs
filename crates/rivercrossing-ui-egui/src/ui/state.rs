// SPDX-License-Identifier: GPL-3.0-only

//! The app state that the views render and the §15 action gates read.

use std::path::PathBuf;

use rivercrossing_ride::{EntryMode, RideConfig, RideEngine, RideStatus, SystemClock};
use rivercrossing_roster::roster::Roster;
use rivercrossing_store::StoreError;
use rivercrossing_store::settings::AppSettings;
use uuid::Uuid;

use crate::ui::action::enable::{GateState, RidePhase};
use crate::ui::dialogs::stack::DialogStack;

#[cfg(test)]
mod tests;

/// The exports that exist for the open ride this session.
#[derive(Debug, Default)]
pub(crate) struct ExportPresence {
    /// Whether an HTML results page exists.
    pub(crate) html: bool,
    /// Whether a podium poster page exists.
    pub(crate) poster_html: bool,
    /// Whether a PDF results file exists.
    pub(crate) pdf: bool,
}

/// One loaded ride and the model that drives it.
#[derive(Debug)]
pub(crate) struct LoadedRide {
    /// The ride's database id.
    pub(crate) ride_id: i64,
    /// The ride's persisted setup.
    pub(crate) config: RideConfig,
    /// The roster the UI edits; a save persists it with `SaveRoster`.
    pub(crate) roster: Roster,
    /// The ride engine, rebuilt from the stored audit log.
    pub(crate) engine: RideEngine<Roster, SystemClock>,
}

/// The whole application state that the views render and the actions mutate.
#[derive(Debug)]
pub(crate) struct AppState {
    /// The persisted settings.
    pub(crate) settings: AppSettings,
    /// Where the settings are written.
    pub(crate) settings_path: PathBuf,
    /// The open ride, when one is loaded.
    pub(crate) ride: Option<LoadedRide>,
    /// The open dialogs.
    pub(crate) dialogs: DialogStack,
    /// The status notice, when one is posted.
    pub(crate) notice: Option<String>,
    /// The exports that exist this session.
    pub(crate) exports: ExportPresence,
    /// The number of audit rows that the last `AuditRows` reply held.
    pub(crate) audit_row_count: usize,
}

impl AppState {
    /// Return the ride state that the §15 action gates read.
    ///
    /// A stopped running ride reads as [`RidePhase::RunningStopped`], because
    /// the gate matrix distinguishes it from a live one.
    pub(crate) fn phase(&self) -> RidePhase {
        let Some(ride) = self.ride.as_ref() else {
            return RidePhase::None;
        };
        match ride.engine.state() {
            RideStatus::Draft => RidePhase::Draft,
            RideStatus::Running if ride.engine.stopped() => RidePhase::RunningStopped,
            RideStatus::Running => RidePhase::Running,
            RideStatus::Finished => RidePhase::Finished,
            RideStatus::Reopened => RidePhase::Reopened,
        }
    }

    /// Return the gate inputs for the §15 "Enabled when" matrix.
    pub(crate) fn gate(&self) -> GateState {
        GateState {
            phase: self.phase(),
            teams_enabled: self
                .ride
                .as_ref()
                .is_some_and(|ride| ride.config.entry_mode == EntryMode::Mixed),
            has_crossing: self
                .ride
                .as_ref()
                .is_some_and(|ride| !ride.engine.crossings().is_empty()),
            has_audit_row: self.audit_row_count > 0,
            html_export: self.exports.html,
            poster_export: self.exports.poster_html,
            pdf_export: self.exports.pdf,
            show_times: self.settings.publish_show_times,
        }
    }

    /// Set the status notice; an empty message clears it.
    pub(crate) fn note(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.notice = (!message.is_empty()).then_some(message);
    }

    /// Write the settings to the state file.
    ///
    /// # Errors
    ///
    /// Return the store's I/O or JSON failure when the write fails.
    pub(crate) fn save_settings(&mut self) -> Result<(), StoreError> {
        rivercrossing_store::save_settings(&self.settings, &self.settings_path)
    }
}

/// Return a new ride's seed: the low 63 bits of a version-4 UUID.
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    reason = "the low 64 bits of the 128-bit UUID are masked to 63 bits"
)]
pub(crate) fn new_ride_seed() -> u64 {
    Uuid::new_v4().as_u128() as u64 & ((1_u64 << 63) - 1)
}
