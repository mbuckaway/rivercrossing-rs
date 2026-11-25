// SPDX-License-Identifier: GPL-3.0-only

//! The automation surface: the selector and scenario models plus the pure path, command and
//! action helpers.

pub mod scenario;
pub mod selector;

pub(crate) mod actions;
pub(crate) mod channel;
pub(crate) mod path;

pub(crate) use actions::{command, set};
pub(crate) use channel::Control;
pub use channel::ControlConfig;

#[cfg(test)]
mod tests;
