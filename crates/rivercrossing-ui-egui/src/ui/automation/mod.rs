// SPDX-License-Identifier: GPL-3.0-only

//! The automation surface: the selector and scenario models plus the pure path, command and
//! action helpers.

pub mod scenario;
pub mod selector;

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Phase 6 wires the automation actions into the control channel; today only the tests drive them"
    )
)]
pub(crate) mod actions;
pub(crate) mod channel;
pub(crate) mod command;
pub(crate) mod path;

pub use channel::ControlConfig;
pub(crate) use channel::{Control, ProbeState};

#[cfg(test)]
mod tests;
