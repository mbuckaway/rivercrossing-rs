// SPDX-License-Identifier: GPL-3.0-only

//! The `RiverCrossing` desktop binary: open the shell window and run it until it closes.

/// Open the `RiverCrossing` window and run the app until the window closes.
fn main() -> eframe::Result {
    rivercrossing_ui_egui::run(smoke_quit_after())
}

/// Return the debug-only self-quit delay from `RIVERCROSSING_SMOKE_QUIT_AFTER_MS`.
///
/// This is the launch-path smoke seam. The functional test sets the variable, so the window
/// closes by itself and the test never has to signal the process. Only a debug build reads it:
/// a release build ignores the variable, because a shipped app must not quit on environment
/// input.
#[cfg(debug_assertions)]
fn smoke_quit_after() -> Option<std::time::Duration> {
    std::env::var("RIVERCROSSING_SMOKE_QUIT_AFTER_MS")
        .ok()
        .and_then(|ms| ms.parse().ok())
        .map(std::time::Duration::from_millis)
}

/// Return no self-quit delay: a release build never quits on environment input.
#[cfg(not(debug_assertions))]
fn smoke_quit_after() -> Option<std::time::Duration> {
    None
}
