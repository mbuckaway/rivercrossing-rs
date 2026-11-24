// SPDX-License-Identifier: GPL-3.0-only

//! The automation surface: the selector and scenario models plus the pure path and command helpers.

pub mod scenario;
pub mod selector;

pub(crate) mod channel;
pub(crate) mod command;
pub(crate) mod path;

pub use channel::ControlConfig;
pub(crate) use channel::{Control, ProbeState};

#[cfg(test)]
mod tests;
