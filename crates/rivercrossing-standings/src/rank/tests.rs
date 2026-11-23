// SPDX-License-Identifier: GPL-3.0-only

//! Tests for ranking.

use pretty_assertions::assert_eq;
use rivercrossing_cards::{Card, Rank, Suit};
use rivercrossing_hands::best_hand;
use rivercrossing_ride::{EntryId, SnapshotKind};

use super::{rank, rank_by_kind};
use crate::error::StandingsError;
use crate::types::{EntryResult, Placed};
use crate::{DEFAULT_TIEBREAK_ORDER, DRAW_TIE_NOTE, LIVE_TIEBREAK_ORDER, TieBreak, ZeroLapMode};

/// Build one natural card.
fn card(rank: Rank, suit: Suit) -> Card {
    Card::Natural { rank, suit }
}

/// The royal flush of spades.
fn royal_spades() -> Vec<Card> {
    [Rank::Ace, Rank::King, Rank::Queen, Rank::Jack, Rank::Ten]
        .map(|rank| card(rank, Suit::Spades))
        .to_vec()
}

/// The royal flush of hearts: equal to [`royal_spades`] in hand strength.
fn royal_hearts() -> Vec<Card> {
    [Rank::Ace, Rank::King, Rank::Queen, Rank::Jack, Rank::Ten]
        .map(|rank| card(rank, Suit::Hearts))
        .to_vec()
}

/// The 9-high straight `9H 8C 7D 6S 5H`.
fn straight_nines() -> Vec<Card> {
    vec![
        card(Rank::Nine, Suit::Hearts),
        card(Rank::Eight, Suit::Clubs),
        card(Rank::Seven, Suit::Diamonds),
        card(Rank::Six, Suit::Spades),
        card(Rank::Five, Suit::Hearts),
    ]
}

/// The 9-high straight in other suits: equal in hand strength to
/// [`straight_nines`].
fn straight_nines_other_suits() -> Vec<Card> {
    vec![
        card(Rank::Nine, Suit::Clubs),
        card(Rank::Eight, Suit::Diamonds),
        card(Rank::Seven, Suit::Hearts),
        card(Rank::Six, Suit::Clubs),
        card(Rank::Five, Suit::Diamonds),
    ]
}

/// Jacks full of fours.
fn full_house_jacks() -> Vec<Card> {
    vec![
        card(Rank::Jack, Suit::Hearts),
        card(Rank::Jack, Suit::Clubs),
        card(Rank::Jack, Suit::Diamonds),
        card(Rank::Four, Suit::Hearts),
        card(Rank::Four, Suit::Clubs),
    ]
}

/// Kings and fives, with an ace kicker.
fn two_pair_kings() -> Vec<Card> {
    vec![
        card(Rank::King, Suit::Hearts),
        card(Rank::King, Suit::Clubs),
        card(Rank::Five, Suit::Hearts),
        card(Rank::Five, Suit::Diamonds),
        card(Rank::Ace, Suit::Spades),
    ]
}

/// Ace-high, no combination.
fn ace_high_nothing() -> Vec<Card> {
    vec![
        card(Rank::Ace, Suit::Spades),
        card(Rank::Queen, Suit::Diamonds),
        card(Rank::Nine, Suit::Hearts),
        card(Rank::Five, Suit::Clubs),
        card(Rank::Three, Suit::Spades),
    ]
}

/// A four-card ace-high hand, below every full hand of its class.
fn partial_ace_high() -> Vec<Card> {
    vec![
        card(Rank::Ace, Suit::Spades),
        card(Rank::King, Suit::Diamonds),
        card(Rank::Queen, Suit::Hearts),
        card(Rank::Jack, Suit::Clubs),
    ]
}

/// Build one ACTIVE solo entry whose hand is the best hand of *cards*.
fn result(id: &str, cards: &[Card]) -> EntryResult {
    EntryResult {
        entry_id: EntryId(id.to_owned()),
        plate: id.to_owned(),
        name: "Rider".to_owned(),
        kind: SnapshotKind::Solo,
        laps: 0,
        total_time: 0.0,
        best_lap: 0.0,
        cards: cards.to_vec(),
        hand: best_hand(cards),
        dnf: false,
        sex: None,
        tiebreak_card: None,
    }
}

/// Return *result* with *laps* recorded laps.
fn with_laps(result: EntryResult, laps: u32) -> EntryResult {
    EntryResult { laps, ..result }
}

/// Return *result* with `total_time` seconds.
fn with_time(result: EntryResult, total_time: f64) -> EntryResult {
    EntryResult {
        total_time,
        ..result
    }
}

/// Return *result* holding *card* from the venue's draw.
fn with_draw(result: EntryResult, card: Card) -> EntryResult {
    EntryResult {
        tiebreak_card: Some(card),
        ..result
    }
}

/// Return *result* marked DNF.
fn as_dnf(result: EntryResult) -> EntryResult {
    EntryResult {
        dnf: true,
        ..result
    }
}

/// Return *result* as a team entry.
fn as_team(result: EntryResult) -> EntryResult {
    EntryResult {
        kind: SnapshotKind::Team,
        ..result
    }
}

/// Return the entry ids of *placed*, in order.
fn ids(placed: &[Placed]) -> Vec<String> {
    placed
        .iter()
        .map(|row| row.result.entry_id.0.clone())
        .collect()
}

/// Return the places of *placed*, in order.
fn places(placed: &[Placed]) -> Vec<Option<u32>> {
    placed.iter().map(|row| row.place).collect()
}

/// Return the draw flags of *placed*, in order.
fn flags(placed: &[Placed]) -> Vec<bool> {
    placed.iter().map(|row| row.draw_required).collect()
}

/// Return the tie notes of *placed*, in order.
fn notes(placed: &[Placed]) -> Vec<Option<String>> {
    placed.iter().map(|row| row.tie_note.clone()).collect()
}

// ------------------------------------------------------ basic ordering

#[test]
fn test_rank_distinct_hands_place_best_first() -> Result<(), StandingsError> {
    let field = vec![
        result("1", &ace_high_nothing()),
        result("2", &royal_spades()),
        result("3", &straight_nines()),
        result("4", &full_house_jacks()),
    ];

    let placed = rank(&field, &DEFAULT_TIEBREAK_ORDER)?;

    assert_eq!(ids(&placed), vec!["2", "4", "3", "1"]);
    assert_eq!(places(&placed), vec![Some(1), Some(2), Some(3), Some(4)]);
    assert_eq!(flags(&placed), vec![false, false, false, false]);
    assert_eq!(notes(&placed), vec![None, None, None, None]);
    Ok(())
}

#[test]
fn test_rank_partial_four_card_hand_ranks_below_full_hand() -> Result<(), StandingsError> {
    let field = vec![
        result("2", &partial_ace_high()),
        result("1", &ace_high_nothing()),
    ];

    let placed = rank(&field, &DEFAULT_TIEBREAK_ORDER)?;

    assert_eq!(ids(&placed), vec!["1", "2"]);
    Ok(())
}

#[test]
fn test_rank_empty_field_returns_empty() -> Result<(), StandingsError> {
    let placed = rank(&[], &DEFAULT_TIEBREAK_ORDER)?;

    assert_eq!(placed.len(), 0);
    Ok(())
}

// -------------------------------------------------------- tie resolution

#[test]
fn test_rank_hand_tie_orders_more_laps_first() -> Result<(), StandingsError> {
    let fewer_laps = with_laps(result("1", &royal_hearts()), 4);
    let more_laps = with_laps(result("2", &royal_spades()), 5);
    let order = [TieBreak::Laps, TieBreak::TotalTime, TieBreak::HighCard];

    let placed = rank(&[fewer_laps, more_laps], &order)?;

    assert_eq!(ids(&placed), vec!["2", "1"]);
    assert_eq!(places(&placed), vec![Some(1), Some(2)]);
    assert_eq!(flags(&placed), vec![false, false]);
    assert_eq!(notes(&placed), vec![None, None]);
    Ok(())
}

#[test]
fn test_rank_hand_tie_orders_shorter_total_time_first() -> Result<(), StandingsError> {
    let slow = with_time(with_laps(result("2", &royal_hearts()), 5), 100.0);
    let fast = with_time(with_laps(result("1", &royal_spades()), 5), 90.0);
    let order = [TieBreak::TotalTime, TieBreak::Laps, TieBreak::HighCard];

    let placed = rank(&[slow, fast], &order)?;

    assert_eq!(ids(&placed), vec!["1", "2"]);
    assert_eq!(flags(&placed), vec![false, false]);
    Ok(())
}

#[test]
fn test_rank_empty_order_flags_every_hand_tie() -> Result<(), StandingsError> {
    let first = with_laps(result("1", &royal_spades()), 5);
    let second = with_laps(result("2", &royal_hearts()), 4);

    let placed = rank(&[first, second], &[])?;

    assert_eq!(places(&placed), vec![Some(1), Some(1)]);
    assert_eq!(flags(&placed), vec![true, true]);
    assert_eq!(notes(&placed), vec![Some(DRAW_TIE_NOTE.to_owned()); 2]);
    Ok(())
}

#[test]
fn test_rank_unresolved_tie_flags_draw_and_next_place_is_third() -> Result<(), StandingsError> {
    let first = with_time(with_laps(result("1", &royal_spades()), 5), 100.0);
    let second = with_time(with_laps(result("2", &royal_hearts()), 5), 100.0);
    let next_entry = result("3", &straight_nines());

    let placed = rank(&[first, second, next_entry], &DEFAULT_TIEBREAK_ORDER)?;

    assert_eq!(places(&placed), vec![Some(1), Some(1), Some(3)]);
    assert_eq!(flags(&placed), vec![true, true, false]);
    assert_eq!(
        notes(&placed),
        vec![
            Some(DRAW_TIE_NOTE.to_owned()),
            Some(DRAW_TIE_NOTE.to_owned()),
            None
        ]
    );
    Ok(())
}

#[test]
fn test_rank_default_order_flags_an_undrawn_laps_tie() -> Result<(), StandingsError> {
    let more_laps = with_time(with_laps(result("1", &royal_spades()), 5), 100.0);
    let fewer_laps = with_time(with_laps(result("2", &royal_hearts()), 4), 50.0);

    let placed = rank(&[fewer_laps, more_laps], &DEFAULT_TIEBREAK_ORDER)?;

    assert_eq!(places(&placed), vec![Some(1), Some(1)]);
    assert_eq!(flags(&placed), vec![true, true]);
    Ok(())
}

#[test]
fn test_rank_live_order_resolves_a_hand_tie_by_laps() -> Result<(), StandingsError> {
    let fewer_laps = with_laps(result("2", &royal_hearts()), 4);
    let more_laps = with_laps(result("1", &royal_spades()), 5);

    let placed = rank(&[fewer_laps, more_laps], &LIVE_TIEBREAK_ORDER)?;

    assert_eq!(ids(&placed), vec!["1", "2"]);
    assert_eq!(flags(&placed), vec![false, false]);
    assert_eq!(notes(&placed), vec![None, None]);
    Ok(())
}

// ------------------------------------------- the venue draw as a resolver

#[test]
fn test_rank_drawn_tie_orders_by_higher_draw_key() -> Result<(), StandingsError> {
    let lower = with_draw(result("1", &royal_spades()), card(Rank::Two, Suit::Clubs));
    let higher = with_draw(result("2", &royal_hearts()), card(Rank::Ace, Suit::Spades));

    let placed = rank(&[lower, higher], &DEFAULT_TIEBREAK_ORDER)?;

    assert_eq!(ids(&placed), vec!["2", "1"]);
    assert_eq!(places(&placed), vec![Some(1), Some(2)]);
    assert_eq!(flags(&placed), vec![false, false]);
    assert_eq!(notes(&placed), vec![None, None]);
    Ok(())
}

#[test]
fn test_rank_drawn_tie_orders_equal_ranks_by_suit() -> Result<(), StandingsError> {
    let clubs = with_draw(result("1", &royal_spades()), card(Rank::Two, Suit::Clubs));
    let diamonds = with_draw(
        result("2", &royal_hearts()),
        card(Rank::Two, Suit::Diamonds),
    );

    let placed = rank(&[clubs, diamonds], &DEFAULT_TIEBREAK_ORDER)?;

    assert_eq!(ids(&placed), vec!["2", "1"]);
    Ok(())
}

#[test]
fn test_rank_drawn_tie_never_consults_a_later_criterion() -> Result<(), StandingsError> {
    let card_winner = with_time(
        with_draw(
            with_laps(result("1", &royal_spades()), 1),
            card(Rank::Ace, Suit::Spades),
        ),
        200.0,
    );
    let laps_leader = with_time(
        with_draw(
            with_laps(result("2", &royal_hearts()), 9),
            card(Rank::Two, Suit::Clubs),
        ),
        100.0,
    );

    let placed = rank(&[laps_leader, card_winner], &DEFAULT_TIEBREAK_ORDER)?;

    assert_eq!(ids(&placed), vec!["1", "2"]);
    Ok(())
}

#[test]
fn test_rank_drawn_tie_orders_laps_before_the_draw() -> Result<(), StandingsError> {
    let lower_card_more_laps = with_draw(
        with_laps(result("1", &royal_spades()), 5),
        card(Rank::Two, Suit::Clubs),
    );
    let higher_card_fewer_laps = with_draw(
        with_laps(result("2", &royal_hearts()), 1),
        card(Rank::Ace, Suit::Spades),
    );
    let order = [TieBreak::Laps, TieBreak::HighCard];

    let placed = rank(&[higher_card_fewer_laps, lower_card_more_laps], &order)?;

    assert_eq!(ids(&placed), vec!["1", "2"]);
    assert_eq!(flags(&placed), vec![false, false]);
    Ok(())
}

#[test]
fn test_rank_drawn_three_way_tie_numbers_them_one_to_three() -> Result<(), StandingsError> {
    let lowest = with_draw(result("1", &royal_spades()), card(Rank::Two, Suit::Clubs));
    let highest = with_draw(result("2", &royal_hearts()), card(Rank::Ace, Suit::Spades));
    let middle = with_draw(
        result("3", &royal_spades()),
        card(Rank::Ten, Suit::Diamonds),
    );

    let placed = rank(&[lowest, highest, middle], &DEFAULT_TIEBREAK_ORDER)?;

    assert_eq!(ids(&placed), vec!["2", "3", "1"]);
    assert_eq!(places(&placed), vec![Some(1), Some(2), Some(3)]);
    assert_eq!(flags(&placed), vec![false, false, false]);
    Ok(())
}

#[test]
fn test_rank_same_drawn_card_stays_a_draw() -> Result<(), StandingsError> {
    let first = with_draw(
        with_laps(result("1", &royal_spades()), 5),
        card(Rank::Ace, Suit::Spades),
    );
    let second = with_draw(
        with_laps(result("2", &royal_hearts()), 4),
        card(Rank::Ace, Suit::Spades),
    );

    let placed = rank(&[first, second], &DEFAULT_TIEBREAK_ORDER)?;

    assert_eq!(places(&placed), vec![Some(1), Some(1)]);
    assert_eq!(flags(&placed), vec![true, true]);
    assert_eq!(notes(&placed), vec![Some(DRAW_TIE_NOTE.to_owned()); 2]);
    Ok(())
}

#[test]
fn test_rank_partially_drawn_group_flags_a_draw() -> Result<(), StandingsError> {
    let drawn = with_draw(
        with_laps(result("1", &royal_spades()), 1),
        card(Rank::Ace, Suit::Spades),
    );
    let undrawn = with_laps(result("2", &royal_hearts()), 9);

    let placed = rank(&[drawn, undrawn], &DEFAULT_TIEBREAK_ORDER)?;

    assert_eq!(places(&placed), vec![Some(1), Some(1)]);
    assert_eq!(flags(&placed), vec![true, true]);
    Ok(())
}

#[test]
fn test_rank_joker_drawn_card_returns_joker_error() {
    let joker_drawn = with_draw(result("1", &royal_spades()), Card::Joker);
    let natural_drawn = with_draw(result("2", &royal_hearts()), card(Rank::Ace, Suit::Spades));

    let outcome = rank(&[joker_drawn, natural_drawn], &DEFAULT_TIEBREAK_ORDER);

    assert_eq!(outcome.err(), Some(StandingsError::JokerDrawKey));
}

#[test]
fn test_rank_live_order_ignores_a_joker_drawn_card() -> Result<(), StandingsError> {
    let fewer_laps = with_draw(with_laps(result("1", &royal_spades()), 4), Card::Joker);
    let more_laps = with_draw(
        with_laps(result("2", &royal_hearts()), 5),
        card(Rank::Ace, Suit::Spades),
    );

    let placed = rank(&[fewer_laps, more_laps], &LIVE_TIEBREAK_ORDER)?;

    assert_eq!(ids(&placed), vec!["2", "1"]);
    assert_eq!(flags(&placed), vec![false, false]);
    Ok(())
}

// -------------------------------------------------------------------- DNF

#[test]
fn test_rank_excludes_a_dnf_amid_distinct_hands() -> Result<(), StandingsError> {
    let active_a = result("1", &royal_spades());
    let dnf = as_dnf(result("2", &full_house_jacks()));
    let active_b = result("3", &straight_nines());

    let placed = rank(&[active_a, dnf, active_b], &DEFAULT_TIEBREAK_ORDER)?;

    assert_eq!(ids(&placed), vec!["1", "3"]);
    assert_eq!(places(&placed), vec![Some(1), Some(2)]);
    Ok(())
}

#[test]
fn test_rank_all_dnf_returns_empty() -> Result<(), StandingsError> {
    let first = as_dnf(result("1", &royal_spades()));
    let second = as_dnf(result("2", &straight_nines()));

    let placed = rank(&[second, first], &DEFAULT_TIEBREAK_ORDER)?;

    assert_eq!(placed.len(), 0);
    Ok(())
}

#[test]
fn test_rank_never_emits_a_dns_place() -> Result<(), StandingsError> {
    let royal = result("1", &royal_spades());
    let zero_lap = result("2", &full_house_jacks());
    let straight = result("3", &straight_nines());

    let placed = rank(&[royal, zero_lap, straight], &DEFAULT_TIEBREAK_ORDER)?;

    assert_eq!(places(&placed), vec![Some(1), Some(2), Some(3)]);
    Ok(())
}

// -------------------------------------------------- team and solo split

#[test]
fn test_rank_by_kind_sections_flag_their_draws_independently() -> Result<(), StandingsError> {
    let field = vec![
        as_team(with_time(with_laps(result("1", &royal_spades()), 5), 100.0)),
        as_team(with_time(with_laps(result("2", &royal_hearts()), 5), 100.0)),
        with_time(with_laps(result("3", &straight_nines()), 4), 50.0),
        with_time(
            with_laps(result("4", &straight_nines_other_suits()), 4),
            50.0,
        ),
    ];

    let (teams, solo) = rank_by_kind(&field, &DEFAULT_TIEBREAK_ORDER, ZeroLapMode::Rank)?;

    assert_eq!(places(&teams), vec![Some(1), Some(1)]);
    assert_eq!(flags(&teams), vec![true, true]);
    assert_eq!(places(&solo), vec![Some(1), Some(1)]);
    assert_eq!(flags(&solo), vec![true, true]);
    Ok(())
}

#[test]
fn test_rank_by_kind_splits_teams_then_solo() -> Result<(), StandingsError> {
    let field = vec![
        as_team(result("1", &ace_high_nothing())),
        result("3", &straight_nines()),
        as_team(result("2", &royal_spades())),
        result("4", &full_house_jacks()),
    ];

    let (teams, solo) = rank_by_kind(&field, &DEFAULT_TIEBREAK_ORDER, ZeroLapMode::Rank)?;

    assert_eq!(ids(&teams), vec!["2", "1"]);
    assert_eq!(ids(&solo), vec!["4", "3"]);
    assert!(
        teams
            .iter()
            .all(|row| row.result.kind == SnapshotKind::Team)
    );
    assert!(solo.iter().all(|row| row.result.kind == SnapshotKind::Solo));
    Ok(())
}

#[test]
fn test_rank_by_kind_renumbers_each_section_from_one() -> Result<(), StandingsError> {
    let field = vec![
        as_team(result("1", &royal_spades())),
        as_team(result("2", &straight_nines())),
        result("3", &ace_high_nothing()),
        result("4", &full_house_jacks()),
        result("5", &two_pair_kings()),
    ];

    let (teams, solo) = rank_by_kind(&field, &DEFAULT_TIEBREAK_ORDER, ZeroLapMode::Rank)?;

    assert_eq!(places(&teams), vec![Some(1), Some(2)]);
    assert_eq!(places(&solo), vec![Some(1), Some(2), Some(3)]);
    assert_eq!(ids(&solo), vec!["4", "5", "3"]);
    Ok(())
}

#[test]
fn test_rank_by_kind_excludes_dnf_entries_from_both_sections() -> Result<(), StandingsError> {
    let field = vec![
        with_laps(as_team(result("1", &royal_spades())), 5),
        as_dnf(as_team(result("2", &straight_nines()))),
        with_laps(result("3", &full_house_jacks()), 4),
        as_dnf(result("4", &two_pair_kings())),
    ];

    let (teams, solo) = rank_by_kind(&field, &DEFAULT_TIEBREAK_ORDER, ZeroLapMode::Rank)?;

    assert_eq!(ids(&teams), vec!["1"]);
    assert_eq!(places(&teams), vec![Some(1)]);
    assert_eq!(ids(&solo), vec!["3"]);
    assert_eq!(places(&solo), vec![Some(1)]);
    Ok(())
}

#[test]
fn test_rank_by_kind_solo_only_returns_empty_teams() -> Result<(), StandingsError> {
    let solo_field = vec![result("1", &royal_spades())];

    let (teams, solo) = rank_by_kind(&solo_field, &DEFAULT_TIEBREAK_ORDER, ZeroLapMode::Rank)?;

    assert_eq!(teams.len(), 0);
    assert_eq!(ids(&solo), vec!["1"]);
    Ok(())
}

#[test]
fn test_rank_by_kind_team_only_returns_empty_solo() -> Result<(), StandingsError> {
    let team_field = vec![as_team(result("1", &royal_spades()))];

    let (teams, solo) = rank_by_kind(&team_field, &DEFAULT_TIEBREAK_ORDER, ZeroLapMode::Rank)?;

    assert_eq!(ids(&teams), vec!["1"]);
    assert_eq!(solo.len(), 0);
    Ok(())
}

#[test]
fn test_rank_by_kind_empty_field_returns_two_empty_sections() -> Result<(), StandingsError> {
    let (teams, solo) = rank_by_kind(&[], &DEFAULT_TIEBREAK_ORDER, ZeroLapMode::Rank)?;

    assert_eq!(teams.len(), 0);
    assert_eq!(solo.len(), 0);
    Ok(())
}

#[test]
fn test_rank_by_kind_joker_drawn_card_returns_joker_error() {
    let field = vec![
        as_team(with_draw(result("1", &royal_spades()), Card::Joker)),
        as_team(with_draw(
            result("2", &royal_hearts()),
            card(Rank::Ace, Suit::Spades),
        )),
    ];

    let outcome = rank_by_kind(&field, &DEFAULT_TIEBREAK_ORDER, ZeroLapMode::Rank);

    assert_eq!(outcome.err(), Some(StandingsError::JokerDrawKey));
}

// -------------------------------------------------------- zero-lap modes

#[test]
fn test_rank_by_kind_rank_mode_places_a_zero_lap_entry() -> Result<(), StandingsError> {
    let zero_lap = result("1", &straight_nines());
    let one_lap = with_laps(result("2", &royal_spades()), 1);

    let (_teams, solo) = rank_by_kind(
        &[zero_lap, one_lap],
        &DEFAULT_TIEBREAK_ORDER,
        ZeroLapMode::Rank,
    )?;

    assert_eq!(ids(&solo), vec!["2", "1"]);
    assert_eq!(places(&solo), vec![Some(1), Some(2)]);
    assert_eq!(flags(&solo), vec![false, false]);
    Ok(())
}

#[test]
fn test_rank_by_kind_dns_appends_the_zero_lap_entry_unplaced() -> Result<(), StandingsError> {
    let zero_lap = result("1", &royal_spades());
    let one_lap = with_laps(result("2", &straight_nines()), 1);

    let (_teams, solo) = rank_by_kind(
        &[zero_lap, one_lap],
        &DEFAULT_TIEBREAK_ORDER,
        ZeroLapMode::Dns,
    )?;

    assert_eq!(ids(&solo), vec!["2", "1"]);
    assert_eq!(places(&solo), vec![Some(1), None]);
    assert_eq!(flags(&solo), vec![false, false]);
    assert_eq!(notes(&solo), vec![None, None]);
    Ok(())
}

#[test]
fn test_rank_by_kind_dns_row_carries_no_note_or_draw_flag() -> Result<(), StandingsError> {
    let zero_lap = result("1", &royal_spades());

    let (_teams, solo) = rank_by_kind(&[zero_lap], &DEFAULT_TIEBREAK_ORDER, ZeroLapMode::Dns)?;

    assert_eq!(places(&solo), vec![None]);
    assert_eq!(flags(&solo), vec![false]);
    assert_eq!(notes(&solo), vec![None]);
    Ok(())
}

#[test]
fn test_rank_by_kind_dns_appends_zero_lap_rows_in_input_order() -> Result<(), StandingsError> {
    let later_input = result("9", &straight_nines());
    let earlier_input = result("1", &royal_spades());

    let (_teams, solo) = rank_by_kind(
        &[later_input, earlier_input],
        &DEFAULT_TIEBREAK_ORDER,
        ZeroLapMode::Dns,
    )?;

    assert_eq!(ids(&solo), vec!["9", "1"]);
    assert_eq!(places(&solo), vec![None, None]);
    Ok(())
}

#[test]
fn test_rank_by_kind_dns_keeps_the_placed_places_contiguous() -> Result<(), StandingsError> {
    let field = vec![
        result("1", &royal_spades()),
        with_laps(result("2", &royal_hearts()), 4),
        result("3", &straight_nines()),
        with_laps(result("4", &full_house_jacks()), 3),
        with_laps(result("5", &two_pair_kings()), 3),
    ];

    let (_teams, solo) = rank_by_kind(&field, &DEFAULT_TIEBREAK_ORDER, ZeroLapMode::Dns)?;

    assert_eq!(ids(&solo), vec!["2", "4", "5", "1", "3"]);
    assert_eq!(places(&solo), vec![Some(1), Some(2), Some(3), None, None]);
    Ok(())
}

#[test]
fn test_rank_by_kind_dns_tied_zero_lap_rows_are_never_flagged() -> Result<(), StandingsError> {
    let first = result("1", &straight_nines());
    let second = result("2", &straight_nines());

    let (_teams, solo) = rank_by_kind(&[first, second], &DEFAULT_TIEBREAK_ORDER, ZeroLapMode::Dns)?;

    assert_eq!(places(&solo), vec![None, None]);
    assert_eq!(flags(&solo), vec![false, false]);
    assert_eq!(notes(&solo), vec![None, None]);
    Ok(())
}

#[test]
fn test_rank_by_kind_dns_drops_a_zero_lap_dnf_entry() -> Result<(), StandingsError> {
    let zero_lap_dnf = as_dnf(result("1", &royal_spades()));
    let one_lap = with_laps(result("2", &straight_nines()), 1);

    let (_teams, solo) = rank_by_kind(
        &[zero_lap_dnf, one_lap],
        &DEFAULT_TIEBREAK_ORDER,
        ZeroLapMode::Dns,
    )?;

    assert_eq!(ids(&solo), vec!["2"]);
    assert_eq!(places(&solo), vec![Some(1)]);
    Ok(())
}

#[test]
fn test_rank_by_kind_dns_drops_a_started_dnf_entry() -> Result<(), StandingsError> {
    let zero_lap = result("1", &royal_spades());
    let started_dnf = as_dnf(with_laps(result("2", &straight_nines()), 3));

    let (_teams, solo) = rank_by_kind(
        &[zero_lap, started_dnf],
        &DEFAULT_TIEBREAK_ORDER,
        ZeroLapMode::Dns,
    )?;

    assert_eq!(ids(&solo), vec!["1"]);
    assert_eq!(places(&solo), vec![None]);
    Ok(())
}

#[test]
fn test_rank_by_kind_dns_keeps_a_started_entry_placed() -> Result<(), StandingsError> {
    let started = with_laps(result("1", &royal_spades()), 1);

    let (_teams, solo) = rank_by_kind(&[started], &DEFAULT_TIEBREAK_ORDER, ZeroLapMode::Dns)?;

    assert_eq!(places(&solo), vec![Some(1)]);
    Ok(())
}

#[test]
fn test_rank_by_kind_dns_partitions_teams_and_solos_independently() -> Result<(), StandingsError> {
    let field = vec![
        as_team(result("1", &royal_spades())),
        as_team(with_laps(result("2", &royal_hearts()), 5)),
        result("3", &straight_nines()),
        with_laps(result("4", &full_house_jacks()), 3),
    ];

    let (teams, solo) = rank_by_kind(&field, &DEFAULT_TIEBREAK_ORDER, ZeroLapMode::Dns)?;

    assert_eq!(ids(&teams), vec!["2", "1"]);
    assert_eq!(places(&teams), vec![Some(1), None]);
    assert_eq!(ids(&solo), vec!["4", "3"]);
    assert_eq!(places(&solo), vec![Some(1), None]);
    Ok(())
}

#[test]
fn test_rank_by_kind_dns_joker_drawn_card_returns_joker_error() {
    let field = vec![
        with_laps(with_draw(result("1", &royal_spades()), Card::Joker), 1),
        with_laps(
            with_draw(result("2", &royal_hearts()), card(Rank::Ace, Suit::Spades)),
            1,
        ),
    ];

    let outcome = rank_by_kind(&field, &DEFAULT_TIEBREAK_ORDER, ZeroLapMode::Dns);

    assert_eq!(outcome.err(), Some(StandingsError::JokerDrawKey));
}

#[test]
fn test_rank_by_kind_hide_drops_the_zero_lap_entry() -> Result<(), StandingsError> {
    let zero_lap = result("1", &royal_spades());
    let one_lap = with_laps(result("2", &straight_nines()), 1);

    let (_teams, solo) = rank_by_kind(
        &[zero_lap, one_lap],
        &DEFAULT_TIEBREAK_ORDER,
        ZeroLapMode::Hide,
    )?;

    assert_eq!(ids(&solo), vec!["2"]);
    assert_eq!(places(&solo), vec![Some(1)]);
    Ok(())
}

#[test]
fn test_rank_by_kind_hide_joker_drawn_card_returns_joker_error() {
    let field = vec![
        with_laps(with_draw(result("1", &royal_spades()), Card::Joker), 1),
        with_laps(
            with_draw(result("2", &royal_hearts()), card(Rank::Ace, Suit::Spades)),
            1,
        ),
    ];

    let outcome = rank_by_kind(&field, &DEFAULT_TIEBREAK_ORDER, ZeroLapMode::Hide);

    assert_eq!(outcome.err(), Some(StandingsError::JokerDrawKey));
}
