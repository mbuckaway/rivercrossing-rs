// SPDX-License-Identifier: GPL-3.0-only

//! The `RiverCrossing` shell app and its eframe entry point.

use std::fmt;
use std::io::Write;
use std::time::{Duration, Instant};

use craft::egui::{Context, Ui, ViewportCommand};
#[cfg(debug_assertions)]
use craft::egui::{Label, TextEdit};
use craft::ui::runtime::env::RootEnv;
use craft::ui::runtime::frame::FrameInput;
use craft::ui::shell::app::{ShellApp, run as run_shell};

use super::automation::{Control, ControlConfig, ProbeState};
use super::window;

/// The `RiverCrossing` app driven by the craft shell.
pub struct RiverCrossingApp {
    /// The instant the app asks the root window to close; `None` keeps the window open.
    quit_after: Option<Instant>,
    /// The live automation channel, present in a debug build launched with `--control`.
    control: Option<Control>,
    /// The debug probe surface's state.
    probe: ProbeState,
}

impl fmt::Debug for RiverCrossingApp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RiverCrossingApp")
            .field("quit_after", &self.quit_after)
            .field("control", &self.control.is_some())
            .field("probe", &self.probe)
            .finish()
    }
}

impl RiverCrossingApp {
    /// Build the app. A `quit_after` duration becomes a deadline counted from now.
    ///
    /// A duration that overshoots the platform clock leaves no deadline, so the window stays open.
    #[must_use]
    pub fn new(quit_after: Option<Duration>, control: Option<Control>) -> Self {
        Self {
            quit_after: quit_after.and_then(|after| Instant::now().checked_add(after)),
            control,
            probe: ProbeState::default(),
        }
    }

    /// Show the debug probe surface: the echo button, the named text field and the echo label.
    #[cfg(debug_assertions)]
    fn probe_panel(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if ui.button("Probe Echo").clicked() {
                self.probe.clicks = self.probe.clicks.saturating_add(1);
                self.probe.echo = Some(format!("clicked {}", self.probe.clicks));
            }
            ui.add(TextEdit::singleline(&mut self.probe.text).hint_text("Probe Input"))
                .accessible_name("Probe Input");
        });
        if let Some(echo) = &self.probe.echo {
            // A selectable label senses clicks and has no accessible name, which `access::violations` flags.
            ui.add(Label::new(echo).selectable(false));
        }
    }
}

impl ShellApp for RiverCrossingApp {
    fn frame_input(&mut self) -> FrameInput {
        FrameInput::new(RootEnv::default())
    }

    fn ui(&mut self, ui: &mut Ui) {
        #[cfg(debug_assertions)]
        self.probe_panel(ui);
        if let Some(control) = self.control.as_mut() {
            control.ui(ui.ctx());
        }
    }

    fn logic(&mut self, ctx: &Context) {
        if let Some(control) = self.control.as_mut() {
            control.logic(&mut self.probe, ctx);
        }
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

/// Open the `RiverCrossing` window and run the app until the window closes.
///
/// # Errors
///
/// Return the eframe error when the window cannot open or the event loop stops with an error.
pub fn run(quit_after: Option<Duration>, control: Option<ControlConfig>) -> eframe::Result {
    run_shell(window::spec(), move |ctx| {
        ctx.enable_accesskit();
        let control = control.map(|config| {
            Control::start(config, ctx).unwrap_or_else(|error| start_failure(&error))
        });
        RiverCrossingApp::new(quit_after, control)
    })
}

/// Report a control-server start failure on stderr and exit with the usage status.
fn start_failure(error: &str) -> ! {
    writeln!(std::io::stderr(), "error: {error}").ok();
    std::process::exit(2)
}

#[cfg(test)]
mod tests;
