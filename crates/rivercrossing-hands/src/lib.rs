// SPDX-License-Identifier: GPL-3.0-only

//! Five-card hand evaluation for `RiverCrossing`: the hand classes, the best-five
//! selection, the comparison rules, and the golden vectors that pin them.

#![forbid(unsafe_code)]

pub mod best;
pub mod class;
pub mod compare;
pub mod error;
pub mod eval;
pub mod self_test;
pub mod vectors;

pub use best::best_hand;
pub use class::HandClass;
pub use error::HandsError;
pub use eval::{EvaluatedHand, eval5};
