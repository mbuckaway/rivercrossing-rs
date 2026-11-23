// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the shared name keys and the canonical name case.

use pretty_assertions::assert_eq;
use rstest::rstest;

use super::{canonical_person_name, rider_name_key, team_name_key};

#[rstest]
#[case("Mary Anne", "Knibbe", "mary anne knibbe")]
#[case("  mary   anne ", " KNIBBE ", "mary anne knibbe")]
#[case("John", "", "john")]
#[case("", "", "")]
fn test_rider_name_key_given_a_name_returns_the_folded_collapsed_key(
    #[case] first_name: &str,
    #[case] last_name: &str,
    #[case] expected: &str,
) {
    assert_eq!(rider_name_key(first_name, last_name), expected);
}

#[test]
fn test_rider_name_key_given_two_casings_is_the_same_key() {
    assert_eq!(rider_name_key("JoHN", ""), rider_name_key("john", ""));
}

#[rstest]
#[case("john", "John")]
#[case("JOHN", "John")]
#[case("John", "John")]
#[case("  john  ", "John")]
#[case("j", "J")]
#[case(" van der berg ", "Van der berg")]
#[case("MCDONALD", "Mcdonald")]
#[case("", "")]
#[case("   ", "")]
#[case("J.", "J.")]
fn test_canonical_person_name_given_a_uniform_case_field_returns_the_capitalised_form(
    #[case] raw: &str,
    #[case] expected: &str,
) {
    assert_eq!(canonical_person_name(raw), expected);
}

#[rstest]
#[case("McDonald")]
#[case("O'Brien")]
#[case("McDonald Jr.")]
#[case("Van Der Berg")]
#[case("jOhN")]
fn test_canonical_person_name_given_a_mixed_case_field_keeps_the_operators_spelling(
    #[case] raw: &str,
) {
    assert_eq!(canonical_person_name(raw), raw);
}

#[rstest]
#[case("BNBA 1", "bnba1")]
#[case("BNBA1", "bnba1")]
#[case("Good 2 Go", "good2go")]
#[case("Good 2Go", "good2go")]
#[case("Win Win More Win", "winwinmorewin")]
#[case("Win Win and more Win", "winwinmorewin")]
#[case("Win AND More", "winmore")]
#[case("A & B", "ab")]
#[case("A&B", "ab")]
#[case("Team #1!", "team1")]
#[case("Bob's Boys", "bobsboys")]
#[case("O\u{2019}Brien\u{2019}s", "obriens")]
#[case("The \u{201C}A\u{201D} Team", "theateam")]
#[case("", "")]
#[case("   ", "")]
fn test_team_name_key_given_a_name_returns_the_fuzzy_key(
    #[case] name: &str,
    #[case] expected: &str,
) {
    assert_eq!(team_name_key(name), expected);
}

#[test]
fn test_team_name_key_given_a_spaced_and_a_joined_spelling_is_the_same_key() {
    assert_eq!(team_name_key("BNBA 1"), team_name_key("BNBA1"));
}
