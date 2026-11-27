# UI conformance audit — shipped app vs approved mockups

**Status:** open · **Date:** 2026-11-27 · **Owner:** EPIC-15 follow-up

**Verdict:** the shipped UI reproduces the *information architecture* of the approved mockups and
almost none of their *presentation*. Worse, the one path that leads anywhere — creating a ride — is
a form with **no field labels**, so a first-run user cannot create a ride; with no ride open nearly
every control is disabled. That is why the app reads as "nothing works". Snapshots pass because a
golden records what was built, not what was approved.

## 1. Method

| Step | Detail |
|---|---|
| App capture | `target/debug/rivercrossing --control 0 --control-token <hex>`, driven over the R-88 control channel (`ui.screenshot`), PNG at 2960×1500 px (1480×750 pt @2×). No screen-recording permission involved, no JPEG artefacts. |
| Live state used | `ui.accessTree` (198 nodes, 65 named) to locate controls; `ui.command mi_new_ride` to open dialogs; `ui.click` + `ui.key` to drive them. |
| Mockup render | `design/mockups/index.html` served over HTTP, headless Chromium at 1450×872 CSS px, section activated, both themes. |
| Code inspection | `crates/rivercrossing-ui-egui/src/ui/views/*.rs` (17 modules), `ui/appearance.rs`, `ui/window.rs`, craft-libs `243d68e` `crates/window/src/policy.rs`. |

Artifacts in `design/screenshots/current-ui/`:

| File | Content |
|---|---|
| `console-dark-window.jpg`, `console-dark-window-2x.png`, `console-light-window.png` | the app, console, both themes |
| `ride-setup-dialog-dark.png` | Ride Setup, first open, before any input |
| `mockup-console-light.png`, `mockup-console-dark.png`, `mockup-ride-setup-light.png` | the approved design |
| `compare-console-dark.png`, `compare-console-light-app-vs-mockup.png`, `compare-ride-setup.png` | app left, mockup right, same logical width |

Reproduce a capture:

```bash
./target/debug/rivercrossing --control 0 --control-token $(openssl rand -hex 32) &
# then, per connection: {"id":1,"method":"auth","params":{"token":"<hex>"}}
# {"id":2,"method":"ui.screenshot","params":{"path":"console.png"}}   → target/control/console.png
```

## 2. Defects

Severity: **S1** blocks use or the ArtCraft claim · **S2** visibly wrong · **S3** polish.

| # | Sev | Area | Mockup | Actual | Site |
|---|---|---|---|---|---|
| D1 | S1 | Window chrome | one slim in-window bar: `RiverCrossing — <ride>` | **partly fixed** (2026-11-27): the macOS window tab strip is gone (`ui/macos.rs` disallows `NSWindow.tabbingMode`), which also removed View ▸ Hide Tab Bar / Show All Tabs. What remains is two title texts — craft's bar (icon + `RiverCrossing`) and the window's own centred title line — where the mockup has one bar carrying the ride name. | `views/console.rs:147-158` (`TitleBar::new().title_when_native()`), `ui/window.rs:29-40`; craft frame policy `crates/window/src/policy.rs:285` |
| D2 | S1 | Tool rail | 96 pt column, 7 entries, glyph + label | 55 pt column, 7 entries, each rendering as `...`. The AccessKit labels are correct (`Ride Library`, `Rider Editor`, …), so only the width is wrong. | `views/console.rs:254-271` + `:131-137` (`SlotSize::Content`, `Button::new(label)`, no minimum) |
| D3 | S2 | Tool rail | glyph per entry (`▤ ☷ ⚑ ⚐ ▦ ⌕ ⚙`) | none | `views/console.rs:97-105` (`RAIL` has no icon field) |
| D4 | S2 | Options bar | segmented `Appearance [System|Light|Dark]`, `Density [Compact|Comfortable]`, `Zoom 100%` | two `Select` dropdowns, `Zoom 110%` (persisted) | `views/console.rs:181-213` |
| D5 | S1 | Panel chrome | every region is a rounded card: header, clocks, entry, feed, review, status | header/clocks/entry are bare rows on the window canvas; only the two docks get a `Panel` | `views/console.rs:348-355` (`body` uses raw `ui.vertical`) |
| D6 | S1 | Ride header | card: `RUNNING` badge, logo tile, six label-above-value stats, right-aligned `Current lap` | one wrapped text line `NO RIDE  NO RIDE  Ride Date Venue Organizer Scorer Lap km  Lap 0`; with no ride the six values are empty strings, so captions run together | `views/console.rs:358-374`, `value() :434-441` |
| D7 | S1 | Clocks | two cards, 28 px tabular readout, progress track | inline `Elapsed 0:00:00`, no card, no track, no remaining column | `views/console.rs:444+` |
| D8 | S2 | Entry row | card: `[Start Ride] [Stop Ride] │ Plate [123] [Record] [Undo] ✓ dealt 9♥` | card-less row; field unlabelled; placeholder `start the ride to record` | `views/console.rs` `entry_row` |
| D9 | S2 | Crossings feed | title `Crossings · 128`, search right-aligned in the title band, table with uppercase muted captions and zebra | title `Crossings`, search box bare and left-aligned, headers plain words | `views/console.rs:643-670` |
| D10 | S2 | Review panel | tab strip with chrome, then a table | `Riders` / `Needs Review` as plain text on a hairline | `views/console.rs` `review_panel` |
| D11 | S2 | Status chips | filled pill, generous padding | outline-only badge, near-zero padding, so it reads as `(Crossings 0)` | `views/console.rs:314-318` |
| D12 | S1 | Status read-out | `Shift 1 · RUNNING · 02:14:07` | the internal backup path, e.g. `…/rides.db.backups/rides.20261127-012635-364331.db` — also present in the AccessKit tree as a status label | `views/console.rs:329-331` |
| D13 | S1 | Fill | panels stretch to the status bar | panels stop ~250 pt short; dead canvas band below | `views/console.rs:618-635`; the shell gives `body` only content height |
| D14 | S2 | Type scale | caption 11 muted · value 13 · readout 28 tabular · uppercase section captions | one ~13 px proportional size for every role; numerals proportional | `views/*` |
| D15 | S2 | Zoom | `Zoom 100%` | `Zoom 110%` from a saved setting, presented as if designed | settings store + `app.rs` zoom |
| **D16** | **S1** | **Ride Setup fields** | every input has a visible label above it (`Name`, `Date`, `Start time`, `Venue`, `Organizer`, `Scorer`, `Lap length (km)`, `Duration (H:MM)`, `Min lap (M:SS)`, `Hand cap`) | **no label on any field**: eight unlabelled boxes in a column, with only `Ride` above them and `Format`/`Shoe` further down. Values (`26-11-27`, `08:00`, `8`, `6:00`, `20:00`) sit in boxes whose meaning is unknowable. The dialog's own validation (`name is required; venue is required; organizer is required; scorer is required`) is the only thing that names the fields. | `views/dialogs.rs` ride-setup form |
| D17 | S2 | Ride Setup layout | two-column labelled grid over `Ride` / `Format` sections, info banner, `Cancel` + `Save` | single narrow column, few fields visible without scrolling, `Cancel` + `OK`, no info banner, no tie-break order, no organizer logo | `views/dialogs.rs` |
| D18 | S1 | First-run path | console opens with the library/resume affordance and a clear way to create a ride | console opens with `NO RIDE`, most controls disabled, and the single forward path (New Ride) leads to D16 — the workflow dead-ends | `views/console.rs` gating + `views/dialogs.rs` |

## 3. Root causes

1. **Widget substitution.** The mockups were approved with specific controls (segmented control,
   glyph+label rail item, filled chip, tab strip, card panels, tabular readout). The views were
   written against the craft API with the nearest primitive (`Select` for a segmented control,
   `Button` for a rail item, `Badge` for a chip).
2. **Labels never composed.** The setup form builds inputs without captions (D16) — the same
   shortcut, one level worse: the control exists, its name does not.
3. **No shell fill contract.** `AppShell`'s body is content-sized, so a full-window screen cannot be
   composed (D13).
4. **Two title owners.** The app accepts the native frame *and* asks craft to draw the title under
   it (`title_when_native`), and the window carries the macOS tab strip (D1).
5. **Internal state surfaced.** Notices carry file paths; the status bar is a notice sink (D12).
6. **Conformance was never a gate.** Tier A-snapshot compares against goldens generated from the
   implementation. Nothing compared a frame against `design/mockups/*.html`, so D1–D18 pass CI.

## 4. Interaction evidence (what was actually verified)

- The window can be frontmost: `lsappinfo front` reports `rivercrossing`.
- Controls do respond to the app's own synthetic input: `ui.click` on the `Appearance` combo opened
  its list, `arrowdown` + `return` changed the theme, and the value in `ui.accessTree` became
  `Light`; restored to `System` afterwards.
- OS-level synthetic clicks (post-to-pid) at the same control produced a hover highlight but no
  activation. That is an injection-path limitation, not proof that a real click fails.
- **Not verified:** a physical mouse click. The user reports none work; the two facts above mean the
  widget layer itself is functional, so the report is most consistent with D18 — everything gated,
  and the only open path (D16) unusable.

## 5. Remediation order

1. **D16/D18 first.** Label every setup field, gate the dialog's sections like the mockup, and give
   the console an unmissable "New ride" affordance. Without this the app cannot be exercised, and
   every other screen stays untestable by hand.
2. **Shell** — one title owner; body fill; segmented options; rail with glyph + label at a fixed
   width; chip/readout status bar; notices that never carry paths. One commit per defect, each with
   a Tier A semantic test and a refreshed golden.
3. **Chrome primitives** — `panel`/`stat`/`readout`/`chip`/`tabstrip` helpers, so screens compose
   designed parts instead of raw widgets.
4. **Console** — rebuild the five regions (D5–D10, D13, D14).
5. **Screens 2–30** — one screen per commit, captured through the control channel and diffed against
   its mockup section in both themes; sign-off on the pair, then commit the golden.
6. **Conformance gate** — render each mockup section and its app screen at the same logical size and
   fail on structural drift. This is the gate that was missing.

## 6. Fixed since the audit

| Date | Defect | Change | Verification |
|---|---|---|---|
| 2026-11-27 | D21 | A card code is drawn with its suit glyph (`9D` → `9♦`), and a joker as `Joker`: `presenters/data_source.rs` `card_text`, with its own unit test. | 4305 unit tests pass, including `test_card_text_draws_the_suit_glyph`. |
| 2026-11-27 | D22 | The review table is the design's `Plate · Name`; the third content column that ran past the dock is gone. | Live capture: the review card reads `Plate Name`, nothing clipped. |
| 2026-11-27 | D23 | The clock read-outs take the title type role, the largest on the screen. | Applied; a populated capture will confirm the size beside the design's. |
| 2026-11-27 | D16 | Every field in the ride-setup form draws its caption above the control, muted and small: the `stack_*` helpers in `views/dialogs.rs` gained `stack_label`, which every form using them inherits. | Live: the dialog reads `Ride name`, `Date`, `Start time`, `Venue`, `Organizer`, `Scorer`, `Lap length (km)`, `Duration (H:MM)`, `Min lap (M:SS)`, `Hand cap`, and each section's own labels. |
| 2026-11-27 | D17 (layout) | The form is the design's two-column grid — `Ride name · Date`, `Start time · Venue`, `Organizer · Scorer`, `Lap length · Duration`, `Min lap · Hand cap` — with its numeric inputs in a narrow cell; `Format` pairs entry mode with team size. Remaining: the tie-break order and organizer logo the design shows, and its `Save` label. | Compared in `compare-ride-setup-fixed.png`. |
| 2026-11-27 | D19 | The feed carries the design's eight columns: `Time · Plate · Name · Team · Card · Lap`, with `Lap time` and `Total` behind the View settings. | Live on a 658-crossing ride: `12:36 202 FIRSTNAME-0162… TEAM-0027 A♣ 12`. |
| 2026-11-27 | D4 | The options bar's two dropdowns are the design's segmented strips, each with its muted caption: `Appearance [System|Light|Dark]` and `Density [Compact|Comfortable]`. | Compared in the options bar of `console-populated.png`. |
| 2026-11-27 | tables | A row's label fills the first column without a cell, so rows that also supplied a `NAME` cell printed the name twice — the rider editor, the issues list, the library, the standings and the shortcuts table all did. Each now supplies only its other cells. | Live: the rider editor reads `56 · FIRSTNAME-0016 LASTNAME-0016 · TEAM-0001` once; the shortcuts table shows the command once and its key text in the second column. |
| 2026-11-27 | dialogs | The settings dialog's four appearance selects and the publish dialog's kind and status selects take the same captions; the shortcuts dialog takes the wide role so its second column fits. | Live captures `settings-labelled.png`, `shortcuts-fixed.png`. |
| 2026-11-27 | D13 | The docks fill: `dock()` paints the panel surface at the rect the layout gives it and lays the content inside (`Panel` is inline and content-sized, and its scrolling body reports an unbounded room, so it could never fill). The feed's heading band now reads `Crossings · <count>` as the design's does. | Live: both cards run from under the entry row to the status bar; measured 508 pt offered, 508 pt painted. |
| 2026-11-27 | D12 | The status bar is no longer a notice sink for store paths: `app.rs` logs a finished backup and posts `Backup written` only when the operator asked for one, and `status_readout` renders `<status word> · <elapsed>` (the design's bar) instead of a path. | Fresh launch: the bar reads `NO RIDE`; the NDJSON log carries `{"event":"backup","fields":{"path":…}}`. |
| 2026-11-27 | D1 (two title rows) | `ui/window.rs` now sets `AppSpec::frame(decide_for(…, TitleBarPref::Integrated))`. An unset frame is drawn by the OS *and* by the console's `TitleBar`; `Integrated` resolves on macOS to a full-size content view with the title bar hidden, so there is one title row. The title is drawn as leading content, left-aligned, not the bar's centred gap title. | Live AX: no `AXStaticText` window title; window 1480×812 → 1480×780 pt; the bar reads `RiverCrossing` at the leading edge. |
| 2026-11-27 | D2, D3 | The rail is a 118 pt labelled column of icon + word (`Library`, `Riders`, `Teams`, `Issues`, `Standings`, `Audit`, `Settings`) with catalog icons: the shell clips a content-sized slot to the width the buttons already elided to, so the rail is body content (`body` allocates the column and the workspace beside it). | Live: every rail word renders in full; the labels previously read `...`. |
| 2026-11-27 | D1 (tab strip) | `ui/macos.rs` sets `NSWindow.tabbingMode = disallowed` for every window at launch (macOS-only dependency, called from `run()` in `ui/app.rs`). AppKit gives a window a tab strip whenever the Mac's tabbing preference is "always", and it injects View ▸ Hide Tab Bar and Show All Tabs only while a tab strip exists. | Live AX after relaunch: no `AXTabGroup`; window 1480×854 → 1480×812 pt; the traffic lights are back at 8,8 / 31,8 / 54,8; the View menu is now *Show Total Times on Crossings Panel*, *Show Lap Time on Crossings Panel*, separator, 90–150 %, separator, *Enter Full Screen*. |

## 7. Populated console — what real data exposes

Captured by driving the app over the control channel: `mi_new_ride`, the eight setup fields filled
through their accessible names, `Generate Riders` (175 riders), `mi_start_ride`, then twelve plates
recorded through the plate field. Artifact: `design/screenshots/current-ui/console-populated.png`,
compared with the mockup in `compare-console-populated.png`.

The shell now matches the design's structure — one title row, labelled rail, header card, two clock
cards, entry card, two dock cards, filled chips. What the data exposes, still open:

| # | Defect | Evidence | Site |
|---|---|---|---|
| D19 | The feed shows four of the design's eight columns. `Time`, `Team`, `Lap time` and `Total` are absent; the two time columns exist behind View ▸ Show … and are off by default, while Time and Team have no column at all. | App: `Name Plate Card Lap`; design: `Time Plate Name Team Card Lap Lap time Total`. | `views/console.rs` `feed_columns`, `feed_rows` |
| ~~D20~~ | **Withdrawn** — not a defect. Twelve crossings inside the minimum lap are *held* for review, and `cards_dealt` counts crossings that are not held, so `Cards 0` beside 12 held crossings is correct. | | |
| D21 | Card cells render as `9D`, `10H`, `4S` — rank plus a suit letter. The design draws the suit glyph, red for hearts and diamonds. | Feed column `Card`. | `presenters/data_source.rs` card text |
| D22 | The review panel's table runs out of room: `Name` is clipped and `Team` is cut at the panel edge; the design's review table is `Plate · Name · Laps`. | Review card, Riders tab. | `views/console.rs` `riders_tab` |
| D23 | The clock read-outs are body-sized; the design's are the largest type on the screen. | `0:00:37` beside the design's `02:14:07`. | `views/console.rs` `clock_column` |

## 8. The last pass, and what it leaves

| # | Item | State |
|---|---|---|
| R1 | The rail stacks its icon over its word, as the design does: each entry is a cell of its own that senses the click, fills under the pointer and reports itself as a button named by its word. | done |
| R2 | The ride header carries the organizer-logo tile between the status word and the statistics; it shows the chosen logo file's name and the design's `LOGO` placeholder until one is picked. | done, but the tile shows the file's name, not the bitmap: rendering the image needs the imaging path |
| R3 | Ride Setup gains the design's tie-break order (three captioned choices over the frozen `tiebreak_list` field) and the organizer-logo row (`logo_preview_bmp`, `logo_status_lbl`, `logo_browse_btn`); its primary button reads `Save`. | done, with two notes: the tie-break is three selects rather than the design's drag chips, and Browse reports the missing platform file picker |
| R4 | The clock read-outs take the display role, the largest type the theme has. | done |
| R5 | The review table is `Plate · Name · Laps`, the laps read from each entry's crossing count. | done |
