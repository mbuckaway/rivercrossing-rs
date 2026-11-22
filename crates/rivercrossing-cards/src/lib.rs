// SPDX-License-Identifier: GPL-3.0-only

//! The card model, the seeded shoe, and the draw helpers for `RiverCrossing`.
//!
//! Every sequence comes from a stored seed through a port of `CPython`'s `Mersenne Twister`,
//! so replaying a seed reproduces every deal exactly.

#![forbid(unsafe_code)]

mod card;
mod codes;
mod draw;
mod error;
mod rng;
mod shoe;

pub use card::{Card, Rank, Suit};
pub use codes::seeded_card_codes;
pub use draw::{draw_key, high_card_draw};
pub use error::CardsError;
pub use shoe::{JokersMode, Shoe};
