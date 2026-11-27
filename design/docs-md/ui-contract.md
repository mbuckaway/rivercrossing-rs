# RiverCrossing — Distilled UI Contract

This document is the consolidated, authoritative description of the RiverCrossing desktop UI. It merges
the Python oracle (`/Users/mark/src/rivercrossing/src/rivercrossing/ui/` + XRC) with the frozen Rust
registry (`design/docs-md/gui-windows.md`) and `spec.md` §15/§15b. It is the source for the ArtCraft
redesign: every window, menu item, control, connection, and gate below must be reproduced in the new
look. It does not re-derive the wx/XRC visual styling — that styling is retired; only the structure,
names, and behaviour are contract.

Authority: `spec.md` §15 (menu map), §15b (frozen names + naming rules), `gui-windows.md` (the reviewed
Rust snapshot), and the Python source tree. The Rust table in `crates/rivercrossing-ui-egui/src/ui/` is
canonical; the ids-drift test fails if this document's machine block and the compiled registry drift.

Counts: **30 top-level windows**, **53 actions** (50 `mi_*` + `wxID_PREFERENCES`/`wxID_EXIT`/`wxID_ABOUT`),
**67 settable fields**.

## 1. Window inventory

| # | id | title | modal | opened by | closed by | purpose |
|---|---|---|---|---|---|---|
| 1 | `main_frame` | RiverCrossing | modeless frame | launch | quit flow | live timing console |
| 2 | `ride_setup_dlg` | Ride Setup | modal | `mi_new_ride` / `mi_edit_ride` | OK Save / Cancel | ride config |
| 3 | `set_start_dlg` | Set Start Time | modal | `mi_set_start_time` | OK / Cancel | back-date start |
| 4 | `resume_dlg` | Resume Ride | modal | launch with prior running ride | `continue_btn` / `library_btn` / Esc→library | resume-or-library |
| 5 | `exit_running_dlg` | Ride Is Running | modal | quit while RUNNING | Cancel / `finish_first_btn` / OK Quit | running-ride quit confirm |
| 6 | `rider_editor_dlg` | Rider Editor | modal | `mi_rider_editor` | Close | roster list + Add/Edit/Delete |
| 7 | `add_rider_dlg` | Add/Edit Rider | modal | `add_btn` / `edit_btn` | Save / Cancel | add/edit one rider |
| 8 | `csv_preview_dlg` | Import Riders — Preview | modal | `mi_import_csv` | Import / Cancel | CSV preview + commit |
| 9 | `team_editor_dlg` | Teams Editor | modal | `mi_team_editor` (mixed only) | Close | team records + members |
| 10 | `add_team_dlg` | Add/Edit Team | modal | `add_btn` / `edit_btn` | Add/Save + Cancel | add/edit one team |
| 11 | `rider_issues_dlg` | Check for Rider Issues | modal | `mi_check_rider_issues` | Close | issue report + one-click fixes |
| 12 | `start_blocked_dlg` | Cannot Start Ride | modal | start on `StartBlockedError` | OK / Cancel | blocked-start reasons |
| 13 | `edit_crossing_dlg` | Edit/Add Crossing at Time | modal | `mi_add_crossing_at` / `edit_time_btn` | OK / Cancel / `void_btn` | add/edit crossing |
| 14 | `manual_deal_dlg` | Deal Bonus Card | modal | `mi_deal_manual` | Deal / Cancel | deal bonus card |
| 15 | `dnf_confirm_dlg` | Mark DNF | modal | `mi_mark_dnf` | Mark DNF / Cancel | mark DNF |
| 16 | `void_card_confirm_dlg` | Void Card | modal | `void_card_btn` | Void / Cancel | void credited card |
| 17 | `results_dlg` | Standings | modal | `mi_standings` (F5) | Close | standings (live/final) |
| 18 | `ride_library_dlg` | Ride Library | modal | `mi_open_library`; also resume `library_btn` | Close | open/duplicate/delete rides |
| 19 | `delete_ride_dlg` | Delete Ride | modal | library `wxID_DELETE` | Delete / Cancel | type-to-confirm delete |
| 20 | `audit_dlg` | Audit Trail | modal | `mi_audit_trail` | Close | read-only audit viewer |
| 21 | `settings_dlg` | Settings | modal | `wxID_PREFERENCES` | OK / Cancel | app-wide settings |
| 22 | `about_dlg` | About RiverCrossing | modal fixed | `wxID_ABOUT` | Close | about box |
| 23 | `shortcuts_dlg` | Keyboard Shortcuts | modal | `mi_shortcuts` | Close | accelerator table |
| 24 | `selftest_dlg` | Evaluator Self-Test | modal | `mi_selftest` / launch hook | Close | evaluator self-test |
| 25 | `crossing_detail_dlg` | Crossing Detail | modal | feed row double-click/Enter/F2; flagged review | OK (Delete also closes) | crossing detail + corrections |
| 26 | `crossing_number_dlg` | Plate | modal | `edit_btn` / Ctrl+E | Save / Cancel | retype plate |
| 27 | `simulation_dlg` | Rider Simulator | modal | `mi_simulation` | Cancel | simulator config |
| 28 | `sim_running_dlg` | Simulating… | modal fixed | `go_btn` | Cancel | simulation progress |
| 29 | `publish_wordpress_dlg` | Publish to WordPress | modal | `mi_publish_wordpress` | Publish / Cancel | WP publish form |
| 30 | `publish_running_dlg` | Publishing to WordPress | modal fixed | publish form OK | Cancel | publish progress |

All dialogs are resizable except `about_dlg`, `sim_running_dlg`, `publish_running_dlg` (fixed).

Nine retired windows (excluded from the count): `stop_confirm_dlg`, `no_ride_dlg`, `finish_confirm_dlg`,
`exit_confirm_dlg`, `duplicate_ride_dlg`, `reopen_ride_dlg`, `reassign_dlg`, `continue_or_new_dlg`,
`entry_detail_dlg`.

## 2. Menu map (sections, items, shortcuts, gates)

Gates come from `commands.py` `ROUTE_TABLE` + `is_route_enabled`. Mnemonics omitted; labels shown plain.
"Ride open" means a ride is loaded; `DRAFT/RUNNING/FINISHED/REOPENED` are the `RideStatus` values (RUNNING
has an `engine.stopped` flag for a stopped-but-not-finished ride).

**File**
- `mi_open_library` — Ride Library — always
- `mi_duplicate_ride` — Duplicate Ride… — a ride is open
- `mi_import_csv` — Import Riders CSV… — ride open
- `mi_export_csv` — Export Riders CSV… — ride open
- `mi_backup_now` — Back Up Database… — always
- `mi_simulation` — Simulation… — DRAFT (open or none) or stopped RUNNING
- `wxID_PREFERENCES` — Settings… — always
- `wxID_EXIT` — Exit — always

**Ride**
- `mi_new_ride` — New Ride… — no ride open
- `mi_edit_ride` — Edit Ride… — a ride is open
- `mi_start_ride` — Start Ride — ride open + DRAFT / stopped RUNNING / REOPENED
- `mi_stop_ride` — Stop Ride… — live RUNNING
- `mi_set_start_time` — Set Start Time… — RUNNING · REOPENED
- `mi_finish_ride` — Finish Ride… — RUNNING · REOPENED
- `mi_reopen_ride` — Reopen Ride — FINISHED
- `mi_audit_trail` — Audit Trail… — ride open + ≥1 audit row
- `mi_clear_ride` — Clear Ride… — DRAFT / stopped RUNNING / FINISHED / REOPENED

**Riders**
- `mi_rider_editor` — Rider Editor — ride open
- `mi_team_editor` — Teams Editor — ride open + teams allowed (mixed)
- `mi_check_rider_issues` — Check for Rider Issues… — ride open
- `mi_mark_dnf` — Mark DNF… — RUNNING · REOPENED

**Cards**
- `mi_undo_crossing` — Undo Last Crossing — Ctrl+Z — RUNNING + ≥1 crossing
- `mi_add_crossing_at` — Add Crossing at Time… — RUNNING · REOPENED
- `mi_deal_manual` — Deal Bonus Card… — RUNNING · REOPENED

**Results**
- `mi_standings` — Standings — F5 — always
- `mi_export_html` — Export HTML… — FINISHED
- `mi_export_pdf` — Export PDF… — FINISHED
- `mi_export_poster` — Podium Poster PDF… — FINISHED
- `mi_export_poster_html` — Podium Poster HTML… — FINISHED
- `mi_export_results_csv` — Export Standings CSV… — FINISHED
- `mi_preview_html_browser` — Preview HTML in Browser — FINISHED + HTML export exists
- `mi_preview_poster_html_browser` — Preview Podium Poster HTML in Browser — FINISHED + poster export exists
- `mi_preview_pdf_browser` — Preview PDF in Browser — FINISHED + PDF export exists
- `mi_publish_wordpress` — Publish to WordPress… — FINISHED
- `mi_show_times` — Show lap & total times (check) — always
- `mi_laps_board` — Laps leaderboard (check) — always
- `mi_time_board` — Fastest-time leaderboard (check) — always; re-disabled when show-times off
- `mi_full_field` — Full field (check) — always
- `mi_all_cards` — All cards drawn (check) — always
- `mi_show_dns_riders` — Show DNS Riders (check) — always

**View**
- `mi_show_total_times` — Show Total Times on Crossings Panel (check) — always
- `mi_show_lap_time` — Show Lap Time on Crossings Panel (check) — always
- `mi_zoom_90` … `mi_zoom_150` — Zoom 90%…150% (radio) — always

**Help**
- `mi_user_guide` — User Guide — F1 — always
- `mi_shortcuts` — Keyboard Shortcuts — always
- `mi_selftest` — Run Evaluator Self-test — always
- `wxID_ABOUT` — About RiverCrossing — always

Accelerators (`accelerators.py`): Enter (record), Ctrl+Z (undo), F5 (standings), F1 (guide), F2 (open
detail), Delete (delete selected crossing), Ctrl+D (delete), Ctrl+E (edit plate).

## 3. Controls per window

The full frozen control list per window is the `windows` map in `gui-windows.md` (machine block). This
section records the behaviour each window's controls express; the redesign maps each name below to a
`craft_ui` widget in `control-mapping.md`.

- **main_frame**: `main_menubar` (resource), `ride_status_panel`/`elapsed_clock_panel`/`remaining_clock_panel`
  (code-side slots), `ride_status_light` (StopLight: green RUNNING / amber stopped-REOPENED / red DRAFT·
  FINISHED), `elapsed_clock`/`remaining_clock` (RaceClock dials, fraction 0.0–1.0), `ride_status_lbl`
  (DRAFT/STOPPED/RUNNING/FINISHED/REOPENED word), six read-only `ride_*_value` fields, `ride_logo_bmp`
  64×64, `current_lap_lbl`, `start_btn`/`stop_btn`, `plate_input` (large type, `wxTE_PROCESS_ENTER`),
  `record_btn`, `last_crossing_lbl`, `undo_btn`, `crossings_search` (Plate/Name/Card filter),
  `main_splitter`, `crossings_list` (whole-ride feed with card bitmaps), six count chips
  (`crossings_count_lbl`, `cards_count_lbl`, `on_course_lbl`, `shoe_lbl`, `riders_count_lbl`,
  `teams_count_lbl` — teams chip hidden on solo), `review_notebook` (`console_riders_list` "Riders" +
  `flagged_list`/`show_held_only_chk`/`review_btn` "Needs Review"), `main_statusbar` 3 fields.
- **ride_setup_dlg**: `name_input`, `venue_input`, `organizer_input`, `scorer_input`, `duration_input`
  (H:MM), `min_lap_input` (M:SS), `date_picker`, `start_time_picker`, `lap_km_spin`, `hold_short_radio`/
  `always_deal_radio`, `solo_radio`/`mixed_radio` + `team_size_spin` (2–10), `pooled_radio`/`relay_radio`,
  `decks_spin`, `jokers_spin` + `jokers_per_deck_radio`/`jokers_total_radio`, `cap_choice`
  (Disabled/5–20), `tiebreak_list` (reorderable), `logo_preview_bmp`/`logo_status_lbl`/`logo_browse_btn`,
  `setup_infobar`. Structural fields lock after DRAFT; event-identity fields always editable.
- **set_start_dlg**: `start_date_picker`, `start_time_picker` (first field = date).
- **resume_dlg**: `message_lbl` (dynamic), `library_btn`, `continue_btn` (affirmative, Enter; Esc→library).
- **exit_running_dlg**: `message_lbl` (dynamic ride name), `finish_first_btn`, stock Cancel/OK (Cancel
  default+focus).
- **rider_editor_dlg**: `rider_search`, `riders_list`, `add_btn`, `delete_btn` (gated by
  `can_delete_entry`), read-only `plate_input`/`first_name_input`/`last_name_input`/`team_choice`,
  `edit_btn`, `roster_infobar`. Team column hidden on solo.
- **add_rider_dlg**: `plate_input` (locked off-DRAFT), `first_name_input`, `last_name_input`, `team_choice`,
  `sex_choice` (blank/M/F), `add_rider_infobar`.
- **csv_preview_dlg**: `summary_lbl` (dynamic), `conflicts_list`, `map_unknown_sex_chk`,
  `convert_teams_of_one_chk`, `csv_infobar`. Import disabled while conflicts > 0.
- **team_editor_dlg**: `single_member_only_chk`, `teams_list` (sortable), `edit_btn` (selection-gated),
  `remove_btn`, read-only `name_input`/`relay_plate_input`/`notes_input`, `members_list`, `add_btn`,
  `teams_infobar`. Relay row hidden unless `TEAM_RELAY`.
- **add_team_dlg**: `name_input`, `relay_plate_input` (relay-only), `notes_input`, `logo_bmp`,
  `pick_card_btn`, `add_team_infobar`.
- **rider_issues_dlg**: `card_check_lbl` (hidden when not threaded), `issues_summary_lbl`,
  `issues_list`, `open_editor_btn`, `convert_solo_btn`, `assign_plate_btn`, `renumber_btn` (gated on
  selection + DRAFT), `issues_infobar`.
- **start_blocked_dlg**: `start_blocked_list`.
- **edit_crossing_dlg**: `plate_input`, `time_picker`, `reason_input`, `void_btn` (edit mode only).
- **manual_deal_dlg**: `plate_input`, `reason_input`; OK label "Deal".
- **dnf_confirm_dlg**: `plate_input`, `entry_lbl` (dynamic), `reason_input`; Cancel default+focus.
- **void_card_confirm_dlg**: `card_lbl`, `entry_lbl` (dynamic), `reason_input`; Cancel default+focus.
- **results_dlg**: `results_notebook` (`teams_standings_list` + `solo_standings_list`) on MIXED, or
  `standings_list` on SOLO; `stale_infobar`. Card cells use per-card red-suit markup.
- **ride_library_dlg**: `rides_list`, `duplicate_btn`, stock `wxID_OPEN`/`wxID_DELETE`/`wxID_CLOSE`.
  Delete disabled on a RUNNING row.
- **delete_ride_dlg**: `message_lbl` (dynamic), `confirm_name_input` (type-to-confirm, exact match);
  Delete disabled until exact byte match.
- **audit_dlg**: `audit_search`, `action_choice` (30 items), `audit_list` (When/Action/Entry/Reason).
- **settings_dlg**: `appearance_system_radio`/`appearance_light_radio`/`appearance_dark_radio`, `sound_chk`,
  `show_total_times_chk`, `show_lap_time_chk`, `verbose_log_chk`, `avg_speed_spin`.
- **about_dlg**: `version_lbl`, `author_lbl`, `copyright_lbl`, `license_lbl`, `gorba_link`.
- **shortcuts_dlg**: `shortcuts_list` (generated from the accelerator table — cannot drift).
- **selftest_dlg**: `selftest_output` (monospace PASS/FAIL), `rerun_btn` (disabled mid-run).
- **crossing_detail_dlg**: nine read-only `crossing_*_lbl` (rider/team/plate/lap/time/lap-time/total/card/
  held), `edit_btn`, `edit_time_btn`, `void_card_btn`, `delete_btn`, `crossing_detail_infobar`.
- **crossing_number_dlg**: `number_input` (4-digit plate).
- **simulation_dlg**: `riders_spin`, `teams_spin`, `solo_spin`, `laps_spin`, `interval_spin`,
  `short_lap_choice`, `lapped_choice`, `team_stop_choice`, `new_ride_btn`, `gen_riders_btn`, `check_btn`,
  `go_btn`, `sim_infobar`.
- **sim_running_dlg / publish_running_dlg**: status label + `progress_gauge` (0–100) + `cancel_btn`
  (Escape pointed at it).
- **publish_wordpress_dlg**: `wp_kind_choice` (Full/Podium), `wp_url_input`, `wp_username_input`,
  `wp_password_input`, `wp_title_input`, `wp_slug_input`, `wp_parent_input`, `wp_status_choice`
  (Draft/Publish/Pending/Private); kind auto-fills title/slug; first field = URL.

## 4. Navigation / flow graph

- **Launch**: `main_frame` shows, then the previous-session check runs. If a ride was running at exit,
  `resume_dlg` opens → Continue loads the engine; Open library (or Esc) opens `ride_library_dlg`.
  Otherwise the console stands empty.
- **Ride lifecycle**: `mi_new_ride` → `ride_setup_dlg` (blank, commits a DRAFT). `mi_edit_ride` → same
  preloaded. `mi_start_ride` → `ConsolePresenter.on_start`; `StartBlockedError` → `start_blocked_dlg`;
  empty roster / incomplete setup refuse. `mi_stop_ride` → native confirm. `mi_finish_ride` → native
  danger confirm → self-test gate → optional "Finish anyway" → finish. `mi_reopen_ride` → native prompt.
  `mi_clear_ride` → native danger confirm → console reset only. `mi_audit_trail` → `audit_dlg`.
- **Roster flows**: `mi_rider_editor` → `rider_editor_dlg` → `add_rider_dlg` (Add/Edit) or native-confirm
  delete. `mi_team_editor` → `team_editor_dlg` → `add_team_dlg` or native-confirm remove.
  `mi_check_rider_issues` → `rider_issues_dlg` → `open_editor_btn` chains into rider/team editor, or
  in-place fixes. `mi_import_csv` → OS picker → `csv_preview_dlg` → commit. `mi_export_csv` → OS save.
- **Crossing corrections**: feed row activation → `crossing_detail_dlg` (or miss detail). From there:
  `edit_btn`/Ctrl+E → `crossing_number_dlg`; `edit_time_btn` → `edit_crossing_dlg` (edit);
  `void_card_btn` → `void_card_confirm_dlg`; `delete_btn`/Delete/Ctrl+D → native danger confirm.
  `mi_add_crossing_at` → `edit_crossing_dlg` (add). `mi_deal_manual` → `manual_deal_dlg`. `mi_mark_dnf` →
  `dnf_confirm_dlg` → native prompt. `mi_undo_crossing`/`undo_btn` → native danger confirm → undo. Review
  tab routes held cards to a 3-choice confirm, duplicates to `crossing_detail_dlg`, credited/voided cards
  to a "Return to Held" prompt.
- **Results/export**: `mi_standings` → `results_dlg` (live while running; empty with no ride).
  Export/preview rows → OS picker → off-loop write → recorded path; previews open the path in browser.
  `mi_publish_wordpress` → `publish_wordpress_dlg` → OK → `publish_running_dlg` + worker thread → success
  (OK/Open) or failure (Retry/Cancel) or silent cancel.
- **Quit**: `wxID_EXIT` / red X / Dock / logout funnel through `_confirm_quit`. RUNNING → `exit_running_dlg`
  (Cancel / Finish ride first / Quit-keep-running); every other state → native confirm. A confirmed Quit
  stamps `closed_at` and force-closes. **macOS red X hides the window instead of quitting**; Dock-icon
  click restores it.

## 5. Ride-state machine and gates

`RideStatus`: DRAFT → RUNNING (with `engine.stopped` flag) → FINISHED → REOPENED.

- **Menu gates** are computed from a `RideState` carrying `status`, `ride_open`, `ride_stopped`,
  `crossings`, `audit_rows`, `entry_has_cards`, `html_exported`, `pdf_exported`, `poster_html_exported`,
  `teams_allowed`; the per-item result is §2.
- **Console button gates**: `start_btn` = DRAFT / REOPENED / stopped RUNNING; `stop_btn` = live RUNNING;
  `undo_btn` = RUNNING + ≥1 crossing. The entry field locks unless live RUNNING and not stopped.
- **Setup dialog gates**: entry-mode/plate-model radios lock after DRAFT for relay rides; structural group
  (decks/jokers/cap/tiebreak) gated by `can_edit_structure` (DRAFT only); name/date/start/venue/organizer/
  scorer always editable.
- **Roster edit gates**: rider delete = `can_delete_entry(status, has_data)`; rider add/edit plate locked
  off-DRAFT; live team move requires RUNNING-stopped or REOPENED; team add/remove DRAFT-only; rider-issue
  plate fixes DRAFT-only; CSV import reshapes DRAFT-only; `mi_time_board` disabled while `mi_show_times` off.

## 6. Parity gaps and their resolved handling

Exactly three Python→Rust gaps are known at the time of writing. Each is a resolved design decision with
concrete implementation steps, verification, and the phase that lands it — none is left open. Any further
gap found during the EPIC-15 phase-4 review is resolved the same way before implementation, never
deferred.

### 6.1 `new_ride_btn` — missing "New Ride" control in `simulation_dlg`

**Current state.** The Python simulator's bottom row begins with a custom "New Ride" button
(`simulation.xrc:231-234`, `ids.py:154`). The Rust registry (`ids.rs` simulation_dlg list,
`gui-windows.md` `simulation_dlg`), the dialog spec (`dialogs/spec.rs` simulation_dlg) and the view render
nothing for it, so a console with no ride makes the simulator a dead end.

**Target behaviour (parity).** The simulator opens in two states. With no ride open, every generator and
race control is disabled and `new_ride_btn` is the row's only enabled action; clicking it runs the New
Ride flow — creating a "GORBA Test Ride #N" (next free test-ride name, dated today, 10:00 gun time) — and
closes the dialog. With a ride open, `new_ride_btn` is disabled (there is no ride to create).

**Resolution.**
1. Add `new_ride_btn` to the `simulation_dlg` control list in `ids.rs` **and** the `gui-windows.md`
   machine block in the same commit (the drift test enforces they match).
2. Add a `Button::Custom("new_ride_btn")` labelled "New Ride" to the `simulation_dlg` spec, left of
   "Generate Riders".
3. Render it as an ArtCraft secondary button (`craft_ui::buttons::Button`) with the accessible name
   "New Ride".
4. Enablement: enabled only when no ride is loaded; disabled and inert otherwise (never a live-looking
   control with no handler).
5. Activation: create the simulator test ride through the existing `ride_defaults` presenter
   (`build_test_ride_config` equivalent) and close `simulation_dlg`; the console then loads the new DRAFT
   ride. No new domain logic.
6. Tests: a unit test for the enabled/disabled gate and the create-and-close route; regenerate the
   `simulation.png` golden.

**Acceptance.** `new_ride_btn` appears in registry + spec + view; enabled only with no ride; activation
creates the test ride and closes the dialog; drift test green; golden re-baselined. **EPIC-15 task 6.**

### 6.2 `go_btn` — dead frozen id (rendered as stock OK)

**Current state.** `go_btn` is listed in the registry (`ids.rs`, `gui-windows.md`) and in the Python dialog
(label "GO", the dialog's default button, `simulation.xrc:260-263`), but the Rust dialog spec renders the
simulator default as stock `Button::Ok` labelled "GO" (`dialogs/spec.rs:323`, `views/dialogs.rs:295`). No
rendered control carries the frozen id `go_btn`, so a selector or automation step targeting `go_btn`
cannot find it — and because nothing checks the spec's buttons against the window's registry, the dead id
stays silent.

**Target behaviour (parity).** `go_btn` is a real "GO" button and the dialog's default (Enter activates
it); it is enabled only when a ride is open; it validates the race settings (laps ≥ 1, interval ≥ 1) and,
on a blocked start, reports the reasons on `sim_infobar` while the dialog stays open.

**Resolution.**
1. Replace the simulator's `Button::Ok` with `Button::Custom("go_btn")` labelled "GO" in
   `dialogs/spec.rs`, and declare it the dialog default so Enter still activates it.
2. Preserve enablement (enabled iff a ride is open) and the blocked-reasons behaviour.
3. Add a **spec-vs-registry consistency test**: every custom `Button::Custom(id)` a window declares must
   be in that window's frozen control list, and every `_btn` in the list must be declared. This closes the
   loophole that let `go_btn` be listed-but-unrendered.
4. Regenerate the `simulation.png` golden.

**Acceptance.** `go_btn` is rendered with its frozen id and "GO" label, is the default, enabled only with
a ride, and reports blocked reasons on `sim_infobar`; the new consistency test passes; drift/spec tests
green. **EPIC-15 task 6.**

### 6.3 macOS close hides/minimizes instead of quitting

**Current state.** The Python oracle vetoes the red-X close on macOS and calls `frame.Hide()`
(`app.py:4564-4566`), restoring the window on a Dock-icon click (`app.py:5659-5666`); on Windows it runs
the quit confirm. The Rust `close_requested` (`app.rs:1010-1016`) instead calls `request_quit()` (the quit
confirm) and returns `CloseDecision::Hold` — wrong for macOS, and also dead-framed by the activation bug.

**Target behaviour (parity).** On macOS, the close button hides/minimizes the main window: the app keeps
running in the background, no quit prompt appears, and a Dock-icon click restores the window. Quitting
happens only via File→Quit / ⌘Q / app-menu Quit, which runs the quit-confirm flow. On Windows/Linux, the
close button runs the quit confirm.

**Resolution.**
1. Rework `close_requested`: keep the existing `quit_confirmed || quit_after.is_some() || control.is_some()`
   guard first. Then, on macOS, return `CloseDecision::Hold` and set a `minimize_next_frame` flag; the
   **next** `logic` pass emits `ViewportCommand::Minimized(true)`. Do not send the minimize in the same
   frame as `hold_close`, because `hold_close` itself sends `Minimized(false) + Focus`. On Windows/Linux,
   run the quit-confirm flow. `CloseDecision::Close` already triggers `craft_quit::quit()` through
   `embed::logic`, so a confirmed quit still terminates.
2. Do **not** route `wxID_EXIT` through `craft_quit::on_menu_event` (it returns false for non-craft ids).
   Tag the Exit row `ItemRole::Quit` so `mac_layout` moves it into the app menu and assigns ⌘Q.
3. Verify the Dock-reopen restore on macOS: a miniaturized window is restored by AppKit on Dock
   activation; a closed one cannot be (winit has no `applicationShouldHandleReopen`). Minimize, not hide.
4. Tests: a unit test for the `close_requested` decision on each platform, parameterised with `rstest`
   (the two existing close tests encode the old behaviour); a manual / operator run on macOS for
   hide-and-restore (the Tier B smoke count stays at two).

**Acceptance.** macOS close hides/minimizes with the app running; Dock restores it; quit only via the menu;
Windows/Linux close runs the confirm; tests green. **EPIC-15 task 3.**
