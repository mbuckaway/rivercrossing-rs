# RiverCrossing — Control Mapping (frozen id → ArtCraft widget)

Every frozen §15b control id mapped to the `craft_ui` widget/pattern that renders it, with its
accessible name. The frozen id stays the widget's `push_id`/`egui::Id` and (where it is an input) its
accessible name, so the automation selector contract and the drift test survive the re-skin. Sources:
`design/docs-md/ui-contract.md` (the control list), `design/docs-md/visual-style.md` (the widget
vocabulary), and the registry (`gui-windows.md` / `ids.rs`).

Rules:

- The frozen id is the widget's `push_id(id, …)`; the visible label doubles as the accessible name.
- Menus dispatch the same `ActionId` as the on-screen buttons.
- Gauges are flattened: clocks are `display::Progress` + numeric `Text`; ride state is a `display::Badge`
  with the state word.
- No colour/size literals; values come from `visual-style.md` tokens.
- **This document is a design map, not a drift-checked registry.** The drift test compares
  `gui-windows.md` ↔ `ids.rs` only. A planned control is marked *(pending EPIC-15 task 6)* until it
  lands in both the registry and the code.

## 1. Window containers

| Window | Container |
|---|---|
| `main_frame` | `shell::AppShell` (title bar + tool rail + docked panels + feed canvas + status bar) |
| `sim_running_dlg`, `publish_running_dlg`, `about_dlg` | fixed-size `dialog::Dialog` (status/progress + Cancel; About + Close) |
| all other dialogs | `dialog::Dialog` (modal; `DialogStack` host) |

`main_menubar` is a resource id rendered by the menu bar (native `craft::menu` on macOS, in-window
`craft::ui::menu` elsewhere); it is not a queryable widget, so it has no `push_id` row below.

## 2. `main_frame` (console)

| Frozen id | ArtCraft widget |
|---|---|
| `ride_status_panel`, `elapsed_clock_panel`, `remaining_clock_panel` | `sections::Panel` slots |
| `ride_status_light` | `display::Badge` (state colour + word) |
| `ride_status_lbl` | `display::Text` (state word) |
| `ride_name_value`…`ride_lap_km_value` (6) | read-only `display::Text` in a `sections::Section` |
| `ride_logo_bmp` | `display::Picture` |
| `current_lap_lbl` | `display::Text` (large) |
| `elapsed_clock`, `remaining_clock` | `display::Progress` + `display::Text` readout |
| `clock_elapsed_lbl`, `clock_remaining_lbl` | `display::Text` (caption) |
| `start_btn`, `stop_btn`, `undo_btn` | `buttons::Button` (primary/secondary per state) — accessible names `Start Ride`/`Stop Ride`/`Undo` |
| `plate_input` | `text::TextField` (large, Enter submits; accessible name "Plate") |
| `record_btn` | `buttons::Button` (accessible name "Record") |
| `last_crossing_lbl` | `display::Text` |
| `crossings_search` | `text::TextField` (search) |
| `main_splitter` | `docking::Splitter` |
| `crossings_list` | `table::Table` (Time/Plate/Name/Team/Card/Lap/Lap time/Total) |
| `review_notebook` | `tabs::TabStrip` (Riders / Needs Review) |
| `console_riders_list` | `table::Table` |
| `flagged_list` | `table::Table` |
| `show_held_only_chk` | `choice::Checkbox` |
| `review_btn` | `buttons::Button` |
| `main_statusbar` | `bars::Bar` (`BarKind::Status`) hosting the six count chips as `display::Badge` readouts |
| `crossings_count_lbl`…`teams_count_lbl` (6) | `display::Badge` chips inside `main_statusbar` (Teams hidden on solo) |

The mockup places the six count chips in the status bar; that is the decided placement (the oracle put
them in the console, but the redesign's status bar is the readout surface).

## 3. Ride / setup / lifecycle

| Window | Frozen id → widget |
|---|---|
| `ride_setup_dlg` | `name_input`/`venue_input`/`organizer_input`/`scorer_input`/`duration_input`/`min_lap_input`→`text::TextField`; `date_picker`/`start_time_picker`→`pickers::DatePicker`/`text::TextField`; `lap_km_spin`→`text::NumberField` (decimal); `hold_short_radio`/`always_deal_radio`→per-option `choice::Radio` in a `RadioGroup`; `solo_radio`/`mixed_radio`→`RadioGroup`; `team_size_spin`→`text::NumberField`; `pooled_radio`/`relay_radio`→`RadioGroup`; `decks_spin`/`jokers_spin`→`text::NumberField`; `jokers_per_deck_radio`/`jokers_total_radio`→`RadioGroup`; `cap_choice`→`pickers::Select`; `tiebreak_list`→`list::List` (reorderable); `logo_preview_bmp`→`display::Picture`; `logo_status_lbl`→`display::Text`; `logo_browse_btn`→`buttons::Button`; `setup_infobar`→`notices::Notice` |
| `set_start_dlg` | `start_date_picker`→`pickers::DatePicker`; `start_time_picker`→`text::TextField` (time codec) |
| `resume_dlg` | `message_lbl`→`display::Text`; `library_btn`/`continue_btn`→`buttons::Button` |
| `exit_running_dlg` | `message_lbl`→`display::Text`; `finish_first_btn`→`buttons::Button`; stock Cancel/OK→dialog footer |
| `crossing_number_dlg` | `number_input`→`text::TextField` |

## 4. Roster / teams / issues

| Window | Frozen id → widget |
|---|---|
| `rider_editor_dlg` | `rider_search`→`text::TextField`; `riders_list`→`table::Table`; `add_btn`/`edit_btn`/`delete_btn`→`buttons::Button`; `plate_input`/`first_name_input`/`last_name_input`/`team_choice`→read-only `display::Text`; `roster_infobar`→`notices::Notice` |
| `add_rider_dlg` | `plate_input`/`first_name_input`/`last_name_input`→`text::TextField`; `team_choice`/`sex_choice`→`pickers::Select`; `add_rider_infobar`→`notices::Notice` |
| `csv_preview_dlg` | `summary_lbl`→`display::Text`; `conflicts_list`→`table::Table`; `map_unknown_sex_chk`/`convert_teams_of_one_chk`→`choice::Checkbox`; `csv_infobar`→`notices::Notice` |
| `team_editor_dlg` | `single_member_only_chk`→`choice::Checkbox`; `teams_list`→`table::Table`; `edit_btn`/`remove_btn`/`add_btn`→`buttons::Button`; `name_input`/`relay_plate_input`/`notes_input`→read-only `display::Text`; `members_list`→`table::Table`; `teams_infobar`→`notices::Notice` |
| `add_team_dlg` | `name_input`/`relay_plate_input`/`notes_input`→`text::TextField`; `logo_bmp`→`display::Picture`; `pick_card_btn`→`buttons::Button`; `add_team_infobar`→`notices::Notice` |
| `rider_issues_dlg` | `card_check_lbl`/`issues_summary_lbl`→`display::Text`; `issues_list`→`table::Table`; `open_editor_btn`/`convert_solo_btn`/`assign_plate_btn`/`renumber_btn`→`buttons::Button`; `issues_infobar`→`notices::Notice` |
| `start_blocked_dlg` | `start_blocked_list`→`list::List` |

## 5. Corrections / cards

| Window | Frozen id → widget |
|---|---|
| `edit_crossing_dlg` | `plate_input`→`text::TextField`; `time_picker`→`text::TextField` (time codec); `reason_input`→`text::TextField`; `void_btn`→`buttons::Button` (danger); stock OK/Cancel→footer |
| `manual_deal_dlg` | `plate_input`/`reason_input`→`text::TextField`; OK labelled "Deal" |
| `dnf_confirm_dlg` | `plate_input`→`text::TextField`; `entry_lbl`→`display::Text`; `reason_input`→`text::TextField`; Cancel is the default button |
| `void_card_confirm_dlg` | `card_lbl`/`entry_lbl`→`display::Text`; `reason_input`→`text::TextField`; Cancel is the default button |
| `crossing_detail_dlg` | `crossing_rider_lbl`…`crossing_held_lbl` (9)→read-only `display::Text`; `edit_btn`/`edit_time_btn`/`void_card_btn`/`delete_btn`→`buttons::Button`; `crossing_detail_infobar`→`notices::Notice` |

## 6. Results / library / audit / system

| Window | Frozen id → widget |
|---|---|
| `results_dlg` | `results_notebook`→`tabs::TabStrip`; `teams_standings_list`/`solo_standings_list`/`standings_list`→`table::Table`; `stale_infobar`→`notices::Notice` |
| `ride_library_dlg` | `rides_list`→`table::Table`; `duplicate_btn`→`buttons::Button`; stock Open/Delete/Close→footer |
| `delete_ride_dlg` | `message_lbl`→`display::Text`; `confirm_name_input`→`text::TextField`; stock Delete/Cancel→footer |
| `audit_dlg` | `audit_search`→`text::TextField`; `action_choice`→`pickers::Select`; `audit_list`→`table::Table` |
| `settings_dlg` | `sound_chk`/`show_total_times_chk`/`show_lap_time_chk`/`verbose_log_chk`→`choice::Checkbox`; `avg_speed_spin`→`text::NumberField` (decimal); the non-frozen Appearance controls `appearance_theme_choice` (the combined theme + mode picker, added 2026-11-27)/`appearance_contrast_choice`/`appearance_density_choice`/`appearance_text_size_choice`→`pickers::Select` |
| `about_dlg` | `version_lbl`/`author_lbl`/`copyright_lbl`/`license_lbl`→`display::Text`; `gorba_link`→`buttons::Link` |
| `shortcuts_dlg` | `shortcuts_list`→`table::Table` (generated from the Action registry) |
| `selftest_dlg` | `selftest_output`→`display::Text` (mono); `rerun_btn`→`buttons::Button` |

**Net-new Settings appearance controls (non-frozen).** The redesign adds the full ArtCraft appearance
model: one combined Theme picker (`appearance_theme_choice`, added 2026-11-27) that replaces the old
Mode radios and writes the theme id and the appearance together, plus Contrast, Density and Text size.
They are **not** part of the 67 §15b frozen fields and are not in the drift-checked registry; they
carry their own stable ids and accessible names and are driven by the `RootEnv` theming hook. The
67-field count is unchanged. (The three `appearance_*_radio` ids are retired.)

## 7. Simulator / publish

| Window | Frozen id → widget |
|---|---|
| `simulation_dlg` | `riders_spin`/`teams_spin`/`solo_spin`/`laps_spin`/`interval_spin`→`text::NumberField`; `short_lap_choice`/`lapped_choice`/`team_stop_choice`→`pickers::Select`; `new_ride_btn`→`buttons::Button` (secondary; implemented (EPIC-15 task 6); enabled only with no ride); `gen_riders_btn`/`check_btn`→`buttons::Button`; `go_btn`→`buttons::Button` (primary, dialog default; rendered as `Button::Custom("go_btn")` (EPIC-15 task 6)); `sim_infobar`→`notices::Notice` |
| `sim_running_dlg` | `sim_status_lbl`→`display::Text`; `progress_gauge`→`display::Progress` (determinate); `cancel_btn`→`buttons::Button` |
| `publish_wordpress_dlg` | `wp_kind_choice`/`wp_status_choice`→`pickers::Select`; `wp_url_input`/`wp_username_input`/`wp_password_input`/`wp_title_input`/`wp_slug_input`/`wp_parent_input`→`text::TextField` (password masked); first focus = URL |
| `publish_running_dlg` | `publish_status_lbl`→`display::Text`; `progress_gauge`→`display::Progress` (indeterminate); `cancel_btn`→`buttons::Button` |

## 8. Menus and actions

All 52 actions map to a `menu::MenuItem` in their §15 section (native macOS bar, in-window elsewhere).
Check rows use `menu::MenuItem`'s own check mark (`Mark::Check`) and density and text-size rows its radio mark
(`Mark::Radio`) — **not** `choice`/`pickers` widgets inside a menu. On-screen buttons for
`mi_start_ride`/`mi_stop_ride`/`mi_undo_crossing` dispatch the same `ActionId`. The `mi_*` ids and
labels are the frozen registry — do not rename them.

## 9. Radio options and automation

The eleven `_radio` frozen ids (format and appearance mode) render as per-option `choice::Radio`s inside a
`RadioGroup`, and **each keeps its frozen id** for the selector contract. The `_radio` suffix is not a
settable-field kind (`ids.rs`), so they are not in the 67 `ui.set` fields; automation drives them by
click. This is intentional and matches the existing contract.

## 10. Verification

- Frozen ids are checked by the drift test against `gui-windows.md` ↔ `ids.rs`; **this mapping is not
  parsed by a test**.
- `new_ride_btn` is now in the registry and rendered; `go_btn` is rendered as a real custom button
  (EPIC-15 task 6, done).
- Footer buttons keep their frozen labels and dialog roles; craft `FooterButton` has no id hook at
  the pinned rev, so they do not carry a `push_id` (body controls do).
- Accessible names: `Start Ride`, `Stop Ride`, `Undo`, `Record`, `Plate` are the Rust selector contract
  (they match `automation/actions.rs` and the Tier B smoke). R-83's Python strings ("Start ride",
  "Stop ride…") are the oracle's labels; the Rust port's stable accessible names are the frozen selector
  contract, and changing them is a separate R-83 decision, not part of this re-skin.
