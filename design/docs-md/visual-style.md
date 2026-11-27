# RiverCrossing — ArtCraft Visual Style

The design system for the RiverCrossing ArtCraft (craft-libs) UI. It supersedes the wx/XRC visual layer
while keeping the product UX contract. Tokens are the ArtCraft suite defaults at the pinned craft-libs
rev; this document records them so mockups and code share one source.

Authority: `docs/artcraft-ui-runbook.md` (process), `design/docs-md/ui-contract.md` (structure), and the
craft-libs checkout `/Users/mark/.cargo/git/checkouts/craft-libs-d830b991a08c6c3d/243d68e/` (tokens,
widgets). Values are cited to the pinned files; a token change is a craft-libs rev change, not a local
edit.

## 1. Identity

ArtCraft is a **flat, dense, pro-app** visual language: a lightness ladder of neutral-grey surfaces, one
calm blue accent, semantic status colours, pill buttons, small radii elsewhere, and compact 24px
controls. Depth comes from surface lightness and hairline dividers, not borders or bevels.

## 2. Colour tokens

Source: `crates/theme/themes/artcraft.json`, `crates/theme/src/tokens/color.rs`, `values.rs`.

### 2.1 Surfaces and text

| Role | Light | Dark |
|---|---|---|
| `surface.background` | `#e2e2e2` | `#424242` |
| `surface.canvas` | `#d6d6d6` | `#282828` |
| `surface.panel` | `#f0f0f0` | `#535353` |
| `surface.sunken` | `#ffffff` | `#454545` |
| `text.normal` | `#141414` | `#eeeeee` |
| `text.muted` | `#404040` | `#d4d4d4` |
| `text.faint` | `#787878` | `#a0a0a0` |
| `line.divider` | `#d6d6d6` | `#3e3e3e` |
| `line.border` | `#d6d6d6` | `#424242` |
| `line.control_edge` | `#bababa` | `#686868` |
| `chrome` (title/rail/status) | `#e8e8e8` | `#3c3c3c` |
| `surface.raised` | `#f7f7f7` | `#5c5c5c` |

### 2.2 State and accent

| Role | Light | Dark |
|---|---|---|
| `state.hover` | `#e2e2e2` | `#626262` |
| `state.pressed` | `#cecece` | `#707070` |
| `state.selected` | `#cccccc` | `#6b6b6b` |
| `accent.solid` | `#378ef0` | `#378ef0` |
| `accent.text` | `#1062c4` | `#378ef0` |
| `accent.strong` (button fill) | `#1062c4` | `#4a8fe8` |
| `accent.soft` | `#e3eefc` | `#26364a` |
| `bad.fill` (danger button) | `#c42b1c` | `#a12a2a` |

`accent.strong` is a darker step than `accent.solid` so white button text meets 4.5:1; `bad.fill` is a
darker red for the same reason (white on `#ff8080` is only 2.43:1). `accent.solid` stays the brand accent
for marks, focus rings and progress fills.

### 2.3 Status / semantic tokens

| Role | Light bg / fg | Dark bg / fg |
|---|---|---|
| good | `#e7f6e0` / `#2f6b1f` | `#24331f` / `#b7e8a8` |
| warn | `#fbefd6` / `#7a5310` | `#3a2f18` / `#f0cf95` |
| bad | `#fbe0e0` / `#8a2a2a` | `#3a2020` / `#ffb3b3` |
| info | `#e3eefc` / `#1d4f8f` | `#243345` / `#b7d4f7` |
| neutral | `#e4e4e4` / `#404040` | `#4a4a4a` / `#d4d4d4` |

Seeds: neutral `#535353`, accent `#78adf6`, good `#85dc6a`, warn `#e8b046`, bad `#ff8080`, info `#78adf6`.

### 2.4 Ride-state mapping

`RUNNING → good`, `stopped RUNNING / REOPENED → warn`, `DRAFT → neutral`, `FINISHED → info`. The
**state word is always rendered beside the colour** (no colour-only meaning). This supersedes the Python
StopLight's red-for-DRAFT/FINISHED lamp colours; behaviour is unchanged (the word carries the state).

### 2.5 Rules

- Every colour is a semantic token. No `Color32::WHITE`, no named constants, no hard-coded hex in code
  (craft `design-lint`; `CODINGSTANDARDS-UX-DESKTOP.md`).
- Light and dark are one token table resolved per mode, not an inversion.
- Contrast: text `≥ 4.5:1` standard, `≥ 3:1` large; focus rings/accent marks `≥ 3:1`; high-contrast mode
  raises to `7:1`.

## 3. Typography

Nine roles (`crates/theme/src/tokens/type_motion.rs`, `sizes/compact.json`):

| Role | size / line | face |
|---|---|---|
| caption | 10.5 / 14 | ui |
| label | 11.5 / 16 | ui |
| body | 12 / 16 | ui |
| body_strong | 12 / 16 | ui semibold |
| title | 13 / 18 | ui semibold |
| heading | 15 / 20 | ui semibold |
| body_large | 15 / 20 | ui |
| mono | 11.5 / 16 | mono |
| display | 38 / 46 | ui semibold |

Comfortable density adds about 1pt. Text-size steps (Tiny/Small/Medium/Large) scale these.

## 4. Spacing, radii, elevation

- **Spacing**: base `2,4,6,8,12,16,20,24,32`; gaps `tight 2 · inline 4 · row 4 · stack 6 · item 8 ·
  section 12 · card_column 12 · card_row 18`; pads `control 6 · regular 8 · roomy 16 · chip 4 ·
  segment 10`; `inset.screen 16`.
- **Radii**: `control 3 · surface 4 · window 6 · panel 0 · button full (pill) · pill full`.
- **Elevation**: flat surfaces; shadows only on floating layers — popup/dialog `[0,6] blur 20`,
  floating window `[0,10] blur 32`, knob `[0,1]`. The app shell itself is **not** a floating layer (no
  shadow on the root window).
- **Edge**: `flat`; no bevels.

## 5. Surfaces and layout

`SurfaceKind` (`crates/ui-contract/src/style/look.rs`) drives fill/radius/edge/shadow.

The app frame is the ArtCraft **docked shell** (`craft_ui::shell::AppShell`):

- **Title bar** (`shell::{TitleBar, Title, CaptionButton}`).
- **Tool rail = navigation** (`rails::ToolRail`): it dispatches the same Action ids as the menu.
- **Docked panels = content** (`docking::{SplitArea, DockGroup, DockPanels}`): live standings, Needs
  Review, ride info.
- **Content canvas**: the crossings feed (`table::Table` / `list::List`).
- **Status bar** (`bars::Bar` of kind Status): the six count chips (`display::Badge`), last action, shoe.

Minimum window 1100×780 (`window.rs` `MIN_SIZE`), resizable.

## 6. Component vocabulary

| Need | Component |
|---|---|
| Primary/secondary/danger buttons | `buttons::{Button, IconButton, ToggleButton, SplitButton, Link}` |
| Check / switch / radio / segmented | `choice::{Checkbox, Radio, RadioGroup, Switch, Segmented}` |
| Text / number / date / select | `text::{TextField, NumberField}`, `pickers::{Select, ComboBox, DatePicker}` |
| Feed / standings / roster / library / audit | `table::{Table, MetadataTable}`, `list::{List, ListRow}` |
| Tabs (Results Teams/Solo; review Riders/Needs Review) | `tabs::{TabStrip, Tab}` |
| Menus (in-window; native on macOS) | `menu::{MenuBar, Menu, MenuItem}` (check/radio marks live on `MenuItem`) |
| Panels / sections / group boxes | `sections::{Panel, Section, GroupBox, TitleRow}` |
| Dialogs / prompts / confirms | `dialog::{Dialog, DialogFrame, Alert, Prompt, DialogStack}` |
| Notices / infobars / stale warning | `notices::{Notice, ToastStack}` |
| Status / ride state / counts | `display::{Badge, Text, Progress, Divider, Spinner, Picture}` |
| Clocks (flattened) | `display::Progress` + `Text` readout |
| Sliders / dials (if any) | `value_drag::{Slider, AngleDial}` |
| Form rows | `form::{Field, FieldGroup}` |
| Splitter / sash | `docking::Splitter` |
| Status/options/tool bars | `bars::{Bar, BarKind, StatusMessage, StatusProgress, StatusReadout}` |
| App shell | `shell::AppShell` |

**Numbers.** Integer spins (`team_size_spin`, `decks_spin`, `jokers_spin`, the simulator `*_spin`) and
decimal fields (`lap_km_spin`, `avg_speed_spin`) both use `text::NumberField`; integer fields set
`.step(1.0).snap_typed(true)` and a range.

**Gauges are flattened.** The Python RaceClock dials and StopLight lamp become a `Progress` bar plus a
numeric `Text` readout for each clock, and a status `Badge`/`Notice` for the ride state.

## 7. Appearance model

`RootEnv` (`crates/ui-contract/src/runtime/env/root.rs`) carries: `mode` (System/Light/Dark — the
primary default), `contrast` (Standard/High/System), `density` (Compact/Comfortable), `text_size`
(Tiny/Small/Medium/Large), `theme` (`craft.artcraft` default).

The app exposes the full model in Settings but keeps **System/Light/Dark** as the primary mode control.
The net-new contrast/density/text-size/theme controls are **non-frozen** settings controls (own ids,
outside the 67 §15b fields). `System` follows the host (`craft::platform::appearance`).

Do **not** call `ctx.set_style`, `ctx.set_visuals`, or `ctx.set_zoom_factor` for appearance. The frozen
View-menu density and text-size rows drive the craft library's `Size` and `TextSize` presets through
`RootEnv`, the same path as mode, contrast and theme.

## 8. Interaction states

- **Default / hover / pressed / selected / disabled / focus** come from the token `state.*` and
  `line.focus` roles; do not invent local colours.
- **Disable, never hide** unavailable commands.
- **Focus ring** is visible on keyboard focus; every control has a stable accessible name.
- **Destructive confirmations** default to the safe button (Cancel).
- **No hover-only essentials**; no colour-only meaning.
- **Contrast**: primary buttons use `accent.strong` and danger buttons `bad.fill` so white text meets
  4.5:1; `text.faint` is used only for non-essential hints at large-enough size.

## 9. Motion

Short, token-driven transitions; reduced-motion turns them off. The wall clock and progress indicators
update on the 1 s tick.

## 10. Mockup tokens

`design/mockups/tokens.css` vendors the §2–§4 values as CSS custom properties under `:root` and
`[data-theme="dark"]`, so the HTML mockups and the Rust token table are the same numbers. A mockup that
needs a colour not in this table is a design bug.

## 11. References

- craft-libs `crates/theme/themes/artcraft.json`, `sizes/compact.json`, `tokens/*`, `ui-gallery/`.
- `design/docs-md/ui-contract.md` — structure and flows.
- `design/docs-md/control-mapping.md` — per-control mapping.
- `docs/artcraft-ui-runbook.md` — the build runbook.
