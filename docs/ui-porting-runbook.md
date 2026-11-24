# UI Porting Runbook — Python (wxPython) → Rust (egui)

A **running, continuously-updated** log of lessons learned converting the RiverCrossing UI from
wxPython/XRC to egui. Update this file at the end of every UI phase so the next agent starts from
accumulated knowledge instead of re-learning it.

## Status

- [x] EPIC-1 empty egui shell — done
- [ ] EPIC-10 UI automation & Python-parity harness — next (the mechanism first; no screens)
- [ ] EPIC-12 egui UI panels + Action registry — not started (inherits the EPIC-10 harness)

## The translation one-liner

The wx "frozen XRC name + `FindWindowByName` + accelerator table" triple becomes:

**frozen command/widget id + `Action` registry + `egui::KeyboardShortcut`**

- §15 menu map is the routing table.
- §15b frozen-name registry is the frozen id namespace.
- A menu row and its on-screen button dispatch the **same** `Action` id.

## wx → egui equivalence table

| wxPython | egui |
|---|---|
| XRC sizer layout (declarative) | immediate-mode layout in code |
| `FindWindowByName("frozen_name")` | `egui::Id::new("frozen_name")` / `ui.push_id(...)` |
| accelerator table (`Ctrl+` → `Cmd`) | `egui::KeyboardShortcut` with `Modifiers::COMMAND` |
| `wxInfoBar` / status bar | bottom/top panel + toast component |
| `wxNotebook` tabs | egui tabs / `selectable_label` |
| `wxSplitterWindow` sash | egui resizable panels |
| `wxStdDialogButtonSizer` ordering | explicit button-order policy (platform-correct) |
| `wx.MessageDialog` / file pickers | `craft-dialogs` (non-blocking) / OS picker |
| `wx.adv.Sound` (WAV) | `rodio` |
| `wx.App.SetAppearance` dark mode | egui `ctx.style()` / dark mode |
| `DataViewIndexListModel` bold rows | per-row paint (bold) |
| per-monitor DPI | egui `pixels_per_point` |

## Drop outright (no egui equivalent)

- XRC authoring as the source of truth (`.xrc` files, `XmlResource.LoadMenuBar`, resource ids).
- `FindWindowByName` — replaced by explicit egui `Id`s.
- wx class authoring quirks (`wxInfoBar`, `wxDataViewListCtrl` name forcing, `wxMenuBar`).
- Stock IDs (`wxID_OK/CANCEL/...`) and macOS app-menu relocation mechanics — remap to `Action`
  platform-relocation flags.
- "Never restyle standard controls" — egui is fully custom-drawn; the rule becomes "follow host
  platform conventions and honour the a11y floor".

## Rust automation harness

The wx harness mechanics do not port — `SetValue()` firing `EVT_TEXT`, posted `wx.CommandEvent`,
`Destroy()`/stale-window contamination have no egui analogue. The Rust harness (R-87…R-92,
`design/docs-md/automation.md`, `design/docs-md/test-plan.md`) drives the real widgets instead, and
the scenario JSON is the cross-language contract.

| Tier | Surface | Runner | Needs |
|---|---|---|---|
| **A — semantic** | `craft::ui::testing::Fixture` over `Rc<RefCell<RiverCrossingApp>>` in `src/**/tests.rs` | `cargo nextest run -P unit --locked` | no display, no GPU |
| **A-snapshot** | `egui_kittest` `Harness::new_ui` + craft `embed::install`, then per frame `embed::before_input`/`embed::logic`/`app.ui(ui)`, asserted with `snapshot` in `tests/ui/main.rs` | `cargo nextest run -P ui --locked` | lavapipe; goldens are written on the Linux golden host only |
| **B — real window** | `open_and_quit_smoke` + `control_channel_smoke` (spawns `rivercrossing --control 0`) | by hand under `xvfb-run -a -s "-screen 0 1400x900x24"` | Xvfb; no WM |
| **C — Python parity** | shared JSON scenarios; in-process pytest driver in the oracle repo (`FindWindowByName`/`SetValue`/posted `wx.CommandEvent` are in-process calls, `spec.md:273`) | on demand at EPIC-12 exit, never in the gate; runs on the Mac | Python 3.14 / wxPython 4.3.1 |

- **Selector contract.** `Selector::Action { id }` — a frozen §15b id — first;
  `Selector::RoleAndLabel { role, label, window }` second. `window` scopes labels that legitimately
  repeat (`plate_input`, `reason_input`, `continue_btn`, `message_lbl`; names are unique only within a
  top-level window, `spec.md:376`). Multiple matches without a `window` are an error, never an
  arbitrary pick. No `NodeId` (per-run, not reproducible) and no bare label lookup.
- **Scenario model.** `Scenario`/`Step`/`Selector`/`Observation`/`Node` live in
  `crates/rivercrossing-ui-egui/tests/ui/scenario.rs` — test-support only, versioned JSON
  (`"version": 1`) mirrored by `tools/parity/schema.json` for the Python driver. `Step::Expect` is
  the only assertion mechanism.
- **Snapshots.** One golden host (Linux / lavapipe); `kittest.toml` sets `output_path =
  "tests/snapshots"` (package-relative) with `[linux] threshold = 0.6` and `[mac] threshold = 2.0`.
  Missing glyphs panic, so the app fonts (`craft::ui::fonts::UiFonts`) are installed. Regenerate
  deliberately on the golden host only: `UPDATE_SNAPSHOTS=force cargo nextest run -P ui --locked`.
- **Artifacts.** Tier B writes `target/gate/control-smoke.log` and PNGs under `target/control/`;
  Tier C writes `Observation` JSON + PNGs under `target/gate/parity/<scenario>/`, or
  `target/gate/parity/SKIP.md` naming the missing prerequisite.

## Keep unchanged (product contract)

- Keyboard-first; single-source shortcut table (generated from the `Action` registry).
- Dialog semantics (Esc cancels, Enter activates default, focus locked in modal, initial-focus rules).
- Destructive = confirm-or-undo, name the object, default to safe button.
- Disable, never hide; one menu tree with platform relocation of About/Settings/Quit.
- Min size fits 1366×768; persist/restore geometry; resolution-aware.
- WCAG 2.2 AA: contrast, stable ids as labels, no colour-only meaning, no hover-only essentials.

## Open decisions

- **Resolved — snapshot rendering and OS normalisation (EPIC-10):** one golden set from one golden
  host (Linux / lavapipe), with per-OS thresholds instead of per-OS goldens or skips —
  `[linux] 0.6` (the golden host) and `[mac] 2.0` in `kittest.toml`. Goldens are regenerated only on
  the golden host with `UPDATE_SNAPSHOTS=force cargo nextest run -P ui --locked`; there is no second
  golden set and no skip path.
- WordPress credential storage: parity (plaintext settings JSON) vs OS keychain.
- Tailwind/Node CSS build: keep the Node step as an xtask, or vendor the already-compiled CSS.
