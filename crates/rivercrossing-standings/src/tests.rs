// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the crate-level constants and the spelling seam.

use pretty_assertions::assert_eq;

use super::{
    DEFAULT_TIEBREAK_ORDER, DEFAULT_TOP, DRAW_TIE_NOTE, LIVE_TIEBREAK_ORDER, StandingsError,
    TieBreak, tiebreak_order_from_spellings,
};

#[test]
fn test_tiebreak_order_from_spellings_high_card_maps_and_round_trips() {
    let member = TieBreak::HighCard;
    assert_eq!(
        tiebreak_order_from_spellings(&["high_card"]),
        Ok(vec![member])
    );
    assert_eq!(
        tiebreak_order_from_spellings(&[member.as_str()]),
        Ok(vec![member])
    );
}

#[test]
fn test_tiebreak_order_from_spellings_laps_maps_and_round_trips() {
    let member = TieBreak::Laps;
    assert_eq!(tiebreak_order_from_spellings(&["laps"]), Ok(vec![member]));
    assert_eq!(
        tiebreak_order_from_spellings(&[member.as_str()]),
        Ok(vec![member])
    );
}

#[test]
fn test_tiebreak_order_from_spellings_total_time_maps_and_round_trips() {
    let member = TieBreak::TotalTime;
    assert_eq!(
        tiebreak_order_from_spellings(&["total_time"]),
        Ok(vec![member])
    );
    assert_eq!(
        tiebreak_order_from_spellings(&[member.as_str()]),
        Ok(vec![member])
    );
}

#[test]
fn test_tiebreak_order_from_spellings_keeps_the_input_priority() {
    let spellings = ["total_time", "laps", "high_card"];
    assert_eq!(
        tiebreak_order_from_spellings(&spellings),
        Ok(vec![
            TieBreak::TotalTime,
            TieBreak::Laps,
            TieBreak::HighCard
        ])
    );
}

#[test]
fn test_tiebreak_order_from_spellings_empty_slice_returns_no_criteria() {
    assert_eq!(tiebreak_order_from_spellings(&[]), Ok(Vec::new()));
}

#[test]
fn test_tiebreak_order_from_spellings_unknown_spelling_returns_its_variant() {
    let result = tiebreak_order_from_spellings(&["fastest"]);
    assert_eq!(
        result,
        Err(StandingsError::UnknownTieBreakSpelling {
            spelling: "fastest".to_owned(),
        })
    );
}

#[test]
fn test_tiebreak_order_from_spellings_unknown_spelling_names_it_quoted() {
    let message = tiebreak_order_from_spellings(&["fastest"]).map_err(|error| error.to_string());
    assert_eq!(
        message,
        Err("unknown tie-break spelling: 'fastest'".to_owned())
    );
}

#[test]
fn test_default_tiebreak_order_leads_with_the_high_card_draw() {
    assert_eq!(
        DEFAULT_TIEBREAK_ORDER,
        [TieBreak::HighCard, TieBreak::Laps, TieBreak::TotalTime]
    );
}

#[test]
fn test_live_tiebreak_order_is_laps_then_total_time() {
    assert_eq!(LIVE_TIEBREAK_ORDER, [TieBreak::Laps, TieBreak::TotalTime]);
}

#[test]
fn test_draw_tie_note_is_the_oracle_sentence() {
    assert_eq!(DRAW_TIE_NOTE, "draw required");
}

#[test]
fn test_default_top_is_ten() {
    assert_eq!(DEFAULT_TOP, 10);
}
