// SPDX-License-Identifier: GPL-3.0-only

//! The pure dialog state machine: the frozen dialog contracts and the stack that owns open, close,
//! Esc, Enter, Tab and focus return. Phase 6 renders each dialog over `egui::Modal`.

pub(crate) mod spec;
pub(crate) mod stack;
