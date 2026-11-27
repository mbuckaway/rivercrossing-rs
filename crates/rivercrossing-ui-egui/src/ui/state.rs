// SPDX-License-Identifier: GPL-3.0-only

//! The app state that the views render and the §15 action gates read.

use std::path::{Path, PathBuf};

use rivercrossing_cards::Card;
use rivercrossing_csvio::ImportPreview;
use rivercrossing_ride::{EntryId, EntryMode, RideConfig, RideEngine, RideStatus, SystemClock};
use rivercrossing_roster::enums::EntryType;
use rivercrossing_roster::roster::Roster;
use rivercrossing_roster::types::Rider;
use rivercrossing_store::settings::AppSettings;
use rivercrossing_store::{AuditRow, PreviousSession, RideRow, StoreError};
use uuid::Uuid;

use time::{Date, OffsetDateTime};

use crate::ui::action::dispatch::dispatch;
use crate::ui::action::effect::{DispatchError, Effect};
use crate::ui::action::enable::{GateState, RidePhase};
use crate::ui::action::id::ActionId;
use crate::ui::action::spec::{CheckKind, CommandId, NativeDialog, Target, spec};
use crate::ui::dialogs::{self, stack::DialogStack};
use crate::ui::presenters::{
    audit, console, corrections, library, publish_wordpress, quit_resume, rider_issues, riders,
    selftest, settings_form, setup, teams,
};
use crate::ui::sound::Cue;
use crate::ui::worker::StoreCommand;

#[cfg(test)]
mod tests;

pub(crate) mod exports;

/// The refusal when the ride-setup dialog is not the open one.
const NO_SETUP_DIALOG: &str = "No ride setup dialog is open";

/// The setup form's preloaded planned duration, in seconds.
const DEFAULT_DURATION_S: i64 = 6 * 3_600;

/// The setup form's preloaded minimum lap, in seconds.
const DEFAULT_MIN_LAP_S: i64 = 20 * 60;

/// The refusal when no ride is open.
const NO_RIDE: &str = "Open a ride first";

/// The refusal when the editor has no selected rider.
const SELECT_RIDER: &str = "Select a rider first";

/// The refusal when the teams editor has no selected team.
const SELECT_TEAM: &str = "Select a team first";

/// The refusal when the issues dialog has no selected issue.
const SELECT_ISSUE: &str = "Select an issue first";

/// The notice for a route that still needs the native file picker.
pub(crate) const FILE_PICKER_UNAVAILABLE: &str = "a file picker is not available in this release";

/// What a confirmed native dialog does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NativeTask {
    /// Run the queued command.
    Command(CommandId),
    /// Duplicate the ride that *id* names.
    DuplicateRide {
        /// The stored ride id.
        id: i64,
    },
    /// Delete the crossing at *entry* and lap *seq*.
    DeleteCrossing {
        /// The entry that owns the crossing.
        entry: EntryId,
        /// The per-entry lap number.
        seq: u32,
    },
    /// Delete the rider at `rider_index` on `entry`.
    DeleteRider {
        /// The entry that owns the rider.
        entry: EntryId,
        /// The rider's index on the entry.
        rider_index: usize,
    },
    /// Remove the team that `entry` names.
    RemoveTeam {
        /// The team's entry id.
        entry: EntryId,
    },
    /// Ask the save picker where to write one export.
    ExportFile {
        /// The file to write.
        kind: exports::ExportKind,
    },
    /// Choose the organizer logo through the platform's open picker.
    ChooseLogo,
    /// Open the page that a successful publish reported.
    OpenPublished {
        /// The page's public URL.
        link: String,
    },
    /// Publish the kept form again after a failure.
    RetryPublish,
}

impl NativeTask {
    /// Return a stable key for the confirmation's identity and the launch log.
    #[must_use]
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Command(command) => command.as_str(),
            Self::DuplicateRide { .. } => "duplicate_ride",
            Self::DeleteRider { .. } => "delete_rider",
            Self::RemoveTeam { .. } => "remove_team",
            Self::DeleteCrossing { .. } => "delete_crossing",
            Self::ExportFile { kind } => match kind {
                exports::ExportKind::Html => "export_html",
                exports::ExportKind::Pdf => "export_pdf",
                exports::ExportKind::Poster => "export_poster",
                exports::ExportKind::PosterHtml => "export_poster_html",
                exports::ExportKind::Csv => "export_csv",
            },
            Self::ChooseLogo => "choose_logo",
            Self::OpenPublished { .. } => "open_published",
            Self::RetryPublish => "retry_publish",
        }
    }
}

/// A native dialog an action raised, and the task that runs once it is confirmed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PendingNative {
    /// The native dialog kind to raise.
    pub(crate) dialog: NativeDialog,
    /// The task to run when the dialog is confirmed.
    pub(crate) then: NativeTask,
}

/// One rider-editor row and the roster record it names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RiderEditorRow {
    /// The rendered Plate, Name and Team cells.
    pub(crate) row: riders::RiderRow,
    /// The entry that owns the rider.
    pub(crate) entry: EntryId,
    /// The rider's index on the entry.
    pub(crate) rider_index: usize,
}

/// The rider record an editor action acts on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RiderSelection {
    /// The entry that owns the rider.
    pub(crate) entry: EntryId,
    /// The rider's index on the entry.
    pub(crate) rider_index: usize,
}

/// The rider editor's cached model.
#[derive(Clone, Debug, Default)]
pub(crate) struct RiderEditorModel {
    /// Every rider row, in roster order.
    pub(crate) rows: Vec<RiderEditorRow>,
    /// The list's search text.
    pub(crate) search: String,
    /// The selected record.
    pub(crate) selected: Option<RiderSelection>,
    /// The record the Add/Edit dialog is editing; `None` adds a new rider.
    pub(crate) editing: Option<RiderSelection>,
    /// The editor's or the add dialog's refusal.
    pub(crate) errors: Option<String>,
}

impl RiderEditorModel {
    /// Return the indices of the rows that the search keeps.
    #[must_use]
    pub(crate) fn visible(&self) -> Vec<usize> {
        self.rows
            .iter()
            .enumerate()
            .filter(|(_, editor)| riders::row_matches(&editor.row, &self.search))
            .map(|(index, _)| index)
            .collect()
    }
}

/// One team-editor row and the entry it names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TeamEditorRow {
    /// The rendered Name, Plate and Members cells.
    pub(crate) row: teams::TeamRow,
    /// The team's entry id.
    pub(crate) entry: EntryId,
}

/// The teams editor's cached model.
#[derive(Clone, Debug, Default)]
pub(crate) struct TeamEditorModel {
    /// Every team row, in roster order.
    pub(crate) rows: Vec<TeamEditorRow>,
    /// The selected team.
    pub(crate) selected: Option<EntryId>,
    /// The team the Add/Edit dialog is editing; `None` adds a new team.
    pub(crate) editing: Option<EntryId>,
    /// The editor's or the add dialog's refusal.
    pub(crate) errors: Option<String>,
}

/// The CSV preview dialog's cached state.
#[derive(Clone, Debug, Default)]
pub(crate) struct CsvPreviewState {
    /// The previewed file, when one was chosen.
    pub(crate) path: Option<PathBuf>,
    /// The preview that Import commits.
    pub(crate) preview: Option<ImportPreview>,
    /// The rendered summary, conflicts and Import gate.
    pub(crate) model: Option<riders::CsvPreviewModel>,
    /// The preview's refusal.
    pub(crate) errors: Option<String>,
}

/// The rider-issues dialog's cached model.
#[derive(Clone, Debug, Default)]
pub(crate) struct RiderIssuesModel {
    /// Every reported issue with a fix, in the checker's order.
    pub(crate) rows: Vec<rider_issues::IssueRow>,
    /// The selected row.
    pub(crate) selected: Option<usize>,
    /// The dialog's refusal.
    pub(crate) errors: Option<String>,
}

/// The ride library's cached rows and selection.
#[derive(Clone, Debug, Default)]
pub(crate) struct LibraryModel {
    /// The rendered rows, oldest first as the store returns them.
    pub(crate) rows: Vec<library::RideRowModel>,
    /// The stored rows the rendered rows came from.
    pub(crate) store_rows: Vec<RideRow>,
    /// The selected row.
    pub(crate) selected: Option<usize>,
    /// The delete confirmation's instruction line.
    pub(crate) delete_message: Option<String>,
    /// The ride the delete dialog is about.
    pub(crate) delete_id: Option<i64>,
}

/// The audit viewer's cached rows and filters.
#[derive(Clone, Debug)]
pub(crate) struct AuditModel {
    /// The stored rows the viewer renders.
    pub(crate) store_rows: Vec<AuditRow>,
    /// The rendered rows, newest first.
    pub(crate) rows: Vec<audit::AuditRowModel>,
    /// The search text.
    pub(crate) search: String,
    /// The selected action filter.
    pub(crate) action: String,
}

impl Default for AuditModel {
    fn default() -> Self {
        Self {
            store_rows: Vec::new(),
            rows: Vec::new(),
            search: String::new(),
            action: audit::ALL_ACTIONS.to_owned(),
        }
    }
}

/// The correction dialog's target and mode.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct CorrectionTarget {
    /// The crossing being corrected, when editing one.
    pub(crate) entry: Option<EntryId>,
    /// The per-entry lap number, when editing one.
    pub(crate) seq: Option<u32>,
    /// Whether the edit-crossing dialog adds a crossing instead of changing one.
    pub(crate) add: bool,
}

/// The Crossing Detail dialog's open state.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CrossingDetailState {
    /// The crossing's entry.
    pub(crate) entry: EntryId,
    /// The per-entry lap number.
    pub(crate) seq: u32,
    /// The rendered cells.
    pub(crate) model: corrections::CrossingDetailModel,
    /// The four button gates.
    pub(crate) actions: corrections::CorrectionActions,
    /// The crossing's credited card, when it holds one.
    pub(crate) card: Option<Card>,
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
    /// The last HTML results page written this session.
    pub(crate) last_html: Option<PathBuf>,
    /// The last podium poster page written this session.
    pub(crate) last_poster_html: Option<PathBuf>,
    /// The last PDF written this session.
    pub(crate) last_pdf: Option<PathBuf>,
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
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is an independent UI toggle or lifecycle latch with its own reader"
)]
pub(crate) struct AppState {
    /// The persisted settings.
    pub(crate) settings: AppSettings,
    /// Where the settings are written.
    pub(crate) settings_path: PathBuf,
    /// The open ride, when one is loaded.
    pub(crate) ride: Option<LoadedRide>,
    /// The organizer logos already decoded, so a tile does not decode on every frame.
    pub(crate) logos: crate::ui::logo::LogoTextures,
    /// The setup form's tie-break order, in stored spellings, first to last. The design's
    /// `tiebreak_list` is a control rather than a frozen field, so the dialog keeps it here.
    pub(crate) setup_tiebreak: [&'static str; 3],
    /// The setup form's chosen logo path, for the same reason.
    pub(crate) setup_logo: String,
    /// Set by the setup form's Browse button: the next logic pass raises the platform file picker.
    pub(crate) choose_logo: bool,
    /// The open dialogs.
    pub(crate) dialogs: DialogStack,
    /// The status notice, when one is posted.
    pub(crate) notice: Option<String>,
    /// The exports that exist this session.
    pub(crate) exports: ExportPresence,
    /// The results window's cached model and export watermark.
    pub(crate) results: exports::ResultsState,
    /// The export job that the next logic pass sends to the worker.
    pub(crate) pending_export: Option<exports::ExportJob>,
    /// The validated publish form, kept for a Retry.
    pub(crate) publish_form: Option<publish_wordpress::PublishForm>,
    /// The publish form's validation messages.
    pub(crate) publish_errors: Option<Vec<String>>,
    /// The progress window's status line.
    pub(crate) publish_status: String,
    /// The publish that the app must start on its own thread.
    pub(crate) pending_publish: Option<exports::PublishStart>,
    /// Set when the operator cancels the running publish.
    pub(crate) publish_cancel: bool,
    /// The simulator's infobar message.
    pub(crate) sim_message: Option<String>,
    /// The simulation progress window's status line.
    pub(crate) sim_status: String,
    /// The simulation's completed fraction, 0.0..=1.0.
    pub(crate) sim_progress: f32,
    /// The simulation that the app must start on its own thread.
    pub(crate) pending_sim: Option<exports::SimStart>,
    /// Set when the operator cancels the running simulation.
    pub(crate) sim_cancel: bool,
    /// The number of audit rows that the last `AuditRows` reply held.
    pub(crate) audit_row_count: usize,
    /// The native dialog an action raised, until the app answers it.
    pub(crate) pending_native: Option<PendingNative>,
    /// Whether the host cannot show a native dialog, so the app shows the pending one in-app.
    pub(crate) native_in_app: bool,
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
    /// The rider editor's cached model.
    pub(crate) rider_editor: RiderEditorModel,
    /// The teams editor's cached model.
    pub(crate) team_editor: TeamEditorModel,
    /// The CSV preview's cached state.
    pub(crate) csv_preview: CsvPreviewState,
    /// The rider-issues dialog's cached model.
    pub(crate) rider_issues: RiderIssuesModel,
    /// The store commands that the next logic pass sends to the worker.
    pub(crate) pending_store: Vec<StoreCommand>,
    /// The ride library's cached model.
    pub(crate) library: LibraryModel,
    /// The audit viewer's cached model.
    pub(crate) audit: AuditModel,
    /// The settings dialog's form values, while it is open.
    pub(crate) settings_form: Option<settings_form::FormValues>,
    /// The settings dialog's refusals.
    pub(crate) settings_errors: Option<Vec<String>>,
    /// The self-test dialog's canvas lines.
    pub(crate) selftest_lines: Vec<String>,
    /// The correction dialog's target and mode.
    pub(crate) correction: CorrectionTarget,
    /// The Crossing Detail dialog's open state.
    pub(crate) crossing_detail: Option<CrossingDetailState>,
    /// The previous session's resume record.
    pub(crate) previous_session: Option<PreviousSession>,
    /// Whether the launch prompt has been decided.
    pub(crate) launch_resolved: bool,
    /// The resume dialog's message, while it is open.
    pub(crate) launch_message: Option<String>,
    /// The quit dialog's message, while it is open.
    pub(crate) quit_message: Option<String>,
    /// The last hour bucket that a backup ran in.
    pub(crate) last_backup_hour: Option<i64>,
    /// Whether the next finished backup was asked for by the operator, so it earns a notice.
    /// The startup and hourly backups write silently: the status bar is the ride's, not the store's.
    pub(crate) announce_backup: bool,
    /// Whether the app may close (the operator confirmed).
    pub(crate) close_requested: bool,
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
    #[expect(
        clippy::too_many_lines,
        reason = "the whole persisted default in one literal, so no field is left at its old value"
    )]
    pub(crate) fn new(settings: AppSettings, settings_path: PathBuf) -> Self {
        Self {
            settings,
            settings_path,
            ride: None,
            dialogs: DialogStack::default(),
            logos: crate::ui::logo::LogoTextures::default(),
            setup_tiebreak: ["high_card", "laps", "total_time"],
            setup_logo: String::new(),
            choose_logo: false,
            notice: None,
            exports: ExportPresence::default(),
            results: exports::ResultsState::default(),
            pending_export: None,
            publish_form: None,
            publish_errors: None,
            publish_status: String::new(),
            pending_publish: None,
            publish_cancel: false,
            sim_message: None,
            sim_status: String::new(),
            sim_progress: 0.0,
            pending_sim: None,
            sim_cancel: false,
            audit_row_count: 0,
            pending_native: None,
            native_in_app: false,
            pending_command: None,
            plate: String::new(),
            search: String::new(),
            held_only: false,
            console_tab: ConsoleTab::Riders,
            pending_cues: Vec::new(),
            pending_create: None,
            clock_frozen: None,
            setup_errors: None,
            rider_editor: RiderEditorModel::default(),
            team_editor: TeamEditorModel::default(),
            csv_preview: CsvPreviewState::default(),
            rider_issues: RiderIssuesModel::default(),
            pending_store: Vec::new(),
            library: LibraryModel::default(),
            audit: AuditModel::default(),
            settings_form: None,
            settings_errors: None,
            selftest_lines: Vec::new(),
            correction: CorrectionTarget::default(),
            crossing_detail: None,
            previous_session: None,
            launch_resolved: false,
            launch_message: None,
            quit_message: None,
            last_backup_hour: None,
            announce_backup: false,
            close_requested: false,
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
                if then == CommandId::AppQuit {
                    // The quit flow chooses the running dialog or the plain confirm itself.
                    self.request_quit();
                } else {
                    self.raise_native(dialog, NativeTask::Command(then));
                }
                Ok(())
            }
        }
    }

    /// Open the dialog a window target names, or refuse a window this release does not build.
    fn open_window(&mut self, window: &'static str) -> Result<(), DispatchError> {
        match window {
            "ride_setup_dlg" => self.open_setup_dialog(time::OffsetDateTime::now_utc().date()),
            "rider_editor_dlg" => self.open_rider_editor(),
            "team_editor_dlg" => self.open_team_editor(),
            "rider_issues_dlg" => self.open_rider_issues(),
            // The OS file picker arrives with PR D's craft_dialogs; until then the Import row
            // names the missing piece instead of opening an empty preview.
            "csv_preview_dlg" => self.note(FILE_PICKER_UNAVAILABLE),
            "ride_library_dlg" => self.open_library(),
            "audit_dlg" => self.open_audit(),
            "settings_dlg" => self.open_settings(),
            "selftest_dlg" => self.open_selftest(),
            "edit_crossing_dlg" => self.open_add_crossing(),
            "manual_deal_dlg" => self.open_manual_deal(),
            "dnf_confirm_dlg" => self.open_dnf(),
            "set_start_dlg" => self.open_set_start(),
            "void_card_confirm_dlg" => self.open_void_card(),
            "crossing_number_dlg" => self.open_crossing_number(),
            "crossing_detail_dlg" => {}
            "results_dlg" => self.open_results(),
            "publish_wordpress_dlg" => self.open_publish(),
            "simulation_dlg" => self.open_simulation(),
            _ => {
                let Some(dialog) = dialogs::spec::spec(window) else {
                    return Err(DispatchError::NotImplemented(window));
                };
                self.dialogs
                    .open(dialog.id, None)
                    .map_err(|_| DispatchError::NotImplemented(window))?;
            }
        }
        Ok(())
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
        // The two controls the design keeps outside the field map join the presenter's input here.
        let mut fields = top.fields.clone();
        let _ = fields.insert("tiebreak_list".to_owned(), self.setup_tiebreak.join(","));
        let _ = fields.insert("logo_preview_bmp".to_owned(), self.setup_logo.clone());
        let params = setup::params_from_fields(&fields)?;
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
        self.setup_tiebreak = ["high_card", "laps", "total_time"];
        self.setup_logo.clear();
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
                ride.roster.set_status(ride.engine.state());
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
                ride.roster.set_status(ride.engine.state());
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
                ride.roster.set_status(ride.engine.state());
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
        ride.roster.set_status(ride.engine.state());
        self.note(message);
    }

    /// Raise a native dialog and clear the in-app fallback flag.
    pub(crate) fn raise_native(&mut self, dialog: NativeDialog, then: NativeTask) {
        self.pending_native = Some(PendingNative { dialog, then });
        self.native_in_app = false;
    }

    /// Confirm the pending native dialog: queue its command and clear the pending dialog.
    ///
    /// Return whether a native dialog was pending. The command runs on the app's next logic pass.
    pub(crate) fn confirm_native(&mut self) -> bool {
        let Some(native) = self.pending_native.take() else {
            return false;
        };
        self.native_in_app = false;
        self.run_native_task(native.then);
        true
    }

    /// Cancel the pending native dialog and run nothing.
    ///
    /// Return whether a native dialog was pending.
    pub(crate) fn cancel_native(&mut self) -> bool {
        self.native_in_app = false;
        self.pending_native.take().is_some()
    }

    // ------------------------------------------------------------ editors

    /// Return the open ride's roster.
    fn roster(&self) -> Option<&Roster> {
        self.ride.as_ref().map(|ride| &ride.roster)
    }

    /// Queue the roster write and the audit rows that follow one mutation, then reload the ride so
    /// the engine's roster view matches the saved model.
    fn persist_roster(&mut self) {
        let Some(ride) = self.ride.as_mut() else {
            return;
        };
        let id = ride.ride_id;
        let events = ride.roster.take_audit_log();
        let roster = ride.roster.clone();
        for event in events {
            self.pending_store.push(StoreCommand::AppendRosterEvent {
                id,
                event: Box::new(event),
            });
        }
        self.pending_store.push(StoreCommand::SaveRoster {
            id,
            roster: Box::new(roster),
        });
        self.pending_store.push(StoreCommand::LoadRide { id });
    }

    /// Queue the engine events that a live roster move appended.
    fn persist_engine_events(&mut self, from: usize) {
        let Some(ride) = self.ride.as_ref() else {
            return;
        };
        let id = ride.ride_id;
        let events = ride
            .engine
            .events()
            .get(from..)
            .unwrap_or_default()
            .to_vec();
        for event in events {
            self.pending_store.push(StoreCommand::Append {
                id,
                event: Box::new(event),
            });
        }
    }

    /// Rebuild each editor model from the open ride.
    pub(crate) fn refresh_models(&mut self) {
        self.refresh_rider_editor();
        self.refresh_team_editor();
        self.refresh_rider_issues();
    }

    /// Submit the open form dialog, when the top dialog is one of the Add/Setup forms.
    ///
    /// Return `None` when the top dialog is not a form. A refusal keeps the dialog open and is
    /// reported in its infobar, exactly as the OK button does.
    pub(crate) fn submit_top_form(&mut self) -> Option<Result<(), String>> {
        let id = self.dialogs.top()?.id;
        match id {
            "ride_setup_dlg" => Some(self.submit_setup().map_err(|errors| errors.join("; "))),
            "add_rider_dlg" if self.rider_editor.editing.is_some() => {
                Some(self.submit_edit_rider())
            }
            "add_rider_dlg" => Some(self.submit_add_rider()),
            "add_team_dlg" if self.team_editor.editing.is_some() => Some(self.submit_edit_team()),
            "add_team_dlg" => Some(self.submit_add_team()),
            "csv_preview_dlg" => Some(self.submit_csv_import()),
            "settings_dlg" => Some(self.submit_settings().map_err(|errors| errors.join("; "))),
            "delete_ride_dlg" => Some(self.submit_delete_ride()),
            "edit_crossing_dlg" => Some(self.submit_edit_crossing()),
            "manual_deal_dlg" => Some(self.submit_manual_deal()),
            "dnf_confirm_dlg" => Some(self.submit_dnf()),
            "void_card_confirm_dlg" => Some(self.submit_void_card()),
            "crossing_number_dlg" => Some(self.submit_crossing_number()),
            "set_start_dlg" => Some(self.submit_set_start()),
            "ride_library_dlg" => Some(self.open_library_selected()),
            "publish_wordpress_dlg" => Some(self.submit_publish()),
            "simulation_dlg" => Some(self.submit_simulation()),
            _ => None,
        }
    }

    /// Open the rider editor on a fresh model.
    pub(crate) fn open_rider_editor(&mut self) {
        let _ = self.dialogs.open("rider_editor_dlg", None);
        self.rider_editor = RiderEditorModel::default();
        self.refresh_rider_editor();
    }

    /// Rebuild the rider editor's rows, keeping a selection that still exists.
    pub(crate) fn refresh_rider_editor(&mut self) {
        self.rider_editor.rows = self.roster().map_or_else(Vec::new, editor_rows);
        if !self.rider_selection_exists() {
            self.rider_editor.selected = None;
        }
    }

    /// Return whether the selected rider record still exists.
    fn rider_selection_exists(&self) -> bool {
        let Some(selection) = self.rider_editor.selected.as_ref() else {
            return false;
        };
        self.roster().is_some_and(|roster| {
            roster
                .entry_by_key(&selection.entry.0)
                .is_some_and(|entry| selection.rider_index < entry.riders.len())
        })
    }

    /// Open the rider editor at the rider whose plate is *plate*.
    ///
    /// The console's Riders tab and its Needs Review rows use this: a team relay row carries the
    /// entry's plate, so the first row with that plate is the record the operator picked.
    pub(crate) fn open_rider_at(&mut self, plate: &str) {
        if self
            .dialogs
            .top()
            .is_none_or(|dialog| dialog.id != "rider_editor_dlg")
        {
            let _ = self.dialogs.open("rider_editor_dlg", None);
        }
        self.rider_editor.search.clear();
        self.refresh_rider_editor();
        self.rider_editor.selected = self
            .rider_editor
            .rows
            .iter()
            .find(|row| row.row.plate == plate)
            .map(|row| RiderSelection {
                entry: row.entry.clone(),
                rider_index: row.rider_index,
            });
    }

    /// Open the Add rider form with the next free plate.
    pub(crate) fn open_add_rider(&mut self) {
        let Some(plate) = self.roster().map(Roster::next_free_plate) else {
            return;
        };
        self.rider_editor.editing = None;
        self.rider_editor.errors = None;
        let _ = self.dialogs.open("add_rider_dlg", None);
        for (field, value) in [
            ("plate_input", plate),
            ("first_name_input", String::new()),
            ("last_name_input", String::new()),
            ("team_choice", "solo".to_owned()),
            ("sex_choice", String::new()),
        ] {
            let _ = self.dialogs.set_field(field, value);
        }
    }

    /// Open the Add rider form on the selected record.
    pub(crate) fn open_edit_rider(&mut self) {
        let Some(selection) = self.rider_editor.selected.clone() else {
            return;
        };
        let Some(record) = self.rider_record(&selection) else {
            return;
        };
        self.rider_editor.editing = Some(selection);
        self.rider_editor.errors = None;
        let _ = self.dialogs.open("add_rider_dlg", None);
        for (field, value) in [
            ("plate_input", record.plate),
            ("first_name_input", record.first),
            ("last_name_input", record.last),
            ("team_choice", record.team),
            ("sex_choice", record.sex.unwrap_or_default()),
        ] {
            let _ = self.dialogs.set_field(field, value);
        }
    }

    /// Return the record that *selection* names.
    pub(crate) fn rider_record(&self, selection: &RiderSelection) -> Option<RiderRecord> {
        let entry = self.roster()?.entry_by_key(&selection.entry.0)?;
        let rider = entry.riders.get(selection.rider_index)?;
        Some(RiderRecord {
            plate: rider.plate.clone().unwrap_or_default(),
            first: rider.first_name.clone(),
            last: rider.last_name.clone(),
            team: if entry.kind == EntryType::Team {
                entry.display_name.clone()
            } else {
                "solo".to_owned()
            },
            sex: rider.sex.clone(),
        })
    }

    /// Return the record's current team and the rider's own plate.
    fn rider_facts(&self, selection: &RiderSelection) -> Option<RiderFacts> {
        let entry = self.roster()?.entry_by_key(&selection.entry.0)?;
        let rider = entry.riders.get(selection.rider_index)?;
        Some(RiderFacts {
            team: (entry.kind == EntryType::Team).then(|| entry.display_name.clone()),
            rider_plate: rider.plate.clone(),
        })
    }

    /// Read the Add/Edit rider form.
    fn rider_form(&self) -> Result<riders::RiderFormValues, String> {
        if self
            .dialogs
            .top()
            .is_none_or(|dialog| dialog.id != "add_rider_dlg")
        {
            return Err("The Add rider dialog is not open".to_owned());
        }
        let field = |id: &str| self.dialogs.field(id).unwrap_or_default().to_owned();
        let team_text = field("team_choice");
        let team = if team_text.trim().is_empty() || team_text.trim() == "solo" {
            riders::TeamChoice::Solo
        } else {
            riders::TeamChoice::Team(team_text)
        };
        let sex = field("sex_choice").trim().to_owned();
        Ok(riders::RiderFormValues {
            plate: field("plate_input"),
            first_name: field("first_name_input"),
            last_name: field("last_name_input"),
            team,
            sex: (!sex.is_empty()).then_some(sex),
        })
    }

    /// Submit the Add rider form.
    ///
    /// # Errors
    ///
    /// Return the form's refusal (blank names, a duplicate plate, a full team) and keep the form
    /// open with the message in its infobar.
    pub(crate) fn submit_add_rider(&mut self) -> Result<(), String> {
        let form = self.rider_form()?;
        let added = self
            .ride
            .as_mut()
            .map(|ride| riders::add_rider(&mut ride.roster, &form));
        match added {
            Some(Ok(entry)) => {
                self.rider_editor.errors = None;
                let index = self.rider_last_index(&entry);
                self.persist_roster();
                self.refresh_rider_editor();
                self.rider_editor.selected = Some(RiderSelection {
                    entry,
                    rider_index: index,
                });
                let _ = self.dialogs.cancel();
                self.note("Rider added");
                Ok(())
            }
            Some(Err(message)) => {
                self.rider_editor.errors = Some(message.clone());
                Err(message)
            }
            None => Err(NO_RIDE.to_owned()),
        }
    }

    /// Return the index of the entry's last rider, where a move lands.
    fn rider_last_index(&self, entry: &EntryId) -> usize {
        self.roster()
            .and_then(|roster| roster.entry_by_key(&entry.0))
            .map_or(0, |entry| entry.riders.len().saturating_sub(1))
    }

    /// Submit the Add/Edit rider form; a live team change moves the rider through the engine.
    ///
    /// # Errors
    ///
    /// Return the form's refusal, the live-move refusal ("Stop the ride first" for a running,
    /// unstopped ride) or the roster's own refusal, and keep the form open with the message.
    pub(crate) fn submit_edit_rider(&mut self) -> Result<(), String> {
        let form = self.rider_form()?;
        let Some(selection) = self
            .rider_editor
            .editing
            .clone()
            .or_else(|| self.rider_editor.selected.clone())
        else {
            return Err(SELECT_RIDER.to_owned());
        };
        let Some(facts) = self.rider_facts(&selection) else {
            return Err(SELECT_RIDER.to_owned());
        };
        let live = self
            .ride
            .as_ref()
            .is_some_and(|ride| riders::is_live_move(&ride.engine, &form, facts.team.as_deref()));
        if live {
            self.commit_live_edit(&facts, &form)?;
        }
        self.apply_rider_edit(&selection, &form)
    }

    /// Write the form back through the presenter and refresh the model.
    ///
    /// # Errors
    ///
    /// Return the presenter's refusal and keep the form open with the message.
    fn apply_rider_edit(
        &mut self,
        selection: &RiderSelection,
        form: &riders::RiderFormValues,
    ) -> Result<(), String> {
        let edited = self.ride.as_mut().map(|ride| {
            riders::edit_rider(
                &mut ride.roster,
                &selection.entry,
                selection.rider_index,
                form,
            )
        });
        match edited {
            Some(Ok(())) => {
                self.rider_editor.errors = None;
                self.rider_editor.editing = None;
                self.persist_roster();
                self.refresh_rider_editor();
                self.rider_editor.selected = self
                    .rider_editor
                    .rows
                    .iter()
                    .find(|row| row.row.plate == form.plate)
                    .map(|row| RiderSelection {
                        entry: row.entry.clone(),
                        rider_index: row.rider_index,
                    });
                let _ = self.dialogs.cancel();
                self.note("Rider updated");
                Ok(())
            }
            Some(Err(message)) => {
                self.rider_editor.errors = Some(message.clone());
                Err(message)
            }
            None => Err(NO_RIDE.to_owned()),
        }
    }

    /// Move the edited rider through the engine on a live ride.
    ///
    /// # Errors
    ///
    /// Return the no-ride refusal and the presenter's live-move refusal.
    fn commit_live_edit(
        &mut self,
        facts: &RiderFacts,
        form: &riders::RiderFormValues,
    ) -> Result<(), String> {
        let plate = facts.rider_plate.clone().unwrap_or_default();
        let from = self
            .ride
            .as_ref()
            .map_or(0, |ride| ride.engine.events().len());
        let moved = self
            .ride
            .as_mut()
            .map(|ride| riders::commit_live_move(&mut ride.engine, &plate, form));
        match moved {
            Some(Ok(_reason)) => {
                self.persist_engine_events(from);
                Ok(())
            }
            Some(Err(message)) => {
                self.rider_editor.errors = Some(message.clone());
                Err(message)
            }
            None => Err(NO_RIDE.to_owned()),
        }
    }

    /// Delete the selected rider.
    ///
    /// # Errors
    ///
    /// Return the select-a-rider refusal and the roster's own delete lock (recorded data, or a ride
    /// past DRAFT).
    pub(crate) fn delete_selected_rider(&mut self) -> Result<(), String> {
        let Some(selection) = self.rider_editor.selected.clone() else {
            return Err(SELECT_RIDER.to_owned());
        };
        let deleted = self.ride.as_mut().map(|ride| {
            riders::delete_rider(&mut ride.roster, &selection.entry, selection.rider_index)
        });
        match deleted {
            Some(Ok(())) => {
                self.rider_editor.errors = None;
                self.rider_editor.selected = None;
                self.persist_roster();
                self.refresh_rider_editor();
                self.note("Rider deleted");
                Ok(())
            }
            Some(Err(message)) => {
                self.rider_editor.errors = Some(message.clone());
                Err(message)
            }
            None => Err(NO_RIDE.to_owned()),
        }
    }

    /// Raise the native warning that guards the delete.
    ///
    /// Return whether a rider was selected.
    pub(crate) fn raise_delete_rider(&mut self) -> bool {
        let Some(selection) = self.rider_editor.selected.clone() else {
            return false;
        };
        self.raise_native(
            NativeDialog::Danger,
            NativeTask::DeleteRider {
                entry: selection.entry,
                rider_index: selection.rider_index,
            },
        );
        true
    }

    // ----------------------------------------------------------------- csv

    /// Return the dialog's two import options.
    fn csv_options(&self) -> rivercrossing_csvio::ImportOptions {
        rivercrossing_csvio::ImportOptions {
            map_unknown_sex_to_male: self.dialogs.field("map_unknown_sex_chk") == Some("true"),
            convert_teams_of_one_to_solo: self.dialogs.field("convert_teams_of_one_chk")
                == Some("true"),
        }
    }

    /// Preview *path* against the open roster and open the preview dialog.
    ///
    /// # Errors
    ///
    /// Return the could-not-read or the CSV reader's refusal; the dialog keeps the message.
    pub(crate) fn preview_csv(&mut self, path: &Path) -> Result<(), String> {
        let Some(roster) = self.roster() else {
            return Err(NO_RIDE.to_owned());
        };
        let options = self.csv_options();
        match riders::csv_preview(path, roster, options) {
            Ok((preview, model)) => {
                self.csv_preview.path = Some(path.to_path_buf());
                self.csv_preview.preview = Some(preview);
                self.csv_preview.model = Some(model);
                self.csv_preview.errors = None;
                if self
                    .dialogs
                    .top()
                    .is_none_or(|dialog| dialog.id != "csv_preview_dlg")
                {
                    let _ = self.dialogs.open("csv_preview_dlg", None);
                }
                Ok(())
            }
            Err(message) => {
                self.csv_preview.errors = Some(message.clone());
                Err(message)
            }
        }
    }

    /// Set one import option and preview the chosen file again.
    pub(crate) fn set_csv_option(&mut self, field: &str, on: bool) {
        let _ = self
            .dialogs
            .set_field(field, if on { "true" } else { "false" });
        if let Some(path) = self.csv_preview.path.clone() {
            let _ = self.preview_csv(&path);
        }
    }

    /// Commit the previewed import.
    ///
    /// # Errors
    ///
    /// Return the no-preview refusal and the CSV crate's refusal (a stale preview, or a mutation
    /// the ride's state now locks).
    pub(crate) fn submit_csv_import(&mut self) -> Result<(), String> {
        let Some(preview) = self.csv_preview.preview.clone() else {
            return Err("No CSV file is previewed".to_owned());
        };
        let imported = self
            .ride
            .as_mut()
            .map(|ride| riders::csv_import(&mut ride.roster, &preview));
        match imported {
            Some(Ok(report)) => {
                self.csv_preview.errors = None;
                self.persist_roster();
                self.refresh_models();
                let _ = self.dialogs.cancel();
                self.csv_preview = CsvPreviewState::default();
                self.note(format!("Imported {} entries", report.inserted));
                Ok(())
            }
            Some(Err(message)) => {
                self.csv_preview.errors = Some(message.clone());
                Err(message)
            }
            None => Err(NO_RIDE.to_owned()),
        }
    }

    /// Export the open roster to *path*.
    ///
    /// # Errors
    ///
    /// Return the no-ride refusal and the CSV crate's refusal (a file that cannot be written).
    pub(crate) fn export_csv(&mut self, path: &Path) -> Result<(), String> {
        let Some(roster) = self.roster() else {
            return Err(NO_RIDE.to_owned());
        };
        riders::csv_export(roster, path, None)?;
        self.note(format!("Exported {}", path.display()));
        Ok(())
    }

    // --------------------------------------------------------------- teams

    /// Open the teams editor on a fresh model.
    pub(crate) fn open_team_editor(&mut self) {
        let _ = self.dialogs.open("team_editor_dlg", None);
        self.team_editor = TeamEditorModel::default();
        self.refresh_team_editor();
    }

    /// Rebuild the teams editor's rows, keeping a selection that still exists.
    pub(crate) fn refresh_team_editor(&mut self) {
        let single = self.team_single_member_only();
        self.team_editor.rows = self
            .roster()
            .map_or_else(Vec::new, |roster| team_rows(roster, single));
        if !self.team_selection_exists() {
            self.team_editor.selected = None;
        }
    }

    /// Return whether the selected team still exists.
    fn team_selection_exists(&self) -> bool {
        let Some(entry) = self.team_editor.selected.as_ref() else {
            return false;
        };
        self.roster()
            .is_some_and(|roster| roster.entry_by_key(&entry.0).is_some())
    }

    /// Return whether the single-member filter is on.
    fn team_single_member_only(&self) -> bool {
        self.dialogs.field("single_member_only_chk") == Some("true")
    }

    /// Set the single-member filter and rebuild the rows.
    pub(crate) fn set_team_single_member_only(&mut self, on: bool) {
        let _ = self
            .dialogs
            .set_field("single_member_only_chk", if on { "true" } else { "false" });
        self.refresh_team_editor();
    }

    /// Open the teams editor at the team named *name*.
    pub(crate) fn open_team_at(&mut self, name: &str) {
        if self
            .dialogs
            .top()
            .is_none_or(|dialog| dialog.id != "team_editor_dlg")
        {
            let _ = self.dialogs.open("team_editor_dlg", None);
        }
        self.refresh_team_editor();
        self.team_editor.selected = self
            .team_editor
            .rows
            .iter()
            .find(|row| row.row.name == name)
            .map(|row| row.entry.clone());
    }

    /// Open the Add team form.
    pub(crate) fn open_add_team(&mut self) {
        self.team_editor.editing = None;
        self.team_editor.errors = None;
        let _ = self.dialogs.open("add_team_dlg", None);
        for (field, value) in [
            ("name_input", String::new()),
            ("relay_plate_input", String::new()),
            ("notes_input", String::new()),
        ] {
            let _ = self.dialogs.set_field(field, value);
        }
    }

    /// Open the Add team form on the selected team.
    pub(crate) fn open_edit_team(&mut self) {
        let Some(entry) = self.team_editor.selected.clone() else {
            return;
        };
        let Some(record) = self.team_record(&entry) else {
            return;
        };
        self.team_editor.editing = Some(entry);
        self.team_editor.errors = None;
        let _ = self.dialogs.open("add_team_dlg", None);
        for (field, value) in [
            ("name_input", record.name),
            ("relay_plate_input", record.plate),
            ("notes_input", record.notes),
        ] {
            let _ = self.dialogs.set_field(field, value);
        }
    }

    /// Return the team record that *entry* names.
    pub(crate) fn team_record(&self, entry: &EntryId) -> Option<TeamRecord> {
        let team = self.roster()?.entry_by_key(&entry.0)?;
        Some(TeamRecord {
            name: team.display_name.clone(),
            plate: team.plate.clone(),
            notes: team.notes.clone(),
        })
    }

    /// Return the team's rider names.
    pub(crate) fn team_members(&self, entry: &EntryId) -> Option<Vec<String>> {
        let team = self.roster()?.entry_by_key(&entry.0)?;
        Some(team.riders.iter().map(Rider::full_name).collect())
    }

    /// Return the team's logo card code, when one is drawn.
    pub(crate) fn team_logo(&self, entry: &EntryId) -> Option<String> {
        self.roster()?.entry_by_key(&entry.0)?.logo_card.clone()
    }

    /// Read the Add/Edit team form.
    fn team_form(&self) -> Result<teams::TeamFormValues, String> {
        if self
            .dialogs
            .top()
            .is_none_or(|dialog| dialog.id != "add_team_dlg")
        {
            return Err("The Add team dialog is not open".to_owned());
        }
        let field = |id: &str| self.dialogs.field(id).unwrap_or_default().to_owned();
        Ok(teams::TeamFormValues {
            name: field("name_input"),
            relay_plate: field("relay_plate_input"),
            notes: field("notes_input"),
        })
    }

    /// Submit the Add team form.
    ///
    /// # Errors
    ///
    /// Return the blank-name, duplicate-name or roster refusal and keep the form open.
    pub(crate) fn submit_add_team(&mut self) -> Result<(), String> {
        let form = self.team_form()?;
        let added = self
            .ride
            .as_mut()
            .map(|ride| teams::add_team(&mut ride.roster, &form));
        match added {
            Some(Ok(entry)) => {
                self.team_editor.errors = None;
                self.persist_roster();
                self.refresh_team_editor();
                self.team_editor.selected = Some(entry);
                let _ = self.dialogs.cancel();
                self.note("Team added");
                Ok(())
            }
            Some(Err(message)) => {
                self.team_editor.errors = Some(message.clone());
                Err(message)
            }
            None => Err(NO_RIDE.to_owned()),
        }
    }

    /// Submit the Add/Edit team form.
    ///
    /// # Errors
    ///
    /// Return the no-selection refusal, the blank or duplicate name and the roster's own refusal.
    pub(crate) fn submit_edit_team(&mut self) -> Result<(), String> {
        let form = self.team_form()?;
        let Some(entry) = self
            .team_editor
            .editing
            .clone()
            .or_else(|| self.team_editor.selected.clone())
        else {
            return Err(SELECT_TEAM.to_owned());
        };
        let edited = self
            .ride
            .as_mut()
            .map(|ride| teams::edit_team(&mut ride.roster, &entry, &form));
        match edited {
            Some(Ok(())) => {
                self.team_editor.errors = None;
                self.team_editor.editing = None;
                self.persist_roster();
                self.refresh_team_editor();
                let _ = self.dialogs.cancel();
                self.note("Team updated");
                Ok(())
            }
            Some(Err(message)) => {
                self.team_editor.errors = Some(message.clone());
                Err(message)
            }
            None => Err(NO_RIDE.to_owned()),
        }
    }

    /// Draw a fresh logo card for the selected team.
    ///
    /// # Errors
    ///
    /// Return the no-selection refusal and the presenter's no-deck or exhausted refusal.
    pub(crate) fn pick_team_card(&mut self) -> Result<(), String> {
        let Some(entry) = self.team_editor.selected.clone() else {
            return Err(SELECT_TEAM.to_owned());
        };
        let seed = new_ride_seed();
        let picked = self
            .ride
            .as_mut()
            .map(|ride| teams::pick_card(&mut ride.roster, &entry, seed));
        match picked {
            Some(Ok(code)) => {
                self.team_editor.errors = None;
                self.persist_roster();
                self.refresh_team_editor();
                self.note(format!("Logo card {code}"));
                Ok(())
            }
            Some(Err(message)) => {
                self.team_editor.errors = Some(message.clone());
                Err(message)
            }
            None => Err(NO_RIDE.to_owned()),
        }
    }

    /// Remove the selected team.
    ///
    /// # Errors
    ///
    /// Return the select-a-team refusal and the roster's delete lock.
    pub(crate) fn remove_selected_team(&mut self) -> Result<(), String> {
        let Some(entry) = self.team_editor.selected.clone() else {
            return Err(SELECT_TEAM.to_owned());
        };
        let removed = self
            .ride
            .as_mut()
            .map(|ride| teams::remove_team(&mut ride.roster, &entry));
        match removed {
            Some(Ok(())) => {
                self.team_editor.errors = None;
                self.team_editor.selected = None;
                self.persist_roster();
                self.refresh_team_editor();
                self.note("Team removed");
                Ok(())
            }
            Some(Err(message)) => {
                self.team_editor.errors = Some(message.clone());
                Err(message)
            }
            None => Err(NO_RIDE.to_owned()),
        }
    }

    /// Raise the native warning that guards the team removal.
    ///
    /// Return whether a team was selected.
    pub(crate) fn raise_remove_team(&mut self) -> bool {
        let Some(entry) = self.team_editor.selected.clone() else {
            return false;
        };
        self.raise_native(NativeDialog::Danger, NativeTask::RemoveTeam { entry });
        true
    }

    // -------------------------------------------------------------- issues

    /// Open the rider-issues report on a fresh model.
    pub(crate) fn open_rider_issues(&mut self) {
        let _ = self.dialogs.open("rider_issues_dlg", None);
        self.rider_issues = RiderIssuesModel::default();
        self.refresh_rider_issues();
    }

    /// Rebuild the issue rows, keeping a selection that still exists.
    pub(crate) fn refresh_rider_issues(&mut self) {
        let rows = self.roster().map_or_else(Vec::new, rider_issues::rows);
        self.rider_issues.rows = rows;
        if self
            .rider_issues
            .selected
            .is_some_and(|index| index >= self.rider_issues.rows.len())
        {
            self.rider_issues.selected = None;
        }
    }

    /// Return the fixes that *row* currently offers.
    pub(crate) fn actions_for(
        &self,
        row: &rider_issues::IssueRow,
    ) -> Option<rider_issues::IssueActions> {
        self.roster()
            .map(|roster| rider_issues::actions(roster, row))
    }

    /// Return the selected issue row.
    fn selected_issue(&self) -> Option<rider_issues::IssueRow> {
        let index = self.rider_issues.selected?;
        self.rider_issues.rows.get(index).cloned()
    }

    /// Open the editor that the selected issue names.
    pub(crate) fn open_issue_editor(&mut self) {
        let Some(row) = self.selected_issue() else {
            return;
        };
        let open_team = self
            .roster()
            .is_some_and(|roster| rider_issues::actions(roster, &row).open_editor == Some("team"));
        if open_team {
            self.open_team_at(&row.name);
        } else {
            self.open_rider_at(&row.plate);
        }
    }

    /// Convert the selected team-of-one into a solo entry.
    ///
    /// # Errors
    ///
    /// Return the select-an-issue refusal and the presenter's own kind, plate-model and DRAFT-only
    /// refusals.
    pub(crate) fn convert_issue_to_solo(&mut self) -> Result<(), String> {
        let Some(row) = self.selected_issue() else {
            return Err(SELECT_ISSUE.to_owned());
        };
        let converted = self
            .ride
            .as_mut()
            .map(|ride| rider_issues::convert_to_solo(&mut ride.roster, &row));
        match converted {
            Some(Ok(entry)) => {
                self.rider_issues.errors = None;
                self.persist_roster();
                self.refresh_models();
                let _ = entry;
                self.note("Converted the team-of-one to solo");
                Ok(())
            }
            Some(Err(message)) => {
                self.rider_issues.errors = Some(message.clone());
                Err(message)
            }
            None => Err(NO_RIDE.to_owned()),
        }
    }

    /// Give the selected missing-number rider the next free plate.
    ///
    /// # Errors
    ///
    /// Return the select-an-issue refusal and the presenter's kind and DRAFT-only refusals.
    pub(crate) fn assign_issue_plate(&mut self) -> Result<(), String> {
        let Some(row) = self.selected_issue() else {
            return Err(SELECT_ISSUE.to_owned());
        };
        let Some(plate) = self.roster().map(Roster::next_free_plate) else {
            return Err(NO_RIDE.to_owned());
        };
        let assigned = self
            .ride
            .as_mut()
            .map(|ride| rider_issues::assign_plate(&mut ride.roster, &row, &plate));
        match assigned {
            Some(Ok(())) => {
                self.persist_roster();
                self.refresh_models();
                self.rider_issues.errors = None;
                self.note(format!("Assigned plate {plate}"));
                Ok(())
            }
            Some(Err(message)) => {
                self.rider_issues.errors = Some(message.clone());
                Err(message)
            }
            None => Err(NO_RIDE.to_owned()),
        }
    }

    /// Renumber the selected duplicate claimant with the next free plate.
    ///
    /// # Errors
    ///
    /// Return the select-an-issue refusal and the presenter's kind and DRAFT-only refusals.
    pub(crate) fn renumber_issue(&mut self) -> Result<(), String> {
        let Some(row) = self.selected_issue() else {
            return Err(SELECT_ISSUE.to_owned());
        };
        let Some(plate) = self.roster().map(Roster::next_free_plate) else {
            return Err(NO_RIDE.to_owned());
        };
        let renumbered = self
            .ride
            .as_mut()
            .map(|ride| rider_issues::renumber(&mut ride.roster, &row, &plate));
        match renumbered {
            Some(Ok(())) => {
                self.persist_roster();
                self.refresh_models();
                self.rider_issues.errors = None;
                self.note(format!("Renumbered to {plate}"));
                Ok(())
            }
            Some(Err(message)) => {
                self.rider_issues.errors = Some(message.clone());
                Err(message)
            }
            None => Err(NO_RIDE.to_owned()),
        }
    }

    // ------------------------------------------------------- native tasks

    /// Run the task a confirmed native dialog names.
    pub(crate) fn run_native_task(&mut self, task: NativeTask) {
        match task {
            NativeTask::Command(command) => self.pending_command = Some(command),
            NativeTask::DuplicateRide { id } => {
                self.pending_store.push(StoreCommand::DuplicateRide { id });
            }
            NativeTask::DeleteRider { entry, rider_index } => {
                self.rider_editor.selected = Some(RiderSelection { entry, rider_index });
                let _ = self.delete_selected_rider();
            }
            NativeTask::RemoveTeam { entry } => {
                self.team_editor.selected = Some(entry);
                let _ = self.remove_selected_team();
            }
            NativeTask::DeleteCrossing { entry, seq } => {
                let _ = self.delete_crossing(&entry, seq);
            }
            // The app runs these itself: a file picker answers with a path, and the publish
            // outcomes own their own button rules.
            NativeTask::ChooseLogo
            | NativeTask::ExportFile { .. }
            | NativeTask::OpenPublished { .. }
            | NativeTask::RetryPublish => {}
        }
    }

    /// Return the native confirmation's title for the dialog kind.
    #[must_use]
    pub(crate) const fn native_title(dialog: NativeDialog) -> &'static str {
        match dialog {
            NativeDialog::Confirm => "Confirm",
            NativeDialog::Danger => "Warning",
            NativeDialog::Prompt => "Input required",
            NativeDialog::Error => "Error",
        }
    }

    /// Return the native confirmation's message, naming the action or record it guards.
    #[must_use]
    pub(crate) fn native_message(&self, pending: &PendingNative) -> String {
        match &pending.then {
            NativeTask::Command(CommandId::UndoCrossing) => "Undo the last crossing?".to_owned(),
            NativeTask::Command(CommandId::StopRide) => "Stop the ride?".to_owned(),
            NativeTask::Command(CommandId::FinishRide) => "Finish the ride?".to_owned(),
            NativeTask::Command(CommandId::ClearRide) => {
                "Clear the ride from the console? The database is untouched.".to_owned()
            }
            NativeTask::Command(CommandId::ReopenRide) => {
                "Reopen the finished ride for corrections?".to_owned()
            }
            NativeTask::Command(CommandId::DuplicateRide) => "Duplicate the open ride?".to_owned(),
            NativeTask::Command(CommandId::AppQuit) => "Quit RiverCrossing?".to_owned(),
            NativeTask::Command(other) => format!("Run {}?", other.as_str()),
            NativeTask::ChooseLogo => "Choose the organizer logo".to_owned(),
            NativeTask::DuplicateRide { id } => self
                .library
                .store_rows
                .iter()
                .find(|row| row.id == *id)
                .map_or_else(
                    || "Duplicate this ride?".to_owned(),
                    |row| library::duplicate_confirm(&row.name),
                ),
            NativeTask::DeleteRider { entry, .. } => self
                .ride
                .as_ref()
                .and_then(|ride| ride.roster.entry_by_key(&entry.0))
                .map_or_else(
                    || "Delete the selected rider?".to_owned(),
                    riders::delete_message,
                ),
            NativeTask::RemoveTeam { entry } => self
                .ride
                .as_ref()
                .and_then(|ride| ride.roster.entry_by_key(&entry.0))
                .map_or_else(
                    || "Remove the selected team?".to_owned(),
                    teams::remove_message,
                ),
            NativeTask::DeleteCrossing { .. } => "Delete this crossing?".to_owned(),
            NativeTask::ExportFile { .. } => "Choose where to save the export.".to_owned(),
            NativeTask::OpenPublished { link } => format!("Published to {link}"),
            NativeTask::RetryPublish => "The publish failed. Retry?".to_owned(),
        }
    }

    /// Return the native confirmation's confirm label.
    #[must_use]
    pub(crate) const fn native_confirm_label(pending: &PendingNative) -> &'static str {
        match &pending.then {
            NativeTask::Command(CommandId::UndoCrossing) => "Undo",
            NativeTask::Command(CommandId::StopRide) => "Stop",
            NativeTask::Command(CommandId::FinishRide) => "Finish",
            NativeTask::Command(CommandId::ClearRide) => "Clear",
            NativeTask::Command(CommandId::ReopenRide) => "Reopen",
            NativeTask::Command(CommandId::AppQuit) => "Quit",
            NativeTask::Command(CommandId::DuplicateRide) | NativeTask::DuplicateRide { .. } => {
                "Duplicate"
            }
            NativeTask::Command(_) => "OK",
            NativeTask::DeleteRider { .. } | NativeTask::DeleteCrossing { .. } => "Delete",
            NativeTask::RemoveTeam { .. } => "Remove",
            NativeTask::ExportFile { .. } | NativeTask::ChooseLogo => "Save",
            NativeTask::OpenPublished { .. } => "Open",
            NativeTask::RetryPublish => "Retry",
        }
    }

    // -------------------------------------------------------- launch/quit

    /// Record the previous session and decide the launch prompt.
    pub(crate) fn apply_previous_session(&mut self, session: PreviousSession) {
        self.previous_session = Some(session);
        self.reconcile_launch();
    }

    /// Decide the launch prompt once the session and the ride list are both known.
    pub(crate) fn reconcile_launch(&mut self) {
        if self.launch_resolved {
            return;
        }
        let Some(session) = self.previous_session.clone() else {
            return;
        };
        self.launch_resolved = true;
        let has_rides = !self.library.store_rows.is_empty();
        match quit_resume::launch_choice(&session, has_rides) {
            quit_resume::LaunchChoice::Nothing => {}
            quit_resume::LaunchChoice::Library => self.open_library(),
            quit_resume::LaunchChoice::Resume => {
                let name = session
                    .ride_id
                    .and_then(|id| {
                        self.library
                            .store_rows
                            .iter()
                            .find(|row| row.id == id)
                            .map(|row| row.name.clone())
                    })
                    .unwrap_or_else(|| "The last ride".to_owned());
                let at = session.ended_at.unwrap_or_else(OffsetDateTime::now_utc);
                let message = quit_resume::continue_message(&name, session.state, at)
                    .unwrap_or_else(|_| format!("\"{name}\" is still open."));
                self.launch_message = Some(message);
                let _ = self.dialogs.open("resume_dlg", None);
            }
        }
    }

    /// Resume the ride that the resume dialog names.
    pub(crate) fn resume_continue(&mut self) {
        if let Some(id) = self
            .previous_session
            .as_ref()
            .and_then(|session| session.ride_id)
        {
            self.pending_store.push(StoreCommand::LoadRide { id });
        }
        self.launch_message = None;
        let _ = self.dialogs.cancel();
    }

    /// Open the ride library from the resume dialog.
    pub(crate) fn resume_library(&mut self) {
        self.launch_message = None;
        let _ = self.dialogs.cancel();
        self.open_library();
    }

    /// Offer the quit flow: the running dialog for a live ride, else the plain confirm.
    pub(crate) fn request_quit(&mut self) {
        let running = self
            .ride
            .as_ref()
            .is_some_and(|ride| ride.engine.state() == RideStatus::Running);
        if running {
            let name = self
                .ride
                .as_ref()
                .map_or_else(String::new, |ride| ride.config.name.clone());
            self.quit_message = Some(quit_resume::running_exit_message(&name));
            let _ = self.dialogs.open("exit_running_dlg", None);
        } else {
            self.raise_native(
                NativeDialog::Confirm,
                NativeTask::Command(CommandId::AppQuit),
            );
        }
    }

    /// Allow the app to close; the app sends the close and stops the store thread.
    pub(crate) fn confirm_quit(&mut self) {
        self.close_requested = true;
    }

    /// Finish the ride, then allow the app to close.
    pub(crate) fn finish_and_quit(&mut self) {
        self.finish_ride();
        self.close_requested = true;
    }

    // ------------------------------------------------------------ backups

    /// Run the startup backup and remember its hour.
    pub(crate) fn startup_backup(&mut self) {
        self.last_backup_hour = Some(hour_bucket(OffsetDateTime::now_utc()));
        self.pending_store.push(StoreCommand::BackupNow);
    }

    /// Queue a backup when *now* enters a new hour.
    pub(crate) fn due_hourly_backup(&mut self, now: OffsetDateTime) -> bool {
        let hour = hour_bucket(now);
        if self.last_backup_hour == Some(hour) {
            return false;
        }
        self.last_backup_hour = Some(hour);
        self.pending_store.push(StoreCommand::BackupNow);
        true
    }

    /// Queue a backup on request (`mi_backup_now`), and announce its result.
    pub(crate) fn queue_backup(&mut self) {
        self.announce_backup = true;
        self.pending_store.push(StoreCommand::BackupNow);
    }

    /// Take the operator-backup flag: `true` when the backup that just finished was asked for.
    pub(crate) fn take_backup_announcement(&mut self) -> bool {
        std::mem::take(&mut self.announce_backup)
    }

    // ----------------------------------------------------------- library

    /// Open the ride library and ask the store for its rows.
    pub(crate) fn open_library(&mut self) {
        self.library.selected = None;
        self.library.delete_message = None;
        self.library.delete_id = None;
        let _ = self.dialogs.open("ride_library_dlg", None);
        self.pending_store.push(StoreCommand::Rides);
    }

    /// Replace the library rows and clamp the selection.
    pub(crate) fn set_library_rows(&mut self, rows: Vec<RideRow>) {
        self.library.rows = library::rows(&rows);
        self.library.store_rows = rows;
        if self
            .library
            .selected
            .is_some_and(|index| index >= self.library.rows.len())
        {
            self.library.selected = None;
        }
    }

    /// Return the selected library row.
    fn selected_library_row(&self) -> Option<&RideRow> {
        self.library
            .selected
            .and_then(|index| self.library.store_rows.get(index))
    }

    /// Load the selected library ride.
    ///
    /// # Errors
    ///
    /// Return the select-a-ride refusal.
    pub(crate) fn open_library_selected(&mut self) -> Result<(), String> {
        let Some(row) = self.selected_library_row() else {
            return Err("Select a ride first".to_owned());
        };
        self.pending_store
            .push(StoreCommand::LoadRide { id: row.id });
        let _ = self.dialogs.cancel();
        Ok(())
    }

    /// Ask to duplicate the selected library ride.
    ///
    /// Return whether a ride was selected.
    pub(crate) fn duplicate_library_selected(&mut self) -> bool {
        let Some(row) = self.selected_library_row() else {
            return false;
        };
        self.raise_native(
            NativeDialog::Prompt,
            NativeTask::DuplicateRide { id: row.id },
        );
        true
    }

    /// Return whether a library row is selected.
    #[must_use]
    pub(crate) fn library_ready(&self) -> bool {
        self.selected_library_row().is_some()
    }

    /// Return whether the selected library ride may be deleted (never a running one).
    #[must_use]
    pub(crate) fn library_delete_ready(&self) -> bool {
        self.selected_library_row()
            .is_some_and(|row| row.status != RideStatus::Running)
    }

    /// Return whether the typed confirmation arms the delete.
    #[must_use]
    pub(crate) fn delete_ride_armed(&self) -> bool {
        let Some(id) = self.library.delete_id else {
            return false;
        };
        let typed = self.dialogs.field("confirm_name_input").unwrap_or_default();
        self.library
            .store_rows
            .iter()
            .find(|row| row.id == id)
            .is_some_and(|row| library::delete_matches(row, typed))
    }

    /// Open the typed delete confirmation for the selected library ride.
    ///
    /// Return whether a ride was selected.
    pub(crate) fn delete_library_selected(&mut self) -> bool {
        let Some(row) = self
            .selected_library_row()
            .map(|row| (row.id, library::delete_requires(row)))
        else {
            return false;
        };
        self.library.delete_message = Some(row.1);
        self.library.delete_id = Some(row.0);
        let _ = self.dialogs.open("delete_ride_dlg", None);
        let _ = self.dialogs.set_field("confirm_name_input", "");
        true
    }

    /// Submit the typed delete confirmation.
    ///
    /// # Errors
    ///
    /// Return the no-selection refusal and the type-the-name gate's refusal.
    pub(crate) fn submit_delete_ride(&mut self) -> Result<(), String> {
        let Some(id) = self.library.delete_id else {
            return Err("Select a ride first".to_owned());
        };
        let Some(row) = self.library.store_rows.iter().find(|row| row.id == id) else {
            return Err("Select a ride first".to_owned());
        };
        let typed = self
            .dialogs
            .field("confirm_name_input")
            .unwrap_or_default()
            .to_owned();
        if !library::delete_matches(row, &typed) {
            return Err(format!("Type \"{}\" to confirm.", row.name));
        }
        self.pending_store.push(StoreCommand::DeleteRide {
            id,
            typed_name: typed,
        });
        let _ = self.dialogs.cancel();
        Ok(())
    }

    // ------------------------------------------------------------- audit

    /// Open the audit viewer and ask the store for the ride's rows.
    pub(crate) fn open_audit(&mut self) {
        self.audit.search.clear();
        audit::ALL_ACTIONS.clone_into(&mut self.audit.action);
        let Some(id) = self.ride.as_ref().map(|ride| ride.ride_id) else {
            self.note("Open a ride first");
            return;
        };
        let _ = self.dialogs.open("audit_dlg", None);
        self.pending_store.push(StoreCommand::AuditRows { id });
    }

    /// Replace the audit rows and render them against the open roster.
    pub(crate) fn set_audit_rows(&mut self, rows: Vec<AuditRow>) {
        self.audit.rows = match self.ride.as_ref() {
            Some(ride) => audit::rows(&rows, &ride.roster),
            None => Vec::new(),
        };
        self.audit.store_rows = rows;
    }

    /// Return the audit rows that the current filters keep.
    #[must_use]
    pub(crate) fn filtered_audit(&self) -> Vec<audit::AuditRowModel> {
        audit::filter(&self.audit.rows, &self.audit.search, &self.audit.action)
    }

    // ---------------------------------------------------------- settings

    /// Open the settings dialog on the stored values.
    pub(crate) fn open_settings(&mut self) {
        self.settings_form = Some(settings_form::from_settings(&self.settings));
        self.settings_errors = None;
        let _ = self.dialogs.open("settings_dlg", None);
    }

    /// Apply and save the settings dialog's form.
    ///
    /// # Errors
    ///
    /// Return the presenter's refusals and keep the dialog open with them in its infobar.
    pub(crate) fn submit_settings(&mut self) -> Result<(), Vec<String>> {
        let Some(values) = self.settings_form.clone() else {
            return Err(vec!["The settings dialog is not open".to_owned()]);
        };
        let messages = settings_form::apply(&values, &mut self.settings);
        if !messages.is_empty() {
            self.settings_errors = Some(messages.clone());
            return Err(messages);
        }
        self.settings_errors = None;
        self.settings_form = None;
        self.persist_settings();
        let _ = self.dialogs.cancel();
        Ok(())
    }

    // ---------------------------------------------------------- self-test

    /// Open the self-test dialog on a fresh run.
    pub(crate) fn open_selftest(&mut self) {
        self.selftest_lines = selftest::run();
        let _ = self.dialogs.open("selftest_dlg", None);
    }

    /// Run the self-test again.
    pub(crate) fn rerun_selftest(&mut self) {
        self.selftest_lines = selftest::run();
    }

    // -------------------------------------------------------- corrections

    /// Open the edit-crossing dialog in add mode.
    pub(crate) fn open_add_crossing(&mut self) {
        self.correction = CorrectionTarget {
            entry: None,
            seq: None,
            add: true,
        };
        let _ = self.dialogs.open("edit_crossing_dlg", None);
        self.seed_correction_fields([
            ("plate_input", ""),
            ("time_picker", ""),
            ("reason_input", ""),
        ]);
    }

    /// Open the edit-crossing dialog on the Crossing Detail's crossing.
    pub(crate) fn open_edit_crossing(&mut self) {
        let Some(detail) = self.crossing_detail.clone() else {
            return;
        };
        self.correction = CorrectionTarget {
            entry: Some(detail.entry),
            seq: Some(detail.seq),
            add: false,
        };
        let plate = detail.model.plate;
        let time = detail.model.time;
        let _ = self.dialogs.open("edit_crossing_dlg", None);
        self.seed_correction_fields([
            ("plate_input", &plate),
            ("time_picker", &time),
            ("reason_input", ""),
        ]);
    }

    /// Open the manual-deal dialog.
    pub(crate) fn open_manual_deal(&mut self) {
        let _ = self.dialogs.open("manual_deal_dlg", None);
        self.seed_correction_fields([
            ("plate_input", ""),
            ("time_picker", ""),
            ("reason_input", ""),
        ]);
    }

    /// Open the DNF dialog.
    pub(crate) fn open_dnf(&mut self) {
        let _ = self.dialogs.open("dnf_confirm_dlg", None);
        self.seed_correction_fields([
            ("plate_input", ""),
            ("time_picker", ""),
            ("reason_input", ""),
        ]);
    }

    /// Open the void-card dialog when the crossing holds a credited card.
    pub(crate) fn open_void_card(&mut self) {
        if self
            .crossing_detail
            .as_ref()
            .is_none_or(|detail| detail.card.is_none())
        {
            return;
        }
        let _ = self.dialogs.open("void_card_confirm_dlg", None);
        self.seed_correction_fields([
            ("plate_input", ""),
            ("time_picker", ""),
            ("reason_input", ""),
        ]);
    }

    /// Raise the native danger that guards the Crossing Detail's delete.
    ///
    /// Return whether a crossing is open.
    pub(crate) fn raise_delete_crossing(&mut self) -> bool {
        let Some(detail) = self.crossing_detail.clone() else {
            return false;
        };
        self.raise_native(
            NativeDialog::Danger,
            NativeTask::DeleteCrossing {
                entry: detail.entry,
                seq: detail.seq,
            },
        );
        true
    }

    /// Open the plate prompt on the Crossing Detail's crossing.
    pub(crate) fn open_crossing_number(&mut self) {
        let Some(detail) = self.crossing_detail.clone() else {
            return;
        };
        let _ = self.dialogs.open("crossing_number_dlg", None);
        let _ = self.dialogs.set_field("number_input", detail.model.plate);
    }

    /// Open the set-start dialog on the ride's event date and start.
    pub(crate) fn open_set_start(&mut self) {
        let Some(ride) = self.ride.as_ref() else {
            return;
        };
        let date = format!(
            "{:04}-{:02}-{:02}",
            ride.config.event_date.year(),
            u8::from(ride.config.event_date.month()),
            ride.config.event_date.day()
        );
        let time = ride.engine.actual_start().map_or_else(
            || "00:00:00".to_owned(),
            |at| format!("{:02}:{:02}:{:02}", at.hour(), at.minute(), at.second()),
        );
        let _ = self.dialogs.open("set_start_dlg", None);
        let _ = self.dialogs.set_field("start_date_picker", date);
        let _ = self.dialogs.set_field("start_time_picker", time);
    }

    /// Seed the three correction form fields.
    fn seed_correction_fields(&mut self, values: [(&str, &str); 3]) {
        for (field, value) in values {
            let _ = self.dialogs.set_field(field, value);
        }
    }

    /// Read one correction field.
    fn correction_field(&self, id: &str) -> String {
        self.dialogs.field(id).unwrap_or_default().to_owned()
    }

    /// Submit the edit-crossing form in its current mode.
    ///
    /// # Errors
    ///
    /// Return the presenter's refusal (a blank reason, a bad time, the engine's own).
    pub(crate) fn submit_edit_crossing(&mut self) -> Result<(), String> {
        let plate = self.correction_field("plate_input");
        let time = self.correction_field("time_picker");
        let reason = self.correction_field("reason_input");
        let target = self.correction.clone();
        let fields = EditFields {
            plate: &plate,
            time: &time,
            reason: &reason,
        };
        let outcome = match self.ride.as_mut() {
            Some(ride) => edit_crossing_outcome(&target, ride, &fields),
            None => Err(NO_RIDE.to_owned()),
        };
        self.finish_correction(outcome)
    }

    /// Submit the manual-deal form.
    ///
    /// # Errors
    ///
    /// Return the presenter's refusal.
    pub(crate) fn submit_manual_deal(&mut self) -> Result<(), String> {
        let plate = self.correction_field("plate_input");
        let reason = self.correction_field("reason_input");
        let outcome = match self.ride.as_mut() {
            Some(ride) => corrections::deal_manual(&mut ride.engine, &plate, &reason),
            None => Err(NO_RIDE.to_owned()),
        };
        self.finish_correction(outcome)
    }

    /// Submit the DNF form.
    ///
    /// # Errors
    ///
    /// Return the presenter's refusal.
    pub(crate) fn submit_dnf(&mut self) -> Result<(), String> {
        let plate = self.correction_field("plate_input");
        let reason = self.correction_field("reason_input");
        let outcome = match self.ride.as_mut() {
            Some(ride) => corrections::mark_dnf(&mut ride.engine, &plate, &reason),
            None => Err(NO_RIDE.to_owned()),
        };
        self.finish_correction(outcome)
    }

    /// Submit the void-card form on the Crossing Detail's card.
    ///
    /// # Errors
    ///
    /// Return the presenter's refusal (a blank reason, the engine's own).
    pub(crate) fn submit_void_card(&mut self) -> Result<(), String> {
        let reason = self.correction_field("reason_input");
        let Some(detail) = self.crossing_detail.clone() else {
            return Err("Open a crossing first".to_owned());
        };
        let Some(card) = detail.card else {
            return Err("This crossing has no credited card".to_owned());
        };
        let plate = detail.model.plate.clone();
        let outcome = match self.ride.as_mut() {
            Some(ride) => corrections::void_card(&mut ride.engine, &plate, &card, &reason),
            None => Err(NO_RIDE.to_owned()),
        };
        self.finish_correction(outcome)
    }

    /// Submit the plate prompt on the Crossing Detail's crossing.
    ///
    /// # Errors
    ///
    /// Return the no-crossing refusal and the engine's own refusal.
    pub(crate) fn submit_crossing_number(&mut self) -> Result<(), String> {
        let Some(detail) = self.crossing_detail.clone() else {
            return Err("Open a crossing first".to_owned());
        };
        let plate = self.correction_field("number_input");
        let outcome = match self.ride.as_mut() {
            Some(ride) => {
                let ordinal = crossing_ordinal(&ride.engine, &detail.entry, detail.seq)
                    .ok_or_else(|| "the crossing is no longer in the ride".to_owned())?;
                ride.engine
                    .reassign_crossing(ordinal, plate.trim(), "crossing detail edit")
                    .map_err(|error| error.to_string())
            }
            None => Err(NO_RIDE.to_owned()),
        };
        match outcome {
            Ok(event) => {
                self.append_event(&event);
                self.note("Plate changed");
                let _ = self.dialogs.cancel();
                self.refresh_crossing_detail();
                Ok(())
            }
            Err(message) => Err(message),
        }
    }

    /// Submit the set-start form.
    ///
    /// # Errors
    ///
    /// Return the presenter's refusal.
    pub(crate) fn submit_set_start(&mut self) -> Result<(), String> {
        let date = self.correction_field("start_date_picker");
        let time = self.correction_field("start_time_picker");
        let outcome = match self.ride.as_mut() {
            Some(ride) => corrections::set_start(&mut ride.engine, &date, &time),
            None => Err(NO_RIDE.to_owned()),
        };
        self.finish_correction(outcome)
    }

    /// Queue one accepted correction's event and refresh the views.
    fn finish_correction(
        &mut self,
        outcome: Result<corrections::CorrectionOutcome, String>,
    ) -> Result<(), String> {
        match outcome {
            Ok(outcome) => {
                self.append_event(&outcome.event);
                self.note(outcome.notice);
                let _ = self.dialogs.cancel();
                self.refresh_models();
                self.refresh_crossing_detail();
                Ok(())
            }
            Err(message) => {
                self.note(message.clone());
                Err(message)
            }
        }
    }

    /// Queue one engine event for the store.
    fn append_event(&mut self, event: &rivercrossing_ride::Event) {
        let Some(id) = self.ride.as_ref().map(|ride| ride.ride_id) else {
            return;
        };
        self.pending_store.push(StoreCommand::Append {
            id,
            event: Box::new(event.clone()),
        });
    }

    /// Open the Crossing Detail dialog on the crossing at *entry* and lap *seq*.
    pub(crate) fn open_crossing_detail(&mut self, entry: &EntryId, seq: u32) {
        let Some(ride) = self.ride.as_ref() else {
            return;
        };
        let Some(crossing) = ride
            .engine
            .crossings()
            .iter()
            .find(|crossing| crossing.entry_id == *entry && crossing.seq == seq)
            .cloned()
        else {
            return;
        };
        let model = corrections::crossing_detail_fields(&crossing, &ride.engine, &ride.roster);
        let actions = corrections::can_edit(&ride.engine, &crossing);
        let card = ride.engine.card_for(&crossing);
        self.crossing_detail = Some(CrossingDetailState {
            entry: entry.clone(),
            seq,
            model,
            actions,
            card,
        });
        let _ = self.dialogs.open("crossing_detail_dlg", None);
    }

    /// Re-render the Crossing Detail dialog from the engine.
    fn refresh_crossing_detail(&mut self) {
        let Some(detail) = self.crossing_detail.clone() else {
            return;
        };
        self.open_crossing_detail(&detail.entry, detail.seq);
    }

    /// Delete one crossing through the engine, after the native confirm.
    ///
    /// # Errors
    ///
    /// Return the no-ride and engine refusals.
    pub(crate) fn delete_crossing(&mut self, entry: &EntryId, seq: u32) -> Result<(), String> {
        let outcome = match self.ride.as_mut() {
            Some(ride) => {
                let newest = ride
                    .engine
                    .crossings()
                    .last()
                    .is_some_and(|last| last.entry_id == *entry && last.seq == seq);
                if newest {
                    ride.engine.undo_last().map_err(|error| error.to_string())
                } else {
                    ride.engine
                        .void_crossing(entry, seq, "crossing detail delete")
                        .map_err(|error| error.to_string())
                }
            }
            None => Err(NO_RIDE.to_owned()),
        };
        match outcome {
            Ok(event) => {
                self.append_event(&event);
                self.note("Crossing deleted");
                self.crossing_detail = None;
                let _ = self.dialogs.cancel();
                self.refresh_models();
                Ok(())
            }
            Err(message) => {
                self.note(message.clone());
                Err(message)
            }
        }
    }

    /// Unload the ride from the console; the database is untouched.
    pub(crate) fn clear_ride(&mut self) {
        self.ride = None;
        self.exports = ExportPresence::default();
        self.audit_row_count = 0;
        self.clock_frozen = None;
        self.crossing_detail = None;
        self.pending_store.push(StoreCommand::ClearActiveRide);
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

/// Return every rider-editor row. The presenter's rows and the roster walk the same order, so the
/// rendered cells pair with the entry and rider index that the actions write.
fn editor_rows(roster: &Roster) -> Vec<RiderEditorRow> {
    let rendered = riders::rows(roster);
    let mut rows = Vec::with_capacity(rendered.len());
    let mut offset = 0;
    for entry in roster.entries() {
        let id = entry.id();
        for rider_index in 0..entry.riders.len() {
            let Some(row) = rendered.get(offset) else {
                break;
            };
            rows.push(RiderEditorRow {
                row: row.clone(),
                entry: id.clone(),
                rider_index,
            });
            offset += 1;
        }
    }
    rows
}

/// Return every team-editor row, honouring the single-member filter.
fn team_rows(roster: &Roster, single_member_only: bool) -> Vec<TeamEditorRow> {
    let rendered = teams::rows(roster, single_member_only);
    let mut rows = Vec::with_capacity(rendered.len());
    let mut offset = 0;
    for entry in roster
        .entries()
        .iter()
        .filter(|entry| entry.kind == EntryType::Team)
    {
        let included = !single_member_only || entry.team_size() == 1;
        if included {
            if let Some(row) = rendered.get(offset) {
                rows.push(TeamEditorRow {
                    row: row.clone(),
                    entry: entry.id(),
                });
            }
            offset += 1;
        }
    }
    rows
}

/// Return the ride-wide ordinal (one-based) of the crossing at *entry* and lap *seq*.
fn crossing_ordinal(
    engine: &RideEngine<Roster, SystemClock>,
    entry: &EntryId,
    seq: u32,
) -> Option<u32> {
    let index = engine
        .crossings()
        .iter()
        .position(|crossing| crossing.entry_id == *entry && crossing.seq == seq)?;
    u32::try_from(index).ok().map(|index| index + 1)
}

/// The three values the edit-crossing form carries.
struct EditFields<'a> {
    /// The typed plate.
    plate: &'a str,
    /// The typed crossing time.
    time: &'a str,
    /// The typed reason.
    reason: &'a str,
}

/// Apply the edit-crossing form to *ride*: add a crossing, or edit the selected one.
fn edit_crossing_outcome(
    target: &CorrectionTarget,
    ride: &mut LoadedRide,
    fields: &EditFields<'_>,
) -> Result<corrections::CorrectionOutcome, String> {
    if target.add {
        return corrections::add_crossing_at(
            &mut ride.engine,
            fields.plate,
            fields.time,
            fields.reason,
        );
    }
    let seq = target.seq.unwrap_or(0);
    let entry = target
        .entry
        .clone()
        .ok_or_else(|| "Select a crossing first".to_owned())?;
    corrections::edit_crossing(&mut ride.engine, &entry, seq, fields.time, fields.reason)
}

/// Return *now*'s hour bucket, the unit a backup runs once per.
fn hour_bucket(now: OffsetDateTime) -> i64 {
    now.unix_timestamp().div_euclid(3_600)
}

/// The rider record the Add/Edit form preloads and the editor displays.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RiderRecord {
    /// The rider's own plate.
    pub(crate) plate: String,
    /// The first name.
    pub(crate) first: String,
    /// The last name.
    pub(crate) last: String,
    /// The team choice's text, or `solo`.
    pub(crate) team: String,
    /// The sex choice, when one is recorded.
    pub(crate) sex: Option<String>,
}

/// The facts a live-move decision reads.
struct RiderFacts {
    /// The entry's team name, or `None` for a solo.
    team: Option<String>,
    /// The rider's own plate.
    rider_plate: Option<String>,
}

/// The team record the Add/Edit form preloads and the editor displays.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TeamRecord {
    /// The team's display name.
    pub(crate) name: String,
    /// The team's plate.
    pub(crate) plate: String,
    /// The team's notes.
    pub(crate) notes: String,
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
