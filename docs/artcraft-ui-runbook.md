# ArtCraft UI Runbook — building a RiverCrossing UI with craft-libs

A **process runbook** for producing an ArtCraft (craft-libs) look and feel in this repository: the
files to read, the crates to use, the token system, the widget mapping, and the verification steps. It
is the operational companion to `design/docs-md/visual-style.md` (the design tokens) and
`docs/ui-porting-runbook.md` (the running wx→egui lessons log). Update it at the end of every UI phase.

## 1. Source of truth (read these first)

All craft-libs paths are relative to the pinned checkout
`/Users/mark/.cargo/git/checkouts/craft-libs-d830b991a08c6c3d/243d68e/` (rev
`243d68e12d05e89122db6d154924bf1992e10d38`).

| Concern | Files |
|---|---|
| Brand theme + tokens | `crates/theme/themes/artcraft.json`, `crates/theme/sizes/compact.json`, `crates/theme/sizes/comfortable.json` |
| Token definitions | `crates/theme/src/tokens/color.rs`, `tokens/size.rs`, `tokens/type_motion.rs`, `crates/theme/src/scale.rs`, `values.rs`, `builtins.rs`, `id.rs` |
| Widget catalog | `crates/ui-widgets/src/{buttons,choice,text,pickers,list,table,tabs,menu,bars,display,notices,popover,form,value_drag}/mod.rs` |
| Panels / dialogs / docking | `crates/ui-panels/src/{sections,rails,dialog,docking}/mod.rs` |
| Patterns | `crates/ui-patterns/src/patterns/` (AppShell, preferences, form dialogs) |
| Shell / window | `crates/ui-shell/src/shell/{app,embed,app_shell}.rs`, `crates/window/src/` |
| Menu (native macOS) | `crates/menu/src/{lib,mac,driver,layout}.rs`, `crates/window/src/native_menu/` |
| Dialogs (native + in-app) | `crates/dialogs/src/lib.rs`, `crates/ui-panels/src/dialog/` |
| Theming hook | `crates/ui-contract/src/runtime/env/root.rs` (`RootEnv`) |
| Reference app | `crates/ui-gallery/src/` (the gallery demonstrates every widget/pattern) |

The **Python behaviour oracle** lives at `/Users/mark/src/rivercrossing/src/rivercrossing/ui/` (windows,
controls, flows, gates). The consolidated inventory is `design/docs-md/ui-contract.md`.

## 2. The ArtCraft look (summary)

Flat, dense, pro-app — closer to a modern Photoshop/Affinity UI than a consumer app. Depth comes from a
lightness ladder of grey panels, not borders or bevels. One calm blue accent; semantic status colors;
pill buttons; small radii everywhere else; compact 24px controls; semibold titles.

### 2.1 Color tokens (light / dark)

| Role | Light | Dark |
|---|---|---|
| `surface.background` | `#e2e2e2` | `#424242` |
| `surface.canvas` | `#d6d6d6` | `#282828` |
| `surface.panel` | `#f0f0f0` | `#535353` |
| `surface.sunken` | `#ffffff` | `#454545` |
| `text.normal` | `#141414` | `#eeeeee` |
| `text.muted` | `#404040` | `#d4d4d4` |
| `text.faint` | `#787878` | `#a0a0a0` |
| `line.divider` / `line.border` | `#d6d6d6` | `#3e3e3e` / `#424242` |
| `line.control_edge` | `#bababa` | `#686868` |
| `state.hover` / `pressed` / `selected` | `#e2e2e2` / `#cecece` / `#cccccc` | `#626262` / `#707070` / `#6b6b6b` |
| `accent.solid` | `#378ef0` | `#378ef0` |

Seeds: neutral `#535353`, accent `#78adf6`, good `#85dc6a`, warn `#e8b046`, bad `#ff8080`, info `#78adf6`.
Status roles (`status.{good,warn,bad,info}.{fg,bg,border,solid,…}`) drive success/caution/error/info.

### 2.2 Typography (compact)

`caption 10.5/14 · label 11.5/16 · body 12/16 · body_strong 12/16 · title 13/18 · heading 15/20 ·
body_large 15/20 · mono 11.5/16 · display 38/46`. Titles/headings/display use the semibold face; body
uses the regular face.

### 2.3 Spacing (compact)

Scale `0,2,4,6,8,12,16,20,24,32`. Semantic gaps `tight 2 · inline 4 · row 4 · stack 6 · item 8 ·
section 12 · card_column 12 · card_row 18`; pads `control 6 · regular 8 · roomy 16 · chip 4 ·
segment 10`; `inset.screen 16`.

### 2.4 Radii and elevation (compact)

`control 3 · surface 4 · window 6 · panel 0 · button full (pill) · pill full`. Edge style `flat` (no
bevels). Shadows only on floating layers: popup/dialog `[0,6] blur 20`, floating window `[0,10] blur 32`,
knob `[0,1]`. Panels and chrome stay flat.

## 3. Widget vocabulary (the mapping menu)

| Need | `craft_ui` module |
|---|---|
| Buttons (primary/secondary/ghost/danger, icon, toggle, split, link) | `buttons::{Button, IconButton, ToggleButton, SplitButton, Link}` |
| Checkbox / switch / radio / segmented / rating | `choice::{Checkbox, Switch, Radio, RadioGroup, Segmented, Rating}` |
| Text / number / hex / chip / linked fields | `text::{TextField, NumberField, NumberStepper, HexField, ChipField}` |
| Select / combo / tree picker / date / autocomplete | `pickers::{Select, ComboBox, TreePicker, DatePicker, Autocomplete}` |
| Lists / trees / tables / grids | `list::{List, ListRow, OpenSet}`, `table::{Table, MetadataTable}`, `tile_grid::TileGrid` |
| Tabs / document tabs | `tabs::{TabStrip, Tab, DocumentTabs, OverflowMenu}` |
| Menu bar (in-window) | `menu::{MenuBar, Menu, MenuItem}` |
| Tool / options / status bars | `bars::{Bar, BarKind, BreadcrumbBar, StatusMessage, StatusProgress, StatusReadout}` |
| Display / status | `display::{Text, Icon, Badge, Kbd, Progress, Spinner, Divider}` |
| Notices / toasts | `notices::{Notice, NoticeCenter, ToastStack}` |
| Popover / tooltip | `popover::{Popover, PopupList, Tooltip}` |
| Forms | `form::{Field, FieldGroup, SliderField}` |
| Sliders / dials / gauges | `value_drag::{Slider, RangeSlider, AngleDial, PointPicker}` |
| Panels / sections / rails | `sections::{Panel, Section, TitleRow, FooterBar, GroupBox}`, `rails::ToolRail` |
| Dialogs / prompts | `dialog::{Dialog, DialogFrame, DialogStack, Alert, Prompt}` |
| Docking / splitter | `docking::{SplitArea, Splitter, DockGroup, DockPanels}` |
| App shell | `patterns::AppShell` + `shell::{TitleBar, Title, CaptionButton}` |

**Status readouts replace custom gauges.** The Python RaceClock dials and StopLight lamp become
`Progress` + numeric `Text` and a status `Badge`/`Notice` (the state word always shows; colour is never
the only channel). Tables and lists replace the wx DataView feeds.

## 4. Shell, menu, dialogs, window — the correct craft app shape

1. Build `AppSpec::new(app_id, title, Defaults)` with a placement `Store` (`craft::ui::shell::app`).
2. Implement `ShellApp` (`frame_input`, `ui`, `logic`, plus optional `tick`, `close_requested`,
   `output`).
3. Call `run(spec, |ctx| make_app(ctx))`; the runtime is installed inside `run`.
4. Return the real `RootEnv` (mode/contrast/density/text_size/theme) from `frame_input`.

**Menus.** Two types share the name:
- `craft::ui::menu::MenuBar` — the in-window egui widget (Windows/Linux).
- `craft::menu::MenuBar` + `craft::menu::mac::install(ctx, &bar) -> Option<NativeMenu>` — the real native
  macOS menu bar (AppKit/muda), enabled by the `craft` façade feature `menu` (which pulls
  `craft-window/appkit`). `install` returns `None` off macOS / already installed / off-main-thread — keep
  the in-window bar on `None`. Per frame: `poll → set_gate → take_clicks → dispatch → sync_with`.

**macOS window behaviour.** The close button hides/minimizes the main window (app keeps running; Dock
restores it); quit only via File→Quit / ⌘Q / app-menu Quit. `ShellApp::close_requested` returns
`CloseDecision::{Close,Hold}`; on macOS return `Hold` and issue `ViewportCommand::Minimized(true)` (no
quit prompt); on Windows/Linux run the quit confirm.

**Theming.** `RootEnv` carries `mode`, `contrast`, `density`, `text_size`, `scale`, `theme`; tokens
resolve per theme × mode × contrast × density × text size. `craft.artcraft` is the suite default. Do not
call `ctx.set_style`/`ctx.set_visuals`/hard-coded colors — express appearance through `RootEnv` and the
semantic tokens.

## 5. The repo's UI crate (where the code lives)

`crates/rivercrossing-ui-egui/src/ui/`:

- `app.rs` — `ShellApp` impl, eframe entry, deferred-effect draining, quit/close.
- `window.rs` — `AppSpec`, product identity, placement.
- `state.rs`, `state/exports.rs` — `AppState`, action dispatch, dialogs, models.
- `action/{id,spec,dispatch,effect,enable,drift}.rs` — the frozen 53-action registry + gates.
- `views/*` — one module per screen (to be rebuilt on `craft_ui`).
- `presenters/*` — UI-free view models/decisions (keep).
- `dialogs/{spec,stack}.rs` — dialog specs and stack.
- `ids.rs` — the 30-window/67-field registry (drift-checked).
- `automation/*` — the control channel, selectors, scenario model.
- `worker.rs`, `sound.rs`, `logging.rs`, `publish_job.rs`, `native.rs`.

Tests: `src/**/tests.rs` (Tier A semantic, `-P unit`) and `tests/ui/main.rs` (30 goldens, `-P ui`).

## 6. Process — how to produce the ArtCraft UI (phases)

1. **Consolidate the contract** — `design/docs-md/ui-contract.md`: every window, menu item, control,
   flow, gate, and parity gap. Source: the Python oracle + `gui-windows.md`.
2. **Capture the design system** — `design/docs-md/visual-style.md`: tokens, typography, spacing,
   radii, elevation, appearance, widget vocabulary (this runbook's §2–§3).
3. **Map controls** — `design/docs-md/control-mapping.md`: every frozen §15b id → a `craft_ui` widget +
   accessible label/role. No frozen id unmapped.
4. **Spec screens** — `design/docs-md/screen-specs.md`: per-window layout (docked shell, panels, feed),
   states, interactions.
5. **Mockups** — `design/mockups/`: plain static HTML over a shared `tokens.css` (vendored ArtCraft
   values as CSS custom properties) and `chrome.css`; one gallery page with per-screen deep links; light + dark via
   `data-theme="dark"`; user approval gate before code.
6. **Implement** — fix the activation/repaint contract first, then the native macOS menu, then
   close-hides, then the `RootEnv` theming hook, then rebuild the views on `craft_ui` per the mapping.
7. **Verify** — re-baseline the 30 goldens on the Linux golden host; run `./scripts/validate-rust.sh`
   and the two Tier B smokes.

## 7. Verification commands

```bash
# Tier A semantic (unit)
cargo nextest run -P unit --locked
# Tier A-snapshot (goldens) — compare
cargo nextest run -P ui --locked
# Re-baseline goldens (Linux golden host only)
UPDATE_SNAPSHOTS=force WGPU_BACKEND=vulkan cargo nextest run -P ui -p rivercrossing-ui-egui --locked
# Full per-PR gate
./scripts/validate-rust.sh
```

The drift tests (`action/drift.rs`) fail if `gui-windows.md` and the compiled registry disagree; the
layer check (`cargo xtask layers`) fails if any crate below `ui-egui` depends on egui/eframe/winit.

## 8. Gotchas (recorded, not theoretical)

- **`craft::menu::mac::install` returns `None`** off macOS / already installed / off-main-thread — keep
  the in-window bar; gate with `#[cfg(target_os = "macos")]`.
- **Two `MenuBar` types** — `craft::menu::MenuBar` (native model) vs `craft::ui::menu::MenuBar` (in-window
  widget). Do not confuse them.
- **macOS close** must hide/minimize, not quit — see §4.
- **Frozen zoom ids** (`mi_zoom_90…150`) map to `ctx.set_zoom_factor`; they are a separate axis from the
  `RootEnv` appearance model and must not be retired.
- **Frozen ids and labels are the automation contract** — `Start Ride`, `Stop Ride`, `Undo`, `Record`,
  `Plate`; renaming a rendered label silently breaks selectors.
- **Coverage carve-out** — `scripts/validate-rust.sh` excludes `src/ui/views/` and `src/ui/ids.rs` but
  not `presenters/` or `state.rs`; keep the presenter/state changes covered.
- **egui already repaints on input** — a pass with input events requests a zero-delay repaint
  (`InputState::wants_repaint_after`); the app's explicit request in `ShellApp::ui` makes the contract
  independent of that. The regression test attributes the request to `ui/app.rs` via
  `repaint_causes()`, because the zero-delay request alone is not a discriminator.
- **Wall clock** — `ShellApp::tick` returns `now + 1 s` while the ride is live and unstopped; without it
  the flattened clock readouts freeze between input events.
- **Goldens are whole-frame PNGs** — any token/spacing change invalidates all 30; re-baseline on one
  golden host, never skip or bump the threshold there.
- **No literal sizes/colors** — craft's `design-lint` rules and `CODINGSTANDARDS-UX-DESKTOP.md` forbid
  magic numbers and named color constants; use semantic tokens.

## 9. References

- `design/docs-md/ui-contract.md` — the window/menu/control/flow contract.
- `design/docs-md/visual-style.md` — the token system and component vocabulary.
- `design/docs-md/control-mapping.md` — frozen id → widget map.
- `design/docs-md/screen-specs.md` — per-window specs.
- `design/docs-md/gui-windows.md` — the drift-checked registry snapshot.
- `docs/ui-porting-runbook.md` — the wx→egui running log.
- craft-libs `crates/ui-gallery/src/` — the worked examples for every widget/pattern.
