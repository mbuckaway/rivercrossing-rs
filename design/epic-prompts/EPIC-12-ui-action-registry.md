# EPIC-12 — egui UI + Action registry (`rivercrossing-ui-egui`)

Load into a `tdd-rust-writer` agent. TDD (RS-14). Run after EPIC-10 (the automation harness) and the
domain crates (EPIC-2..9 and EPIC-11).
This is the largest EPIC — break it into window/area chunks, one agent per chunk, sequentially.

## Goal

The egui front end that reproduces the Python app's screens and functionality, driven by a command/
Action registry (the replacement for XRC frozen names + `ids.py`). It lands as five PRs: **12a**
foundation, **12b** editors/CSV, **12c** lifecycle/corrections/system, **12d** results/exports/
publish/simulator, **12e** close-out.

## Chosen approach (recorded before 12a)

- **Window inventory: 30** (§15b file map, `spec.md:392`), not 23 (the stale EPIC-1 screenshot set)
  and not the prompt's undefined "27". `design/docs-md/gui-windows.md` is produced first, with one
  fenced JSON machine block (windows/controls/actions/fields) that the ids-drift unit test compares
  to the Rust registry; the human tables document it. The registry has **53 unique actions** = 50
  `mi_*` + 3 stock ids (the View row expands to 9 items).
- **Settings:** `rivercrossing-store::settings` reads/writes the Python `settings.json` schema-
  compatibly (29 fields, same path, same coercers, atomic, never-raising); `craft-settings` is TOML
  and is not used. Credentials stay plaintext at parity.
- **Geometry:** craft-window's `window.json` via `AppSpec.placement`; `splitter_sash` in
  `settings.json`; Python's `window_geometry` seeds the first run only.
- **Sound:** universal `rodio` 0.23 (`playback` + `wav`), WAVs embedded with `include_bytes!`, a
  `SoundBackend` seam; a missing device/asset/cue is silent. Linux needs `libasound2-dev` to build.
- **Dialogs:** in-crate `DialogStack` over `egui::Modal` owns Esc/Enter/default/focus; native
  file/message dialogs via `craft-dialogs` with `set_override` as the test seam.
- **Menu:** `craft::menu` model (`Item`/`CommandRef`/`Chord`) rendered by `craft::ui::menu::MenuBar`.
- **Async:** one `StoreWorker` thread owns `Store` (R-02); exports/imports/publish/backups run there.
- **Automation:** `ui.command` dispatches any Action id, `ui.inspect`/`ui.set` use qualified
  `{field, window?}` ids, `ui.dialog.*` drives the host; `probe_*` is retired for `app_quit`.
- **Verification split:** every gate, golden and Tier B smoke runs on the headless Linux box (Xvfb +
  lavapipe); the Mac is needed only for the operator visual pass, audio, native dialogs and Tier C.
- **Empty Enter is silent** (the Python presenter returns early, `console.py:491-493`); the spec's
  buzz line (`spec.md:242`) is recorded as a discrepancy to amend.

## Context

Reproduce the Python app's screens from `design/screenshots/` and the window inventory, not invented
fresh. The wx "frozen XRC name + `FindWindowByName` + accelerator table" triple becomes "frozen
command/widget id + `Action` registry + `egui::KeyboardShortcut`" (`CODINGSTANDARDS-UX-DESKTOP.md`).
The Python oracles: `src/rivercrossing/ui/{app.py,views/*,presenters/*,ids.py}` in
`/Users/mark/src/rivercrossing`.

## Refs

- `CODE_STRUCTURE.md` (rivercrossing-ui-egui) · `architecture.md` §5, §6
- `requirements.md` R-02, R-03, R-04, R-05, R-21, R-24, R-30..R-38, R-51, R-65, R-73, R-76, R-79,
  R-81, R-82, R-83, R-84, R-85
- `design/docs-md/xrc-windows.md` (window inventory + frozen names), `spec.md` §15 (menu map)
  and §15b (frozen-name registry), `design/screenshots/`

## Tests-first (Red)

Write failing command/registry tests first: every `Action` resolves a stable id; the §15 menu map and
§15b frozen ids are fully registered; ids-drift check (a name appears once per window and is
referenced); menu rows and on-screen buttons dispatch the same id; keyboard-first shortcuts;
dialog/destructive semantics (Esc/Enter/focus, confirm-or-undo).

## Implement (Green → Refactor)

1. **Produce `gui-windows.md`** — the frozen command/widget-id registry, enumerated from the Python
   `xrc-windows.md` + §15/§15b + screenshots (settle the 23-vs-27 window count).
2. Build the `Action`/command registry and the panel/dialog map (console, riders/teams editors,
   results, audit, settings, simulator, about, publish).
3. Wire the CSV surface: File ▸ Import Riders CSV… and Export Riders CSV…, `csv_preview_dlg`
   (summary label, `Row | Problem` list, both opt-in checkboxes with *Convert teams of 1* unchecked,
   Import disabled while conflicts > 0), and Results ▸ Export Standings CSV…; dispatch
   `rivercrossing-csvio` and persist the mutated roster through the store.
4. Port `ui/presenters/*` decision logic; wire panels to the domain crates; sound via `rodio`;
   settings via `craft-settings`; native dialogs via `craft-dialogs`.
5. SPDX line on every `.rs`.

## Done when

- The open/quit smoke and command-level tests pass; `xtask layers` green (no egui below ui-egui);
  ids-drift green.
- The Python screens are reproduced; every §15 route is drivable in all ride states.
- `cargo fmt --check` + `cargo check` pass; update `docs/ui-porting-runbook.md`.
