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
        reason = "Phase 6 renders the dialog stack; today only the tests read it"
    )
)]
pub(crate) mod dialogs;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Phase 6 addresses each control by these frozen ids, so only the tests read them today"
    )
)]
pub(crate) mod ids;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Phase 5 and Phase 6 read the app state; today only the tests build it"
    )
)]
pub(crate) mod state;
pub mod window;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Phase 6 starts the store worker; today only the tests drive it"
    )
)]
pub(crate) mod worker;
