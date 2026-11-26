# RiverCrossing — frozen windows, controls and actions

## 1. Purpose, count and naming

This document is the reviewed snapshot of the frozen §15b control registry and the §15 action
table for the Rust port. The Rust table in `crates/rivercrossing-ui-egui/src/ui/` is canonical;
this document is the reviewed snapshot, and `cargo nextest run -P unit` fails when the two drift.
Rename a control or an action in only one place and the drift test names the first mismatch.

**The count is 30 top-level windows.** The §15b file map enumerates them (`design/docs-md/spec.md:392`),
and `design/README-python.md:30` and `:38` declare the count authoritative at W15. The registry holds
**53 actions**: the 45 §15 rows with the one View row expanded into its 9 items, which is 50 `mi_*`
items plus the 3 stock ids `wxID_PREFERENCES`, `wxID_EXIT` and `wxID_ABOUT`. The `README-python.md` prose
count of 43 `mi_*` items is a stale Python figure; the registry is authoritative.

**Screenshot caveat.** `design/screenshots/windows/` holds the 23 JPGs of the EPIC-1 screenshot set.
Five of them show retired windows (`stop_confirm_dlg.jpg`, `finish_confirm_dlg.jpg`,
`continue_or_new_dlg.jpg`, `reassign_dlg.jpg`, `entry_detail_dlg.jpg`). `results_frame.jpg` is the stale
screenshot of `results_dlg`, kept for layout only. Twelve current windows have no screenshot at all.
The screenshots are HTML approximations, never the native chrome.

**Nine retired windows** are named here and excluded from the count: `stop_confirm_dlg`, `no_ride_dlg`,
`finish_confirm_dlg`, `exit_confirm_dlg`, `duplicate_ride_dlg`, `reopen_ride_dlg`, `reassign_dlg`,
`continue_or_new_dlg` and `entry_detail_dlg`.

**`results_frame.jpg` maps to `results_dlg`.** The batch renamed the modeless `wxFrame` to the modal
`wxDialog` and kept the old image as a layout reference only.

**The twelve windows with no screenshot:** `add_rider_dlg`, `team_editor_dlg`, `add_team_dlg`,
`rider_issues_dlg`, `start_blocked_dlg`, `void_card_confirm_dlg`, `crossing_detail_dlg`,
`crossing_number_dlg`, `simulation_dlg`, `sim_running_dlg`, `publish_wordpress_dlg` and
`publish_running_dlg`.

**Naming rules** (§15b):

- Control names are snake_case, suffixed by role: `_frame`, `_dlg`, `_panel`, `_lbl`, `_input`, `_btn`,
  `_chk`, `_radio`, `_choice`, `_list`, `_picker`, `_spin`, `_infobar`, `_value`.
- Menu items are `mi_<action>`.
- Standard dialog buttons use stock ids (`wxID_OK`, `wxID_CANCEL`, `wxID_CLOSE`, `wxID_DELETE`,
  `wxID_OPEN`, `wxID_NEW`, `wxID_EXIT`, `wxID_ABOUT`). A stock id is an id, not a frozen control name,
  so the machine block lists no stock id as a control.
- Names are unique within one top-level window. A name may repeat across windows, and several do:
  `plate_input`, `reason_input`, `message_lbl`, `cancel_btn`, `progress_gauge`, `edit_btn`,
  `delete_btn`, `name_input`, `notes_input`, `relay_plate_input`, `first_name_input`,
  `last_name_input`, `team_choice`, `time_picker`, `start_time_picker` and `entry_lbl`.
- The `windows` map in the machine block is written in sorted key order, because the serialiser sorts
  object keys. The control array inside each window keeps document order.

## 2. The reviewed machine block

The block below is generated from the Rust registry. `src/ui/action/drift.rs` parses it, and
`test_ids_drift_registry_matches_gui_windows` asserts each part against the compiled table.

```json
{
  "actions": [
    {
      "id": "mi_open_library",
      "label": "Ride Library",
      "shortcut": null,
      "target": {
        "kind": "window",
        "window": "ride_library_dlg"
      }
    },
    {
      "id": "mi_duplicate_ride",
      "label": "Duplicate Ride…",
      "shortcut": null,
      "target": {
        "dialog": "prompt",
        "kind": "native",
        "then": "duplicate_ride"
      }
    },
    {
      "id": "mi_import_csv",
      "label": "Import Riders CSV…",
      "shortcut": null,
      "target": {
        "kind": "window",
        "window": "csv_preview_dlg"
      }
    },
    {
      "id": "mi_export_csv",
      "label": "Export Riders CSV…",
      "shortcut": null,
      "target": {
        "command": "export_roster_csv",
        "kind": "command"
      }
    },
    {
      "id": "mi_backup_now",
      "label": "Back Up Database…",
      "shortcut": null,
      "target": {
        "command": "backup_now",
        "kind": "command"
      }
    },
    {
      "id": "mi_simulation",
      "label": "Simulation…",
      "shortcut": null,
      "target": {
        "kind": "window",
        "window": "simulation_dlg"
      }
    },
    {
      "id": "wxID_PREFERENCES",
      "label": "Settings…",
      "shortcut": null,
      "target": {
        "kind": "window",
        "window": "settings_dlg"
      }
    },
    {
      "id": "wxID_EXIT",
      "label": "Exit",
      "shortcut": null,
      "target": {
        "dialog": "confirm",
        "kind": "native",
        "then": "app_quit"
      }
    },
    {
      "id": "mi_new_ride",
      "label": "New Ride…",
      "shortcut": null,
      "target": {
        "kind": "window",
        "window": "ride_setup_dlg"
      }
    },
    {
      "id": "mi_edit_ride",
      "label": "Edit Ride…",
      "shortcut": null,
      "target": {
        "kind": "window",
        "window": "ride_setup_dlg"
      }
    },
    {
      "id": "mi_start_ride",
      "label": "Start Ride",
      "shortcut": null,
      "target": {
        "command": "start_ride",
        "kind": "command"
      }
    },
    {
      "id": "mi_stop_ride",
      "label": "Stop Ride…",
      "shortcut": null,
      "target": {
        "dialog": "confirm",
        "kind": "native",
        "then": "stop_ride"
      }
    },
    {
      "id": "mi_set_start_time",
      "label": "Set Start Time…",
      "shortcut": null,
      "target": {
        "kind": "window",
        "window": "set_start_dlg"
      }
    },
    {
      "id": "mi_finish_ride",
      "label": "Finish Ride…",
      "shortcut": null,
      "target": {
        "dialog": "danger",
        "kind": "native",
        "then": "finish_ride"
      }
    },
    {
      "id": "mi_reopen_ride",
      "label": "Reopen Ride",
      "shortcut": null,
      "target": {
        "dialog": "prompt",
        "kind": "native",
        "then": "reopen_ride"
      }
    },
    {
      "id": "mi_audit_trail",
      "label": "Audit Trail…",
      "shortcut": null,
      "target": {
        "kind": "window",
        "window": "audit_dlg"
      }
    },
    {
      "id": "mi_clear_ride",
      "label": "Clear Ride…",
      "shortcut": null,
      "target": {
        "dialog": "danger",
        "kind": "native",
        "then": "clear_ride"
      }
    },
    {
      "id": "mi_rider_editor",
      "label": "Rider Editor",
      "shortcut": null,
      "target": {
        "kind": "window",
        "window": "rider_editor_dlg"
      }
    },
    {
      "id": "mi_team_editor",
      "label": "Teams Editor",
      "shortcut": null,
      "target": {
        "kind": "window",
        "window": "team_editor_dlg"
      }
    },
    {
      "id": "mi_check_rider_issues",
      "label": "Check for Rider Issues…",
      "shortcut": null,
      "target": {
        "kind": "window",
        "window": "rider_issues_dlg"
      }
    },
    {
      "id": "mi_mark_dnf",
      "label": "Mark DNF…",
      "shortcut": null,
      "target": {
        "kind": "window",
        "window": "dnf_confirm_dlg"
      }
    },
    {
      "id": "mi_undo_crossing",
      "label": "Undo Last Crossing",
      "shortcut": {
        "command": true,
        "key": "z"
      },
      "target": {
        "dialog": "danger",
        "kind": "native",
        "then": "undo_crossing"
      }
    },
    {
      "id": "mi_add_crossing_at",
      "label": "Add Crossing at Time…",
      "shortcut": null,
      "target": {
        "kind": "window",
        "window": "edit_crossing_dlg"
      }
    },
    {
      "id": "mi_deal_manual",
      "label": "Deal Bonus Card…",
      "shortcut": null,
      "target": {
        "kind": "window",
        "window": "manual_deal_dlg"
      }
    },
    {
      "id": "mi_standings",
      "label": "Standings",
      "shortcut": {
        "command": false,
        "key": "f5"
      },
      "target": {
        "kind": "window",
        "window": "results_dlg"
      }
    },
    {
      "id": "mi_export_html",
      "label": "Export HTML…",
      "shortcut": null,
      "target": {
        "command": "export_html",
        "kind": "command"
      }
    },
    {
      "id": "mi_export_pdf",
      "label": "Export PDF…",
      "shortcut": null,
      "target": {
        "command": "export_pdf",
        "kind": "command"
      }
    },
    {
      "id": "mi_export_poster",
      "label": "Podium Poster PDF…",
      "shortcut": null,
      "target": {
        "command": "export_poster",
        "kind": "command"
      }
    },
    {
      "id": "mi_export_poster_html",
      "label": "Podium Poster HTML…",
      "shortcut": null,
      "target": {
        "command": "export_poster_html",
        "kind": "command"
      }
    },
    {
      "id": "mi_export_results_csv",
      "label": "Export Standings CSV…",
      "shortcut": null,
      "target": {
        "command": "export_csv",
        "kind": "command"
      }
    },
    {
      "id": "mi_preview_html_browser",
      "label": "Preview HTML in Browser",
      "shortcut": null,
      "target": {
        "command": "preview_html",
        "kind": "command"
      }
    },
    {
      "id": "mi_preview_poster_html_browser",
      "label": "Preview Podium Poster HTML in Browser",
      "shortcut": null,
      "target": {
        "command": "preview_poster_html",
        "kind": "command"
      }
    },
    {
      "id": "mi_preview_pdf_browser",
      "label": "Preview PDF in Browser",
      "shortcut": null,
      "target": {
        "command": "preview_pdf",
        "kind": "command"
      }
    },
    {
      "id": "mi_publish_wordpress",
      "label": "Publish to WordPress…",
      "shortcut": null,
      "target": {
        "kind": "window",
        "window": "publish_wordpress_dlg"
      }
    },
    {
      "id": "mi_show_times",
      "label": "Show lap & total times",
      "shortcut": null,
      "target": {
        "check": "publish_show_times",
        "kind": "check"
      }
    },
    {
      "id": "mi_laps_board",
      "label": "Laps leaderboard",
      "shortcut": null,
      "target": {
        "check": "laps_board",
        "kind": "check"
      }
    },
    {
      "id": "mi_time_board",
      "label": "Fastest-time leaderboard",
      "shortcut": null,
      "target": {
        "check": "time_board",
        "kind": "check"
      }
    },
    {
      "id": "mi_full_field",
      "label": "Full field",
      "shortcut": null,
      "target": {
        "check": "full_field",
        "kind": "check"
      }
    },
    {
      "id": "mi_all_cards",
      "label": "All cards drawn",
      "shortcut": null,
      "target": {
        "check": "all_cards",
        "kind": "check"
      }
    },
    {
      "id": "mi_show_dns_riders",
      "label": "Show DNS Riders",
      "shortcut": null,
      "target": {
        "check": "show_dns_riders",
        "kind": "check"
      }
    },
    {
      "id": "mi_show_total_times",
      "label": "Show Total Times on Crossings Panel",
      "shortcut": null,
      "target": {
        "check": "show_total_times",
        "kind": "check"
      }
    },
    {
      "id": "mi_show_lap_time",
      "label": "Show Lap Time on Crossings Panel",
      "shortcut": null,
      "target": {
        "check": "show_lap_time",
        "kind": "check"
      }
    },
    {
      "id": "mi_zoom_90",
      "label": "90%",
      "shortcut": null,
      "target": {
        "kind": "zoom",
        "percent": 90
      }
    },
    {
      "id": "mi_zoom_100",
      "label": "100%",
      "shortcut": null,
      "target": {
        "kind": "zoom",
        "percent": 100
      }
    },
    {
      "id": "mi_zoom_110",
      "label": "110%",
      "shortcut": null,
      "target": {
        "kind": "zoom",
        "percent": 110
      }
    },
    {
      "id": "mi_zoom_120",
      "label": "120%",
      "shortcut": null,
      "target": {
        "kind": "zoom",
        "percent": 120
      }
    },
    {
      "id": "mi_zoom_130",
      "label": "130%",
      "shortcut": null,
      "target": {
        "kind": "zoom",
        "percent": 130
      }
    },
    {
      "id": "mi_zoom_140",
      "label": "140%",
      "shortcut": null,
      "target": {
        "kind": "zoom",
        "percent": 140
      }
    },
    {
      "id": "mi_zoom_150",
      "label": "150%",
      "shortcut": null,
      "target": {
        "kind": "zoom",
        "percent": 150
      }
    },
    {
      "id": "mi_user_guide",
      "label": "User Guide",
      "shortcut": {
        "command": false,
        "key": "f1"
      },
      "target": {
        "command": "open_user_guide",
        "kind": "command"
      }
    },
    {
      "id": "mi_shortcuts",
      "label": "Keyboard Shortcuts",
      "shortcut": null,
      "target": {
        "kind": "window",
        "window": "shortcuts_dlg"
      }
    },
    {
      "id": "mi_selftest",
      "label": "Run Evaluator Self-test",
      "shortcut": null,
      "target": {
        "kind": "window",
        "window": "selftest_dlg"
      }
    },
    {
      "id": "wxID_ABOUT",
      "label": "About RiverCrossing",
      "shortcut": null,
      "target": {
        "kind": "window",
        "window": "about_dlg"
      }
    }
  ],
  "commands": [
    "app_quit"
  ],
  "fields": [
    {
      "id": "plate_input",
      "kind": "text",
      "window": "main_frame"
    },
    {
      "id": "show_held_only_chk",
      "kind": "check",
      "window": "main_frame"
    },
    {
      "id": "name_input",
      "kind": "text",
      "window": "ride_setup_dlg"
    },
    {
      "id": "date_picker",
      "kind": "text",
      "window": "ride_setup_dlg"
    },
    {
      "id": "start_time_picker",
      "kind": "text",
      "window": "ride_setup_dlg"
    },
    {
      "id": "venue_input",
      "kind": "text",
      "window": "ride_setup_dlg"
    },
    {
      "id": "lap_km_spin",
      "kind": "spin",
      "window": "ride_setup_dlg"
    },
    {
      "id": "organizer_input",
      "kind": "text",
      "window": "ride_setup_dlg"
    },
    {
      "id": "scorer_input",
      "kind": "text",
      "window": "ride_setup_dlg"
    },
    {
      "id": "duration_input",
      "kind": "text",
      "window": "ride_setup_dlg"
    },
    {
      "id": "min_lap_input",
      "kind": "text",
      "window": "ride_setup_dlg"
    },
    {
      "id": "team_size_spin",
      "kind": "spin",
      "window": "ride_setup_dlg"
    },
    {
      "id": "decks_spin",
      "kind": "spin",
      "window": "ride_setup_dlg"
    },
    {
      "id": "jokers_spin",
      "kind": "spin",
      "window": "ride_setup_dlg"
    },
    {
      "id": "cap_choice",
      "kind": "choice",
      "window": "ride_setup_dlg"
    },
    {
      "id": "start_date_picker",
      "kind": "text",
      "window": "set_start_dlg"
    },
    {
      "id": "start_time_picker",
      "kind": "text",
      "window": "set_start_dlg"
    },
    {
      "id": "plate_input",
      "kind": "text",
      "window": "rider_editor_dlg"
    },
    {
      "id": "first_name_input",
      "kind": "text",
      "window": "rider_editor_dlg"
    },
    {
      "id": "last_name_input",
      "kind": "text",
      "window": "rider_editor_dlg"
    },
    {
      "id": "team_choice",
      "kind": "choice",
      "window": "rider_editor_dlg"
    },
    {
      "id": "plate_input",
      "kind": "text",
      "window": "add_rider_dlg"
    },
    {
      "id": "first_name_input",
      "kind": "text",
      "window": "add_rider_dlg"
    },
    {
      "id": "last_name_input",
      "kind": "text",
      "window": "add_rider_dlg"
    },
    {
      "id": "team_choice",
      "kind": "choice",
      "window": "add_rider_dlg"
    },
    {
      "id": "sex_choice",
      "kind": "choice",
      "window": "add_rider_dlg"
    },
    {
      "id": "map_unknown_sex_chk",
      "kind": "check",
      "window": "csv_preview_dlg"
    },
    {
      "id": "convert_teams_of_one_chk",
      "kind": "check",
      "window": "csv_preview_dlg"
    },
    {
      "id": "single_member_only_chk",
      "kind": "check",
      "window": "team_editor_dlg"
    },
    {
      "id": "name_input",
      "kind": "text",
      "window": "team_editor_dlg"
    },
    {
      "id": "relay_plate_input",
      "kind": "text",
      "window": "team_editor_dlg"
    },
    {
      "id": "notes_input",
      "kind": "text",
      "window": "team_editor_dlg"
    },
    {
      "id": "name_input",
      "kind": "text",
      "window": "add_team_dlg"
    },
    {
      "id": "relay_plate_input",
      "kind": "text",
      "window": "add_team_dlg"
    },
    {
      "id": "notes_input",
      "kind": "text",
      "window": "add_team_dlg"
    },
    {
      "id": "plate_input",
      "kind": "text",
      "window": "edit_crossing_dlg"
    },
    {
      "id": "time_picker",
      "kind": "text",
      "window": "edit_crossing_dlg"
    },
    {
      "id": "reason_input",
      "kind": "text",
      "window": "edit_crossing_dlg"
    },
    {
      "id": "plate_input",
      "kind": "text",
      "window": "manual_deal_dlg"
    },
    {
      "id": "reason_input",
      "kind": "text",
      "window": "manual_deal_dlg"
    },
    {
      "id": "plate_input",
      "kind": "text",
      "window": "dnf_confirm_dlg"
    },
    {
      "id": "reason_input",
      "kind": "text",
      "window": "dnf_confirm_dlg"
    },
    {
      "id": "reason_input",
      "kind": "text",
      "window": "void_card_confirm_dlg"
    },
    {
      "id": "confirm_name_input",
      "kind": "text",
      "window": "delete_ride_dlg"
    },
    {
      "id": "action_choice",
      "kind": "choice",
      "window": "audit_dlg"
    },
    {
      "id": "sound_chk",
      "kind": "check",
      "window": "settings_dlg"
    },
    {
      "id": "show_total_times_chk",
      "kind": "check",
      "window": "settings_dlg"
    },
    {
      "id": "show_lap_time_chk",
      "kind": "check",
      "window": "settings_dlg"
    },
    {
      "id": "verbose_log_chk",
      "kind": "check",
      "window": "settings_dlg"
    },
    {
      "id": "avg_speed_spin",
      "kind": "spin",
      "window": "settings_dlg"
    },
    {
      "id": "number_input",
      "kind": "text",
      "window": "crossing_number_dlg"
    },
    {
      "id": "riders_spin",
      "kind": "spin",
      "window": "simulation_dlg"
    },
    {
      "id": "teams_spin",
      "kind": "spin",
      "window": "simulation_dlg"
    },
    {
      "id": "solo_spin",
      "kind": "spin",
      "window": "simulation_dlg"
    },
    {
      "id": "laps_spin",
      "kind": "spin",
      "window": "simulation_dlg"
    },
    {
      "id": "interval_spin",
      "kind": "spin",
      "window": "simulation_dlg"
    },
    {
      "id": "short_lap_choice",
      "kind": "choice",
      "window": "simulation_dlg"
    },
    {
      "id": "lapped_choice",
      "kind": "choice",
      "window": "simulation_dlg"
    },
    {
      "id": "team_stop_choice",
      "kind": "choice",
      "window": "simulation_dlg"
    },
    {
      "id": "wp_kind_choice",
      "kind": "choice",
      "window": "publish_wordpress_dlg"
    },
    {
      "id": "wp_url_input",
      "kind": "text",
      "window": "publish_wordpress_dlg"
    },
    {
      "id": "wp_username_input",
      "kind": "text",
      "window": "publish_wordpress_dlg"
    },
    {
      "id": "wp_password_input",
      "kind": "text",
      "window": "publish_wordpress_dlg"
    },
    {
      "id": "wp_title_input",
      "kind": "text",
      "window": "publish_wordpress_dlg"
    },
    {
      "id": "wp_slug_input",
      "kind": "text",
      "window": "publish_wordpress_dlg"
    },
    {
      "id": "wp_parent_input",
      "kind": "text",
      "window": "publish_wordpress_dlg"
    },
    {
      "id": "wp_status_choice",
      "kind": "choice",
      "window": "publish_wordpress_dlg"
    }
  ],
  "windows": {
    "about_dlg": [
      "version_lbl",
      "author_lbl",
      "copyright_lbl",
      "license_lbl",
      "gorba_link"
    ],
    "add_rider_dlg": [
      "plate_input",
      "first_name_input",
      "last_name_input",
      "team_choice",
      "sex_choice",
      "add_rider_infobar"
    ],
    "add_team_dlg": [
      "name_input",
      "relay_plate_input",
      "notes_input",
      "logo_bmp",
      "pick_card_btn",
      "add_team_infobar"
    ],
    "audit_dlg": [
      "audit_search",
      "action_choice",
      "audit_list"
    ],
    "crossing_detail_dlg": [
      "crossing_rider_lbl",
      "crossing_team_lbl",
      "crossing_plate_lbl",
      "crossing_lap_lbl",
      "crossing_time_lbl",
      "crossing_lap_time_lbl",
      "crossing_total_lbl",
      "crossing_card_lbl",
      "crossing_held_lbl",
      "edit_btn",
      "edit_time_btn",
      "void_card_btn",
      "delete_btn",
      "crossing_detail_infobar"
    ],
    "crossing_number_dlg": [
      "number_input"
    ],
    "csv_preview_dlg": [
      "summary_lbl",
      "conflicts_list",
      "map_unknown_sex_chk",
      "convert_teams_of_one_chk",
      "csv_infobar"
    ],
    "delete_ride_dlg": [
      "message_lbl",
      "confirm_name_input"
    ],
    "dnf_confirm_dlg": [
      "plate_input",
      "entry_lbl",
      "reason_input"
    ],
    "edit_crossing_dlg": [
      "plate_input",
      "time_picker",
      "reason_input",
      "void_btn"
    ],
    "exit_running_dlg": [
      "finish_first_btn",
      "message_lbl"
    ],
    "main_frame": [
      "main_menubar",
      "ride_status_panel",
      "ride_status_light",
      "ride_status_lbl",
      "ride_name_value",
      "ride_date_value",
      "ride_venue_value",
      "ride_organizer_value",
      "ride_scorer_value",
      "ride_lap_km_value",
      "ride_logo_bmp",
      "current_lap_lbl",
      "elapsed_clock_panel",
      "remaining_clock_panel",
      "elapsed_clock",
      "remaining_clock",
      "clock_elapsed_lbl",
      "clock_remaining_lbl",
      "start_btn",
      "stop_btn",
      "plate_input",
      "record_btn",
      "last_crossing_lbl",
      "undo_btn",
      "crossings_search",
      "main_splitter",
      "crossings_list",
      "crossings_count_lbl",
      "cards_count_lbl",
      "on_course_lbl",
      "shoe_lbl",
      "riders_count_lbl",
      "teams_count_lbl",
      "review_notebook",
      "console_riders_list",
      "flagged_list",
      "show_held_only_chk",
      "review_btn",
      "main_statusbar"
    ],
    "manual_deal_dlg": [
      "plate_input",
      "reason_input"
    ],
    "publish_running_dlg": [
      "publish_status_lbl",
      "progress_gauge",
      "cancel_btn"
    ],
    "publish_wordpress_dlg": [
      "wp_kind_choice",
      "wp_url_input",
      "wp_username_input",
      "wp_password_input",
      "wp_title_input",
      "wp_slug_input",
      "wp_parent_input",
      "wp_status_choice"
    ],
    "results_dlg": [
      "results_notebook",
      "teams_standings_list",
      "solo_standings_list",
      "standings_list",
      "stale_infobar"
    ],
    "resume_dlg": [
      "message_lbl",
      "continue_btn",
      "library_btn"
    ],
    "ride_library_dlg": [
      "rides_list",
      "duplicate_btn"
    ],
    "ride_setup_dlg": [
      "name_input",
      "date_picker",
      "start_time_picker",
      "venue_input",
      "lap_km_spin",
      "organizer_input",
      "scorer_input",
      "duration_input",
      "min_lap_input",
      "hold_short_radio",
      "always_deal_radio",
      "tiebreak_list",
      "logo_preview_bmp",
      "logo_status_lbl",
      "logo_browse_btn",
      "solo_radio",
      "mixed_radio",
      "team_size_spin",
      "pooled_radio",
      "relay_radio",
      "decks_spin",
      "jokers_spin",
      "jokers_per_deck_radio",
      "jokers_total_radio",
      "cap_choice",
      "setup_infobar"
    ],
    "rider_editor_dlg": [
      "rider_search",
      "riders_list",
      "add_btn",
      "delete_btn",
      "plate_input",
      "first_name_input",
      "last_name_input",
      "team_choice",
      "edit_btn",
      "roster_infobar"
    ],
    "rider_issues_dlg": [
      "card_check_lbl",
      "issues_summary_lbl",
      "issues_list",
      "open_editor_btn",
      "convert_solo_btn",
      "assign_plate_btn",
      "renumber_btn",
      "issues_infobar"
    ],
    "selftest_dlg": [
      "selftest_output",
      "rerun_btn"
    ],
    "set_start_dlg": [
      "start_date_picker",
      "start_time_picker"
    ],
    "settings_dlg": [
      "appearance_system_radio",
      "appearance_light_radio",
      "appearance_dark_radio",
      "sound_chk",
      "show_total_times_chk",
      "show_lap_time_chk",
      "verbose_log_chk",
      "avg_speed_spin"
    ],
    "shortcuts_dlg": [
      "shortcuts_list"
    ],
    "sim_running_dlg": [
      "sim_status_lbl",
      "progress_gauge",
      "cancel_btn"
    ],
    "simulation_dlg": [
      "riders_spin",
      "teams_spin",
      "solo_spin",
      "laps_spin",
      "interval_spin",
      "short_lap_choice",
      "lapped_choice",
      "team_stop_choice",
      "gen_riders_btn",
      "check_btn",
      "go_btn",
      "sim_infobar"
    ],
    "start_blocked_dlg": [
      "start_blocked_list"
    ],
    "team_editor_dlg": [
      "teams_list",
      "single_member_only_chk",
      "edit_btn",
      "remove_btn",
      "name_input",
      "relay_plate_input",
      "notes_input",
      "members_list",
      "add_btn",
      "teams_infobar"
    ],
    "void_card_confirm_dlg": [
      "card_lbl",
      "entry_lbl",
      "reason_input"
    ]
  }
}
```

## 3. Windows, controls and screenshots

| Window | Controls | Screenshot |
|---|---|---|
| `main_frame` | `main_menubar`, `ride_status_panel`, `ride_status_light`, `ride_status_lbl`, `ride_name_value`, `ride_date_value`, `ride_venue_value`, `ride_organizer_value`, `ride_scorer_value`, `ride_lap_km_value`, `ride_logo_bmp`, `current_lap_lbl`, `elapsed_clock_panel`, `remaining_clock_panel`, `elapsed_clock`, `remaining_clock`, `clock_elapsed_lbl`, `clock_remaining_lbl`, `start_btn`, `stop_btn`, `plate_input`, `record_btn`, `last_crossing_lbl`, `undo_btn`, `crossings_search`, `main_splitter`, `crossings_list`, `crossings_count_lbl`, `cards_count_lbl`, `on_course_lbl`, `shoe_lbl`, `riders_count_lbl`, `teams_count_lbl`, `review_notebook`, `console_riders_list`, `flagged_list`, `show_held_only_chk`, `review_btn`, `main_statusbar` | `main_frame.jpg` |
| `ride_setup_dlg` | `name_input`, `date_picker`, `start_time_picker`, `venue_input`, `lap_km_spin`, `organizer_input`, `scorer_input`, `duration_input`, `min_lap_input`, `hold_short_radio`, `always_deal_radio`, `tiebreak_list`, `logo_preview_bmp`, `logo_status_lbl`, `logo_browse_btn`, `solo_radio`, `mixed_radio`, `team_size_spin`, `pooled_radio`, `relay_radio`, `decks_spin`, `jokers_spin`, `jokers_per_deck_radio`, `jokers_total_radio`, `cap_choice`, `setup_infobar` | `ride_setup_dlg.jpg` |
| `set_start_dlg` | `start_date_picker`, `start_time_picker` | `set_start_dlg.jpg` |
| `resume_dlg` | `message_lbl`, `continue_btn`, `library_btn` | `resume_dlg.jpg` |
| `exit_running_dlg` | `finish_first_btn`, `message_lbl` | `exit_running_dlg.jpg` |
| `rider_editor_dlg` | `rider_search`, `riders_list`, `add_btn`, `delete_btn`, `plate_input`, `first_name_input`, `last_name_input`, `team_choice`, `edit_btn`, `roster_infobar` | `rider_editor_dlg.jpg` |
| `add_rider_dlg` | `plate_input`, `first_name_input`, `last_name_input`, `team_choice`, `sex_choice`, `add_rider_infobar` | no screenshot |
| `csv_preview_dlg` | `summary_lbl`, `conflicts_list`, `map_unknown_sex_chk`, `convert_teams_of_one_chk`, `csv_infobar` | `csv_preview_dlg.jpg` |
| `team_editor_dlg` | `teams_list`, `single_member_only_chk`, `edit_btn`, `remove_btn`, `name_input`, `relay_plate_input`, `notes_input`, `members_list`, `add_btn`, `teams_infobar` | no screenshot |
| `add_team_dlg` | `name_input`, `relay_plate_input`, `notes_input`, `logo_bmp`, `pick_card_btn`, `add_team_infobar` | no screenshot |
| `rider_issues_dlg` | `card_check_lbl`, `issues_summary_lbl`, `issues_list`, `open_editor_btn`, `convert_solo_btn`, `assign_plate_btn`, `renumber_btn`, `issues_infobar` | no screenshot |
| `start_blocked_dlg` | `start_blocked_list` | no screenshot |
| `edit_crossing_dlg` | `plate_input`, `time_picker`, `reason_input`, `void_btn` | `edit_crossing_dlg.jpg` |
| `manual_deal_dlg` | `plate_input`, `reason_input` | `manual_deal_dlg.jpg` |
| `dnf_confirm_dlg` | `plate_input`, `entry_lbl`, `reason_input` | `dnf_confirm_dlg.jpg` |
| `void_card_confirm_dlg` | `card_lbl`, `entry_lbl`, `reason_input` | no screenshot |
| `results_dlg` | `results_notebook`, `teams_standings_list`, `solo_standings_list`, `standings_list`, `stale_infobar` | `results_frame.jpg` (stale, layout only) |
| `ride_library_dlg` | `rides_list`, `duplicate_btn` | `ride_library_dlg.jpg` |
| `delete_ride_dlg` | `message_lbl`, `confirm_name_input` | `delete_ride_dlg.jpg` |
| `audit_dlg` | `audit_search`, `action_choice`, `audit_list` | `audit_dlg.jpg` |
| `settings_dlg` | `appearance_system_radio`, `appearance_light_radio`, `appearance_dark_radio`, `sound_chk`, `show_total_times_chk`, `show_lap_time_chk`, `verbose_log_chk`, `avg_speed_spin` | `settings_dlg.jpg` |
| `about_dlg` | `version_lbl`, `author_lbl`, `copyright_lbl`, `license_lbl`, `gorba_link` | `about_dlg.jpg` |
| `shortcuts_dlg` | `shortcuts_list` | `shortcuts_dlg.jpg` |
| `selftest_dlg` | `selftest_output`, `rerun_btn` | `selftest_dlg.jpg` |
| `crossing_detail_dlg` | `crossing_rider_lbl`, `crossing_team_lbl`, `crossing_plate_lbl`, `crossing_lap_lbl`, `crossing_time_lbl`, `crossing_lap_time_lbl`, `crossing_total_lbl`, `crossing_card_lbl`, `crossing_held_lbl`, `edit_btn`, `edit_time_btn`, `void_card_btn`, `delete_btn`, `crossing_detail_infobar` | no screenshot |
| `crossing_number_dlg` | `number_input` | no screenshot |
| `simulation_dlg` | `riders_spin`, `teams_spin`, `solo_spin`, `laps_spin`, `interval_spin`, `short_lap_choice`, `lapped_choice`, `team_stop_choice`, `gen_riders_btn`, `check_btn`, `go_btn`, `sim_infobar` | no screenshot |
| `sim_running_dlg` | `sim_status_lbl`, `progress_gauge`, `cancel_btn` | no screenshot |
| `publish_wordpress_dlg` | `wp_kind_choice`, `wp_url_input`, `wp_username_input`, `wp_password_input`, `wp_title_input`, `wp_slug_input`, `wp_parent_input`, `wp_status_choice` | no screenshot |
| `publish_running_dlg` | `publish_status_lbl`, `progress_gauge`, `cancel_btn` | no screenshot |

## 4. The §15 menu map

Copied verbatim from `design/docs-md/spec.md:328-372`: all 45 rows with their "Enabled when" column.

| Menu item | Opens / does | Design | Enabled when |
|---|---|---|---|
| **File ▸ Ride Library** | Ride library window | 1g | always |
| **File ▸ Duplicate Ride…** | Native non-destructive confirm (`ui.std_dialogs.show_prompt` — the XRC `duplicate_ride_dlg` retired in H2; names the ride, Enter-safe) → new DRAFT ride, setup + roster only, no timing data (R-15) | 3d | a ride is open |
| **File ▸ Import Riders CSV…** | OS-native picker → import report dialog | 3e | ride open (structure edits: DRAFT only) |
| **File ▸ Export Riders CSV…** | OS-native save dialog | — | ride open |
| **File ▸ Back Up Database…** | Writes one timestamped backup into `<db>.backups/` and posts the path (R-54) | — | always |
| **File ▸ Simulation…** | Rider Simulator (`simulation.xrc`) — generates a placeholder field into the open draft ride, then replays every crossing through the console's own `RideEngine.record_crossing(plate, at=…)` seam from a modal progress dialog (percent complete, cancellable) | — | DRAFT (open or none), or stopped RUNNING |
| **File ▸ Settings…** | Settings window | 3a | always |
| **File ▸ Exit** | Ride RUNNING → exit dialog (Cancel · Finish first · Quit-keep-running); otherwise → native quit confirm (`ui.std_dialogs.show_confirm`, its copy in `quit_flow.EXIT_CONFIRM_*` — the XRC `exit_confirm_dlg` retired in H2) — the app never exits without confirmation. Reopen → resume dialog | 4a · 1h | always |
| **Ride ▸ New Ride…** | Ride setup window (blank) — the single entry point to the setup window, moved here from File in D1 so one surface owns the whole ride lifecycle (the duplicate Ride ▸ Ride Setup… row left in W14: its "edit this ride" semantics were never built) | 1c | no ride open |
| **Ride ▸ Edit Ride…** | The same setup window PRELOADED with the live ride's config, titled "Edit Ride" and committing on "Save" (D2) — the settings are written back to the live engine and the database; the ride's shoe, roster and event log are untouched | 1c | a ride is open |
| **Ride ▸ Start Ride** | Starts; R-79's start gate refuses a DRAFT ride whose roster is empty or whose setup misses a minimum field — the refusal surfaces as a native warning dialog naming what is missing (W5, R-85); a stopped ride with data continues directly — the continue-vs-new dialog is retired | 1h | DRAFT, REOPENED, or stopped RUNNING |
| **Ride ▸ Stop Ride…** | Native stop confirm (`ui.std_dialogs.show_confirm` — the XRC `stop_confirm_dlg` retired in W5; the row dispatches as a command to the live presenter's `on_stop_requested`, the identical handler the console Stop button fires, so row and button cannot drift; a riderless roster gets a native warning instead; the Arm checkbox is retired in C2, so a live-RUNNING ride needs no separate arming act) | 3d | RUNNING |
| **Ride ▸ Set Start Time…** | Set-start-time dialog (lap-1 recompute) | 3d | RUNNING · REOPENED |
| **Ride ▸ Finish Ride…** | Native danger confirm (`ui.std_dialogs.show_danger` — the XRC `finish_confirm_dlg` retired in H2; "Finish again" from REOPENED); the finish route then runs the evaluator self-test once (`console.FINISH_GATE`) and, when a BLOCKING check failed, asks one more danger confirm naming every failed check ("Finish anyway" — Cancel is the safe path, R-44) → Standings window (the operator exports from the Results menu; the E2 automatic write of HTML/PDF into the per-user `exports` folder was removed in the 1.0.17 follow-up) | 3d → 1f | RUNNING · REOPENED |
| **Ride ▸ Reopen Ride** | Native non-destructive confirm (`ui.std_dialogs.show_prompt` — the XRC `reopen_ride_dlg` retired in H2; names the ride, Enter-safe) → REOPENED | 3d | FINISHED |
| **Ride ▸ Audit Trail…** | Read-only audit viewer (§3) — when · action · entry · reason, newest first, every column sortable, filterable by entry and action | — | ride open, ≥1 audit row |
| **Ride ▸ Clear Ride…** | Native danger confirm → the ride is removed from the screen only: the console returns to the no-ride empty state (Edit Ride / Clear Ride / Start Ride disable, exactly as at launch), the in-memory ride, roster and export state are dropped, and the database is NOT touched — the ride, its riders and every crossing stay saved and reopen from the library unchanged (D3) — a REOPENED ride is clearable too, clearing being the only way to unload it from memory, and it is never a delete | — | DRAFT, or stopped RUNNING, or FINISHED, or REOPENED |
| **Riders ▸ Rider Editor** | Rider editor window — a read-only roster list and record panel; Add rider… and Edit Rider… open the record's own dialog (Phase 3) | 1d / 2b | ride open |
| **Riders ▸ Teams Editor** | Teams editor window (Phase 4) — team records: name, relay plate, notes, logo card (the logo image is retired in Phase 3); membership read-only (added in the Rider Editor) | — | ride open, teams enabled (mixed) |
| **Riders ▸ Check for Rider Issues…** | `rider_issues_dlg` (R-78) — a roster-wide issue report: teams of one, missing name/number, duplicate name/number — with "Convert to Solo" for a pooled DRAFT team-of-one and "Open Editor…" jumping to the teams/rider editor for the rest | 3b | ride open |
| **Riders ▸ Mark DNF…** | DNF confirm dialog | 3b | RUNNING · REOPENED |
| **Cards ▸ Undo Last Crossing** | Native danger confirm (`ui.std_dialogs.show_danger`, I): undo is confirmed through the native danger dialog, and the direct command posts its status-bar notice only on confirm — no XRC confirm window (the status-bar-only direct command is retired) | 1a | RUNNING, ≥1 crossing |
| **Cards ▸ Add Crossing at Time…** | Edit-crossing dialog in "add" mode — plate + explicit time (a rider missed at the line) | 3c · 8c | RUNNING · REOPENED |
| **Cards ▸ Deal Bonus Card…** | Manual-deal dialog (the renamed `mi_deal_manual` row — its OK label stays "Deal") | 3c | RUNNING · REOPENED |
| **Results ▸ Standings** | Standings window (live while running) | 1f | always |
| **Results ▸ Export HTML…** | OS-native save dialog → writes file (§8) | 1f | FINISHED |
| **Results ▸ Export PDF…** | OS-native save dialog → writes file (§8b) | 1f | FINISHED |
| **Results ▸ Podium Poster PDF…** | OS-native save dialog → one-page poster (§8b) | 5d | FINISHED |
| **Results ▸ Podium Poster HTML…** | OS-native save dialog → one-page poster page — the PDF poster's HTML sibling (§8), the results page's own HTML export never satisfying it | 5d | FINISHED |
| **Results ▸ Export Standings CSV…** | OS-native save dialog → standings rows (place, plate, entry, type, sex, laps, hand, draw[, total_time]) — `draw` the entry's tie-break card code, blank on every other row | 1f | FINISHED |
| **Results ▸ Preview HTML in Browser** | Opens the last HTML export in the default browser | — | FINISHED, an HTML export exists this session |
| **Results ▸ Preview Podium Poster HTML in Browser** | Opens the last podium-poster page export in the default browser | — | FINISHED, a poster page export exists this session |
| **Results ▸ Preview PDF in Browser** | Opens the last PDF export — the results PDF or the podium poster — in the default browser | — | FINISHED, a PDF export exists this session |
| **Results ▸ Publish to WordPress…** | WordPress publish form (`dialogs.xrc`) — a **What to publish** choice (Full results / Podium results) first, then the site and page fields, prefilled from the stored settings and the live ride; the choice auto-fills the title and slug. OK persists the site, renders the chosen fragment (`htmlexport.render_wordpress` for full results, `htmlexport.render_poster_wordpress` for podium) and publishes it through the site's REST API on a background thread — creating the page, or updating the one already at that slug. A failed publish reports in a native **Retry / Cancel** dialog (Phase 4b; R-86) | — | FINISHED |
| **Results ▸ Show lap & total times** | Checkable publish option (the six sit after a separator) — times in the published results, and the enabler for the Fastest-time leaderboard (R-63); persisted as `AppSettings.publish_show_times` (off by default) | — | always |
| **Results ▸ Laps leaderboard** | Checkable publish option — the laps board in the exports; `AppSettings.publish_laps_board` (on by default) | — | always |
| **Results ▸ Fastest-time leaderboard** | Checkable publish option — the time board in the exports; `AppSettings.publish_time_board` (off by default), unchecked and disabled while Show lap & total times is off (R-63) | — | always |
| **Results ▸ Full field** | Checkable publish option — the full field in the exports; `AppSettings.publish_full_field` (on by default) | — | always |
| **Results ▸ All cards drawn** | Checkable publish option — every drawn card in the exports; `AppSettings.publish_all_cards` (on by default) | — | always |
| **Results ▸ Show DNS Riders** | Checkable — show/hide the DNS (0-lap) rows on a finished ride's results, listed at the bottom of their section with a blank Place and Laps "DNS"; `AppSettings.show_dns_riders` (on by default, no effect while the ride is live) | — | always |
| **View ▸ Times · Zoom** | Direct commands — the two time-column check items (Show Total Times / Show Lap Time on Crossings Panel, each mirroring its own Settings checkbox) and the seven zoom radios; the Settings window's appearance radios are the single theme surface (W13 removed the View-menu theme trio and the Settings text-zoom choice — the duplicate surfaces testing notes #14/#12 removed; R-37's time columns keep both surfaces) | 1a · 3a | always |
| **Help ▸ User Guide** | Opens bundled docs/user-guide.html in browser — 11 chapters + 2 appendices, section anchors deep-linked from dialog Help buttons; screenshots regenerated by the functional-test harness each release | 6a | always |
| **Help ▸ Keyboard Shortcuts** | Shortcuts dialog | 3f | always |
| **Help ▸ Run Evaluator Self-test** | Self-test result dialog (§12) | 3f | always |
| **Help ▸ About RiverCrossing** | About box | 3f | always |

## 5. Accelerators

Copied verbatim from `design/docs-md/xrc-windows.md:513-522`, the table the Keyboard Shortcuts dialog
renders. Enter and F2 are not action shortcuts: they belong to the console's plate field and the
Crossing Detail route.

| Key | Action |
|---|---|
| Enter | Record crossing for typed plate |
| Ctrl+Z | Undo last crossing |
| F5 | Standings window |
| F1 | User guide |
| F2 | Edit crossing (open detail) |
| Delete | Delete selected crossing |
| Ctrl+D | Delete selected crossing |
| Ctrl+E | Edit crossing plate |

## 6. The automation contract

- **Action ids are the `actions` table.** Each row carries `id`, `label`, `shortcut` and `target`.
  `shortcut` is `null` or `{"key": "z" | "f1" | "f5", "command": <bool>}`; only three rows carry one.
- **`ui.command` takes any action id or `app_quit`.** `app_quit` is the automation-only command id
  (`commands`); it quits the app without a §15 row of its own.
- **`ui.inspect` and `ui.set` take `{field, window?}`.** `window` defaults to the top dialog. The
  `fields` array lists the 67 settable controls with their kind: `text`, `choice`, `check` or `spin`.
- **`ui.dialog.*` addresses the top dialog by its window id**, one of the 30 keys of the `windows` map.
- **Targets are typed.** `{"kind": "window"}` opens a frozen window, `{"kind": "command"}` runs a
  command, `{"kind": "check"}` toggles a check, `{"kind": "zoom"}` sets the text zoom and
  `{"kind": "native"}` raises a native dialog and names the command the app runs on confirm.
- **Field kinds are derived from the frozen suffix.** `_input` and `_picker` map to `text`, `_choice`
  to `choice`, `_chk` to `check` and `_spin` to `spin`. Every other control is not a field. The three
  search boxes (`crossings_search`, `rider_search`, `audit_search`) are view-local filters, so they
  stay out of the field list.
- **`wp_kind_choice` is part of `publish_wordpress_dlg`** even though `xrc-windows.md` omits it. The
  frozen id comes from `spec.md:388`, and R-86 requires the choice.
