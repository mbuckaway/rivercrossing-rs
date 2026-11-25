// SPDX-License-Identifier: GPL-3.0-only

//! The app state that the views render and the §15 action gates read.

use std::path::PathBuf;

use rivercrossing_ride::{EntryMode, RideConfig, RideEngine, RideStatus, SystemClock};
use rivercrossing_roster::roster::Roster;
use rivercrossing_store::StoreError;
use rivercrossing_store::settings::AppSettings;
use uuid::Uuid;

use crate::ui::action::dispatch::dispatch;
use crate::ui::action::effect::{DispatchError, Effect};
use crate::ui::action::enable::{GateState, RidePhase};
use crate::ui::action::id::ActionId;
use crate::ui::action::spec::{CheckKind, CommandId, NativeDialog, Target, spec};
use crate::ui::dialogs::{self, stack::DialogStack};

#[cfg(test)]
mod tests;

/// A native dialog an action raised, and the command that runs once it is confirmed.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PendingNative {
    /// The native dialog kind to raise.
    pub(crate) dialog: NativeDialog,
    /// The command to run when the dialog is confirmed.
    pub(crate) then: CommandId,
}

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
    /// The native dialog an action raised, until the app answers it.
    pub(crate) pending_native: Option<PendingNative>,
    /// The command an action queued, until the app runs it.
    pub(crate) pending_command: Option<CommandId>,
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

    /// Execute one action: gate it, dispatch it, then apply the effect to the state.
    ///
    /// `OpenWindow` opens the dialog when this release builds it, `RunCommand` stores
    /// [`Self::pending_command`], `ToggleCheck` flips its setting and saves, `SetZoom` stores the
    /// percentage and saves, and `OpenNative` stores [`Self::pending_native`].
    ///
    /// # Errors
    ///
    /// Returns [`DispatchError::UnknownAction`] for an id outside the action table,
    /// [`DispatchError::Disabled`] when the §15 gate refuses the action in this ride state, and
    /// [`DispatchError::NotImplemented`] when a window target is not built in this release.
    pub(crate) fn act(&mut self, id: ActionId) -> Result<Effect, DispatchError> {
        let effect = dispatch(id, &self.gate())?;
        self.apply(effect)?;
        Ok(effect)
    }

    /// Return whether the action's target exists in this release: a dialog this build renders, or
    /// a command, check, zoom or native dialog the app handles directly.
    #[must_use]
    #[expect(
        clippy::unused_self,
        reason = "the automation caller asks the state which actions this release implements"
    )]
    pub(crate) fn implemented(&self, id: ActionId) -> bool {
        match spec(id).map(|action| action.target) {
            Some(Target::Window(window)) => dialogs::spec::spec(window).is_some(),
            Some(_) => true,
            None => false,
        }
    }

    /// Apply one dispatched effect to the state.
    fn apply(&mut self, effect: Effect) -> Result<(), DispatchError> {
        match effect {
            Effect::OpenWindow(window) => self.open_window(window),
            Effect::RunCommand(command) => {
                self.pending_command = Some(command);
                Ok(())
            }
            Effect::ToggleCheck(check) => {
                self.toggle_check(check);
                Ok(())
            }
            Effect::SetZoom(percent) => {
                self.settings.zoom_percent = percent;
                self.persist_settings();
                Ok(())
            }
            Effect::OpenNative { dialog, then } => {
                self.pending_native = Some(PendingNative { dialog, then });
                Ok(())
            }
        }
    }

    /// Open the dialog a window target names, or refuse a window this release does not build.
    fn open_window(&mut self, window: &'static str) -> Result<(), DispatchError> {
        let Some(dialog) = dialogs::spec::spec(window) else {
            return Err(DispatchError::NotImplemented(window));
        };
        self.dialogs
            .open(dialog.id, None)
            .map_err(|_| DispatchError::NotImplemented(window))
    }

    /// Flip the setting that the check names and persist the settings.
    fn toggle_check(&mut self, check: CheckKind) {
        let field = self.check_field(check);
        *field = !*field;
        self.persist_settings();
    }

    /// Return the setting field that the check toggles.
    fn check_field(&mut self, check: CheckKind) -> &mut bool {
        match check {
            CheckKind::PublishShowTimes => &mut self.settings.publish_show_times,
            CheckKind::LapsBoard => &mut self.settings.publish_laps_board,
            CheckKind::TimeBoard => &mut self.settings.publish_time_board,
            CheckKind::FullField => &mut self.settings.publish_full_field,
            CheckKind::AllCards => &mut self.settings.publish_all_cards,
            CheckKind::ShowDnsRiders => &mut self.settings.show_dns_riders,
            CheckKind::ShowTotalTimes => &mut self.settings.show_total_times,
            CheckKind::ShowLapTime => &mut self.settings.show_lap_time,
        }
    }

    /// Save the settings, and post the store's message as the notice when the write fails.
    fn persist_settings(&mut self) {
        if let Err(error) = self.save_settings() {
            self.note(error.to_string());
        }
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
