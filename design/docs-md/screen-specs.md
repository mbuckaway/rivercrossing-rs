# RiverCrossing — Screen Specs (ArtCraft docked shell)

Per-window layout, controls, states, and interactions for the ArtCraft redesign. The structure and flows
are fixed by `design/docs-md/ui-contract.md`; the widgets by `design/docs-md/control-mapping.md`; the
tokens by `design/docs-md/visual-style.md`. Each window must render the states that apply to it (§I) —
no blank panels, no happy-path-only screens.

## Global shell

`shell::AppShell`: title bar → tool rail + docked panels → crossings feed canvas → status bar. Dialogs
are `dialog::Dialog` modals over the shell, hosted by `DialogStack`. Appearance (theme, mode, contrast,
density, text size) lives in Settings; the title bar carries no controls. Every window inherits
the tokens (light/dark, compact/comfortable, contrast, text size, theme) and the focus rules (Esc
cancels, Enter activates the default, focus locked in a modal, opener focus restored).

- **Tool rail = navigation.** The seven rail entries dispatch the **same Action ids** as the menu
  (`mi_open_library`, `mi_rider_editor`, `mi_team_editor`, `mi_check_rider_issues`, `mi_standings`,
  `mi_audit_trail`, `wxID_PREFERENCES`). The rail is a shortcut surface, not a second navigation model.
- **Docked panels = content.** The right dock holds live standings / Needs Review / ride info; it is
  content, not navigation.

## A. Console — `main_frame`

**Layout.** Header strip (status badge + ride identity + logo) over a clocks row (two `Progress` bars with
numeric readouts) over the entry row (large Plate field + Record + Undo + last-crossing line), then the
docked review panel (Riders / Needs Review tabs) beside the crossings `Table`; the six count chips and a
status readout live in the status bar. The tool rail holds the primary views.

**Controls present.** `start_btn`, `stop_btn`, `undo_btn`, `plate_input`, `record_btn`,
`last_crossing_lbl`, `crossings_search`, `main_splitter`, `current_lap_lbl`, the six `ride_*_value`
fields, `ride_logo_bmp`, `crossings_list`, `console_riders_list`, `flagged_list`,
`show_held_only_chk`, `review_btn`, the two clocks, and the six count chips.

**States.** No ride (empty console, meters zero, entry disabled), DRAFT, RUNNING, stopped-RUNNING,
FINISHED, REOPENED. RUNNING advances the wall clock; stopped/finished freeze it. Each state disables the
controls it cannot use (start/stop/undo/entry).

**Interactions.** Plate + Enter records; space returns focus to the plate field; Ctrl+Z undoes; F5 opens
Standings; F1 help; F2 opens the selected crossing; row activation opens Crossing Detail. Start/Stop/Undo
dispatch the same Action ids as the menu.

## B. Ride setup — `ride_setup_dlg`

**Layout.** Two-column form groups: Event identity (name, date, start time, venue, organizer, scorer, lap
km, duration, min lap), Format (entry mode, team size, plate model, decks, jokers, jokers counting, cap,
tie-break order), the short-lap policy (`hold_short_radio`/`always_deal_radio`), and the logo column
(`logo_preview_bmp`, `logo_status_lbl`, `logo_browse_btn`). Infobar at the top.

**States.** Blank (New Ride) and preloaded (Edit Ride); structural fields lock per the ride state as
`ui-contract.md` §5 specifies (entry/plate-model radios lock after DRAFT **for relay rides**; the
decks/jokers/cap/tie-break group locks off DRAFT; event identity stays editable).

**Interactions.** OK saves and closes; Cancel discards; invalid input posts the infobar message and keeps
the dialog open.

## C. Lifecycle dialogs

| Window | Layout | States / interactions |
|---|---|---|
| `set_start_dlg` | Date + time fields | OK recomputes lap 1; Cancel keeps; invalid time refuses inline. Reached only in RUNNING·REOPENED |
| `resume_dlg` | Message + Library / Continue | Continue loads the prior ride; Library (or Esc) opens the library |
| `exit_running_dlg` | Message + Finish first / Cancel / Quit | Cancel is default+focus; Finish first runs the finish flow; Quit keeps the ride running |
| `start_blocked_dlg` | Reason list + OK | Read-only reasons; OK/Cancel close |

## D. Roster and teams

| Window | Layout | States / interactions |
|---|---|---|
| `rider_editor_dlg` | Search + roster table + read-only record panel + Add/Edit/Delete + `roster_infobar` | Delete gated by `can_delete_entry`; team column hidden on solo; empty-roster state shows a hint |
| `add_rider_dlg` | Plate / first / last / team / sex form + `add_rider_infobar` | Add vs Edit title; plate locked off DRAFT; Save/Cancel |
| `csv_preview_dlg` | Summary + conflicts table + two option checkboxes + `csv_infobar` + Import/Cancel | Import disabled while conflicts remain; a parsing/loading state shows a spinner |
| `team_editor_dlg` | Teams table + read-only record panel + `members_list` + Add/Edit/Remove + `teams_infobar` | Relay row hidden unless relay; edits DRAFT-only; empty-teams state shows a hint |
| `add_team_dlg` | Name / relay plate / notes / logo / pick-card + `add_team_infobar` | Add vs Edit; Save/Cancel |
| `rider_issues_dlg` | `issues_summary_lbl` + `card_check_lbl` + issues table + Open/Convert/Assign/Renumber + `issues_infobar` | Fixes gated on selection + DRAFT; fix refusals land on the infobar |
| `start_blocked_dlg` | Reason list | Read-only (listed once, under Corrections/lifecycle) |

## E. Corrections and cards

| Window | Layout | States / interactions |
|---|---|---|
| `crossing_detail_dlg` | 9 read-only value rows + Edit / Edit Time / Void Card / Delete + `crossing_detail_infobar` | Delete closes after confirm; void requires a reason |
| `crossing_number_dlg` | Plate field | Save/Cancel; 4-digit plate |
| `edit_crossing_dlg` | Plate / time / reason + void (edit only) | Add vs Edit mode; OK/Cancel |
| `manual_deal_dlg` | Plate / reason | OK labelled "Deal" |
| `dnf_confirm_dlg` | Plate / entry / reason | Cancel default+focus |
| `void_card_confirm_dlg` | Card / entry / reason | Cancel default+focus; reason required |

## F. Results, library, audit

| Window | Layout | States / interactions |
|---|---|---|
| `results_dlg` | Notebook (Teams/Solo) or single standings table + `stale_infobar` | Live standings while running (a "computing" state), empty with no ride, FINISHED ranks with tie-break order |
| `ride_library_dlg` | Ride table + Open/Duplicate/Delete/Close | Delete disabled on a RUNNING row; empty-library state shows a hint |
| `delete_ride_dlg` | Message + type-to-confirm field + Delete/Cancel | Delete disabled until exact name match |
| `audit_dlg` | Search + action filter + audit table | Read-only; empty state shows a hint |

## G. System

| Window | Layout | States / interactions |
|---|---|---|
| `settings_dlg` | Appearance (one combined **Theme** picker listing each built-in theme's Light/Dark/System variants, plus **non-frozen** Contrast/Density/Text-size — added 2026-11-27, retiring the Mode radios) + sound + columns + verbose + avg speed | OK persists; Cancel discards. The appearance picker and its three axes are non-frozen controls (§control-mapping note) |
| `about_dlg` | Version / author / copyright / license / link | Fixed size; Close |
| `shortcuts_dlg` | Shortcut table generated from the Action registry | Read-only |
| `selftest_dlg` | Mono PASS/FAIL output + Rerun | Rerun disabled mid-run |

## H. Simulator and publish

| Window | Layout | States / interactions |
|---|---|---|
| `simulation_dlg` | Counts / interval / three behaviour selects + New Ride / Generate / Check / GO / Cancel + `sim_infobar` | **No-ride state:** only New Ride enabled. **With a ride:** New Ride disabled, generators + GO enabled; blocked reasons on the infobar, dialog stays open |
| `sim_running_dlg` | Status line + determinate progress + Cancel | Cancel abandons at the next checkpoint |
| `publish_wordpress_dlg` | Kind + Site (URL/user/password) + Page (title/slug/parent/status) + help + Publish/Cancel | Kind re-fills title/slug; blank or non-https URL refuses and keeps the dialog open; first focus = URL |
| `publish_running_dlg` | Status line + **indeterminate** progress + Cancel | Cancel records the request; worker abandons at the next checkpoint |

## I. States each window designs (where they apply)

- **Empty** — no ride selected / no rows / no audit rows / no rides / no teams: a one-line hint, never a
  blank panel.
- **Loading** — a `display::Spinner` or disabled controls with a status line (CSV parse, results
  computing).
- **Error** — a `notices::Notice` (bad tone) with the reason and the next step.
- **Success** — a muted notice or status-bar readout (no modal).
- **Ride state** — DRAFT/RUNNING/FINISHED/REOPENED gate the relevant controls (disable, never hide).

Not every window has every state; each row above says which apply.

## J. macOS specifics

- Native menu bar (`craft::menu`), About/Settings/Quit in the app menu; one menu tree with `Ctrl`⇔`⌘`.
- Close hides/minimizes the console (app keeps running; Dock restores); quit only via File→Quit / ⌘Q.
- Native file/save pickers (`craft::dialogs`); sheets on the parent where applicable.
- Light/dark follows the host under System; manual override in Settings.

## K. Verification

- One Tier A-snapshot golden per window (30), rendered at 1100×780 under a pinned appearance. The console
  golden is the empty state today; the redesign adds a RUNNING console golden (the state that matters).
- The generic `DialogStack` tests already cover Esc/Enter/focus/opener once; no per-dialog × 4 matrix is
  added. Per-dialog gate tests and the window→`DialogSpec` drift check stay.
- Selector contract: frozen Action ids and RoleAndLabel names (`Start Ride`, `Stop Ride`, `Undo`,
  `Record`, `Plate`) preserved.
