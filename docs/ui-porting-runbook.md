# UI Porting Runbook — Python (wxPython) → Rust (egui)

A **running, continuously-updated** log of lessons learned converting the RiverCrossing UI from
wxPython/XRC to egui. Update this file at the end of every UI phase so the next agent starts from
accumulated knowledge instead of re-learning it.

## Status

- [ ] EPIC-1 empty egui shell (not started — Phase 2 of the repo setup)
- [ ] EPIC-11 egui UI panels + Action registry (not started)

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
- wx test harness mechanics (`SetValue()` fires `EVT_TEXT`, posted `wx.CommandEvent`, `Destroy()`/
  stale-window contamination) — remap to `egui_kittest` / command-id assertions.

## Keep unchanged (product contract)

- Keyboard-first; single-source shortcut table (generated from the `Action` registry).
- Dialog semantics (Esc cancels, Enter activates default, focus locked in modal, initial-focus rules).
- Destructive = confirm-or-undo, name the object, default to safe button.
- Disable, never hide; one menu tree with platform relocation of About/Settings/Quit.
- Min size fits 1366×768; persist/restore geometry; resolution-aware.
- WCAG 2.2 AA: contrast, stable ids as labels, no colour-only meaning, no hover-only essentials.

## Open decisions

- WordPress credential storage: parity (plaintext settings JSON) vs OS keychain.
- Tailwind/Node CSS build: keep the Node step as an xtask, or vendor the already-compiled CSS.
