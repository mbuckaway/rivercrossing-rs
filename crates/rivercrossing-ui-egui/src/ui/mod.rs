// SPDX-License-Identifier: GPL-3.0-only

//! The UI layer: the shell app, its window identity and the automation surface.

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Phase 6 wires the action registry into the menu, the console and the automation channel"
    )
)]
pub(crate) mod action;
pub mod app;
pub mod automation;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Phase 6 addresses each control by these frozen ids, so only the tests read them today"
    )
)]
pub(crate) mod ids;
pub mod window;
