// SPDX-License-Identifier: GPL-3.0-only

//! The pure presenters: the console's read-only view models, the plate-entry
//! and lifecycle decisions, the ride-setup parsing and validation, the audit
//! trail, the lifecycle and correction decisions, the results standings and
//! stale flag, the export composition, the publish-dialog rules and the
//! simulator's scripted race.
//!
//! Every function here is wx-free and egui-free, so the decisions are
//! unit-testable without a window. Phase 6 renders their results.

pub(crate) mod audit;
pub(crate) mod console;
pub(crate) mod corrections;
pub(crate) mod data_source;
pub(crate) mod export_input;
pub(crate) mod library;
pub(crate) mod publish_wordpress;
pub(crate) mod quit_resume;
pub(crate) mod results;
pub(crate) mod ride_defaults;
pub(crate) mod rider_issues;
pub(crate) mod riders;
pub(crate) mod selftest;
pub(crate) mod settings_form;
pub(crate) mod setup;
pub(crate) mod simulator;
pub(crate) mod teams;
