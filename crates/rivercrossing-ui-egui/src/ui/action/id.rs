// SPDX-License-Identifier: GPL-3.0-only

//! The frozen action ids: 50 `mi_*` names and the 3 stock ids from `design/docs-md/spec.md` §15b.

/// A frozen action id. The same value is the menu id, the automation selector and the registry key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ActionId(&'static str);

impl ActionId {
    /// Return the frozen id text.
    #[must_use]
    pub(crate) const fn as_str(self) -> &'static str {
        self.0
    }

    /// Wrap a name so the table can look it up. An unknown name is not an action.
    #[must_use]
    pub(crate) const fn from_name(name: &'static str) -> Self {
        Self(name)
    }
}

/// File ▸ Ride Library.
pub(crate) const MI_OPEN_LIBRARY: ActionId = ActionId("mi_open_library");
/// File ▸ Duplicate Ride….
pub(crate) const MI_DUPLICATE_RIDE: ActionId = ActionId("mi_duplicate_ride");
/// File ▸ Import Riders CSV….
pub(crate) const MI_IMPORT_CSV: ActionId = ActionId("mi_import_csv");
/// File ▸ Export Riders CSV….
pub(crate) const MI_EXPORT_CSV: ActionId = ActionId("mi_export_csv");
/// File ▸ Back Up Database….
pub(crate) const MI_BACKUP_NOW: ActionId = ActionId("mi_backup_now");
/// File ▸ Simulation….
pub(crate) const MI_SIMULATION: ActionId = ActionId("mi_simulation");
/// File ▸ Settings….
pub(crate) const WX_ID_PREFERENCES: ActionId = ActionId("wxID_PREFERENCES");
/// File ▸ Exit.
pub(crate) const WX_ID_EXIT: ActionId = ActionId("wxID_EXIT");

/// Ride ▸ New Ride….
pub(crate) const MI_NEW_RIDE: ActionId = ActionId("mi_new_ride");
/// Ride ▸ Edit Ride….
pub(crate) const MI_EDIT_RIDE: ActionId = ActionId("mi_edit_ride");
/// Ride ▸ Start Ride.
pub(crate) const MI_START_RIDE: ActionId = ActionId("mi_start_ride");
/// Ride ▸ Stop Ride….
pub(crate) const MI_STOP_RIDE: ActionId = ActionId("mi_stop_ride");
/// Ride ▸ Set Start Time….
pub(crate) const MI_SET_START_TIME: ActionId = ActionId("mi_set_start_time");
/// Ride ▸ Finish Ride….
pub(crate) const MI_FINISH_RIDE: ActionId = ActionId("mi_finish_ride");
/// Ride ▸ Reopen Ride.
pub(crate) const MI_REOPEN_RIDE: ActionId = ActionId("mi_reopen_ride");
/// Ride ▸ Audit Trail….
pub(crate) const MI_AUDIT_TRAIL: ActionId = ActionId("mi_audit_trail");
/// Ride ▸ Clear Ride….
pub(crate) const MI_CLEAR_RIDE: ActionId = ActionId("mi_clear_ride");

/// Riders ▸ Rider Editor.
pub(crate) const MI_RIDER_EDITOR: ActionId = ActionId("mi_rider_editor");
/// Riders ▸ Teams Editor.
pub(crate) const MI_TEAM_EDITOR: ActionId = ActionId("mi_team_editor");
/// Riders ▸ Check for Rider Issues….
pub(crate) const MI_CHECK_RIDER_ISSUES: ActionId = ActionId("mi_check_rider_issues");
/// Riders ▸ Mark DNF….
pub(crate) const MI_MARK_DNF: ActionId = ActionId("mi_mark_dnf");

/// Cards ▸ Undo Last Crossing.
pub(crate) const MI_UNDO_CROSSING: ActionId = ActionId("mi_undo_crossing");
/// Cards ▸ Add Crossing at Time….
pub(crate) const MI_ADD_CROSSING_AT: ActionId = ActionId("mi_add_crossing_at");
/// Cards ▸ Deal Bonus Card….
pub(crate) const MI_DEAL_MANUAL: ActionId = ActionId("mi_deal_manual");

/// Results ▸ Standings.
pub(crate) const MI_STANDINGS: ActionId = ActionId("mi_standings");
/// Results ▸ Export HTML….
pub(crate) const MI_EXPORT_HTML: ActionId = ActionId("mi_export_html");
/// Results ▸ Export PDF….
pub(crate) const MI_EXPORT_PDF: ActionId = ActionId("mi_export_pdf");
/// Results ▸ Podium Poster PDF….
pub(crate) const MI_EXPORT_POSTER: ActionId = ActionId("mi_export_poster");
/// Results ▸ Podium Poster HTML….
pub(crate) const MI_EXPORT_POSTER_HTML: ActionId = ActionId("mi_export_poster_html");
/// Results ▸ Export Standings CSV….
pub(crate) const MI_EXPORT_RESULTS_CSV: ActionId = ActionId("mi_export_results_csv");
/// Results ▸ Preview HTML in Browser.
pub(crate) const MI_PREVIEW_HTML_BROWSER: ActionId = ActionId("mi_preview_html_browser");
/// Results ▸ Preview Podium Poster HTML in Browser.
pub(crate) const MI_PREVIEW_POSTER_HTML_BROWSER: ActionId =
    ActionId("mi_preview_poster_html_browser");
/// Results ▸ Preview PDF in Browser.
pub(crate) const MI_PREVIEW_PDF_BROWSER: ActionId = ActionId("mi_preview_pdf_browser");
/// Results ▸ Publish to `WordPress`….
pub(crate) const MI_PUBLISH_WORDPRESS: ActionId = ActionId("mi_publish_wordpress");
/// Results ▸ Show lap & total times.
pub(crate) const MI_SHOW_TIMES: ActionId = ActionId("mi_show_times");
/// Results ▸ Laps leaderboard.
pub(crate) const MI_LAPS_BOARD: ActionId = ActionId("mi_laps_board");
/// Results ▸ Fastest-time leaderboard.
pub(crate) const MI_TIME_BOARD: ActionId = ActionId("mi_time_board");
/// Results ▸ Full field.
pub(crate) const MI_FULL_FIELD: ActionId = ActionId("mi_full_field");
/// Results ▸ All cards drawn.
pub(crate) const MI_ALL_CARDS: ActionId = ActionId("mi_all_cards");
/// Results ▸ Show DNS Riders.
pub(crate) const MI_SHOW_DNS_RIDERS: ActionId = ActionId("mi_show_dns_riders");

/// View ▸ Show Total Times on Crossings Panel.
pub(crate) const MI_SHOW_TOTAL_TIMES: ActionId = ActionId("mi_show_total_times");
/// View ▸ Show Lap Time on Crossings Panel.
pub(crate) const MI_SHOW_LAP_TIME: ActionId = ActionId("mi_show_lap_time");
/// View ▸ Zoom 90%.
pub(crate) const MI_ZOOM_90: ActionId = ActionId("mi_zoom_90");
/// View ▸ Zoom 100%.
pub(crate) const MI_ZOOM_100: ActionId = ActionId("mi_zoom_100");
/// View ▸ Zoom 110%.
pub(crate) const MI_ZOOM_110: ActionId = ActionId("mi_zoom_110");
/// View ▸ Zoom 120%.
pub(crate) const MI_ZOOM_120: ActionId = ActionId("mi_zoom_120");
/// View ▸ Zoom 130%.
pub(crate) const MI_ZOOM_130: ActionId = ActionId("mi_zoom_130");
/// View ▸ Zoom 140%.
pub(crate) const MI_ZOOM_140: ActionId = ActionId("mi_zoom_140");
/// View ▸ Zoom 150%.
pub(crate) const MI_ZOOM_150: ActionId = ActionId("mi_zoom_150");

/// Help ▸ User Guide.
pub(crate) const MI_USER_GUIDE: ActionId = ActionId("mi_user_guide");
/// Help ▸ Keyboard Shortcuts.
pub(crate) const MI_SHORTCUTS: ActionId = ActionId("mi_shortcuts");
/// Help ▸ Run Evaluator Self-test.
pub(crate) const MI_SELFTEST: ActionId = ActionId("mi_selftest");
/// Help ▸ About `RiverCrossing`.
pub(crate) const WX_ID_ABOUT: ActionId = ActionId("wxID_ABOUT");

#[cfg(test)]
mod tests;
