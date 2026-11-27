// SPDX-License-Identifier: GPL-3.0-only

//! The results window: the Teams/Solo notebook and the stale-export infobar.

use std::borrow::Cow;

use craft::behavior::{DisplayOrder, ItemSource, SelectionChange};
use craft::egui::Ui;
use craft::theme::variant::Tone;
use craft::ui::contract::{CollectionEvent, Show as _, ShowCollection as _};
use craft::ui::display::Text;
use craft::ui::tabs::{Tab, TabStrip, TabStripOutput};

use super::dialogs::{
    HAND, KIND, LAP, NAME, PLACE, PLATE, TOTAL, TableData, TableRow, columns, notice, table,
};
use crate::ui::presenters::results::StandingsRowModel;
use crate::ui::state::AppState;
use crate::ui::state::exports::ResultsTab;

/// The standings rows' visible height before the table scrolls.
const VISIBLE_ROWS: u16 = 8;

/// Show the standings notebook.
pub(crate) fn body(ui: &mut Ui, state: &mut AppState) {
    let headline = state.results_headline();
    let _ = Text::new(headline).show(ui);
    if let Some(error) = state.results.error.clone() {
        notice(ui, "results_error", Tone::Bad, error);
    }
    let Some(model) = state.results.model.clone() else {
        return;
    };
    let solo_only = state
        .ride
        .as_ref()
        .is_some_and(|ride| ride.config.entry_mode == rivercrossing_ride::EntryMode::Solo);
    if solo_only {
        let data = standings(&model.solo);
        let _ = table(
            ui,
            "standings_list",
            &data,
            VISIBLE_ROWS,
            "No standings",
            None,
        );
    } else {
        notebook(ui, state);
        match state.results.tab {
            ResultsTab::Teams => {
                let data = standings(&model.teams);
                let _ = table(
                    ui,
                    "teams_standings_list",
                    &data,
                    VISIBLE_ROWS,
                    "No standings",
                    None,
                );
            }
            ResultsTab::Solo => {
                let data = standings(&model.solo);
                let _ = table(
                    ui,
                    "solo_standings_list",
                    &data,
                    VISIBLE_ROWS,
                    "No standings",
                    None,
                );
            }
        }
    }
    if model.stale {
        notice(
            ui,
            "stale_infobar",
            Tone::Warn,
            "A correction landed after the last export; export again.",
        );
    }
}

/// Show the Teams/Solo notebook under its frozen id.
fn notebook(ui: &mut Ui, state: &mut AppState) {
    let shown = tab_index(state.results.tab);
    let out = ui
        .push_id("results_notebook", |ui| {
            TabStrip::new("Standings notebook")
                .shown(shown)
                .show_items(ui, &TABS, &mut |key, _| {
                    Tab::new(tab_title(*key)).closable(false)
                })
        })
        .inner;
    if let Some(picked) = tab_pick(&out) {
        state.results.tab = tab_of(picked);
    }
}

/// Return the tab the user picked: a selection change, an activation, or the overflow menu's pick.
fn tab_pick(out: &TabStripOutput<u8>) -> Option<u8> {
    out.response
        .events
        .iter()
        .find_map(|event| match event {
            CollectionEvent::Selection(SelectionChange::Set(keys)) => keys.last().copied(),
            CollectionEvent::Activate(key) => Some(*key),
            _ => None,
        })
        .or(out.picked)
}

/// Return the tab key the displayed tab shows.
fn tab_index(tab: ResultsTab) -> u8 {
    match tab {
        ResultsTab::Teams => 0,
        ResultsTab::Solo => 1,
    }
}

/// Return the tab for a picked key.
fn tab_of(key: u8) -> ResultsTab {
    if key == 0 {
        ResultsTab::Teams
    } else {
        ResultsTab::Solo
    }
}

/// Return a tab's title.
fn tab_title(key: u8) -> &'static str {
    if key == 0 { "Teams" } else { "Solo" }
}

/// The results notebook's two tabs as a keyed collection source.
#[derive(Clone, Copy, Debug, Default)]
struct Tabs;

impl DisplayOrder<u8> for Tabs {
    fn len(&self) -> usize {
        2
    }

    fn key(&self, at: usize) -> u8 {
        u8::from(at != 0)
    }

    fn index_of(&self, key: &u8) -> Option<usize> {
        match key {
            0 => Some(0),
            1 => Some(1),
            _ => None,
        }
    }

    fn text(&self, at: usize) -> Option<Cow<'_, str>> {
        Some(Cow::Borrowed(tab_title(u8::from(at != 0))))
    }
}

impl ItemSource<u8> for Tabs {}

/// The shared results notebook source.
const TABS: Tabs = Tabs;

/// Return one section's rows as a table.
fn standings(rows: &[StandingsRowModel]) -> TableData {
    TableData {
        columns: columns(&[
            (PLACE, "Place"),
            (PLATE, "Plate"),
            (NAME, "Entry"),
            (KIND, "Type"),
            (LAP, "Laps"),
            (HAND, "Hand"),
            (TOTAL, "Total"),
        ]),
        rows: rows
            .iter()
            .map(|row| TableRow {
                label: row.name.clone(),
                // The label carries the entry's name into its column.
                cells: vec![
                    (PLACE, place_text(row)),
                    (PLATE, row.plate.clone()),
                    (KIND, kind_text(row).to_owned()),
                    (LAP, row.laps.to_string()),
                    (HAND, row.hand.clone()),
                    (TOTAL, duration_text(row.total_time)),
                ],
            })
            .collect(),
    }
}

/// Return the Place cell: the number, or the DNS marker.
fn place_text(row: &StandingsRowModel) -> String {
    row.place
        .map_or_else(|| "DNS".to_owned(), |place| place.to_string())
}

/// Return the Type cell.
fn kind_text(row: &StandingsRowModel) -> &'static str {
    match row.kind {
        rivercrossing_ride::SnapshotKind::Team => "Team",
        rivercrossing_ride::SnapshotKind::Solo => "Solo",
    }
}

/// Return *seconds* as `H:MM:SS`, zero when no crossing credited a time.
fn duration_text(seconds: f64) -> String {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the board shows whole seconds of a non-negative ride clock"
    )]
    let whole = seconds.max(0.0) as u64;
    format!(
        "{}:{:02}:{:02}",
        whole / 3600,
        whole % 3600 / 60,
        whole % 60
    )
}
