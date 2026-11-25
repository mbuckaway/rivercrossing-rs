// SPDX-License-Identifier: GPL-3.0-only

//! The `RiverCrossing` shell app and its eframe entry point.
//!
//! The app owns the state the views render, the one store thread, the sound player, the launch log
//! and the debug control channel. `logic` drains the store replies, persists a submitted ride, runs
//! the queued commands, answers the native dialogs, plays the queued cues and applies the theme and
//! zoom; `ui` renders the menu bar, the console and the top dialog.

use std::fmt;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use craft::egui::{Context, ThemePreference, Ui, ViewportCommand};
use craft::ui::runtime::env::RootEnv;
use craft::ui::runtime::frame::FrameInput;
use craft::ui::shell::app::{ShellApp, run as run_shell};
use craft::window::restore::Store;
use serde_json::json;
use time::OffsetDateTime;

use super::action::spec::CommandId;
use super::automation::{self, Control, ControlConfig};
use super::logging::LaunchLog;
use super::sound::{SilentBackend, SoundPlayer};
use super::state::{AppState, LoadedRide, new_ride_seed};
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
    /// The appearance last applied to the context.
    applied_appearance: Option<ThemePreference>,
    /// The zoom percentage last applied to the context.
    applied_zoom: Option<u16>,
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
            .field("applied_appearance", &self.applied_appearance)
            .field("applied_zoom", &self.applied_zoom)
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
        )
    }

    /// Build the app over *paths* and *sound*.
    #[expect(
        clippy::too_many_arguments,
        reason = "the launch seams: the quit deadline, the control channel, the file paths and the player"
    )]
    fn build(
        quit_after: Option<Duration>,
        control: Option<Control>,
        paths: AppPaths,
        sound: SoundPlayer,
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
            applied_appearance: None,
            applied_zoom: None,
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
                app.worker = Some(worker);
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

    /// Write one field on the top dialog, as `ui.set` does.
    ///
    /// # Errors
    ///
    /// Return the automation message when no dialog is open or it does not declare the field.
    pub fn set_dialog_field(&mut self, field: &str, value: &str) -> Result<(), String> {
        automation::set(&mut self.state, &json!({ "field": field, "value": value }))
            .map(|_reply| ())
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
        {
            match reply {
                Ok(StoreReply::Ride {
                    id,
                    config,
                    roster,
                    engine,
                }) => {
                    let ride = LoadedRide {
                        ride_id: id,
                        config,
                        roster: *roster,
                        engine: *engine,
                    };
                    self.event("ride-loaded", json!({ "ride_id": ride.ride_id }));
                    self.state.ride = Some(ride);
                    self.state.audit_row_count = 0;
                }
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
                }
                Ok(StoreReply::AuditRows(rows)) => {
                    self.state.audit_row_count = rows.len();
                }
                Ok(StoreReply::Backup(path)) => {
                    self.state.note(path.display().to_string());
                }
                Ok(_other) => {}
                Err(message) => {
                    self.state.note(message.clone());
                    self.event("store-error", json!({ "message": message }));
                }
            }
        }
    }

    /// Ask the store thread to rebuild *id*.
    fn request_load(&self, id: i64) {
        if let Some(worker) = &self.worker {
            worker.send(StoreCommand::LoadRide { id });
        }
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
    fn run_pending_command(&mut self, ctx: &Context) {
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
            CommandId::ClearRide => self.state.clear_ride(),
            CommandId::AppQuit => ctx.send_viewport_cmd(ViewportCommand::Close),
            other => self.state.note(format!(
                "{} is not available in this release",
                other.as_str()
            )),
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

    /// Apply the appearance and the text zoom whenever either setting changes.
    fn apply_appearance(&mut self, ctx: &Context) {
        let appearance = match self.state.settings.appearance.as_str() {
            "light" => ThemePreference::Light,
            "dark" => ThemePreference::Dark,
            _ => ThemePreference::System,
        };
        if self.applied_appearance != Some(appearance) {
            ctx.set_theme(appearance);
            self.applied_appearance = Some(appearance);
        }
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
        FrameInput::new(RootEnv::default())
    }

    fn ui(&mut self, ui: &mut Ui) {
        views::menu::show(ui, &mut self.state);
        ui.separator();
        views::console::show(ui, &mut self.state);
        views::dialogs::show(ui.ctx(), &mut self.state);
        views::dialogs::show_native(ui.ctx(), &mut self.state);
        if let Some(control) = self.control.as_mut() {
            control.ui(&mut self.state, ui.ctx());
        }
    }

    fn logic(&mut self, ctx: &Context) {
        self.drain_worker();
        self.start_pending_create();
        self.run_pending_command(ctx);
        self.play_pending_cues();
        self.apply_appearance(ctx);
        if let Some(control) = self.control.as_mut() {
            control.logic(&mut self.state, ctx);
        }
        self.quit_deadline(ctx);
    }

    fn tick(&mut self, now: Instant) -> Option<Instant> {
        self.quit_after.filter(|deadline| *deadline > now)
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
        let control = control.map(|config| {
            Control::start(config, ctx).unwrap_or_else(|error| start_failure(&error))
        });
        RiverCrossingApp::new(quit_after, control)
    })
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
