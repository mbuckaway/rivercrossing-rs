# UI conformance matrix — ArtCraft redesign

Status of every screen against `design/mockups/index.html`. **Captured** means a screenshot exists in
`design/screenshots/review/`; **reviewed** means it was compared with the mockup and any delta is
recorded here. A screen with no capture is not yet verified — this file must not claim otherwise.

Regenerate captures with `python3 tools/ui-review/capture.py --all` (see `tools/ui-review/README.md`).

## Console

| Screen / state | Light | Dark | Verdict |
|---|---|---|---|
| `console_empty` (no ride) | — | captured | Bar removed; rail 7 entries; header shows one badge, logo tile and six captions with em-dash values. Remaining: header caption/value spacing is tighter than the mockup (P2). |
| RUNNING, populated | — | pending | Needs a recipe that creates and starts a ride. |
| DRAFT / STOPPED / FINISHED / REOPENED | — | pending | Same recipe set, one per phase. |

## Dialogs captured

| Screen | Verdict |
|---|---|
| `settings` | Reviewed. Horizontal rows: Theme picker first, Contrast│Density, Text size; Console section; OK. Avg speed now half-row. |
| `ride_setup` | Reviewed. 3-column Format grid, Cards group, lock notice, ISO date, tie-break pills, logo row with `NO LOGO` + Browse; the 2-column Ride grid and the Min lap row render their values. |
| `ride_library` | Captured, not yet reviewed. |
| `standings` (no ride) | Captured; empty state added, not yet reviewed against the mockup. |
| `simulation` | Captured, not yet reviewed. |
| `shortcuts` | Captured; generated from the registry (3 documented shortcuts). Mockup shows 7 rows including non-menu keys — recorded drift. |
| `self_test` | Captured, not yet reviewed. |
| `about` | Captured, not yet reviewed. |

## Dialogs not yet captured

`csv_preview`, `crossing_detail`, `crossing_number`, `edit_crossing`, `manual_deal`,
`dnf_confirm`, `set_start`, `void_card_confirm`, `delete_ride`, `resume`, `start_blocked`,
`exit_running`, `rider_editor`, `add_rider`, `team_editor`, `add_team`, `rider_issues`,
`audit`, `publish_wordpress`, `publish_running`, `sim_running`.

These are all golden-covered by `-P ui`; what is missing is the mockup comparison, which needs the
state recipes (a ride, a selected crossing, a running publish/simulation).

## Known deltas carried forward

- **Console header spacing** — captions and values are tighter than the mockup's `.stat` rhythm (P2).
- **Shortcuts** — the mockup lists keys that are not menu actions (Delete, ⌘D, ⌘E, Enter); the
  dialog is registry-driven, so it shows only the documented menu shortcuts (P2, needs a second
  source for non-menu keys).
- **Standings columns** — §15b wants `Best 5` cards; the window model carries `best_lap` seconds
  only, so the card column needs a presenter-model change (P2).
- **Date picker** — craft's `DatePicker` can only render a locale short date, so Ride Setup uses an
  ISO text field and loses the calendar popover (P2, craft API gap).
- **Button size** — `Button` has no `.small` step, so `Browse…` is a default-height secondary button.
- **Chip preview** — the organizer-logo chip shows the file name; the 64×64 preview size is not
  exposed by the decoder path.
