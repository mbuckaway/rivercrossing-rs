// SPDX-License-Identifier: GPL-3.0-only

//! The export composition and the writer wrappers the Results menu calls.
//!
//! One place builds the [`ExportInput`] every exporter reads: the ride info from
//! the config, the placed rows from the engine's snapshot through the results
//! window's own ranking seam, the six publish options from the settings, the
//! team logos from the roster's logo cards, the header's rider count, the
//! self-test flag and the caller's footer stamp. The wrappers then render the
//! HTML page, the two `WordPress` fragments and the poster, or write the page,
//! the two PDFs and the standings CSV to a path.
//!
//! The rules follow the Python oracle's `ui/app.py` (`_export_options`,
//! `_placed_for_export`, `_team_logo_srcs`, `_unique_rider_count` and
//! `_write_export`). Every wrapper returns the written path or the error's own
//! sentence, so the app can post one status notice.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use rivercrossing_htmlexport::format::png_data_uri;
use rivercrossing_htmlexport::render::{
    render as render_page, render_poster as poster_page, render_poster_wordpress, render_wordpress,
};
use rivercrossing_htmlexport::types::{ExportInput, ExportOptions, RideInfo};
use rivercrossing_pdfexport::poster::podium_poster;
use rivercrossing_pdfexport::report::{PdfInput, render as report};
use rivercrossing_ride::{RideConfig, RideEngine, SystemClock};
use rivercrossing_roster::enums::EntryType;
use rivercrossing_roster::roster::Roster;
use rivercrossing_store::settings::AppSettings;
use time::OffsetDateTime;

use super::results::ranked_sections;

#[cfg(test)]
mod tests;

/// The packaged card bitmaps that a team's logo card resolves to.
///
/// The bitmaps ship in the source tree's `design/assets/cards`; a packaged
/// build will need them installed beside the binary.
const CARD_ASSETS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/assets/cards");

/// The bitmap scale the page embeds: the 2x variant stays crisp at ~48 px.
const LOGO_SCALE: &str = "-2x";

/// The stored code of the joker, which has its own asset name.
const JOKER_CODE: &str = "JK";

/// The asset name of the joker.
const JOKER_KEY: &str = "joker";

/// The rank tokens the card assets ship, as the stored code spells them.
const ASSET_RANKS: [&str; 13] = [
    "2", "3", "4", "5", "6", "7", "8", "9", "10", "J", "Q", "K", "A",
];

/// One PDF render's options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PdfOptions {
    /// Whether the page is Letter instead of A4.
    pub(crate) letter: bool,
    /// The caller's pinned creation instant.
    pub(crate) created_at: OffsetDateTime,
    /// The organizer logo's PNG bytes.
    pub(crate) logo_png: Option<Vec<u8>>,
}

/// Build the export input for *engine*.
///
/// The placed rows come from the same ranking seam the results window uses, so
/// the window and the exported file can never disagree about a place or a
/// DNS row. The publish options read the five `publish_*` settings; the loop
/// length in the options keeps the exporter's own default, because the page's
/// course length is not an operator setting.
///
/// The app passes no residual-tie note: each row's own tie marker reaches the
/// page through the placed result, exactly as the oracle's export leaves the
/// payload's note unset.
///
/// # Errors
///
/// Return the standings refusal when a drawn tie reaches a joker draw card.
#[expect(
    clippy::too_many_arguments,
    reason = "the composition takes the engine, the field, the config, the settings and the pinned stamp"
)]
pub(crate) fn build(
    engine: &RideEngine<Roster, SystemClock>,
    roster: &Roster,
    config: &RideConfig,
    settings: &AppSettings,
    generated: &str,
) -> Result<ExportInput, String> {
    let (teams, solo) = ranked_sections(engine, config.tiebreak_order, settings.show_dns_riders)?;
    let mut placed = teams;
    placed.extend(solo);
    Ok(ExportInput {
        ride: ride_info(config),
        placed,
        options: export_options(settings),
        generated: generated.to_owned(),
        logo_src: ride_logo_src(config),
        team_logos: team_logo_srcs(roster),
        riders: unique_rider_count(engine, roster),
        self_test_unverified: engine.self_test_unverified(),
        tie_note: None,
    })
}

/// Return the exporter's ride facts for *config*.
fn ride_info(config: &RideConfig) -> RideInfo {
    RideInfo {
        name: config.name.clone(),
        event_date: config.event_date,
        venue: config.venue.clone(),
        lap_km: config.lap_km,
        organizer: config.organizer.clone(),
        scorer: config.scorer.clone(),
    }
}

/// Return the publish options that *settings* carries.
fn export_options(settings: &AppSettings) -> ExportOptions {
    ExportOptions {
        show_times: settings.publish_show_times,
        laps_board: settings.publish_laps_board,
        time_board: settings.publish_time_board,
        full_field: settings.publish_full_field,
        all_cards: settings.publish_all_cards,
        ..ExportOptions::default()
    }
}

/// Return the roster's team logos as `plate -> data URI`.
///
/// A team with no logo card, or a code with no packaged asset behind it,
/// contributes no entry, so the page simply renders nothing for that row.
fn team_logo_srcs(roster: &Roster) -> BTreeMap<String, String> {
    roster
        .entries()
        .iter()
        .filter(|entry| entry.kind == EntryType::Team)
        .filter_map(|entry| {
            let key = asset_key(entry.logo_card.as_deref()?)?;
            let bytes = std::fs::read(card_asset_path(&key)).ok()?;
            Some((entry.plate.clone(), png_data_uri(&bytes)))
        })
        .collect()
}

/// Return the ride's own logo as a data URI, when one is staged and readable.
///
/// An unreadable logo falls back to the exporter's transparent PNG, so a
/// missing file never fails an export.
fn ride_logo_src(config: &RideConfig) -> Option<String> {
    let bytes = std::fs::read(config.logo_path.as_deref()?).ok()?;
    Some(png_data_uri(&bytes))
}

/// Return the path of the bitmap for asset *key*.
fn card_asset_path(key: &str) -> PathBuf {
    Path::new(CARD_ASSETS_DIR).join(format!("{key}{LOGO_SCALE}.png"))
}

/// Return the bitmap filename stem for the stored card *code*.
///
/// The stored form is upper case with the ten spelled `10` (`"AS"`, `"10D"`,
/// `"JK"`), while the assets use a lower-case suit (`"As"`, `"10d"`,
/// `"joker"`). A code that names no card returns `None`.
fn asset_key(code: &str) -> Option<String> {
    if code == JOKER_CODE {
        return Some(JOKER_KEY.to_owned());
    }
    let last = code.chars().next_back()?;
    let rank = code.strip_suffix(last)?;
    let suit = last.to_ascii_lowercase();
    let known = ASSET_RANKS.contains(&rank) && matches!(suit, 'c' | 'd' | 'h' | 's');
    known.then(|| format!("{rank}{suit}"))
}

/// Return the header's rider count: the riders of every started entry.
fn unique_rider_count(engine: &RideEngine<Roster, SystemClock>, roster: &Roster) -> usize {
    let snapshot = engine.snapshot();
    let laps_by_key: HashMap<&str, u32> = snapshot
        .iter()
        .map(|entry| (entry.entry_id.0.as_str(), entry.laps))
        .collect();
    roster
        .entries()
        .iter()
        .filter(|entry| {
            laps_by_key
                .get(entry.key.as_str())
                .is_some_and(|laps| *laps > 0)
        })
        .map(|entry| entry.riders.len())
        .sum()
}

/// Render *input* as the self-contained results page.
///
/// # Errors
///
/// Return the exporter's own sentence when the payload or the template
/// refuses.
pub(crate) fn render_html(input: &ExportInput) -> Result<String, String> {
    render_page(input).map_err(|error| error.to_string())
}

/// Render *input* as the `WordPress` full-results fragment.
///
/// # Errors
///
/// Return the exporter's own sentence when the payload or the template
/// refuses.
pub(crate) fn render_wordpress_full(input: &ExportInput) -> Result<String, String> {
    render_wordpress(input).map_err(|error| error.to_string())
}

/// Render *input* as the `WordPress` podium fragment.
///
/// # Errors
///
/// Return the exporter's own sentence when the payload or the template
/// refuses.
pub(crate) fn render_wordpress_podium(input: &ExportInput) -> Result<String, String> {
    render_poster_wordpress(input).map_err(|error| error.to_string())
}

/// Render *input* as the self-contained podium poster page.
///
/// # Errors
///
/// Return the exporter's own sentence when the payload or the template
/// refuses.
pub(crate) fn render_poster(input: &ExportInput) -> Result<String, String> {
    poster_page(input).map_err(|error| error.to_string())
}

/// Write the results page for *input* to *path* and return the path.
///
/// # Errors
///
/// Return the render refusal, or the write refusal when the temporary file or
/// the rename fails.
pub(crate) fn write_html(input: &ExportInput, path: &Path) -> Result<PathBuf, String> {
    let page = render_html(input)?;
    write_atomic(path, page.as_bytes())?;
    Ok(path.to_path_buf())
}

/// Write the podium poster for *input* to *path* and return the path.
///
/// # Errors
///
/// Return the render refusal, or the write refusal when the temporary file or
/// the rename fails.
pub(crate) fn write_poster_html(input: &ExportInput, path: &Path) -> Result<PathBuf, String> {
    let page = render_poster(input)?;
    write_atomic(path, page.as_bytes())?;
    Ok(path.to_path_buf())
}

/// Write the results report for *input* to *path* and return the path.
///
/// # Errors
///
/// Return the PDF exporter's own sentence when the layout, the logo, the
/// serialization or the file write fails.
pub(crate) fn write_pdf(
    input: &ExportInput,
    options: &PdfOptions,
    path: &Path,
) -> Result<PathBuf, String> {
    report(&pdf_input(input, options), path).map_err(|error| error.to_string())?;
    Ok(path.to_path_buf())
}

/// Write the podium poster for *input* to *path* and return the path.
///
/// # Errors
///
/// Return the PDF exporter's own sentence when the layout, the logo, the
/// serialization or the file write fails.
pub(crate) fn write_poster_pdf(
    input: &ExportInput,
    options: &PdfOptions,
    path: &Path,
) -> Result<PathBuf, String> {
    podium_poster(&pdf_input(input, options), path).map_err(|error| error.to_string())?;
    Ok(path.to_path_buf())
}

/// Write the standings CSV for *input* to *path* and return the path.
///
/// # Errors
///
/// Return the CSV exporter's own sentence when a hand cannot be named or the
/// file write fails.
pub(crate) fn export_standings_csv(input: &ExportInput, path: &Path) -> Result<PathBuf, String> {
    rivercrossing_csvio::export_standings(&input.placed, path, input.options.show_times)
        .map_err(|error| error.to_string())?;
    Ok(path.to_path_buf())
}

/// Return the PDF exporter's input for *input* and *options*.
fn pdf_input(input: &ExportInput, options: &PdfOptions) -> PdfInput {
    PdfInput {
        export: input.clone(),
        letter: options.letter,
        created_at: options.created_at,
        logo_png: options.logo_png.clone(),
    }
}

/// Write *bytes* to *path* through a same-directory temporary file.
///
/// The rename is atomic, so a crash mid-write never leaves a truncated page
/// behind.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temporary = temporary_sibling(path)?;
    std::fs::write(&temporary, bytes)
        .map_err(|error| format!("cannot write {}: {error}", temporary.display()))?;
    std::fs::rename(&temporary, path)
        .map_err(|error| format!("cannot replace {}: {error}", path.display()))
}

/// Return *path*'s same-directory temporary sibling: its name plus `.tmp`.
fn temporary_sibling(path: &Path) -> Result<PathBuf, String> {
    let name = path
        .file_name()
        .ok_or_else(|| format!("cannot write {}: the path has no file name", path.display()))?;
    let mut temporary = name.to_os_string();
    temporary.push(".tmp");
    Ok(path.with_file_name(temporary))
}
