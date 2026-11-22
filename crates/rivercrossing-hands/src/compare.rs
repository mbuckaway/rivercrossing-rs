// SPDX-License-Identifier: GPL-3.0-only

//! The comparison order between two evaluated hands.

use std::cmp::{Ordering, Reverse};

use crate::eval::EvaluatedHand;

/// Compare two evaluated hands: class first, then the tiebreak, then fewer jokers.
///
/// The key is `(class, tiebreak, -jokers_played)`, so a natural hand beats an equal
/// wild one and one joker beats two. The comparison is total and infallible.
#[must_use]
pub fn compare(a: &EvaluatedHand, b: &EvaluatedHand) -> Ordering {
    (a.cls(), a.tiebreak(), Reverse(a.jokers_played_as().len())).cmp(&(
        b.cls(),
        b.tiebreak(),
        Reverse(b.jokers_played_as().len()),
    ))
}

#[cfg(test)]
mod tests;
