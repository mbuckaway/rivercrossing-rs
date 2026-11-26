// SPDX-License-Identifier: GPL-3.0-only

//! The UI layer: the shell app, its window identity and the automation surface.

pub(crate) mod action;
pub mod app;
pub mod automation;
pub(crate) mod dialogs;
pub(crate) mod ids;
pub(crate) mod logging;
pub(crate) mod native;
pub(crate) mod presenters;
pub(crate) mod publish_job;
#[cfg_attr(
    test,
    expect(
        dead_code,
        reason = "the rodio backend needs a real output device, so the unit tests never build it"
    )
)]
pub(crate) mod sound;
pub(crate) mod state;
#[expect(
    clippy::excessive_nesting,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "the view layer nests one closure per layout container and passes one frozen id per widget; the presenters hold the decisions"
)]
pub(crate) mod views;
pub mod window;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the store commands and reply fields the later screens drive; today the unit tests exercise them"
    )
)]
pub(crate) mod worker;
