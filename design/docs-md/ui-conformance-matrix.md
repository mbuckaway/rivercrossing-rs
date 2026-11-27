# UI conformance matrix — ArtCraft redesign

Status of every screen against `design/mockups/index.html`. **Captured** means a screenshot exists in
`design/screenshots/review/`; **reviewed** means it was compared with the mockup and any delta is
recorded here. A screen with no capture is not yet verified — this file must not claim otherwise.

Regenerate with `python3 tools/ui-review/capture.py --all` (see `tools/ui-review/README.md`).

## Console

| State | Capture | Verdict |
|---|---|---|
| No ride | `console-no-ride` | Reviewed. Bar removed; rail 7 entries; header badge, logo tile and six aligned stat columns with em-dash values. |
| DRAFT | `console-draft` | Reviewed. One badge, populated stats, empty feed copy. |
| STOPPED (simulated) | `console-finished` | Reviewed. Header grid and feed verified; feed Name/Team now take the slack instead of clipping to `FIRST…`. |
| RUNNING, populated | — | Pending. Reachable by resuming a stopped simulated ride. |
| FINISHED / REOPENED | — | Pending. |

## Dialogs captured

| Screen | Capture | Verdict |
|---|---|---|
| Settings | `settings` | Reviewed. Horizontal rows, Theme picker first, Contrast│Density, Text size, OK, half-row speed field. |
| Ride Setup | `ride-setup` | Reviewed. 3-column Format grid, Cards group, lock notice, ISO date, tie-break pills, logo row. |
| Ride Library | `ride-library` | Reviewed. Status badge column widened so `FINISHED` fits. |
| Standings (populated) | `standings-full` | Reviewed. Columns Place·Plate·Entry·Laps·Hand·Draw·Total; Solo pane data. |
| Standings (no ride) | `standings-no-ride` | Captured; empty-state copy present, not yet compared. |
| Audit Trail | `audit` | Captured; rows present after a simulated ride, not yet compared. |
| Edit / Add Crossing | `edit-crossing` | Captured, not yet compared. |
| Deal Bonus Card | `manual-deal` | Captured, not yet compared. |
| Mark DNF | `dnf` | Captured, not yet compared. |
| Set Start Time | `set-start` | Captured, not yet compared. |
| Simulating… | `sim-running` | Captured, not yet compared. |
| Rider Simulator | `simulation` | Captured, not yet compared. |
| About | `about` | Captured, not yet compared. |
| Evaluator Self-Test | `self-test` | Captured, not yet compared. |
| Keyboard Shortcuts | `shortcuts` | Captured; registry-driven, 3 rows (see delta). |

## Dialogs not yet captured

`csv_preview`, `crossing_detail`, `crossing_number`, `void_card_confirm`, `delete_ride`, `resume`,
`start_blocked`, `exit_running`, `rider_editor`, `add_rider`, `team_editor`, `add_team`,
`rider_issues`, `publish_wordpress`, `publish_running`.

All are golden-covered by `-P ui`; what is missing is the mockup comparison, which needs the
remaining state recipes (a selected crossing, a roster edit, a publish).

## Recipes

Every recipe begins with a reset prefix (`mi_stop_ride` → `mi_finish_ride` → `mi_clear_ride`, each
answered with `try_dialog_confirm`) so a persisted active ride cannot leak between runs. The
ride-dependent recipes then create a ride, run the simulator with 40 riders / 6 teams / 2 laps
(`Generate Riders` then `GO`), and resume it where a correction dialog needs a live ride.

## Known deltas carried forward

- **Shortcuts** — the mockup lists keys that are not menu actions (Delete, ⌘D, ⌘E, Enter); the
  dialog is registry-driven, so it shows only the documented menu shortcuts (P2).
- **Standings** — §15b wants `Best 5` cards; the window model carries `best_lap` seconds only, so the
  card column needs a presenter-model change (P2).
- **Date picker** — craft's `DatePicker` can only render a locale short date, so Ride Setup uses an
  ISO text field and loses the calendar popover (P2, craft API gap).
- **Button size** — `Button` has no `.small` step, so `Browse…` is a default-height secondary button.
- **Chip preview** — the organizer-logo chip shows the file name; the 64×64 preview size is not
  exposed by the decoder path.
