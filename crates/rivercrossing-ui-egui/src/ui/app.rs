// SPDX-License-Identifier: GPL-3.0-only

//! The `RiverCrossing` shell app and its eframe entry point.

use std::time::{Duration, Instant};

use craft::egui::{Context, Ui, ViewportCommand};
use craft::ui::runtime::env::RootEnv;
use craft::ui::runtime::frame::FrameInput;
use craft::ui::shell::app::{ShellApp, run as run_shell};

use super::window;

/// The `RiverCrossing` app driven by the craft shell.
#[derive(Debug)]
pub struct RiverCrossingApp {
    /// The instant the app asks the root window to close; `None` keeps the window open.
    quit_after: Option<Instant>,
}

impl RiverCrossingApp {
    /// Build the app. A `quit_after` duration becomes a deadline counted from now.
    ///
    /// A duration that overshoots the platform clock leaves no deadline, so the window stays open.
    #[must_use]
    pub fn new(quit_after: Option<Duration>) -> Self {
        Self {
            quit_after: quit_after.and_then(|after| Instant::now().checked_add(after)),
        }
    }
}

impl ShellApp for RiverCrossingApp {
    fn frame_input(&mut self) -> FrameInput {
        FrameInput::new(RootEnv::default())
    }

    /// Mount the real `RiverCrossing` shell here in EPIC-11; this shell is empty by design.
    fn ui(&mut self, _ui: &mut Ui) {}

    fn logic(&mut self, ctx: &Context) {
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
pub fn run(quit_after: Option<Duration>) -> eframe::Result {
    run_shell(window::spec(), move |_| RiverCrossingApp::new(quit_after))
}

#[cfg(test)]
mod tests;
