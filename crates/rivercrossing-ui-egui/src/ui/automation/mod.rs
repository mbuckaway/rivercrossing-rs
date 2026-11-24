// SPDX-License-Identifier: GPL-3.0-only

//! The automation surface: pure selector, path and command helpers over the live craft channel.

pub mod selector;

pub(crate) mod channel;
pub(crate) mod command;
pub(crate) mod path;

pub use channel::ControlConfig;
pub(crate) use channel::{Control, ProbeState};

#[cfg(test)]
mod tests;
