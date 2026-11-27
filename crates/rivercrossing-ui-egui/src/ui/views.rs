// SPDX-License-Identifier: GPL-3.0-only

//! The rendered screens: the menu bar, the console, the ride-setup form and the dialog host.
//!
//! The views hold no decisions of their own: they read [`crate::ui::state::AppState`], show the
//! presenter results and forward each activation to the §15 action registry.

pub(crate) mod audit;
pub(crate) mod console;
pub(crate) mod corrections;
pub(crate) mod csv;
pub(crate) mod dialogs;
pub(crate) mod issues;
pub(crate) mod library;
pub(crate) mod menu;
pub(crate) mod publish;
pub(crate) mod results;
pub(crate) mod riders;
pub(crate) mod selftest;
pub(crate) mod settings;
pub(crate) mod setup;
pub(crate) mod simulation;
pub(crate) mod system;
pub(crate) mod teams;
