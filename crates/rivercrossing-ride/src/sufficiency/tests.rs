// SPDX-License-Identifier: GPL-3.0-only

use pretty_assertions::assert_eq;
use rstest::rstest;
use time::{Date, Month, OffsetDateTime, PrimitiveDateTime, Time};

use super::{CardCheck, Sufficiency, check_card_sufficiency, estimate_cards_needed};
use crate::config::{EntryMode, PlateModel, RideConfig, RideConfigParams};
use crate::roster_view::{EntryId, EntrySnapshot, RiderSnapshot};

const ONE_SECOND_LAP_KM: f64 = 1.0;
const ONE_SECOND_LAP_SPEED_KMH: f64 = 3600.0;
const DEFAULT_SHOE_CARDS: u32 = 8 * 52 + 1;

fn fixture_date() -> Date {
    Date::from_calendar_date(2026, Month::September, 20).expect("the fixture date is valid")
}

fn fixture_start() -> OffsetDateTime {
    let time = Time::from_hms(10, 0, 0).expect("the fixture time is valid");
    PrimitiveDateTime::new(fixture_date(), time).assume_utc()
}

fn params() -> RideConfigParams {
    RideConfigParams {
        name: "GORBA EPIC 2026".to_owned(),
        event_date: fixture_date(),
        venue: "Sea to Sky Gondola".to_owned(),
        lap_km: 8.0,
        organizer: "GORBA".to_owned(),
        scorer: "K. Singh".to_owned(),
        planned_start: fixture_start(),
        planned_duration_s: 21_600,
        min_lap_s: 1_080,
        entry_mode: EntryMode::Mixed,
        plate_model: PlateModel::RiderPooled,
        ..RideConfigParams::default()
    }
}

fn config() -> RideConfig {
    RideConfig::new(params()).expect("the fixture params are valid")
}

fn rider(name: &str) -> RiderSnapshot {
    RiderSnapshot {
        plate: Some(name.to_owned()),
        full_name: name.to_owned(),
        sex: None,
    }
}

fn solo(plate: &str) -> EntrySnapshot {
    EntrySnapshot {
        id: EntryId(format!("id-{plate}")),
        plate: plate.to_owned(),
        display_name: format!("Solo {plate}"),
        is_team: false,
        is_dnf: false,
        riders: vec![rider(plate)],
    }
}

fn team(plate: &str, riders: Vec<RiderSnapshot>) -> EntrySnapshot {
    EntrySnapshot {
        id: EntryId(format!("id-{plate}")),
        plate: plate.to_owned(),
        display_name: format!("Team {plate}"),
        is_team: true,
        is_dnf: false,
        riders,
    }
}

fn two_entry_field() -> Vec<EntrySnapshot> {
    vec![
        solo("1"),
        team("2", vec![rider("2"), rider("3"), rider("4")]),
    ]
}

fn two_rider_field() -> Vec<EntrySnapshot> {
    vec![team("2", vec![rider("2"), rider("3")])]
}

fn single_rider_field() -> Vec<EntrySnapshot> {
    vec![solo("7")]
}

// ----------------------------------------------- estimate_cards_needed

#[test]
fn test_estimate_cards_needed_given_rider_pooled_counts_every_rider() {
    let result = estimate_cards_needed(&config(), &two_entry_field(), 36.0);

    assert_eq!(result, Some(108));
}

#[test]
fn test_estimate_cards_needed_given_team_relay_counts_entries_only() {
    let config = RideConfig::new(RideConfigParams {
        plate_model: PlateModel::TeamRelay,
        ..params()
    })
    .expect("the fixture params are valid");

    let result = estimate_cards_needed(&config, &two_entry_field(), 36.0);

    assert_eq!(result, Some(54));
}

#[test]
fn test_estimate_cards_needed_given_a_partial_lap_rounds_up() {
    let result = estimate_cards_needed(&config(), &two_entry_field(), 30.0);

    assert_eq!(result, Some(92));
}

#[test]
fn test_estimate_cards_needed_given_an_exact_lap_multiple_does_not_round_up() {
    let result = estimate_cards_needed(&config(), &two_entry_field(), 32.0);

    assert_eq!(result, Some(96));
}

#[rstest]
#[case(0.0)]
#[case(-1.0)]
fn test_estimate_cards_needed_given_a_nonpositive_lap_returns_none(#[case] lap_km: f64) {
    let config = RideConfig::new(RideConfigParams { lap_km, ..params() })
        .expect("the fixture params are valid");

    let result = estimate_cards_needed(&config, &two_entry_field(), 36.0);

    assert_eq!(result, None);
}

#[rstest]
#[case(0.0)]
#[case(-12.0)]
fn test_estimate_cards_needed_given_a_nonpositive_speed_returns_none(#[case] speed: f64) {
    let result = estimate_cards_needed(&config(), &two_entry_field(), speed);

    assert_eq!(result, None);
}

#[test]
fn test_estimate_cards_needed_given_a_zero_duration_returns_none() {
    let config = RideConfig {
        planned_duration_s: 0,
        ..config()
    };

    let result = estimate_cards_needed(&config, &two_entry_field(), 36.0);

    assert_eq!(result, None);
}

#[test]
fn test_estimate_cards_needed_given_no_entries_returns_none() {
    let result = estimate_cards_needed(&config(), &[], 36.0);

    assert_eq!(result, None);
}

#[test]
fn test_estimate_cards_needed_given_a_single_rider_returns_the_lap_count() {
    let result = estimate_cards_needed(&config(), &single_rider_field(), 36.0);

    assert_eq!(result, Some(27));
}

#[rstest]
#[case(f64::NAN)]
#[case(f64::INFINITY)]
fn test_estimate_cards_needed_given_a_non_finite_speed_returns_none(#[case] speed: f64) {
    let result = estimate_cards_needed(&config(), &two_entry_field(), speed);

    assert_eq!(result, None);
}

#[test]
fn test_estimate_cards_needed_given_a_lap_count_above_u32_returns_none() {
    let result = estimate_cards_needed(&config(), &two_entry_field(), 1.0e10);

    assert_eq!(result, None);
}

#[test]
fn test_estimate_cards_needed_given_a_product_above_u32_returns_none() {
    let result = estimate_cards_needed(&config(), &two_rider_field(), 3.0e9);

    assert_eq!(result, None);
}

fn assert_estimate_is_monotonic_in_speed() {
    let config = config();
    let field = two_entry_field();
    let mut previous =
        estimate_cards_needed(&config, &field, 1.0).expect("a one km/h field still laps");
    for step in 1..=400 {
        let speed = f64::from(step);
        let current = estimate_cards_needed(&config, &field, speed).expect("the estimate exists");
        assert!(
            current >= previous,
            "speed {speed}: {current} is below {previous}"
        );
        previous = current;
    }
}

#[test]
fn test_estimate_cards_needed_given_increasing_speeds_never_decreases() {
    assert_estimate_is_monotonic_in_speed();
}

// --------------------------------------------- check_card_sufficiency

#[rstest]
#[case(1, Sufficiency::FarTooMany)]
#[case(208, Sufficiency::FarTooMany)]
#[case(209, Sufficiency::Ok)]
#[case(417, Sufficiency::Ok)]
#[case(418, Sufficiency::NotEnough)]
fn test_check_card_sufficiency_given_each_boundary_returns_its_verdict(
    #[case] planned_duration_s: i32,
    #[case] verdict: Sufficiency,
) {
    let config = RideConfig::new(RideConfigParams {
        lap_km: ONE_SECOND_LAP_KM,
        planned_duration_s,
        jokers_mode: "total".to_owned(),
        ..params()
    })
    .expect("the fixture params are valid");

    let result = check_card_sufficiency(&config, &single_rider_field(), ONE_SECOND_LAP_SPEED_KMH);

    assert_eq!(
        result,
        Some(CardCheck {
            shoe_cards: DEFAULT_SHOE_CARDS,
            expected: u32::try_from(planned_duration_s).expect("the duration is positive"),
            verdict,
        })
    );
}

#[rstest]
#[case("per_deck", 112)]
#[case("total", 108)]
fn test_check_card_sufficiency_given_a_custom_shoe_counts_the_rides_jokers_mode(
    #[case] jokers_mode: &str,
    #[case] expected_shoe_cards: u32,
) {
    let config = RideConfig::new(RideConfigParams {
        lap_km: ONE_SECOND_LAP_KM,
        planned_duration_s: 100,
        deck_count: 2,
        jokers_per_deck: 4,
        jokers_mode: jokers_mode.to_owned(),
        ..params()
    })
    .expect("the fixture params are valid");

    let result = check_card_sufficiency(&config, &single_rider_field(), ONE_SECOND_LAP_SPEED_KMH);

    assert_eq!(
        result,
        Some(CardCheck {
            shoe_cards: expected_shoe_cards,
            expected: 100,
            verdict: Sufficiency::Ok,
        })
    );
}

#[test]
fn test_check_card_sufficiency_given_no_estimate_returns_none() {
    let config = RideConfig::new(RideConfigParams {
        lap_km: 0.0,
        ..params()
    })
    .expect("the fixture params are valid");

    let result = check_card_sufficiency(&config, &single_rider_field(), 36.0);

    assert_eq!(result, None);
}

#[test]
fn test_check_card_sufficiency_given_no_entries_returns_none() {
    let result = check_card_sufficiency(&config(), &[], 36.0);

    assert_eq!(result, None);
}

#[test]
fn test_check_card_sufficiency_given_a_shoe_above_u32_returns_none() {
    let config = RideConfig::new(RideConfigParams {
        lap_km: ONE_SECOND_LAP_KM,
        planned_duration_s: 1,
        deck_count: i32::MAX,
        jokers_per_deck: 10,
        jokers_mode: "per_deck".to_owned(),
        ..params()
    })
    .expect("the fixture params are valid");

    let result = check_card_sufficiency(&config, &single_rider_field(), ONE_SECOND_LAP_SPEED_KMH);

    assert_eq!(result, None);
}

// -------------------------------------------------------- Sufficiency text

#[rstest]
#[case(Sufficiency::NotEnough, "not_enough")]
#[case(Sufficiency::Ok, "ok")]
#[case(Sufficiency::FarTooMany, "far_too_many")]
fn test_sufficiency_as_str_returns_the_oracle_spelling(
    #[case] value: Sufficiency,
    #[case] expected: &str,
) {
    assert_eq!(value.as_str(), expected);
}
