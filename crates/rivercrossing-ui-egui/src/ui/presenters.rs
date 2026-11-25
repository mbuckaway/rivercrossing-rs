// SPDX-License-Identifier: GPL-3.0-only

//! The pure presenters: the console's read-only view models, the plate-entry
//! and lifecycle decisions, and the ride-setup parsing and validation.
//!
//! Every function here is wx-free and egui-free, so the decisions are
//! unit-testable without a window. Phase 6 renders their results.

pub(crate) mod console;
pub(crate) mod data_source;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the editor views that call these presenters land in the following PR; today the unit tests exercise them"
    )
)]
pub(crate) mod rider_issues;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the editor views that call these presenters land in the following PR; today the unit tests exercise them"
    )
)]
pub(crate) mod riders;
pub(crate) mod setup;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the editor views that call these presenters land in the following PR; today the unit tests exercise them"
    )
)]
pub(crate) mod teams;
