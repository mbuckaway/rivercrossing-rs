// SPDX-License-Identifier: GPL-3.0-only

//! The deterministic input that the committed PDF goldens and the content
//! oracle render.
//!
//! The fixture is explicit by construction: fixed cards, fixed readings, a
//! fixed timestamp and a fixed footer stamp, so two renders are byte-equal and
//! the frozen goldens never drift. It compiles only under `cfg(test)`.

#![expect(
    clippy::too_many_arguments,
    reason = "the fixture builder names each field of a synthetic entry separately"
)]

use std::collections::BTreeMap;

use rivercrossing_cards::Card;
use rivercrossing_hands::best_hand;
use rivercrossing_htmlexport::error::ExportError;
use rivercrossing_htmlexport::payload::build_payload;
use rivercrossing_htmlexport::sections::sections;
use rivercrossing_htmlexport::types::{
    ExportInput, ExportOptions, RacePayload, RideInfo, Sections,
};
use rivercrossing_ride::EntryId;
use rivercrossing_standings::{EntryResult, Placed, SnapshotKind};
use time::{Date, Month, OffsetDateTime, Time, UtcOffset};

use crate::report::PdfInput;

/// The ride name every golden render titles itself with.
const RIDE_NAME: &str = "Golden Poker Run 2026";

/// The footer stamp every golden render carries.
const GENERATED: &str = "Generated 20:07, Sept 20 2026";

/// The six solo riders' names for the solo-only poster fixture.
const SOLO_NAMES: [&str; 6] = [
    "Rider One",
    "Rider Two",
    "Rider Three",
    "Rider Four",
    "Rider Five",
    "Rider Six",
];

/// The six solo riders' explicit cards for the solo-only poster fixture.
const SOLO_CARDS: [[&str; 5]; 6] = [
    ["JS", "JD", "JC", "QH", "QS"],
    ["QS", "QD", "QC", "JH", "JS"],
    ["KS", "KD", "KC", "AH", "AS"],
    ["6S", "6D", "6C", "7H", "8S"],
    ["3S", "3D", "3C", "4H", "5S"],
    ["2S", "2D", "2C", "3H", "4S"],
];

/// Return the mixed golden input: Letter paper, no logo.
pub(crate) fn golden_input() -> PdfInput {
    PdfInput {
        export: golden_export(),
        letter: true,
        created_at: golden_created_at(),
        logo_png: None,
    }
}

/// Return the mixed golden payload and its section plan.
///
/// # Errors
///
/// Return the payload builder's error when a fixture plate is not numeric.
pub(crate) fn golden_payload_and_plan() -> Result<(RacePayload, Sections), ExportError> {
    let export = golden_export();
    let payload = build_payload(&export)?;
    let plan = sections(&payload, &export.placed);
    Ok((payload, plan))
}

/// Return a solo-only poster input with six ranked riders.
pub(crate) fn solo_input() -> PdfInput {
    PdfInput {
        export: ExportInput {
            ride: ride_info(),
            placed: solo_placed(),
            options: all_options(),
            generated: GENERATED.to_owned(),
            logo_src: None,
            team_logos: BTreeMap::new(),
            riders: SOLO_NAMES.len(),
            self_test_unverified: false,
            tie_note: None,
        },
        letter: true,
        created_at: golden_created_at(),
        logo_png: None,
    }
}

/// The mixed fixture's explicit five-card hands.
const ALPHA_CARDS: [&str; 5] = ["9S", "9D", "9C", "KH", "2S"];
const BRAVO_CARDS: [&str; 5] = ["AS", "AD", "AC", "KH", "2S"];
const CHARLIE_CARDS: [&str; 5] = ["5H", "5D", "5C", "4S", "2D"];
const RIDER_ONE_CARDS: [&str; 5] = ["KS", "KD", "KC", "QH", "JS"];
const RIDER_TWO_CARDS: [&str; 5] = ["7S", "7D", "7C", "6H", "5S"];
const RIDER_THREE_CARDS: [&str; 5] = ["4S", "4D", "4C", "3H", "2S"];

/// Return the mixed field: two ranked teams, three ranked solo riders, a DNF
/// team and a DNS solo rider, so both kinds and both absent states are drawn.
fn golden_export() -> ExportInput {
    ExportInput {
        ride: ride_info(),
        placed: mixed_placed(),
        options: all_options(),
        generated: GENERATED.to_owned(),
        logo_src: None,
        team_logos: BTreeMap::new(),
        riders: 7,
        self_test_unverified: false,
        tie_note: None,
    }
}

/// Return the mixed field's rows, teams first.
fn mixed_placed() -> Vec<Placed> {
    let mut rows = mixed_teams();
    rows.extend(mixed_solo());
    rows
}

/// Return the mixed field's two ranked teams and its DNF team.
fn mixed_teams() -> Vec<Placed> {
    vec![
        placed(
            Some(1),
            entry(
                "901",
                "Team Alpha",
                SnapshotKind::Team,
                12,
                five(ALPHA_CARDS),
            ),
        ),
        placed(
            Some(2),
            entry(
                "902",
                "Team Bravo",
                SnapshotKind::Team,
                10,
                five(BRAVO_CARDS),
            ),
        ),
        placed(
            Some(6),
            dnf(entry(
                "903",
                "Team Charlie",
                SnapshotKind::Team,
                4,
                five(CHARLIE_CARDS),
            )),
        ),
    ]
}

/// Return the mixed field's three ranked solo riders and its DNS rider.
fn mixed_solo() -> Vec<Placed> {
    vec![
        placed(
            Some(3),
            entry(
                "801",
                "Rider One",
                SnapshotKind::Solo,
                9,
                five(RIDER_ONE_CARDS),
            ),
        ),
        placed(
            Some(4),
            entry(
                "802",
                "Rider Two",
                SnapshotKind::Solo,
                8,
                five(RIDER_TWO_CARDS),
            ),
        ),
        placed(
            Some(5),
            entry(
                "803",
                "Rider Three",
                SnapshotKind::Solo,
                7,
                five(RIDER_THREE_CARDS),
            ),
        ),
        placed(
            None,
            entry("701", "No Show", SnapshotKind::Solo, 0, Vec::new()),
        ),
    ]
}

/// Return the six ranked solo riders of the solo-only fixture.
fn solo_placed() -> Vec<Placed> {
    SOLO_NAMES
        .iter()
        .zip(SOLO_CARDS.iter())
        .enumerate()
        .map(|(index, (name, codes))| {
            let number = u32::try_from(index).expect("the fixture index fits");
            let plate = format!("{}", 601 + index);
            placed(
                Some(number + 1),
                entry(
                    &plate,
                    name,
                    SnapshotKind::Solo,
                    20u32.saturating_sub(number),
                    five(*codes),
                ),
            )
        })
        .collect()
}

/// Return the fixed ride facts.
fn ride_info() -> RideInfo {
    RideInfo {
        name: RIDE_NAME.to_owned(),
        event_date: fixture_date(),
        venue: "Golden Venue".to_owned(),
        lap_km: 8.0,
        organizer: "Golden Org".to_owned(),
        scorer: "T. Ester".to_owned(),
    }
}

/// Return every publish option that the golden fixture exercises.
fn all_options() -> ExportOptions {
    ExportOptions {
        show_times: true,
        laps_board: true,
        time_board: true,
        full_field: true,
        all_cards: true,
        lap_km: 8.0,
    }
}

/// Return the fixed creation timestamp, 2026-09-20T20:07:00Z.
fn golden_created_at() -> OffsetDateTime {
    let clock = Time::from_hms(20, 7, 0).expect("the fixture clock");
    OffsetDateTime::new_in_offset(fixture_date(), clock, UtcOffset::UTC)
}

/// Return the fixed calendar date.
fn fixture_date() -> Date {
    Date::from_calendar_date(2026, Month::September, 20).expect("the fixture date")
}

/// Return one explicit five-card hand.
fn five(codes: [&str; 5]) -> Vec<Card> {
    codes
        .iter()
        .map(|code| Card::parse(code).expect("the fixture card code is valid"))
        .collect()
}

/// Return one synthetic started entry.
fn entry(plate: &str, name: &str, kind: SnapshotKind, laps: u32, cards: Vec<Card>) -> EntryResult {
    EntryResult {
        entry_id: EntryId(plate.to_owned()),
        plate: plate.to_owned(),
        name: name.to_owned(),
        kind,
        laps,
        total_time: f64::from(laps) * 1800.0 + 120.0,
        best_lap: 1800.0,
        hand: best_hand(&cards),
        cards,
        dnf: false,
        sex: None,
        tiebreak_card: None,
    }
}

/// Return *entry* marked as a did-not-finish row.
fn dnf(mut entry: EntryResult) -> EntryResult {
    entry.dnf = true;
    entry
}

/// Return one ranked row; `None` marks a did-not-start row.
fn placed(place: Option<u32>, result: EntryResult) -> Placed {
    Placed {
        place,
        result,
        tie_note: None,
        draw_required: false,
    }
}
