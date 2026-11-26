// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the export composition and its writer wrappers.
//!
//! Every composition test builds a real ride through a temporary store, so the
//! placed rows, the rider count and the self-test flag are the engine's own.
//! The writers land their files in a temporary directory.

#![expect(
    clippy::float_cmp,
    reason = "the asserted loop lengths are exact literals that the config copies unchanged"
)]

use pretty_assertions::assert_eq;
use rivercrossing_htmlexport::format::png_data_uri;
use rivercrossing_htmlexport::types::ExportInput;
use rivercrossing_ride::{
    EntryMode, Event, EventAction, PlateModel, RideConfig, RideConfigParams, RideEngine,
    SnapshotKind, SystemClock, Timestamp,
};
use rivercrossing_roster::roster::{Roster, TeamFields};
use rivercrossing_roster::types::Rider;
use rivercrossing_store::Store;
use rivercrossing_store::settings::{AppSettings, default_settings};
use rstest::rstest;
use serde_json::json;
use time::{Date, Month, OffsetDateTime, Time};

use super::{
    PdfOptions, build, export_standings_csv, packaged_card_assets_dir, render_html, render_poster,
    render_wordpress_full, render_wordpress_podium, write_html, write_pdf, write_poster_pdf,
};

/// The ride name every fixture uses.
const RIDE_NAME: &str = "GORBA EPIC 2026";

/// The fixed gun every fixture starts from, in whole seconds.
const GUN_EPOCH: i64 = 1_790_000_000;

/// The ride seed the store writes.
const SEED: u64 = 20_260_920;

/// The footer stamp the composition is handed.
const GENERATED: &str = "Generated 10:00, Sept 20 2026";

// ------------------------------------------------------------- fixtures

/// Return a valid fixture config with the given short-lap policy.
fn fixture_config(min_lap_s: i32, hold_short_laps: bool) -> Result<RideConfig, String> {
    RideConfig::new(RideConfigParams {
        name: RIDE_NAME.to_owned(),
        venue: "Sea to Sky Gondola".to_owned(),
        lap_km: 8.0,
        organizer: "GORBA".to_owned(),
        scorer: "K. Singh".to_owned(),
        entry_mode: EntryMode::Mixed,
        plate_model: PlateModel::RiderPooled,
        min_lap_s,
        hold_short_laps,
        ..RideConfigParams::default()
    })
    .map_err(|error| error.to_string())
}

/// Return one rider carrying *plate*.
fn rider(first: &str, plate: &str) -> Rider {
    Rider {
        first_name: first.to_owned(),
        last_name: "Rider".to_owned(),
        plate: Some(plate.to_owned()),
        ..Rider::default()
    }
}

/// Return a mixed roster: one two-rider team and two solo entries.
fn mixed_roster() -> Result<Roster, String> {
    let mut roster = Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, Some(SEED))
        .map_err(|error| error.to_string())?;
    roster
        .create_team_entry(TeamFields {
            display_name: "Trail Blazers",
            riders: vec![rider("Ada", "30"), rider("Grace", "31")],
            plate: None,
            logo_card: None,
        })
        .map_err(|error| error.to_string())?;
    for (first, plate) in [("Sam", "10"), ("Robin", "11")] {
        roster
            .create_solo_entry(rider(first, plate), plate)
            .map_err(|error| error.to_string())?;
    }
    Ok(roster)
}

/// Return a roster holding two solo entries on plates `10` and `11`.
fn two_solo_roster() -> Result<Roster, String> {
    let mut roster = Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, Some(SEED))
        .map_err(|error| error.to_string())?;
    for (first, plate) in [("Sam", "10"), ("Robin", "11")] {
        roster
            .create_solo_entry(rider(first, plate), plate)
            .map_err(|error| error.to_string())?;
    }
    Ok(roster)
}

/// Return *gun* moved by *seconds*.
fn shifted(gun: Timestamp, seconds: i64) -> Result<Timestamp, String> {
    Timestamp::from_unix_timestamp(gun.unix_timestamp() + seconds)
        .map_err(|error| error.to_string())
}

/// Start *engine* at the fixed gun and return that gun.
fn start(engine: &mut RideEngine<Roster, SystemClock>) -> Result<Timestamp, String> {
    let gun = Timestamp::from_unix_timestamp(GUN_EPOCH).map_err(|error| error.to_string())?;
    engine.start(Some(gun)).map_err(|error| error.to_string())?;
    Ok(gun)
}

/// Record one lap for *plate* at `gun + seconds`.
fn lap(
    engine: &mut RideEngine<Roster, SystemClock>,
    plate: &str,
    seconds: i64,
) -> Result<(), String> {
    let gun = engine
        .actual_start()
        .ok_or_else(|| "the ride has no start instant".to_owned())?;
    let at = shifted(gun, seconds)?;
    let _crossing = engine.record_crossing(plate, Some(at));
    Ok(())
}

/// Return a finished mixed ride: the team and one solo each rode one lap.
fn finished_mixed() -> Result<(Roster, RideConfig, RideEngine<Roster, SystemClock>), String> {
    let roster = mixed_roster()?;
    let config = fixture_config(1, false)?;
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let mut store =
        Store::open(directory.path().join("rides.db")).map_err(|error| error.to_string())?;
    let ride_id = store
        .create_ride(&config, Some(SEED))
        .map_err(|error| error.to_string())?;
    store
        .save_roster(ride_id, &roster)
        .map_err(|error| error.to_string())?;
    let mut engine = store
        .load_engine(ride_id)
        .map_err(|error| error.to_string())?;
    start(&mut engine)?;
    lap(&mut engine, "30", 60)?;
    lap(&mut engine, "10", 60)?;
    engine.finish(&[]).map_err(|error| error.to_string())?;
    Ok((roster, config, engine))
}

/// Return a FINISHED engine whose two held-card entries drew `cards`.
///
/// Both solo entries ride one short lap, whose card the hold policy keeps, so
/// the two hands tie on no credited card. The draw rows name each entry's card
/// in roster order, so a test can hand the ranking a card the venue draw never
/// deals.
fn finished_engine_with_manual_draw(
    roster: &Roster,
    config: &RideConfig,
    cards: [&str; 2],
) -> Result<RideEngine<Roster, SystemClock>, String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let mut store =
        Store::open(directory.path().join("rides.db")).map_err(|error| error.to_string())?;
    let ride_id = store
        .create_ride(config, Some(SEED))
        .map_err(|error| error.to_string())?;
    store
        .save_roster(ride_id, roster)
        .map_err(|error| error.to_string())?;
    let mut engine = store
        .load_engine(ride_id)
        .map_err(|error| error.to_string())?;
    start(&mut engine)?;
    lap(&mut engine, "10", 60)?;
    lap(&mut engine, "11", 60)?;
    engine.finish(&[]).map_err(|error| error.to_string())?;
    for event in engine.events().to_vec() {
        store
            .append(ride_id, &event)
            .map_err(|error| error.to_string())?;
    }
    let draws: Vec<serde_json::Value> = roster
        .entries()
        .iter()
        .zip(cards)
        .map(|(entry, card)| json!({"entry_id": entry.key.clone(), "card": card}))
        .collect();
    store
        .append(
            ride_id,
            &Event {
                action: EventAction::TiebreakDraw,
                payload: json!({"draws": draws}),
            },
        )
        .map_err(|error| error.to_string())?;
    store
        .load_engine(ride_id)
        .map_err(|error| error.to_string())
}

/// Return a fixed PDF creation instant.
fn created_at() -> Result<OffsetDateTime, String> {
    let date =
        Date::from_calendar_date(2026, Month::September, 20).map_err(|error| error.to_string())?;
    let time = Time::from_hms(16, 2, 11).map_err(|error| error.to_string())?;
    Ok(OffsetDateTime::new_utc(date, time))
}

/// Return the PDF options every writer test uses.
fn pdf_options() -> Result<PdfOptions, String> {
    Ok(PdfOptions {
        letter: false,
        created_at: created_at()?,
        logo_png: None,
    })
}

/// Build the composition over *engine*, *roster* and *config*.
fn compose(
    engine: &RideEngine<Roster, SystemClock>,
    roster: &Roster,
    config: &RideConfig,
) -> Result<ExportInput, String> {
    build(engine, roster, config, &default_settings(), GENERATED)
}

// ------------------------------------------------- composition: options

#[test]
fn test_build_given_the_default_settings_maps_the_five_publish_flags() -> Result<(), String> {
    // Arrange
    let (roster, config, engine) = finished_mixed()?;

    // Act
    let input = compose(&engine, &roster, &config)?;

    // Assert
    assert!(!input.options.show_times);
    assert!(input.options.laps_board);
    assert!(!input.options.time_board);
    assert!(input.options.full_field);
    assert!(input.options.all_cards);
    assert_eq!(input.options.lap_km, 8.0);
    Ok(())
}

#[test]
fn test_build_given_a_changed_setting_maps_every_publish_flag() -> Result<(), String> {
    // Arrange
    let (roster, config, engine) = finished_mixed()?;
    let settings = AppSettings {
        publish_show_times: true,
        publish_laps_board: false,
        publish_time_board: true,
        publish_full_field: false,
        publish_all_cards: false,
        ..default_settings()
    };

    // Act
    let input = build(&engine, &roster, &config, &settings, GENERATED)?;

    // Assert
    assert!(input.options.show_times);
    assert!(!input.options.laps_board);
    assert!(input.options.time_board);
    assert!(!input.options.full_field);
    assert!(!input.options.all_cards);
    Ok(())
}

// ---------------------------------------------------- composition: ride

#[test]
fn test_build_given_a_ride_reads_the_ride_info_from_the_config() -> Result<(), String> {
    // Arrange
    let (roster, config, engine) = finished_mixed()?;

    // Act
    let input = compose(&engine, &roster, &config)?;

    // Assert
    assert_eq!(input.ride.name, RIDE_NAME);
    assert_eq!(input.ride.venue, "Sea to Sky Gondola");
    assert_eq!(input.ride.organizer, "GORBA");
    assert_eq!(input.ride.scorer, "K. Singh");
    assert_eq!(input.ride.lap_km, 8.0);
    assert_eq!(input.ride.event_date, config.event_date);
    assert_eq!(input.generated, GENERATED);
    Ok(())
}

// ------------------------------------------------- composition: rows

#[test]
fn test_build_given_a_finished_mixed_ride_places_teams_before_solo() -> Result<(), String> {
    // Arrange
    let (roster, config, engine) = finished_mixed()?;

    // Act
    let input = compose(&engine, &roster, &config)?;

    // Assert
    assert_eq!(input.placed.len(), 3);
    let kinds: Vec<SnapshotKind> = input
        .placed
        .iter()
        .map(|placed| placed.result.kind)
        .collect();
    assert_eq!(kinds.first().copied(), Some(SnapshotKind::Team));
    Ok(())
}

#[test]
fn test_build_given_the_default_toggle_keeps_the_dns_row() -> Result<(), String> {
    // Arrange
    let (roster, config, engine) = finished_mixed()?;

    // Act
    let input = compose(&engine, &roster, &config)?;

    // Assert
    assert_eq!(
        input.placed.iter().filter(|placed| placed.is_dns()).count(),
        1
    );
    Ok(())
}

#[test]
fn test_build_given_the_toggle_off_drops_the_dns_row() -> Result<(), String> {
    // Arrange
    let (roster, config, engine) = finished_mixed()?;
    let settings = AppSettings {
        show_dns_riders: false,
        ..default_settings()
    };

    // Act
    let input = build(&engine, &roster, &config, &settings, GENERATED)?;

    // Assert
    assert_eq!(input.placed.len(), 2);
    assert_eq!(
        input.placed.iter().filter(|placed| placed.is_dns()).count(),
        0
    );
    Ok(())
}

#[test]
fn test_build_given_a_joker_draw_returns_the_standings_refusal() -> Result<(), String> {
    // Arrange
    let roster = two_solo_roster()?;
    let config = fixture_config(1_080, true)?;
    let engine = finished_engine_with_manual_draw(&roster, &config, ["JK", "AS"])?;

    // Act
    let result = build(&engine, &roster, &config, &default_settings(), GENERATED);

    // Assert
    assert_eq!(result.err(), Some("a joker has no draw key".to_owned()));
    Ok(())
}

// ------------------------------------------------ composition: logos

#[test]
fn test_build_given_a_team_reads_its_logo_card_as_a_data_uri() -> Result<(), String> {
    // Arrange
    let (roster, config, engine) = finished_mixed()?;
    let team_plate = roster
        .entries()
        .iter()
        .find(|entry| entry.kind == rivercrossing_roster::enums::EntryType::Team)
        .map(|entry| entry.plate.clone())
        .ok_or_else(|| "no team".to_owned())?;

    // Act
    let input = compose(&engine, &roster, &config)?;

    // Assert
    let logo = input
        .team_logos
        .get(&team_plate)
        .ok_or_else(|| "no team logo".to_owned())?;
    assert!(logo.starts_with("data:image/png;base64,"));
    Ok(())
}

#[test]
fn test_build_given_the_joker_logo_card_reads_the_joker_bitmap() -> Result<(), String> {
    // Arrange
    let mut roster = mixed_roster()?;
    let team = roster
        .entries()
        .iter()
        .find(|entry| entry.kind == rivercrossing_roster::enums::EntryType::Team)
        .map(rivercrossing_roster::types::Entry::id)
        .ok_or_else(|| "no team".to_owned())?;
    roster
        .set_team_logo_card(&team, "JK")
        .map_err(|error| error.to_string())?;
    let plate = roster
        .entries()
        .first()
        .map(|entry| entry.plate.clone())
        .ok_or_else(|| "no team".to_owned())?;
    let config = fixture_config(1, false)?;
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let mut store =
        Store::open(directory.path().join("rides.db")).map_err(|error| error.to_string())?;
    let ride_id = store
        .create_ride(&config, Some(SEED))
        .map_err(|error| error.to_string())?;
    store
        .save_roster(ride_id, &roster)
        .map_err(|error| error.to_string())?;
    let engine = store
        .load_engine(ride_id)
        .map_err(|error| error.to_string())?;

    // Act
    let input = compose(&engine, &roster, &config)?;

    // Assert
    assert!(input.team_logos.contains_key(&plate));
    Ok(())
}

#[test]
fn test_build_given_a_solo_only_field_leaves_the_team_logo_map_empty() -> Result<(), String> {
    // Arrange
    let roster = two_solo_roster()?;
    let config = fixture_config(1, false)?;
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let mut store =
        Store::open(directory.path().join("rides.db")).map_err(|error| error.to_string())?;
    let ride_id = store
        .create_ride(&config, Some(SEED))
        .map_err(|error| error.to_string())?;
    store
        .save_roster(ride_id, &roster)
        .map_err(|error| error.to_string())?;
    let engine = store
        .load_engine(ride_id)
        .map_err(|error| error.to_string())?;

    // Act
    let input = compose(&engine, &roster, &config)?;

    // Assert
    assert_eq!(input.team_logos.len(), 0);
    Ok(())
}

#[rstest]
#[case("ZZ")]
#[case("")]
fn test_build_given_an_unknown_logo_card_leaves_no_entry(#[case] code: &str) -> Result<(), String> {
    // Arrange
    let mut roster = mixed_roster()?;
    let team = roster
        .entries()
        .iter()
        .find(|entry| entry.kind == rivercrossing_roster::enums::EntryType::Team)
        .map(rivercrossing_roster::types::Entry::id)
        .ok_or_else(|| "no team".to_owned())?;
    roster
        .set_team_logo_card(&team, code)
        .map_err(|error| error.to_string())?;
    let config = fixture_config(1, false)?;
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let mut store =
        Store::open(directory.path().join("rides.db")).map_err(|error| error.to_string())?;
    let ride_id = store
        .create_ride(&config, Some(SEED))
        .map_err(|error| error.to_string())?;
    store
        .save_roster(ride_id, &roster)
        .map_err(|error| error.to_string())?;
    let engine = store
        .load_engine(ride_id)
        .map_err(|error| error.to_string())?;

    // Act
    let input = compose(&engine, &roster, &config)?;

    // Assert
    assert_eq!(input.team_logos.len(), 0);
    Ok(())
}

#[test]
fn test_build_given_a_team_without_a_logo_card_leaves_no_entry() -> Result<(), String> {
    // Arrange: a roster with no logo seed assigns no card, so the map stays
    // empty even though a team stands.
    let mut roster = Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, None)
        .map_err(|error| error.to_string())?;
    roster
        .create_team_entry(TeamFields {
            display_name: "Trail Blazers",
            riders: vec![rider("Ada", "30"), rider("Grace", "31")],
            plate: None,
            logo_card: None,
        })
        .map_err(|error| error.to_string())?;
    roster
        .create_solo_entry(rider("Sam", "10"), "10")
        .map_err(|error| error.to_string())?;
    let config = fixture_config(1, false)?;
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let mut store =
        Store::open(directory.path().join("rides.db")).map_err(|error| error.to_string())?;
    let ride_id = store
        .create_ride(&config, Some(SEED))
        .map_err(|error| error.to_string())?;
    store
        .save_roster(ride_id, &roster)
        .map_err(|error| error.to_string())?;
    let engine = store
        .load_engine(ride_id)
        .map_err(|error| error.to_string())?;

    // Act
    let input = compose(&engine, &roster, &config)?;

    // Assert
    assert_eq!(input.team_logos.len(), 0);
    Ok(())
}

// ---------------------------------------------- composition: card assets

#[test]
fn test_packaged_card_assets_dir_prefers_cards_beside_the_binary() -> Result<(), String> {
    // Arrange: the Windows/portable-zip layout — `cards/` next to the binary.
    let root = tempfile::tempdir().map_err(|error| error.to_string())?;
    let cards = root.path().join("cards");
    std::fs::create_dir(&cards).map_err(|error| error.to_string())?;

    // Act
    let found = packaged_card_assets_dir(root.path());

    // Assert
    assert_eq!(found.as_deref(), Some(cards.as_path()));
    Ok(())
}

#[test]
fn test_packaged_card_assets_dir_finds_the_macos_resources_layout() -> Result<(), String> {
    // Arrange: the macOS app-bundle layout — `Contents/Resources/cards` reached
    // from the binary at `Contents/MacOS/rivercrossing`.
    let app = tempfile::tempdir().map_err(|error| error.to_string())?;
    let macos = app.path().join("Contents/MacOS");
    let resources_cards = app.path().join("Contents/Resources/cards");
    std::fs::create_dir_all(&macos).map_err(|error| error.to_string())?;
    std::fs::create_dir_all(&resources_cards).map_err(|error| error.to_string())?;

    // Act
    let found = packaged_card_assets_dir(&macos);

    // Assert
    let expected = macos.join("../Resources/cards");
    assert_eq!(found.as_deref(), Some(expected.as_path()));
    Ok(())
}

#[test]
fn test_packaged_card_assets_dir_finds_the_linux_usr_lib_layout() -> Result<(), String> {
    // Arrange: the Linux `.deb`/`.rpm` layout — `/usr/bin/rivercrossing` beside
    // `/usr/lib/rivercrossing/cards`.
    let root = tempfile::tempdir().map_err(|error| error.to_string())?;
    let bin = root.path().join("usr/bin");
    let lib_cards = root.path().join("usr/lib/rivercrossing/cards");
    std::fs::create_dir_all(&bin).map_err(|error| error.to_string())?;
    std::fs::create_dir_all(&lib_cards).map_err(|error| error.to_string())?;

    // Act
    let found = packaged_card_assets_dir(&bin);

    // Assert
    let expected = bin.join("../lib/rivercrossing/cards");
    assert_eq!(found.as_deref(), Some(expected.as_path()));
    Ok(())
}

#[test]
fn test_packaged_card_assets_dir_returns_none_without_any_layout() -> Result<(), String> {
    // Arrange: a bare directory with none of the three install layouts.
    let root = tempfile::tempdir().map_err(|error| error.to_string())?;
    let bin = root.path().join("usr/bin");
    std::fs::create_dir_all(&bin).map_err(|error| error.to_string())?;

    // Act
    let found = packaged_card_assets_dir(&bin);

    // Assert
    assert_eq!(found, None);
    Ok(())
}

#[test]
fn test_build_given_a_ride_logo_reads_it_as_a_data_uri() -> Result<(), String> {
    // Arrange
    let (roster, mut config, engine) = finished_mixed()?;
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let path = directory.path().join("logo.png");
    std::fs::write(&path, b"logo-bytes").map_err(|error| error.to_string())?;
    config.logo_path = Some(path);

    // Act
    let input = compose(&engine, &roster, &config)?;

    // Assert
    assert_eq!(input.logo_src, Some(png_data_uri(b"logo-bytes")));
    Ok(())
}

#[test]
fn test_build_given_a_missing_ride_logo_leaves_the_fallback() -> Result<(), String> {
    // Arrange
    let (roster, mut config, engine) = finished_mixed()?;
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    config.logo_path = Some(directory.path().join("absent.png"));

    // Act
    let input = compose(&engine, &roster, &config)?;

    // Assert
    assert_eq!(input.logo_src, None);
    Ok(())
}

// ------------------------------------------------ composition: counts

#[test]
fn test_build_given_laps_counts_only_the_started_entries_riders() -> Result<(), String> {
    // Arrange
    let (roster, config, engine) = finished_mixed()?;

    // Act
    let input = compose(&engine, &roster, &config)?;

    // Assert: the two-rider team and the one started solo, not the DNS solo.
    assert_eq!(input.riders, 3);
    Ok(())
}

#[test]
fn test_build_given_a_clean_finish_leaves_the_self_test_flag_clear() -> Result<(), String> {
    // Arrange
    let (roster, config, engine) = finished_mixed()?;

    // Act
    let input = compose(&engine, &roster, &config)?;

    // Assert
    assert!(!input.self_test_unverified);
    Ok(())
}

#[test]
fn test_build_given_an_override_finish_sets_the_self_test_flag() -> Result<(), String> {
    // Arrange
    let roster = two_solo_roster()?;
    let config = fixture_config(1, false)?;
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let mut store =
        Store::open(directory.path().join("rides.db")).map_err(|error| error.to_string())?;
    let ride_id = store
        .create_ride(&config, Some(SEED))
        .map_err(|error| error.to_string())?;
    store
        .save_roster(ride_id, &roster)
        .map_err(|error| error.to_string())?;
    let mut engine = store
        .load_engine(ride_id)
        .map_err(|error| error.to_string())?;
    start(&mut engine)?;
    lap(&mut engine, "10", 60)?;
    engine
        .finish(&["7,462 distinct ranks".to_owned()])
        .map_err(|error| error.to_string())?;

    // Act
    let input = compose(&engine, &roster, &config)?;

    // Assert
    assert!(input.self_test_unverified);
    Ok(())
}

// ------------------------------------------------------ render wrappers

#[test]
fn test_render_html_given_a_finished_ride_returns_the_page() -> Result<(), String> {
    // Arrange
    let (roster, config, engine) = finished_mixed()?;
    let input = compose(&engine, &roster, &config)?;

    // Act
    let page = render_html(&input)?;

    // Assert
    assert!(page.contains("<!DOCTYPE html>"));
    assert!(page.contains(RIDE_NAME));
    assert!(page.contains("race-data"));
    Ok(())
}

#[test]
fn test_render_wordpress_full_given_a_finished_ride_returns_the_fragment() -> Result<(), String> {
    // Arrange
    let (roster, config, engine) = finished_mixed()?;
    let input = compose(&engine, &roster, &config)?;

    // Act
    let fragment = render_wordpress_full(&input)?;

    // Assert
    assert_ne!(fragment.as_str(), "");
    assert!(fragment.contains(RIDE_NAME));
    assert!(!fragment.contains("<!DOCTYPE"));
    Ok(())
}

#[test]
fn test_render_wordpress_podium_given_a_finished_ride_returns_the_fragment() -> Result<(), String> {
    // Arrange
    let (roster, config, engine) = finished_mixed()?;
    let input = compose(&engine, &roster, &config)?;

    // Act
    let fragment = render_wordpress_podium(&input)?;

    // Assert
    assert_ne!(fragment.as_str(), "");
    assert!(fragment.contains(RIDE_NAME));
    Ok(())
}

#[test]
fn test_render_poster_given_a_finished_ride_returns_the_page() -> Result<(), String> {
    // Arrange
    let (roster, config, engine) = finished_mixed()?;
    let input = compose(&engine, &roster, &config)?;

    // Act
    let page = render_poster(&input)?;

    // Assert
    assert!(page.contains("<!DOCTYPE html>"));
    assert!(page.contains(RIDE_NAME));
    Ok(())
}

// ------------------------------------------------------- file writers

/// Return a directory that the writer tests keep alive.
fn temp_dir() -> Result<tempfile::TempDir, String> {
    tempfile::tempdir().map_err(|error| error.to_string())
}

#[test]
fn test_write_poster_html_given_a_tempdir_writes_the_poster_page() -> Result<(), String> {
    // Arrange
    let (roster, config, engine) = finished_mixed()?;
    let input = compose(&engine, &roster, &config)?;
    let directory = temp_dir()?;
    let path = directory.path().join("poster.html");

    // Act
    let written = super::write_poster_html(&input, &path)?;

    // Assert
    assert_eq!(written, path);
    let page = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
    assert!(page.contains("<!DOCTYPE html>"));
    Ok(())
}

#[test]
fn test_write_poster_html_given_a_directory_at_the_path_returns_the_replace_refusal()
-> Result<(), String> {
    // Arrange
    let (roster, config, engine) = finished_mixed()?;
    let input = compose(&engine, &roster, &config)?;
    let directory = temp_dir()?;
    let path = directory.path().join("poster.html");
    std::fs::create_dir(&path).map_err(|error| error.to_string())?;

    // Act
    let result = super::write_poster_html(&input, &path);

    // Assert
    assert!(
        result
            .err()
            .is_some_and(|message| message.starts_with("cannot replace"))
    );
    Ok(())
}

#[test]
fn test_write_html_given_a_tempdir_writes_the_page() -> Result<(), String> {
    // Arrange
    let (roster, config, engine) = finished_mixed()?;
    let input = compose(&engine, &roster, &config)?;
    let directory = temp_dir()?;
    let path = directory.path().join("results.html");

    // Act
    let written = write_html(&input, &path)?;

    // Assert
    assert_eq!(written, path);
    let page = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
    assert!(page.contains("<!DOCTYPE html>"));
    Ok(())
}

#[test]
fn test_write_html_given_a_directory_at_the_path_returns_the_replace_refusal() -> Result<(), String>
{
    // Arrange
    let (roster, config, engine) = finished_mixed()?;
    let input = compose(&engine, &roster, &config)?;
    let directory = temp_dir()?;
    let path = directory.path().join("results.html");
    std::fs::create_dir(&path).map_err(|error| error.to_string())?;

    // Act
    let result = write_html(&input, &path);

    // Assert
    assert!(
        result
            .err()
            .is_some_and(|message| message.starts_with("cannot replace"))
    );
    Ok(())
}

#[test]
fn test_write_html_given_a_path_with_no_file_name_returns_the_write_refusal() -> Result<(), String>
{
    // Arrange
    let (roster, config, engine) = finished_mixed()?;
    let input = compose(&engine, &roster, &config)?;
    let path = std::path::Path::new("");

    // Act
    let result = write_html(&input, path);

    // Assert
    assert_eq!(
        result.err(),
        Some("cannot write : the path has no file name".to_owned())
    );
    Ok(())
}

#[test]
fn test_write_html_given_a_missing_parent_returns_the_write_refusal() -> Result<(), String> {
    // Arrange
    let (roster, config, engine) = finished_mixed()?;
    let input = compose(&engine, &roster, &config)?;
    let directory = temp_dir()?;
    let path = directory.path().join("missing").join("results.html");

    // Act
    let result = write_html(&input, &path);

    // Assert
    assert!(
        result
            .err()
            .is_some_and(|message| message.starts_with("cannot write"))
    );
    Ok(())
}

#[test]
fn test_write_pdf_given_a_tempdir_writes_the_report() -> Result<(), String> {
    // Arrange
    let (roster, config, engine) = finished_mixed()?;
    let input = compose(&engine, &roster, &config)?;
    let options = pdf_options()?;
    let directory = temp_dir()?;
    let path = directory.path().join("results.pdf");

    // Act
    let written = write_pdf(&input, &options, &path)?;

    // Assert
    assert_eq!(written, path);
    let bytes = std::fs::read(&path).map_err(|error| error.to_string())?;
    assert!(bytes.starts_with(b"%PDF"));
    Ok(())
}

#[test]
fn test_write_pdf_given_a_missing_parent_returns_the_error() -> Result<(), String> {
    // Arrange
    let (roster, config, engine) = finished_mixed()?;
    let input = compose(&engine, &roster, &config)?;
    let options = pdf_options()?;
    let directory = temp_dir()?;
    let path = directory.path().join("missing").join("results.pdf");

    // Act
    let result = write_pdf(&input, &options, &path);

    // Assert
    assert!(
        result
            .err()
            .is_some_and(|message| message.starts_with("io:"))
    );
    Ok(())
}

#[test]
fn test_write_poster_pdf_given_a_tempdir_writes_the_poster() -> Result<(), String> {
    // Arrange
    let (roster, config, engine) = finished_mixed()?;
    let input = compose(&engine, &roster, &config)?;
    let options = pdf_options()?;
    let directory = temp_dir()?;
    let path = directory.path().join("poster.pdf");

    // Act
    let written = write_poster_pdf(&input, &options, &path)?;

    // Assert
    assert_eq!(written, path);
    let bytes = std::fs::read(&path).map_err(|error| error.to_string())?;
    assert!(bytes.starts_with(b"%PDF"));
    Ok(())
}

#[test]
fn test_export_standings_csv_given_a_tempdir_writes_the_rows() -> Result<(), String> {
    // Arrange
    let (roster, config, engine) = finished_mixed()?;
    let input = compose(&engine, &roster, &config)?;
    let directory = temp_dir()?;
    let path = directory.path().join("standings.csv");

    // Act
    let written = export_standings_csv(&input, &path)?;

    // Assert
    assert_eq!(written, path);
    let csv = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
    assert!(csv.contains("place"));
    assert!(csv.contains("Trail Blazers"));
    Ok(())
}

#[test]
fn test_export_standings_csv_given_a_missing_parent_returns_the_error() -> Result<(), String> {
    // Arrange
    let (roster, config, engine) = finished_mixed()?;
    let input = compose(&engine, &roster, &config)?;
    let directory = temp_dir()?;
    let path = directory.path().join("missing").join("standings.csv");

    // Act
    let result = export_standings_csv(&input, &path);

    // Assert
    assert!(
        result
            .err()
            .is_some_and(|message| message.starts_with("io:"))
    );
    Ok(())
}
