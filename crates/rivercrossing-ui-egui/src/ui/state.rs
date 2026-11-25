// SPDX-License-Identifier: GPL-3.0-only

//! The app state that the views render and the §15 action gates read.

use std::path::PathBuf;

use rivercrossing_ride::{EntryMode, RideConfig, RideEngine, RideStatus, SystemClock};
use rivercrossing_roster::roster::Roster;
use rivercrossing_store::StoreError;
use rivercrossing_store::settings::AppSettings;
use uuid::Uuid;

use time::Date;

use crate::ui::action::dispatch::dispatch;
use crate::ui::action::effect::{DispatchError, Effect};
use crate::ui::action::enable::{GateState, RidePhase};
use crate::ui::action::id::ActionId;
use crate::ui::action::spec::{CheckKind, CommandId, NativeDialog, Target, spec};
use crate::ui::dialogs::{self, stack::DialogStack};
use crate::ui::presenters::{console, setup};
use crate::ui::sound::Cue;

#[cfg(test)]
mod tests;

/// The refusal when the ride-setup dialog is not the open one.
const NO_SETUP_DIALOG: &str = "No ride setup dialog is open";

/// The setup form's preloaded planned duration, in seconds.
const DEFAULT_DURATION_S: i64 = 6 * 3_600;

/// The setup form's preloaded minimum lap, in seconds.
const DEFAULT_MIN_LAP_S: i64 = 20 * 60;

/// A native dialog an action raised, and the command that runs once it is confirmed.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PendingNative {
    /// The native dialog kind to raise.
    pub(crate) dialog: NativeDialog,
    /// The command to run when the dialog is confirmed.
    pub(crate) then: CommandId,
}

/// The exports that exist for the open ride this session.
#[derive(Debug, Default, PartialEq, Eq)]
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
    /// The plate-entry field's text.
    pub(crate) plate: String,
    /// The crossings feed's search text.
    pub(crate) search: String,
    /// Whether the review tab shows held cards only.
    pub(crate) held_only: bool,
    /// The review notebook's selected tab.
    pub(crate) console_tab: ConsoleTab,
    /// The cues that the console queued, until the app plays them.
    pub(crate) pending_cues: Vec<Cue>,
    /// The ride setup the operator submitted, until the app persists it.
    pub(crate) pending_create: Option<RideConfig>,
    /// The clock reading captured when the ride stopped.
    pub(crate) clock_frozen: Option<f64>,
    /// The ride-setup refusal that the dialog's infobar shows.
    pub(crate) setup_errors: Option<Vec<String>>,
}

/// The review notebook's two tabs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ConsoleTab {
    /// The live roster.
    #[default]
    Riders,
    /// The crossings that need review.
    NeedsReview,
}

#[cfg(test)]
impl AppState {
    /// Build an empty state over the default settings, for the unit tests.
    pub(crate) fn blank() -> Self {
        Self::new(rivercrossing_store::default_settings(), PathBuf::new())
    }
}

impl AppState {
    /// Build the launch state over *settings*, written to `settings_path`.
    pub(crate) fn new(settings: AppSettings, settings_path: PathBuf) -> Self {
        Self {
            settings,
            settings_path,
            ride: None,
            dialogs: DialogStack::default(),
            notice: None,
            exports: ExportPresence::default(),
            audit_row_count: 0,
            pending_native: None,
            pending_command: None,
            plate: String::new(),
            search: String::new(),
            held_only: false,
            console_tab: ConsoleTab::Riders,
            pending_cues: Vec::new(),
            pending_create: None,
            clock_frozen: None,
            setup_errors: None,
        }
    }

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
        if window == "ride_setup_dlg" {
            self.open_setup_dialog(time::OffsetDateTime::now_utc().date());
            return Ok(());
        }
        let Some(dialog) = dialogs::spec::spec(window) else {
            return Err(DispatchError::NotImplemented(window));
        };
        self.dialogs
            .open(dialog.id, None)
            .map_err(|_| DispatchError::NotImplemented(window))
    }

    // ------------------------------------------------------------- setup

    /// Open the ride-setup dialog with its preloaded values for *today*.
    pub(crate) fn open_setup_dialog(&mut self, today: Date) {
        let _ = self.dialogs.open("ride_setup_dlg", None);
        self.setup_errors = None;
        self.seed_setup_defaults(today);
    }

    /// Submit the open ride-setup dialog: validate its fields and store the config to persist.
    ///
    /// # Errors
    ///
    /// Return the field messages, the bound violation or the minimum-setup reasons when the form is
    /// refused. The dialog stays open so the operator can correct it.
    pub(crate) fn submit_setup(&mut self) -> Result<(), Vec<String>> {
        match self.validate_setup() {
            Ok(()) => {
                self.setup_errors = None;
                Ok(())
            }
            Err(errors) => {
                self.setup_errors = Some(errors.clone());
                Err(errors)
            }
        }
    }

    /// Validate the open setup dialog and store the config to persist.
    fn validate_setup(&mut self) -> Result<(), Vec<String>> {
        let Some(top) = self.dialogs.top() else {
            return Err(vec![NO_SETUP_DIALOG.to_owned()]);
        };
        if top.id != "ride_setup_dlg" {
            return Err(vec![NO_SETUP_DIALOG.to_owned()]);
        }
        let params = setup::params_from_fields(&top.fields)?;
        let config = setup::validated_config(&params)?;
        self.pending_create = Some(config);
        let _ = self.dialogs.cancel();
        Ok(())
    }

    /// Seed each ride-setup field with its preloaded value.
    fn seed_setup_defaults(&mut self, today: Date) {
        let defaults = setup::defaults();
        let seeds = [
            ("date_picker", format_date(today)),
            ("start_time_picker", "08:00".to_owned()),
            ("lap_km_spin", format!("{:.1}", defaults.lap_km)),
            ("duration_input", setup::format_duration(DEFAULT_DURATION_S)),
            ("min_lap_input", setup::format_min_lap(DEFAULT_MIN_LAP_S)),
            ("team_size_spin", defaults.team_size.to_string()),
            ("decks_spin", defaults.decks.to_string()),
            ("jokers_spin", defaults.jokers_per_deck.to_string()),
            ("cap_choice", "Disabled".to_owned()),
            ("hold_short_radio", "true".to_owned()),
            ("solo_radio", "true".to_owned()),
            ("pooled_radio", "true".to_owned()),
            ("jokers_total_radio", "true".to_owned()),
        ];
        for (field, value) in seeds {
            let _ = self.dialogs.set_field(field, value);
        }
    }

    // --------------------------------------------------------- plate entry

    /// Submit the plate-entry field through the console's entry rules.
    pub(crate) fn submit_plate(&mut self) {
        let Some(ride) = self.ride.as_mut() else {
            return;
        };
        let text = self.plate.clone();
        let outcome = console::plate_entered(&mut ride.engine, &text);
        if outcome.cleared {
            self.plate.clear();
        }
        if let Some(notice) = outcome.notice {
            self.note(notice);
        }
        if let Some(cue) = outcome.cue {
            self.pending_cues.push(cue);
        }
    }

    // ----------------------------------------------------------- lifecycle

    /// Start the ride, or open the blocked-start dialog when the R-79 gate refuses it.
    pub(crate) fn start_ride(&mut self) {
        let Some(ride) = self.ride.as_mut() else {
            return;
        };
        match console::start(&mut ride.engine) {
            Ok(None) => {
                self.clock_frozen = None;
                self.note(console::RIDE_STARTED);
            }
            Ok(Some(reasons)) => {
                self.note(reasons.join("; "));
                let _ = self.dialogs.open("start_blocked_dlg", None);
            }
            Err(message) => self.note(message),
        }
    }

    /// Stop the live ride and capture the clock reading that the display freezes at.
    pub(crate) fn stop_ride(&mut self) {
        let Some(ride) = self.ride.as_mut() else {
            return;
        };
        let frozen = ride.engine.elapsed().ok();
        match console::stop(&mut ride.engine) {
            Ok(message) => {
                self.clock_frozen = frozen;
                self.note(message);
            }
            Err(message) => self.note(message),
        }
    }

    /// Undo the newest crossing, or post why it cannot run.
    pub(crate) fn undo_crossing(&mut self) {
        let Some(ride) = self.ride.as_mut() else {
            self.note("Undo unavailable: nothing to undo");
            return;
        };
        let outcome = console::undo(&mut ride.engine);
        if let Some(notice) = outcome.notice {
            self.note(notice);
        }
    }

    /// Finish the ride.
    pub(crate) fn finish_ride(&mut self) {
        let Some(ride) = self.ride.as_mut() else {
            return;
        };
        match console::finish(&mut ride.engine, &[]) {
            Ok(message) => {
                self.clock_frozen = None;
                self.note(message);
            }
            Err(message) => self.note(message),
        }
    }

    /// Reopen the finished ride for corrections.
    pub(crate) fn reopen_ride(&mut self) {
        let Some(ride) = self.ride.as_mut() else {
            return;
        };
        let message = console::reopen(&mut ride.engine).unwrap_or_else(|error| error);
        self.note(message);
    }

    /// Confirm the pending native dialog: queue its command and clear the pending dialog.
    ///
    /// Return whether a native dialog was pending. The command runs on the app's next logic pass.
    pub(crate) fn confirm_native(&mut self) -> bool {
        let Some(native) = self.pending_native.take() else {
            return false;
        };
        self.pending_command = Some(native.then);
        true
    }

    /// Cancel the pending native dialog and run nothing.
    ///
    /// Return whether a native dialog was pending.
    pub(crate) fn cancel_native(&mut self) -> bool {
        self.pending_native.take().is_some()
    }

    /// Unload the ride from the console; the database is untouched.
    pub(crate) fn clear_ride(&mut self) {
        self.ride = None;
        self.exports = ExportPresence::default();
        self.audit_row_count = 0;
        self.clock_frozen = None;
        self.note("Ride cleared");
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

/// Return *date* as the setup form's `YYYY-MM-DD` text.
fn format_date(date: Date) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        date.year(),
        u8::from(date.month()),
        date.day()
    )
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
