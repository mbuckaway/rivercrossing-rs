// SPDX-License-Identifier: GPL-3.0-only

//! The `RiverCrossing` window identity and its documented defaults.

use craft::behavior::placement::Defaults;
use craft::ui::shell::app::AppSpec;

/// The product name shown in the window title (R-82).
pub const APP_NAME: &str = "RiverCrossing";

/// The app id: the Wayland app id and the name eframe stores the app under.
pub const APP_ID: &str = "rivercrossing";

/// The documented console minimum, 1100 by 780 points (`design/docs-md/spec.md` line 291).
pub const MIN_SIZE: [f32; 2] = [1100.0, 780.0];

/// Return the root window spec with the product title, the binary app id, and the console minimum.
///
/// The first window and the minimum are both the documented console size. The window stays
/// resizable because `native_options` never sets `ViewportBuilder::resizable`, and egui-winit
/// reads the unset value as `true`.
#[must_use]
pub fn spec() -> AppSpec {
    AppSpec::new(APP_ID, APP_NAME, Defaults::new(MIN_SIZE, MIN_SIZE))
}

#[cfg(test)]
mod tests;
