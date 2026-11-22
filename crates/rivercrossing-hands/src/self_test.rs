// SPDX-License-Identifier: GPL-3.0-only

//! A self-test that replays the golden vectors against the evaluator.
//!
//! The suite runs six checks in a fixed order: the 7,462 ranked naturals, the 28
//! authored joker vectors, five-of-a-kind ordering, the whole-field timing (the
//! lone advisory check), the total order of [`compare`], and the joker cap of
//! [`best_hand`]. A corrupted committed table turns its own check red rather than
//! aborting the run.

use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::time::Instant;

use rivercrossing_cards::{Card, Rank, Suit};

use crate::best::best_hand;
use crate::class::HandClass;
use crate::compare::compare;
use crate::eval::{EvaluatedHand, HAND_SIZE, eval5};
use crate::vectors::{JokerVectorRow, RankSweepRow, joker_vector_rows, rank_sweep_rows};

/// The frozen name of the rank-sweep check.
const RANK_SWEEP_CHECK_NAME: &str = "7,462 distinct ranks";
/// The frozen name of the joker-vector check.
const JOKER_VECTOR_CHECK_NAME: &str = "Joker vector table (28)";
/// The frozen name of the five-of-a-kind ordering check.
const FIVE_OF_A_KIND_CHECK_NAME: &str = "Five-of-a-kind ordering";
/// The frozen name of the whole-field timing check.
const FIELD_TIMING_CHECK_NAME: &str = "Whole-field 180×12 timing";
/// The frozen name of the comparison total-order check.
const COMPARE_TOTAL_ORDER_CHECK_NAME: &str = "compare() total order";
/// The frozen name of the joker-bound check.
const JOKER_BOUND_CHECK_NAME: &str = "best_hand() joker bound";

/// The number of distinct natural ranks that the sweep pins.
const NATURAL_RANK_COUNT: usize = 7462;
/// The number of authored joker vectors that the table must hold.
const JOKER_VECTOR_COUNT: usize = 28;
/// The wild five-of-a-kind that the ordering check compares.
const WILD_FIVE: [Card; 5] = [
    Card::Natural {
        rank: Rank::Ace,
        suit: Suit::Spades,
    },
    Card::Natural {
        rank: Rank::Ace,
        suit: Suit::Diamonds,
    },
    Card::Natural {
        rank: Rank::Ace,
        suit: Suit::Hearts,
    },
    Card::Natural {
        rank: Rank::Ace,
        suit: Suit::Clubs,
    },
    Card::Joker,
];
/// The royal flush that the ordering check compares.
const ROYAL_FLUSH: [Card; 5] = [
    Card::Natural {
        rank: Rank::Ace,
        suit: Suit::Spades,
    },
    Card::Natural {
        rank: Rank::King,
        suit: Suit::Spades,
    },
    Card::Natural {
        rank: Rank::Queen,
        suit: Suit::Spades,
    },
    Card::Natural {
        rank: Rank::Jack,
        suit: Suit::Spades,
    },
    Card::Natural {
        rank: Rank::Ten,
        suit: Suit::Spades,
    },
];

/// The number of entries in the whole-field timing check.
const FIELD_ENTRIES: usize = 180;
/// The number of cards that each field entry holds.
const FIELD_CARDS_PER_ENTRY: usize = 12;
/// The timing budget of the whole-field check, in seconds.
const FIELD_TIMING_BUDGET_SECONDS: f64 = 1.0;
/// The seed of the whole-field fixture.
const FIELD_SEED: u64 = 20_260_807;
/// The joker rolls among the default shoe's 424 cards.
const FIELD_JOKER_ROLLS: usize = 8;
/// The roll denominator of the default shoe's joker share.
const FIELD_ROLL_DENOMINATOR: usize = 424;

/// The seed of the comparison sample.
const COMPARE_SAMPLE_SEED: u64 = 20_260_808;
/// The number of hands in the comparison sample.
const COMPARE_SAMPLE_SIZE: usize = 48;
/// One hand in this many of the sample plays a joker.
const COMPARE_SAMPLE_JOKER_EVERY: usize = 3;

/// The all-joker pool sizes of the joker-bound check.
const JOKER_BOUND_POOL_SIZES: [usize; 3] = [5, 6, 7];
/// The joker count of the over-length mixed pool.
const JOKER_BOUND_MIXED_JOKERS: usize = 7;
/// The natural anchor cards of the mixed pool.
const JOKER_BOUND_ANCHOR_CODES: [&str; 3] = ["AS", "AD", "KH"];

/// The multiplier of the deterministic 64-bit generator.
const LCG_MULTIPLIER: u64 = 6_364_136_223_846_793_005;
/// The increment of the deterministic 64-bit generator.
const LCG_INCREMENT: u64 = 1_442_695_040_888_963_407;

/// The comparison seam of the suite.
type CompareFn = fn(&EvaluatedHand, &EvaluatedHand) -> Ordering;
/// The best-hand seam of the suite.
type BestHandFn = fn(&[Card]) -> EvaluatedHand;
/// The clock seam of the suite.
type ClockFn = fn() -> Instant;

/// One evaluator self-test line.
///
/// The derives stop at `PartialEq`, because a measured `duration_seconds` is a float.
#[derive(Debug, Clone, PartialEq)]
pub struct SelfTestCheck {
    /// The frozen check name.
    pub name: &'static str,
    /// Whether the check passed.
    pub passed: bool,
    /// The measured seconds; zero for every check except the timing one.
    pub duration_seconds: f64,
    /// The short human detail of the check.
    pub detail: String,
    /// Whether a failure refuses a finish.
    pub blocking: bool,
}

/// The evaluator self-test result, one line per check in its fixed order.
#[derive(Debug, Clone, PartialEq)]
pub struct SelfTestReport {
    /// The six checks in their frozen order.
    pub checks: Vec<SelfTestCheck>,
}

/// Run the evaluator self-test suite and return its report.
#[must_use]
pub fn self_test() -> SelfTestReport {
    let sweep_rows = rank_sweep_rows().unwrap_or_default();
    let joker_rows = joker_vector_rows().unwrap_or_default();
    build_report(&sweep_rows, &joker_rows, compare, best_hand, Instant::now)
}

/// Build the report from the checks' dependencies, so a unit test can drive each
/// failure branch.
#[must_use]
#[expect(
    clippy::too_many_arguments,
    reason = "the five arguments are the documented seams of the six checks"
)]
pub(crate) fn build_report(
    sweep_rows: &[RankSweepRow],
    joker_rows: &[JokerVectorRow],
    compare_fn: CompareFn,
    best_hand_fn: BestHandFn,
    now: ClockFn,
) -> SelfTestReport {
    let timed = check_field_timing(best_hand_fn, now);
    SelfTestReport {
        checks: vec![
            blocking_check(RANK_SWEEP_CHECK_NAME, check_rank_sweep(sweep_rows)),
            blocking_check(JOKER_VECTOR_CHECK_NAME, check_joker_vectors(joker_rows)),
            blocking_check(
                FIVE_OF_A_KIND_CHECK_NAME,
                check_five_of_a_kind_ordering(compare_fn),
            ),
            advisory_check(FIELD_TIMING_CHECK_NAME, timed),
            blocking_check(
                COMPARE_TOTAL_ORDER_CHECK_NAME,
                check_compare_total_order(compare_fn),
            ),
            blocking_check(JOKER_BOUND_CHECK_NAME, check_joker_bound(best_hand_fn)),
        ],
    }
}

/// The outcome of one check.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Verdict {
    /// Whether the check passed.
    passed: bool,
    /// The short human detail.
    detail: String,
}

impl Verdict {
    /// Return a passing verdict.
    fn pass() -> Self {
        Self {
            passed: true,
            detail: "ok".to_owned(),
        }
    }

    /// Return a failing verdict with its detail.
    fn fail(detail: impl Into<String>) -> Self {
        Self {
            passed: false,
            detail: detail.into(),
        }
    }
}

/// A verdict with the seconds that its check measured.
#[derive(Debug, Clone, PartialEq)]
struct TimedVerdict {
    /// The check outcome.
    verdict: Verdict,
    /// The measured seconds.
    duration_seconds: f64,
}

/// Wrap a blocking check's verdict in its report line.
fn blocking_check(name: &'static str, verdict: Verdict) -> SelfTestCheck {
    SelfTestCheck {
        name,
        passed: verdict.passed,
        duration_seconds: 0.0,
        detail: verdict.detail,
        blocking: true,
    }
}

/// Wrap the timing check's verdict in its advisory report line.
fn advisory_check(name: &'static str, timed: TimedVerdict) -> SelfTestCheck {
    SelfTestCheck {
        name,
        passed: timed.verdict.passed,
        duration_seconds: timed.duration_seconds,
        detail: timed.verdict.detail,
        blocking: false,
    }
}

/// Check that every sweep row has a distinct key and comes back in rank order.
fn check_rank_sweep(rows: &[RankSweepRow]) -> Verdict {
    if rows.len() != NATURAL_RANK_COUNT {
        return Verdict::fail(format!("got {} rows", rows.len()));
    }
    let mut entries: Vec<(HandClass, Vec<u8>, usize)> = Vec::with_capacity(rows.len());
    for row in rows {
        let Ok(hand) = eval5(row.cards()) else {
            return Verdict::fail("a sweep row does not hold 5 cards".to_owned());
        };
        entries.push((hand.cls(), hand.tiebreak().to_vec(), row.rank()));
    }
    let distinct = entries
        .iter()
        .map(|(class, tiebreak, _)| (*class, tiebreak.clone()))
        .collect::<BTreeSet<_>>()
        .len();
    if distinct != NATURAL_RANK_COUNT {
        return Verdict::fail(format!("got {distinct} distinct ranks"));
    }
    entries.sort_by(|left, right| (right.0, &right.1).cmp(&(left.0, &left.1)));
    let ordered = entries
        .iter()
        .enumerate()
        .all(|(position, (_, _, rank))| *rank == position + 1);
    if ordered {
        Verdict::pass()
    } else {
        Verdict::fail("ranks are not 1..=7462".to_owned())
    }
}

/// Check that every joker vector matches its recorded class and tiebreak.
fn check_joker_vectors(rows: &[JokerVectorRow]) -> Verdict {
    if rows.len() != JOKER_VECTOR_COUNT {
        return Verdict::fail(format!("got {} rows", rows.len()));
    }
    for row in rows {
        let Ok(hand) = eval5(row.cards()) else {
            return Verdict::fail("a joker row does not hold 5 cards".to_owned());
        };
        if hand.cls() != row.expected_class() {
            return Verdict::fail(format!(
                "{}: class {} is not {}",
                row.description(),
                hand.cls().name(),
                row.expected_class().name()
            ));
        }
        if hand.tiebreak() != row.expected_tiebreak() {
            return Verdict::fail(format!("{}: tiebreak does not match", row.description()));
        }
    }
    Verdict::pass()
}

/// Check that a wild five-of-a-kind compares above a royal flush.
fn check_five_of_a_kind_ordering(compare_fn: CompareFn) -> Verdict {
    let passed = eval5(&WILD_FIVE)
        .ok()
        .zip(eval5(&ROYAL_FLUSH).ok())
        .is_some_and(|(five, royal)| compare_fn(&five, &royal) == Ordering::Greater);
    if passed {
        Verdict::pass()
    } else {
        Verdict::fail("five of a kind must beat a royal flush".to_owned())
    }
}

/// Time the best-hand search over the seeded 180x12 field.
fn check_field_timing(best_hand_fn: BestHandFn, now: ClockFn) -> TimedVerdict {
    let field = seeded_field();
    let start = now();
    for entry in &field {
        best_hand_fn(entry);
    }
    let elapsed = now().duration_since(start).as_secs_f64();
    TimedVerdict {
        verdict: Verdict {
            passed: elapsed < FIELD_TIMING_BUDGET_SECONDS,
            detail: format!("{elapsed:.2} s"),
        },
        duration_seconds: elapsed,
    }
}

/// Check that the comparison is a total order over the seeded sample.
fn check_compare_total_order(compare_fn: CompareFn) -> Verdict {
    compare_total_order(&seeded_compare_sample(), compare_fn)
}

/// Check antisymmetry, reflexivity and transitivity over one sample.
fn compare_total_order(sample: &[EvaluatedHand], compare_fn: CompareFn) -> Verdict {
    let legs = [
        (
            "antisymmetric",
            compare_is_antisymmetric(sample, compare_fn),
        ),
        ("reflexive", compare_is_reflexive(sample, compare_fn)),
        ("transitive", compare_is_transitive(sample, compare_fn)),
    ];
    let failed: Vec<&str> = legs
        .into_iter()
        .filter_map(|(name, passed)| (!passed).then_some(name))
        .collect();
    if failed.is_empty() {
        Verdict::pass()
    } else {
        Verdict::fail(format!("{} failed", failed.join(", ")))
    }
}

/// Return whether each pair of the sample compares against its reverse.
fn compare_is_antisymmetric(sample: &[EvaluatedHand], compare_fn: CompareFn) -> bool {
    sample.iter().all(|left| {
        sample
            .iter()
            .all(|right| compare_fn(left, right) == compare_fn(right, left).reverse())
    })
}

/// Return whether each hand of the sample ties with itself.
fn compare_is_reflexive(sample: &[EvaluatedHand], compare_fn: CompareFn) -> bool {
    sample
        .iter()
        .all(|hand| compare_fn(hand, hand) == Ordering::Equal)
}

/// Return whether the sample sorts without a contradiction.
fn compare_is_transitive(sample: &[EvaluatedHand], compare_fn: CompareFn) -> bool {
    let mut ordered: Vec<&EvaluatedHand> = sample.iter().collect();
    ordered.sort_by(|left, right| compare_fn(left, right));
    ordered.iter().enumerate().all(|(index, lower)| {
        ordered
            .iter()
            .skip(index + 1)
            .all(|higher| compare_fn(lower, higher) != Ordering::Greater)
    })
}

/// Check that no best-hand pool plays more jokers than a hand holds.
fn check_joker_bound(best_hand_fn: BestHandFn) -> Verdict {
    for pool in joker_bound_pools() {
        let played = best_hand_fn(&pool).jokers_played_as().len();
        if played > HAND_SIZE {
            return Verdict::fail(format!("a pool played {played} jokers"));
        }
    }
    Verdict::pass()
}

/// Return the four joker pools of the bound check.
fn joker_bound_pools() -> Vec<Vec<Card>> {
    let mut pools: Vec<Vec<Card>> = JOKER_BOUND_POOL_SIZES
        .iter()
        .map(|size| vec![Card::Joker; *size])
        .collect();
    let mut mixed = vec![Card::Joker; JOKER_BOUND_MIXED_JOKERS];
    mixed.extend(
        JOKER_BOUND_ANCHOR_CODES
            .iter()
            .filter_map(|code| Card::parse(code).ok()),
    );
    pools.push(mixed);
    pools
}

/// Return the deterministic 180x12 whole-field fixture.
#[must_use]
pub(crate) fn seeded_field() -> Vec<Vec<Card>> {
    let deck = natural_deck();
    let mut state = FIELD_SEED;
    let mut field = Vec::with_capacity(FIELD_ENTRIES);
    for _ in 0..FIELD_ENTRIES {
        let mut entry = Vec::with_capacity(FIELD_CARDS_PER_ENTRY);
        for _ in 0..FIELD_CARDS_PER_ENTRY {
            entry.push(draw_card(&mut state, &deck));
        }
        field.push(entry);
    }
    field
}

/// Return the deterministic 48-hand comparison sample.
#[must_use]
pub(crate) fn seeded_compare_sample() -> Vec<EvaluatedHand> {
    let deck = natural_deck();
    let mut state = COMPARE_SAMPLE_SEED;
    let mut sample = Vec::with_capacity(COMPARE_SAMPLE_SIZE);
    for index in 0..COMPARE_SAMPLE_SIZE {
        let joker_count = usize::from(index % COMPARE_SAMPLE_JOKER_EVERY == 0);
        let mut cards: Vec<Card> = (0..HAND_SIZE.saturating_sub(joker_count))
            .filter_map(|_| deck.get(seeded_below(&mut state, deck.len())).cloned())
            .collect();
        cards.extend(std::iter::repeat_n(Card::Joker, joker_count));
        if let Ok(hand) = eval5(&cards) {
            sample.push(hand);
        }
    }
    sample
}

/// Return the 52 natural cards, rank-major then suit order.
fn natural_deck() -> Vec<Card> {
    (2..=14)
        .filter_map(|value| Rank::from_u8(value).ok())
        .flat_map(|rank| {
            [Suit::Clubs, Suit::Diamonds, Suit::Hearts, Suit::Spades]
                .into_iter()
                .map(move |suit| Card::Natural { rank, suit })
        })
        .collect()
}

/// Return one card of the seeded field: a joker at the shoe's own rate, else a natural.
fn draw_card(state: &mut u64, deck: &[Card]) -> Card {
    if seeded_below(state, FIELD_ROLL_DENOMINATOR) < FIELD_JOKER_ROLLS {
        return Card::Joker;
    }
    deck.get(seeded_below(state, deck.len()))
        .cloned()
        .unwrap_or(Card::Joker)
}

/// Return the next value of the deterministic 64-bit generator.
fn lcg_next(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(LCG_MULTIPLIER)
        .wrapping_add(LCG_INCREMENT);
    *state
}

/// Return the next value of the generator below `bound`.
fn seeded_below(state: &mut u64, bound: usize) -> usize {
    let bound = u64::try_from(bound).unwrap_or(u64::MAX).max(1);
    let value = lcg_next(state) >> 33;
    usize::try_from(value % bound).unwrap_or(0)
}

#[cfg(test)]
mod tests;
