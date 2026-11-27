// SPDX-License-Identifier: GPL-3.0-only

//! The `RiverCrossing` window identity, its documented defaults and its placement store.

use std::sync::{Arc, Mutex};

use craft::behavior::placement::{Defaults, Placement};
use craft::ui::shell::app::AppSpec;
use craft::window::Prefix;
use craft::window::policy::{Backend, TitleBarPref, decide_for};
use craft::window::restore::{MAIN, Store};
use rivercrossing_store::settings::AppSettings;

/// The title-bar prefix this app reads: an override is `RIVERCROSSING_TITLEBAR`.
const TITLEBAR_PREFIX: Prefix = Prefix::new("RIVERCROSSING");

/// The product name shown in the window title (R-82).
pub const APP_NAME: &str = "RiverCrossing";

/// The app id: the Wayland app id and the name eframe stores the app under.
pub const APP_ID: &str = "rivercrossing";

/// The documented console minimum, 1100 by 780 points (`design/docs-md/spec.md` line 291).
pub const MIN_SIZE: [f32; 2] = [1100.0, 780.0];

/// Return the root window spec with the product title, the binary app id, the console minimum and
/// the per-app placement store.
///
/// The first window and the minimum are both the documented console size. The window stays
/// resizable because `native_options` never sets `ViewportBuilder::resizable`, and egui-winit
/// reads the unset value as `true`.
#[must_use]
pub fn spec() -> AppSpec {
    spec_with_store(Arc::new(Mutex::new(Store::open(APP_ID))))
}

/// Return the root window spec over an explicit placement store.
///
/// The frame is the app's own bar, not the OS's: a spec that leaves `frame` unset is drawn by the
/// platform *and* by the console's `TitleBar`, which puts two titles on the window. `Integrated`
/// resolves to the full-size content view with the title bar hidden on macOS, so the console's bar
/// is the one title row (craft `crates/window/src/policy.rs`, `Frame::Inset`).
#[must_use]
pub(crate) fn spec_with_store(placement: Arc<Mutex<Store>>) -> AppSpec {
    AppSpec::new(APP_ID, APP_NAME, Defaults::new(MIN_SIZE, MIN_SIZE))
        .placement(placement)
        .frame(decide_for(
            TITLEBAR_PREFIX,
            Backend::from_env(),
            TitleBarPref::Integrated,
        ))
}

/// Seed the placement store's first window from the Python settings geometry, on first run only.
///
/// A store that already holds the `MAIN` placement keeps it: the craft-window file is the runtime
/// record, and `settings.json`'s `window_geometry` is read-only.
pub(crate) fn seeded_store(mut store: Store, settings: &AppSettings) -> Store {
    if store.get(MAIN).is_none()
        && let Some(placement) = first_run_placement(settings)
    {
        store.set(MAIN, &placement);
    }
    store
}

/// Return the placement to store for the saved `window_geometry` array, if one is usable.
///
/// The Python rectangle is `[x, y, width, height]` in physical pixels; the placement keeps the size
/// in points at scale 1.0. A rectangle whose size is not a size returns `None`, so a damaged
/// settings file never yields an unopenable window.
#[must_use]
#[expect(
    clippy::cast_precision_loss,
    reason = "a saved window rectangle is far inside the exactly representable f32 range"
)]
pub(crate) fn first_run_placement(settings: &AppSettings) -> Option<Placement> {
    let [x, y, width, height] = settings.window_geometry?;
    let mut placement = Placement::new([width as f32, height as f32], 1.0);
    placement.position_px = Some([x, y]);
    placement.normal.pos = [x as f32, y as f32];
    placement.repaired()
}

#[cfg(test)]
mod tests;
