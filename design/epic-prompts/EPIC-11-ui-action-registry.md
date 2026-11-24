# EPIC-11 — egui UI + Action registry (`rivercrossing-ui-egui`)

Load into a `tdd-rust-writer` agent. TDD (RS-14). Run after EPIC-1 and the domain crates (EPIC-2..10).
This is the largest EPIC — break it into window/area chunks, one agent per chunk, sequentially.

## Goal

The egui front end that reproduces the Python app's screens and functionality, driven by a command/
Action registry (the replacement for XRC frozen names + `ids.py`).

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
