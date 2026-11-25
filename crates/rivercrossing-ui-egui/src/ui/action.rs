// SPDX-License-Identifier: GPL-3.0-only

//! The §15/§15b action registry: the frozen ids, the action table, the enablement gates and the
//! pure dispatch.

pub(crate) mod dispatch;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the ids-drift machine block and its comparisons run in the unit tests"
    )
)]
pub(crate) mod drift;
pub(crate) mod effect;
pub(crate) mod enable;
pub(crate) mod id;
pub(crate) mod spec;
