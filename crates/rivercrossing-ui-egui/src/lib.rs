// SPDX-License-Identifier: GPL-3.0-only

//! The `RiverCrossing` desktop shell: an empty egui window over the `ArtCraft` `craft` façade.

#![forbid(unsafe_code)]

pub mod ui;

pub use ui::app::run;
pub use ui::window::{APP_NAME, spec};
