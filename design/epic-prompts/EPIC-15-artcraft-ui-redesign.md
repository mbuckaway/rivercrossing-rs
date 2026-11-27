# EPIC-15 — ArtCraft UI redesign

Executes the approved design. This EPIC replaces the wx/XRC-style egui views with the ArtCraft
(craft-libs) look and behaviour while keeping the backend crates and the frozen contract. Load each
task into a `tdd-rust-writer` agent (sequential, Red→Green); the main session owns the docs.

## References (all in this repo)

- `design/docs-md/ui-contract.md` — the window/menu/control/flow/gate contract and the parity gaps.
- `design/docs-md/visual-style.md` — ArtCraft tokens and widget vocabulary.
- `design/docs-md/control-mapping.md` — frozen id → `craft_ui` widget.
- `design/docs-md/screen-specs.md` — per-window layout, states, interactions.
- `design/mockups/index.html` — the approved mockups (light/dark; deep-link each screen).
- `docs/artcraft-ui-runbook.md` — the build runbook and gotchas.
- `design/docs-md/gui-windows.md` — the drift-checked registry (do not rename frozen ids).

## Task order

1. **Activation/repaint fix (do first, red→green).** Add the one-line request at the end of
   `RiverCrossingApp::ui` (after the views and the control channel): if the pass consumed a pointer click
   or a key press, call `ctx.request_repaint()`. Also make `tick` return `Some(now + 1 s)` while a ride is
   live and unstopped (the wall clock). Write the red test against the repaint **request** (not a pumped
   `logic`): `set_repaint_observer` plus a console-button click, asserting a zero-delay request on the
   click pass. Single-source the console buttons' enabled state from the Action registry so a
   clickable-but-refused button cannot exist. (`AppState::act`/`apply` have no `Context`; do not thread
   one through 17 call sites.)
2. **Native macOS menu.** Add the `craft` feature `menu` (regenerate `Cargo.lock`). Build
   `craft::menu::MenuBar` from the Action registry (constructors, not struct literals), tag roles
   (`MenuRole::{File,View,Help}`, `ItemRole::{About,Settings,Quit}`), call `craft::menu::mac_layout` and
   then `craft::menu::mac::install` in the eframe creator closure; on `None` fall back to
   `install_app_menu`. Per frame in `logic`: `poll → set_gate → take_clicks → map `CommandRef` back to
   `ActionId` → act → sync_with(mac_layout)`. Keep the in-window bar when the native handle is `None`
   (key on the `Option`, not `cfg`). Add the chord mapping (Ctrl ⇔ ⌘) and key handling for the documented
   shortcuts.
3. **macOS close-hides.** In `close_requested`, after the existing `quit_confirmed || quit_after ||
   control` guard, on macOS return `Hold` and set a `minimize_next_frame` flag; the next `logic` pass
   emits `ViewportCommand::Minimized(true)` (never in the same frame as `hold_close`). On Windows/Linux run
   the quit confirm. `CloseDecision::Close` already triggers `craft_quit::quit()` via `embed::logic`.
   Keep the Exit row tagged `ItemRole::Quit` (do not route `wxID_EXIT` through `craft_quit::on_menu_event`).
   Parameterise the two existing close tests with `rstest`.
4. **ArtCraft theming hook.** Return the real `RootEnv` (mode/contrast/density/text size/theme) from
   `frame_input`; keep the seven frozen `Target::Zoom` ids on `ctx.set_zoom_factor`; migrate
   `settings.appearance` to the mode model; add the net-new contrast/density/text-size/theme controls.
5. **Docked-shell view layer.** Rebuild the console and dialogs on `craft_ui` per `control-mapping.md`
   and `screen-specs.md`: `AppShell` + rail + docked panels + feed `Table`; dialogs via `craft_ui::dialog`;
   flattened gauges (`Progress` + readout; state `Badge` with the word).
6. **Parity gaps.** Implement `new_ride_btn` (enabled only with no ride; creates the test ride and
   closes); render `go_btn` as a real `Button::Custom("go_btn")` labelled "GO" and the dialog default;
   add the spec-vs-registry consistency test; single-source the console button labels.
7. **Preserve the frozen contract.** All 53 action ids, 30 window ids, 67 fields, and the accessible
   names `Start Ride`/`Stop Ride`/`Undo`/`Record`/`Plate`.

## Done when

- `cargo nextest run -P unit --locked` green (drift/spec/automation), activation proven by a red→green
  test.
- `./scripts/validate-rust.sh` green; both Tier B smokes pass; macOS menu/close verified by a real-window
  run on the Mac.
- All 30 snapshot goldens re-baselined on the Linux golden host and committed; drift/spec tests green;
  coverage ≥90/90 on the enforced crates.
- The app reads as the approved ArtCraft mockups in light and dark.
