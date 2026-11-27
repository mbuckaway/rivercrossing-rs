// SPDX-License-Identifier: GPL-3.0-only

//! The `RiverCrossing` shell app and its eframe entry point.
//!
//! The app owns the state the views render, the one store thread, the sound player, the launch log
//! and the debug control channel. `logic` drives the native menu bar and the documented
//! accelerators, drains the store replies, persists a submitted ride, runs the queued commands,
//! answers the native dialogs, plays the queued cues and applies the zoom; `frame_input` hands the
//! runtime the `ArtCraft` appearance the settings describe; `ui` renders
//! the menu bar (the in-window bar when no platform bar is installed), the console and the top
//! dialog.

use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use craft::dialogs::{FileDialog, Wake};
use craft::egui::{Context, Event, Ui, ViewportCommand};
use craft::ui::runtime::frame::FrameInput;
use craft::ui::shell::app::{ShellApp, run as run_shell};
use craft::ui::shell::embed::CloseDecision;
use craft::window::restore::Store;
use serde_json::json;
use time::OffsetDateTime;

use super::action::enable::RidePhase;
use super::action::spec::{CommandId, NativeDialog};
use super::automation::{self, Control, ControlConfig};
use super::logging::LaunchLog;
#[cfg(target_os = "macos")]
use super::macos;
use super::native::{self, NativeDialogs};
use super::native_menu::NativeBar;
use super::presenters::simulator;
use super::publish_job::{self, PublishOutcome, WordPressWriter};
use super::sound::{SilentBackend, SoundPlayer};
use super::state::exports::ExportKind;
use super::state::{
    AppState, FILE_PICKER_UNAVAILABLE, LoadedRide, NativeTask, PendingNative, new_ride_seed,
};
use super::views;
use super::window::{self, APP_ID};
use super::worker::{StoreCommand, StoreReply, StoreWorker};

/// The per-launch file locations the app opens.
#[derive(Clone, Debug)]
pub struct AppPaths {
    /// The settings file, Python-schema compatible.
    pub settings: PathBuf,
    /// The SQLite database.
    pub db: PathBuf,
    /// The directory the launch log writes into.
    pub log_dir: PathBuf,
}

impl AppPaths {
    /// Return the current user's platform paths.
    #[must_use]
    pub fn defaults() -> Self {
        let settings = rivercrossing_store::default_settings_path();
        let log_dir = settings
            .parent()
            .map_or_else(PathBuf::new, std::path::Path::to_path_buf);
        Self {
            settings,
            db: rivercrossing_store::default_db_path(None),
            log_dir,
        }
    }

    /// Return explicit paths, for a headless host or a test.
    #[must_use]
    pub fn at(settings: PathBuf, db: PathBuf, log_dir: PathBuf) -> Self {
        Self {
            settings,
            db,
            log_dir,
        }
    }
}

/// One running publish attempt.
struct PublishJob {
    /// The attempt's one report.
    reports: Receiver<Result<PublishOutcome, String>>,
    /// Set when the operator cancels.
    cancel: Arc<AtomicBool>,
}

/// One progress or finished report from the simulation thread.
enum SimReport {
    /// The completed fraction, 0.0..=1.0.
    Progress(f32),
    /// The finished ride, its outcome and the event count before the run.
    Done(Box<LoadedRide>, simulator::SimOutcome, usize),
}

/// One running simulation.
struct SimJob {
    /// The progress and final reports.
    reports: Receiver<SimReport>,
    /// Set when the operator cancels.
    cancel: Arc<AtomicBool>,
}

/// The `RiverCrossing` app driven by the craft shell.
pub struct RiverCrossingApp {
    /// The instant the app asks the root window to close; `None` keeps the window open.
    quit_after: Option<Instant>,
    /// The live automation channel, present in a debug build launched with `--control`.
    control: Option<Control>,
    /// The state the views render.
    state: AppState,
    /// The one store thread, when the database opened.
    worker: Option<StoreWorker>,
    /// The console's cue player.
    sound: SoundPlayer,
    /// This launch's NDJSON log.
    log: Option<LaunchLog>,
    /// The zoom percentage last applied to the context.
    applied_zoom: Option<u16>,
    /// The native message boxes (R-85).
    native: NativeDialogs,
    /// The root window's platform menu bar; uninstalled off macOS and in a headless host.
    menubar: NativeBar,
    /// Set by a macOS close request: the next pass minimizes the window instead of quitting.
    minimize_next_frame: bool,
    /// Whether the operator confirmed the quit and the window may close.
    quit_confirmed: bool,
    /// The publish attempt running on its own thread.
    publish_job: Option<PublishJob>,
    /// The simulation running on its own thread.
    sim_job: Option<SimJob>,
}

impl fmt::Debug for RiverCrossingApp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RiverCrossingApp")
            .field("quit_after", &self.quit_after)
            .field("control", &self.control.is_some())
            .field("state", &self.state)
            .field("worker", &self.worker.is_some())
            .field("sound_muted", &self.sound.is_muted())
            .field("log", &self.log.as_ref().map(LaunchLog::path))
            .field("applied_zoom", &self.applied_zoom)
            .field("native", &self.native)
            .field("menubar", &self.menubar.is_installed())
            .field("minimize_next_frame", &self.minimize_next_frame)
            .field("quit_confirmed", &self.quit_confirmed)
            .field("publish_job", &self.publish_job.is_some())
            .field("sim_job", &self.sim_job.is_some())
            .finish()
    }
}

impl RiverCrossingApp {
    /// Build the app over the current user's files and the default sound device.
    ///
    /// A `quit_after` duration becomes a deadline counted from now; a duration that overshoots the
    /// platform clock leaves no deadline, so the window stays open.
    #[must_use]
    pub fn new(quit_after: Option<Duration>, control: Option<Control>) -> Self {
        Self::build(
            quit_after,
            control,
            AppPaths::defaults(),
            SoundPlayer::with_default_backend(),
            NativeDialogs::new(Wake::none()),
        )
    }

    /// Build the running app over *ctx*'s repaint wake, with the native message boxes.
    #[must_use]
    pub fn new_with_context(
        ctx: &Context,
        quit_after: Option<Duration>,
        control: Option<Control>,
    ) -> Self {
        let repaint = ctx.clone();
        Self::build(
            quit_after,
            control,
            AppPaths::defaults(),
            SoundPlayer::with_default_backend(),
            NativeDialogs::new(Wake::new(move || repaint.request_repaint())),
        )
    }

    /// Build the app over explicit paths and the silent sound backend.
    ///
    /// A headless host or a snapshot test uses this; the console still renders, but a cue never
    /// opens an audio device.
    #[must_use]
    pub fn headless(
        quit_after: Option<Duration>,
        control: Option<Control>,
        paths: AppPaths,
    ) -> Self {
        Self::build(
            quit_after,
            control,
            paths,
            SoundPlayer::new(Box::new(SilentBackend)),
            NativeDialogs::headless(Wake::none()),
        )
    }

    /// Build the app over *paths* and *sound*.
    #[expect(
        clippy::too_many_arguments,
        reason = "the launch seams: the quit deadline, the control channel, the file paths, the player and the native dialogs"
    )]
    fn build(
        quit_after: Option<Duration>,
        control: Option<Control>,
        paths: AppPaths,
        sound: SoundPlayer,
        native: NativeDialogs,
    ) -> Self {
        let (settings, warnings) = rivercrossing_store::load_settings(&paths.settings);
        let verbose = settings.verbose_logging;
        let mut app = Self {
            quit_after: quit_after.and_then(|after| Instant::now().checked_add(after)),
            control,
            state: AppState::new(settings, paths.settings),
            worker: None,
            sound,
            log: None,
            applied_zoom: None,
            native,
            menubar: NativeBar::default(),
            minimize_next_frame: false,
            quit_confirmed: false,
            publish_job: None,
            sim_job: None,
        };
        app.sound.set_muted(!app.state.settings.sound_on);
        let (log, log_warnings) =
            LaunchLog::open(&paths.log_dir, verbose, OffsetDateTime::now_utc());
        app.log = Some(log);
        for warning in warnings.iter().chain(&log_warnings) {
            report(warning);
        }
        app.event("startup", json!({ "app_id": APP_ID }));
        match StoreWorker::start(paths.db) {
            Ok(worker) => {
                worker.send(StoreCommand::PreviousSession);
                worker.send(StoreCommand::Rides);
                app.worker = Some(worker);
                app.state.startup_backup();
            }
            Err(error) => {
                let message = error.to_string();
                app.state.note(message.clone());
                app.event("store-error", json!({ "message": message }));
                report(&format!("the database did not open: {error}"));
            }
        }
        app
    }

    /// Execute one automation command, as `ui.command` does without the control channel.
    ///
    /// # Errors
    ///
    /// Return the automation message for an unknown, gated or unbuilt command.
    pub fn command(&mut self, name: &str) -> Result<serde_json::Value, String> {
        automation::command(&mut self.state, name)
    }

    /// Load one ride from *path*'s database into the state, synchronously.
    ///
    /// A headless host and the snapshot tests seed a fixture ride this way; the running app loads
    /// rides through the store worker instead.
    ///
    /// # Errors
    ///
    /// Return the store's failure when the database, the ride or its roster cannot be read.
    pub fn load_ride_from(
        &mut self,
        path: &Path,
        ride_id: i64,
    ) -> Result<(), rivercrossing_store::StoreError> {
        let store = rivercrossing_store::Store::open(path)?;
        let engine = store.load_engine(ride_id)?;
        let roster = store.roster_for(ride_id)?;
        let config = engine.config().clone();
        let mut ride = LoadedRide {
            ride_id,
            config,
            roster,
            engine,
        };
        ride.roster.set_status(ride.engine.state());
        self.state.ride = Some(ride);
        self.state.audit_row_count = 0;
        self.state.refresh_models();
        Ok(())
    }

    /// Answer every save picker with *paths* (tests).
    #[cfg(test)]
    pub(crate) fn answer_file_pickers_with(&mut self, paths: Vec<PathBuf>) {
        self.native.set_override(Some(Box::new(move |_request| {
            craft::dialogs::Answer::Paths(paths.clone())
        })));
    }

    /// Export the open ride's roster as CSV.
    ///
    /// The OS save picker that chooses *path* arrives with PR D's `craft_dialogs`; a headless host
    /// and the snapshot tests call this directly.
    ///
    /// # Errors
    ///
    /// Return the no-ride refusal and the CSV crate's refusal.
    pub fn export_csv(&mut self, path: &Path) -> Result<(), String> {
        self.state.export_csv(path)
    }

    /// Preview *path* and open the CSV preview dialog.
    ///
    /// The OS open picker that chooses *path* arrives with PR D's `craft_dialogs`.
    ///
    /// # Errors
    ///
    /// Return the CSV reader's refusal.
    pub fn open_csv_preview(&mut self, path: &Path) -> Result<(), String> {
        self.state.preview_csv(path)
    }

    /// Open the ride library with *rows* (a headless host and the snapshot tests).
    pub fn show_library(&mut self, rows: Vec<rivercrossing_store::RideRow>) {
        self.state.set_library_rows(rows);
        self.state.open_library();
    }

    /// Open the typed delete confirmation for *row* (a headless host and the snapshot tests).
    pub fn show_delete_ride(&mut self, row: rivercrossing_store::RideRow) {
        self.state.set_library_rows(vec![row]);
        self.state.library.selected = Some(0);
        let _ = self.state.delete_library_selected();
    }

    /// Open the audit viewer with *rows* (a headless host and the snapshot tests).
    pub fn show_audit(&mut self, rows: Vec<rivercrossing_store::AuditRow>) {
        self.state.open_audit();
        self.state.set_audit_rows(rows);
    }

    /// Open the publish form (a headless host and the snapshot tests).
    pub fn show_publish(&mut self) {
        self.state.open_publish();
    }

    /// Open the resume prompt on a pinned previous session (a headless host and the snapshot tests).
    pub fn show_resume(&mut self) {
        let Some(ride) = self.state.ride.as_ref() else {
            return;
        };
        let row = rivercrossing_store::RideRow {
            id: ride.ride_id,
            name: ride.config.name.clone(),
            event_date: ride.config.event_date,
            status: ride.engine.state(),
            entries: u32::try_from(ride.roster.entries().len()).unwrap_or(0),
        };
        let session = rivercrossing_store::PreviousSession {
            state: rivercrossing_store::session::SessionState::RunningAtExit,
            ride_id: Some(ride.ride_id),
            ended_at: Some(time::OffsetDateTime::UNIX_EPOCH),
        };
        self.state.set_library_rows(vec![row]);
        self.state.previous_session = Some(session);
        self.state.reconcile_launch();
    }

    /// Open the blocked-start dialog through the console's own start gate.
    pub fn show_start_blocked(&mut self) {
        self.state.start_ride();
    }

    /// Open the rider editor's Add form.
    pub fn show_add_rider(&mut self) {
        self.state.open_rider_editor();
        self.state.open_add_rider();
    }

    /// Open the team editor's Add form.
    pub fn show_add_team(&mut self) {
        self.state.open_team_editor();
        self.state.open_add_team();
    }

    /// Open the set-start dialog on the open ride.
    pub fn show_set_start(&mut self) {
        self.state.open_set_start();
    }

    /// Open the add-crossing form.
    pub fn show_edit_crossing(&mut self) {
        self.state.open_add_crossing();
    }

    /// Open the manual-deal form.
    pub fn show_manual_deal(&mut self) {
        self.state.open_manual_deal();
    }

    /// Open the DNF form.
    pub fn show_dnf(&mut self) {
        self.state.open_dnf();
    }

    /// Open the void-card form on the open crossing.
    pub fn show_void_card(&mut self) {
        self.state.open_void_card();
    }

    /// Open the plate prompt on the open crossing.
    pub fn show_crossing_number(&mut self) {
        self.state.open_crossing_number();
    }

    /// Open the publish progress window in its running state.
    pub fn show_publish_running(&mut self) {
        "Publishing…".clone_into(&mut self.state.publish_status);
        let _ = self.state.dialogs.open("publish_running_dlg", None);
    }

    /// Open the simulation progress window in its running state.
    pub fn show_sim_running(&mut self) {
        "Simulating…".clone_into(&mut self.state.sim_status);
        self.state.sim_progress = 0.25;
        let _ = self.state.dialogs.open("sim_running_dlg", None);
    }

    /// Open the first crossing's detail (a headless host and the snapshot tests).
    pub fn open_first_crossing_detail(&mut self) -> bool {
        let Some((entry, seq)) = self.state.ride.as_ref().and_then(|ride| {
            ride.engine
                .crossings()
                .first()
                .map(|crossing| (crossing.entry_id.clone(), crossing.seq))
        }) else {
            return false;
        };
        self.state.open_crossing_detail(&entry, seq);
        true
    }

    /// Queue the hourly backup check now (a headless host and the snapshot tests).
    pub fn backup_now(&mut self) {
        self.state.queue_backup();
    }

    /// Write one field on the top dialog, as `ui.set` does.
    ///
    /// # Errors
    ///
    /// Return the automation message when no dialog is open or it does not declare the field.
    pub fn set_dialog_field(&mut self, field: &str, value: &str) -> Result<(), String> {
        automation::set(&mut self.state, &json!({ "field": field, "value": value }))
            .map(|_reply| ())
    }

    /// Answer the native dialogs in-app (the control channel's host).
    pub fn set_native_unavailable(&mut self) {
        self.native.set_unavailable();
    }

    /// Ask the native message box for the pending confirmation.
    fn raise_native_dialogs(&mut self) {
        if self.state.native_in_app {
            // The host cannot show native dialogs; the in-app modal owns the pending task.
            return;
        }
        let Some(pending) = self.state.pending_native.take() else {
            return;
        };
        let message = self.state.native_message(&pending);
        self.event(
            "native-dialog",
            json!({ "kind": pending.dialog.as_str(), "task": pending.then.as_str() }),
        );
        self.native.ask(&pending, &message);
    }

    /// Apply the native answers: a confirm runs the task, an unavailable host restores the in-app
    /// modal, a save picker queues its export, and a cancel runs nothing.
    fn answer_native_dialogs(&mut self) {
        for (pending, answer) in self.native.answers() {
            if let (NativeTask::ChooseLogo, craft::dialogs::Answer::Paths(paths)) =
                (&pending.then, &answer)
            {
                // The picker's answer is the form's logo path; there is nothing else to raise.
                self.state.setup_logo = paths
                    .first()
                    .map_or_else(String::new, |path| path.to_string_lossy().into_owned());
                continue;
            }
            match (&pending.then, &answer) {
                (NativeTask::ExportFile { kind }, craft::dialogs::Answer::Paths(paths)) => {
                    self.pick_export(*kind, paths);
                }
                (
                    NativeTask::ChooseLogo | NativeTask::ExportFile { .. },
                    craft::dialogs::Answer::Unavailable(reason),
                ) => {
                    self.event("native-unavailable", json!({ "reason": reason }));
                    self.state.note(FILE_PICKER_UNAVAILABLE);
                }
                (_, craft::dialogs::Answer::Unavailable(reason)) => {
                    self.restore_in_app(pending, reason);
                }
                _ => {
                    self.run_accepted(&pending, &answer);
                }
            }
        }
    }

    /// Restore a native box as the in-app modal after an unavailable host.
    fn restore_in_app(&mut self, pending: PendingNative, reason: &str) {
        self.event("native-unavailable", json!({ "reason": reason }));
        self.state.pending_native = Some(pending);
        self.state.native_in_app = true;
    }

    /// Run the task that *answer* accepts, when it accepts one.
    fn run_accepted(&mut self, pending: &PendingNative, answer: &craft::dialogs::Answer) {
        if let Some(task) = native::accepted_task(pending, answer) {
            self.run_outgoing_task(task);
        }
    }

    /// Queue the picked export, when the picker returned a path.
    fn pick_export(&mut self, kind: ExportKind, paths: &[PathBuf]) {
        if let Some(path) = paths.first() {
            self.queue_picked_export(kind, path);
        }
    }

    /// Queue *kind*'s export to the picked *path*, noting a composition refusal.
    fn queue_picked_export(&mut self, kind: ExportKind, path: &Path) {
        if let Err(message) = self.state.queue_export(kind, path) {
            self.state.note(message);
        }
        self.event("export-chosen", json!({ "kind": kind.as_str() }));
    }

    /// Run a confirmed task, including the publish outcomes the app itself owns.
    fn run_outgoing_task(&mut self, task: NativeTask) {
        match task {
            NativeTask::OpenPublished { link } => {
                self.open_target(&link);
            }
            NativeTask::RetryPublish => {
                if let Err(message) = self.state.retry_publish() {
                    self.state.note(message);
                }
            }
            other => self.state.run_native_task(other),
        }
    }

    /// Record a finished backup: the path goes to the log, and only a backup the operator asked
    /// for earns a notice. The status bar belongs to the ride, and it names no directory.
    fn backup_reply(&mut self, path: &Path) {
        self.event("backup", json!({ "path": path.display().to_string() }));
        if self.state.take_backup_announcement() {
            self.state.note(BACKUP_WRITTEN);
        }
    }

    /// Open *target* with the platform's handler, posting a notice when it cannot.
    fn open_target(&mut self, target: &str) {
        if let Some(notice) =
            open_notice(target, &|value| craft::platform::open::open_external(value))
        {
            self.state.note(notice);
        }
    }

    /// Append one lifecycle record to the launch log.
    fn event(&mut self, name: &str, fields: serde_json::Value) {
        if let Some(log) = self.log.as_mut() {
            log.event(name, fields);
        }
    }

    /// Drain the store thread's replies into the state.
    fn drain_worker(&mut self) {
        let replies = self
            .worker
            .as_ref()
            .map(StoreWorker::drain)
            .unwrap_or_default();
        for reply in replies {
            self.apply_reply(reply);
        }
    }

    /// Apply one store reply to the state.
    fn apply_reply(&mut self, reply: Result<StoreReply, String>) {
        match reply {
            Ok(reply @ StoreReply::Ride { .. }) => self.apply_ride_reply(reply),
            Ok(StoreReply::RideCreated(id)) => {
                self.event("ride-created", json!({ "ride_id": id }));
                self.request_load(id);
            }
            Ok(StoreReply::PreviousSession(session)) => {
                self.event(
                    "previous-session",
                    json!({
                        "state": session.state.as_str(),
                        "ride_id": session.ride_id,
                    }),
                );
                self.state.apply_previous_session(session);
            }
            Ok(StoreReply::Rides(rows)) => {
                let count = rows.len();
                self.event("rides", json!({ "count": count }));
                self.state.set_library_rows(rows);
                self.state.reconcile_launch();
            }
            Ok(StoreReply::RideDeleted { id }) => {
                self.library_changed(("ride-deleted", "Ride deleted"), id);
            }
            Ok(StoreReply::RideDuplicated(id)) => {
                self.library_changed(("ride-duplicated", "Ride duplicated"), id);
            }
            Ok(StoreReply::AuditRows(rows)) => self.state.audit_row_count = rows.len(),
            Ok(StoreReply::Backup(path)) => self.backup_reply(&path),
            Ok(StoreReply::Exported { kind, path }) => {
                self.event(
                    "export",
                    json!({ "kind": kind.as_str(), "path": path.display().to_string() }),
                );
                self.state.record_export(kind, &path);
            }
            Ok(StoreReply::ExportFailed { kind, message }) => {
                self.event(
                    "export-failed",
                    json!({ "kind": kind.as_str(), "message": message }),
                );
                self.state.note(message);
            }
            Ok(_other) => {}
            Err(message) => {
                self.state.note(message.clone());
                self.event("store-error", json!({ "message": message }));
            }
        }
    }

    /// Report a library change and ask the store for the fresh rows.
    fn library_changed(&mut self, change: (&str, &str), id: i64) {
        let (event, note) = change;
        self.event(event, json!({ "ride_id": id }));
        self.state.note(note);
        self.send_store(StoreCommand::Rides);
    }

    /// Load the ride a store reply carries, and point the store at it.
    fn apply_ride_reply(&mut self, reply: StoreReply) {
        let StoreReply::Ride {
            id: ride_id,
            config,
            roster,
            engine,
        } = reply
        else {
            return;
        };
        let mut ride = LoadedRide {
            ride_id,
            config,
            roster: *roster,
            engine: *engine,
        };
        ride.roster.set_status(ride.engine.state());
        self.event("ride-loaded", json!({ "ride_id": ride_id }));
        self.state.ride = Some(ride);
        self.state.audit_row_count = 0;
        self.state.refresh_models();
        self.send_store(StoreCommand::SetActiveRide { id: ride_id });
    }

    /// Send one command to the store thread, when it is running.
    fn send_store(&self, command: StoreCommand) {
        if let Some(worker) = &self.worker {
            worker.send(command);
        }
    }

    /// Ask the store thread to rebuild *id*.
    fn request_load(&self, id: i64) {
        self.send_store(StoreCommand::LoadRide { id });
    }

    /// Persist the ride setup the operator submitted, and ask for its rebuild.
    fn start_pending_create(&mut self) {
        let Some(config) = self.state.pending_create.take() else {
            return;
        };
        let seed = new_ride_seed();
        self.event("create-ride", json!({ "name": config.name, "seed": seed }));
        if let Some(worker) = &self.worker {
            worker.send(StoreCommand::CreateRide {
                config,
                seed: Some(seed),
            });
        }
    }

    /// Run the command an action queued.
    fn run_pending_command(&mut self) {
        let Some(command) = self.state.pending_command.take() else {
            return;
        };
        self.event("command", json!({ "command": command.as_str() }));
        match command {
            CommandId::StartRide => self.state.start_ride(),
            CommandId::StopRide => self.state.stop_ride(),
            CommandId::UndoCrossing => self.state.undo_crossing(),
            CommandId::FinishRide => self.state.finish_ride(),
            CommandId::ReopenRide => self.state.reopen_ride(),
            CommandId::BackupNow => self.state.queue_backup(),
            CommandId::ClearRide => self.state.clear_ride(),
            CommandId::AppQuit => self.state.confirm_quit(),
            CommandId::ExportHtml
            | CommandId::ExportPdf
            | CommandId::ExportPoster
            | CommandId::ExportPosterHtml
            | CommandId::ExportCsv => {
                if let Some(kind) = ExportKind::from_command(command) {
                    self.raise_export_picker(kind);
                }
            }
            // The roster CSV's own save picker is deferred; the export names the missing piece.
            CommandId::ExportRosterCsv => self.state.note(FILE_PICKER_UNAVAILABLE),
            CommandId::PreviewHtml => self.preview(ExportKind::Html),
            CommandId::PreviewPosterHtml => self.preview(ExportKind::PosterHtml),
            CommandId::PreviewPdf => self.preview(ExportKind::Pdf),
            other => self.state.note(format!(
                "{} is not available in this release",
                other.as_str()
            )),
        }
    }

    /// Ask the save picker where to write *kind*.
    fn raise_export_picker(&mut self, kind: ExportKind) {
        let pending = PendingNative {
            dialog: NativeDialog::Confirm,
            then: NativeTask::ExportFile { kind },
        };
        let dialog = FileDialog::save()
            .title("Save export")
            .filter(kind.extension().to_uppercase(), &[kind.extension()])
            .file_name(self.state.export_file_name(kind));
        self.event("export-picker", json!({ "kind": kind.as_str() }));
        self.native.ask_file(&pending, dialog);
    }

    /// Raise the platform's open picker for the organizer logo.
    fn choose_logo(&mut self) {
        let pending = PendingNative {
            dialog: NativeDialog::Confirm,
            then: NativeTask::ChooseLogo,
        };
        let dialog = FileDialog::open()
            .title("Choose the organizer logo")
            .filter("PNG", &["png"]);
        self.event("logo-picker", json!({}));
        self.native.ask_file(&pending, dialog);
    }

    /// Open the last file that *kind* wrote, or post why it cannot.
    fn preview(&mut self, kind: ExportKind) {
        match self.state.preview_target(kind) {
            Ok(path) => self.open_target(&path.to_string_lossy()),
            Err(message) => self.state.note(message),
        }
    }

    /// Start the publish a confirmed form queued, on its own thread.
    fn start_pending_publish(&mut self) {
        let Some(start) = self.state.pending_publish.take() else {
            return;
        };
        if self.publish_job.is_some() {
            self.state.pending_publish = Some(start);
            return;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let thread_cancel = Arc::clone(&cancel);
        let (sender, reports) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("rivercrossing-publish".to_owned())
            .spawn(move || {
                let writer = WordPressWriter;
                let report = publish_job::run_publish(&writer, &start, &|| {
                    thread_cancel.load(Ordering::Relaxed)
                });
                let _ = sender.send(report);
            });
        if spawned.is_err() {
            self.state.note("the publish thread could not start");
            let _ = self.state.dialogs.cancel();
            return;
        }
        self.event("publish-started", json!({}));
        self.publish_job = Some(PublishJob { reports, cancel });
    }

    /// Apply the publish thread's report: close the window and raise the native outcome.
    fn drain_publish(&mut self) {
        let Some(job) = self.publish_job.as_ref() else {
            return;
        };
        let Some(report) = job.reports.try_iter().last() else {
            return;
        };
        self.publish_job = None;
        let _ = self.state.dialogs.cancel();
        match report {
            Ok(PublishOutcome::Published(page)) => {
                self.event(
                    "publish",
                    json!({ "link": page.link, "created": page.created }),
                );
                self.state.publish_form = None;
                self.state.raise_native(
                    NativeDialog::Confirm,
                    NativeTask::OpenPublished { link: page.link },
                );
            }
            Ok(PublishOutcome::Cancelled) => {
                self.event("publish", json!({ "cancelled": true }));
                self.state.note("Publish cancelled");
            }
            Err(message) => {
                self.event("publish", json!({ "error": message }));
                self.state.note(message);
                self.state
                    .raise_native(NativeDialog::Confirm, NativeTask::RetryPublish);
            }
        }
    }

    /// Start the simulation a confirmed form queued, on its own thread.
    fn start_pending_sim(&mut self) {
        let Some(start) = self.state.pending_sim.take() else {
            return;
        };
        if self.sim_job.is_some() {
            self.state.pending_sim = Some(start);
            return;
        }
        let Some(mut ride) = self.state.ride.take() else {
            self.state.note("Open a ride first");
            return;
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let thread_cancel = Arc::clone(&cancel);
        let (sender, reports) = mpsc::channel();
        let progress_sender = sender.clone();
        let spawned = std::thread::Builder::new()
            .name("rivercrossing-sim".to_owned())
            .spawn(move || {
                let outcome = simulator::run_simulation(
                    &mut ride.engine,
                    &ride.roster,
                    &start.settings,
                    &mut |fraction| {
                        let _ = progress_sender.send(SimReport::Progress(fraction));
                    },
                    &|| thread_cancel.load(Ordering::Relaxed),
                );
                let _ = sender.send(SimReport::Done(Box::new(ride), outcome, start.pre_events));
            });
        if spawned.is_err() {
            self.state.note("the simulation thread could not start");
            let _ = self.state.dialogs.cancel();
            return;
        }
        self.event("simulation-started", json!({}));
        self.sim_job = Some(SimJob { reports, cancel });
    }

    /// Apply the simulation thread's progress and final report.
    fn drain_sim(&mut self) {
        let Some(job) = self.sim_job.as_ref() else {
            return;
        };
        let mut finished = None;
        for report in job.reports.try_iter() {
            match report {
                SimReport::Progress(fraction) => self.state.set_sim_progress(fraction),
                SimReport::Done(ride, outcome, pre_events) => {
                    finished = Some((ride, outcome, pre_events));
                }
            }
        }
        if let Some((ride, outcome, pre_events)) = finished {
            self.sim_job = None;
            self.event("simulation", json!({ "recorded": outcome.recorded }));
            self.state.finish_simulation(*ride, outcome, pre_events);
        }
    }

    /// Pass the in-app cancel requests to the running jobs.
    fn sync_cancel_flags(&mut self) {
        if std::mem::take(&mut self.state.publish_cancel)
            && let Some(job) = self.publish_job.as_ref()
        {
            job.cancel.store(true, Ordering::Relaxed);
        }
        if std::mem::take(&mut self.state.sim_cancel)
            && let Some(job) = self.sim_job.as_ref()
        {
            job.cancel.store(true, Ordering::Relaxed);
        }
    }

    /// Send the store commands that the views queued.
    fn drain_pending_store(&mut self) {
        let commands = std::mem::take(&mut self.state.pending_store);
        if let Some(worker) = &self.worker {
            if let Some(job) = self.state.pending_export.take() {
                worker.send(StoreCommand::Export(Box::new(job)));
            }
            for command in commands {
                worker.send(command);
            }
        }
    }

    /// Play the cues the console queued.
    fn play_pending_cues(&mut self) {
        let cues = std::mem::take(&mut self.state.pending_cues);
        self.sound.set_muted(!self.state.settings.sound_on);
        for cue in cues {
            if self.sound.play(cue).is_some() {
                self.event("cue", json!({ "cue": cue.as_str() }));
            }
        }
    }

    /// Apply the text zoom whenever it changes.
    ///
    /// The mode, contrast, density, text size and theme reach the runtime through
    /// [`ShellApp::frame_input`]; the frozen zoom rungs stay a separate axis.
    fn apply_zoom(&mut self, ctx: &Context) {
        let zoom = self.state.settings.zoom_percent;
        if self.applied_zoom != Some(zoom) {
            let factor = f32::from(zoom.max(10)) / 100.0;
            ctx.set_zoom_factor(factor);
            self.applied_zoom = Some(zoom);
        }
    }

    /// Close the window at the quit deadline, or ask for a frame at it.
    fn quit_deadline(&self, ctx: &Context) {
        let Some(deadline) = self.quit_after else {
            return;
        };
        let now = Instant::now();
        if now >= deadline {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        } else {
            ctx.request_repaint_after(deadline - now);
        }
    }
}

impl Drop for RiverCrossingApp {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take()
            && let Err(error) = worker.shutdown()
        {
            report(&format!("the database did not close cleanly: {error}"));
        }
    }
}

impl ShellApp for RiverCrossingApp {
    fn frame_input(&mut self) -> FrameInput {
        FrameInput::new(super::appearance::root_env(&self.state.settings))
    }

    fn ui(&mut self, ui: &mut Ui) {
        // The in-window bar is the fallback: a platform menu bar replaces it where one is
        // installed (macOS). It renders in the console shell's title bar.
        views::console::show(ui, &mut self.state, !self.menubar.is_installed());
        views::dialogs::show(ui, &mut self.state);
        views::dialogs::show_native(ui, &mut self.state);
        self.native.show_pending();
        if let Some(control) = self.control.as_mut() {
            control.ui(&mut self.state, ui.ctx());
        }
        if activation_consumed(ui.ctx()) {
            // The activation only queues its effect; the next frame's logic pass runs it, so ask
            // for that frame rather than waiting for the next input or the scheduled deadline.
            ui.ctx().request_repaint();
        }
    }

    fn logic(&mut self, ctx: &Context) {
        self.menubar.drive(ctx, &mut self.state);
        self.answer_native_dialogs();
        self.drain_worker();
        self.drain_pending_store();
        self.start_pending_create();
        self.raise_native_dialogs();
        if std::mem::take(&mut self.state.choose_logo) {
            self.choose_logo();
        }
        self.run_pending_command();
        self.sync_cancel_flags();
        self.start_pending_publish();
        self.drain_publish();
        self.start_pending_sim();
        self.drain_sim();
        self.play_pending_cues();
        self.apply_zoom(ctx);
        let _ = self.state.due_hourly_backup(OffsetDateTime::now_utc());
        self.drain_pending_store();
        if let Some(control) = self.control.as_mut() {
            control.logic(&mut self.state, ctx);
        }
        if std::mem::take(&mut self.state.close_requested) {
            self.quit_confirmed = true;
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
        if std::mem::take(&mut self.minimize_next_frame) {
            // A macOS close request hides the window. `hold_close` already sent Minimized(false)
            // and Focus in the pass that held the close, so the minimize goes in this next pass.
            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
        }
        self.quit_deadline(ctx);
    }

    fn tick(&mut self, now: Instant) -> Option<Instant> {
        let mut next = next_hour_instant(now);
        if let Some(deadline) = self.quit_after {
            next = next.min(deadline);
        }
        if self.publish_job.is_some() || self.sim_job.is_some() {
            next = next.min(now + JOB_POLL_INTERVAL);
        }
        if self.state.phase() == RidePhase::Running {
            next = next.min(now + CLOCK_TICK_INTERVAL);
        }
        Some(next)
    }

    fn close_requested(&mut self) -> CloseDecision {
        if self.quit_confirmed || self.quit_after.is_some() || self.control.is_some() {
            return CloseDecision::Close;
        }
        if cfg!(target_os = "macos") {
            // macOS hides the window instead of quitting: the app keeps running and the Dock
            // icon brings it back. Quit runs only from File ▸ Exit (⌘Q), which asks its confirm.
            self.minimize_next_frame = true;
            return CloseDecision::Hold;
        }
        self.state.request_quit();
        CloseDecision::Hold
    }
}

/// Open the `RiverCrossing` window and run the app until the window closes.
///
/// # Errors
///
/// Return the eframe error when the window cannot open or the event loop stops with an error.
pub fn run(quit_after: Option<Duration>, control: Option<ControlConfig>) -> eframe::Result {
    let paths = AppPaths::defaults();
    let (settings, _) = rivercrossing_store::load_settings(&paths.settings);
    let placement = Arc::new(Mutex::new(window::seeded_store(
        Store::open(APP_ID),
        &settings,
    )));
    run_shell(window::spec_with_store(placement), move |ctx| {
        ctx.enable_accesskit();
        // AppKit's window tab bar is off before the first frame: it is a title-row strip the app
        // never uses, and it contributes View ▸ Hide Tab Bar and Show All Tabs.
        #[cfg(target_os = "macos")]
        macos::disallow_window_tabs();
        let automation = control.is_some();
        let control = control.map(|config| {
            Control::start(config, ctx).unwrap_or_else(|error| start_failure(&error))
        });
        let mut app = RiverCrossingApp::new_with_context(ctx, quit_after, control);
        if automation {
            // The control host cannot press an OS dialog; its confirmations render in-app.
            app.set_native_unavailable();
        }
        // Install the platform menu bar here, in eframe's creator closure: winit builds its default
        // menu before this and would replace one installed earlier.
        app.menubar.install(ctx, &app.state);
        app
    })
}

/// The pause between two polls of a running job's channel.
const JOB_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// The wall clock's tick: one second, so the console's elapsed and remaining readouts advance while a
/// ride is live.
const CLOCK_TICK_INTERVAL: Duration = Duration::from_secs(1);

/// What the status bar says after a backup the operator asked for. It names no file: the path is in
/// the log.
const BACKUP_WRITTEN: &str = "Backup written";

/// Whether this pass consumed a pointer click or a key press.
fn activation_consumed(ctx: &Context) -> bool {
    ctx.input(|input| {
        input.pointer.any_click()
            || input
                .events
                .iter()
                .any(|event| matches!(event, Event::Key { pressed: true, .. }))
    })
}

/// Return the notice for a failed `open`, or `None` when the handler started.
fn open_notice(target: &str, opener: &dyn Fn(&str) -> Result<(), String>) -> Option<String> {
    opener(target)
        .err()
        .map(|error| format!("cannot open {target}: {error}"))
}

/// Return the instant of the next whole UTC hour, or *now* when the clock is at it.
fn next_hour_instant(now: Instant) -> Instant {
    let now_utc = OffsetDateTime::now_utc();
    let seconds = 3_600 - (i64::from(now_utc.minute()) * 60 + i64::from(now_utc.second()));
    let wait = u64::try_from(seconds.clamp(1, 3_600)).unwrap_or(3_600);
    now + Duration::from_secs(wait)
}

/// Report one warning on stderr.
fn report(message: &str) {
    writeln!(std::io::stderr(), "warning: {message}").ok();
}

/// Report a control-server start failure on stderr and exit with the usage status.
fn start_failure(error: &str) -> ! {
    writeln!(std::io::stderr(), "error: {error}").ok();
    std::process::exit(2)
}

#[cfg(test)]
mod tests;
