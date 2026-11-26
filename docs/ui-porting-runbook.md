# UI Porting Runbook — Python (wxPython) → Rust (egui)

A **running, continuously-updated** log of lessons learned converting the RiverCrossing UI from
wxPython/XRC to egui. Update this file at the end of every UI phase so the next agent starts from
accumulated knowledge instead of re-learning it.

## Status

- [x] EPIC-1 empty egui shell — done
- [x] EPIC-10 UI automation & Python-parity harness — done (Tier A harness, shell golden, live control
  channel)
- [ ] EPIC-12 egui UI panels + Action registry — in progress, five PRs: **12a** foundation
  (`gui-windows.md`, settings, registry, dialogs, state/worker, ride setup + `main_frame` console,
  sound, logging, theme/zoom, automation), **12b** editors/CSV, **12c** lifecycle/corrections/system,
  **12d** results/exports/publish/simulator, **12e** close-out (R-73 sweep, R-76 per dialog, Tier C).

## Build prerequisites and the platform split

- **Linux (the build/gate host).** `libasound2-dev` is required once `rodio` lands (cpal's ALSA
  backend); install it before the first build (`sudo apt-get install -y libasound2-dev`;
  `pkg-config --exists alsa` must succeed). Offscreen rendering uses Mesa lavapipe, and the Tier B
  smokes run under `Xvfb` — no display or GPU is needed.
- **Windows cross-check (not the gate).** `rustup target add x86_64-pc-windows-gnu` plus
  `gcc-mingw-w64-x86-64`; `scripts/check-windows.sh` wraps
  `cargo check -p rivercrossing-ui-egui --target x86_64-pc-windows-gnu --locked`. Bundled SQLite is
  the C dependency; `ring` arrives with the WordPress crate and is re-checked. The authoritative
  Windows run is at EPIC-13 on a Windows host.
- **macOS.** CoreAudio needs nothing extra. The Mac checkout is used per PR for the unit+ui legs over
  SSH; the visual acceptance pass is run at the Mac itself (a GUI app launched over SSH may not
  attach to the Aqua session).
- **What the headless host can and cannot verify.** It runs the unit sweep, the snapshot goldens, the
  Xvfb real-window smokes and every gate item. It cannot play audio (`/dev/snd` is absent — the sound
  backend must fall back to silence), show a real native dialog without a portal (rfd may answer
  `Unavailable`), or give a human the visual acceptance judgement. Those are Mac steps; Tier C parity
  is the Mac's by design.
- **Recorded decisions.** Geometry lives in craft-window's `window.json` (`splitter_sash` in
  `settings.json`); settings stay Python-schema JSON in `rivercrossing-store`; WordPress credentials
  stay plaintext at parity; empty Enter on the plate field is **silent** (Python behaviour —
  `spec.md:242`'s buzz is the recorded discrepancy); `verbose_logging` gates an NDJSON per-launch log
  (prune 20).

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

Reusable craft-libs setup: `docs/porting-python-to-rust.md` §12.1. This section records the
RiverCrossing-specific translation.

The wx harness mechanics do not port — `SetValue()` firing `EVT_TEXT`, posted `wx.CommandEvent`,
`Destroy()`/stale-window contamination have no egui analogue. The Rust harness (R-87…R-92,
`design/docs-md/automation.md`, `design/docs-md/test-plan.md`) drives the real widgets instead, and
the scenario JSON is the cross-language contract.

| Tier | Surface | Runner | Needs |
|---|---|---|---|
| **A — semantic** | `craft::ui::testing::Fixture` over `Rc<RefCell<RiverCrossingApp>>` in `src/**/tests.rs` | `cargo nextest run -P unit --locked` | no display, no GPU |
| **A-snapshot** | `egui_kittest` `Harness::new_ui` + craft `embed::install`, then per frame `embed::before_input`/`embed::logic`/`app.ui(ui)`, asserted with `snapshot` in `tests/ui/main.rs` | `cargo nextest run -P ui --locked` | lavapipe; goldens are written on the Linux golden host only |
| **B — real window** | `open_and_quit_smoke` + `control_channel_smoke` (spawns `rivercrossing --control 0`) | by hand under `xvfb-run -a -s "-screen 0 1400x900x24"` | Xvfb; no WM |
| **C — Python parity** | shared JSON scenarios; in-process pytest driver in the oracle repo (`FindWindowByName`/`SetValue`/posted `wx.CommandEvent` are in-process calls, `spec.md` §12) | on demand at EPIC-12 exit, never in the gate; runs on the Mac | Python 3.14 / wxPython 4.3.1 |

- **Selector contract.** `Selector::Action { id }` — a frozen §15b id — first;
  `Selector::RoleAndLabel { role, label, window }` second. `window` scopes labels that legitimately
  repeat (`plate_input`, `reason_input`, `continue_btn`, `message_lbl`; names are unique only within a
  top-level window, `spec.md` §15b). Multiple matches without a `window` are an error, never an
  arbitrary pick. No `NodeId` (per-run, not reproducible) and no bare label lookup.
- **Scenario model.** `Scenario`/`Step`/`Selector`/`Observation`/`WireTree`/`Node` live in
  `crates/rivercrossing-ui-egui/src/ui/automation/scenario.rs` (public automation API), versioned
  JSON (`"version": 1`) mirrored by `tools/parity/schema.json` for the Python driver. `Step::Expect`
  is the only assertion mechanism; `Observation.tree` mirrors craft's flat `accessTree` reply.
- **Snapshots.** One golden host (Linux / lavapipe); `kittest.toml` sets `output_path =
  "tests/snapshots"` (package-relative) with `[linux] threshold = 0.6` and `[mac] threshold = 2.0`.
  Missing glyphs panic, so the app fonts (`craft::ui::fonts::UiFonts`) are installed. Regenerate
  deliberately on the golden host only:
  `UPDATE_SNAPSHOTS=force WGPU_BACKEND=vulkan cargo nextest run -P ui -p rivercrossing-ui-egui --locked`.
- **Artifacts.** Tier B writes `target/gate/control-smoke.log` and PNGs under `target/control/`;
  Tier C writes `Observation` JSON + PNGs under `target/gate/parity/<scenario>/`, or
  `docs/parity/SKIP.md` naming the missing prerequisite.

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
  the golden host with
  `UPDATE_SNAPSHOTS=force WGPU_BACKEND=vulkan cargo nextest run -P ui -p rivercrossing-ui-egui --locked`;
  there is no second golden set and no skip path.
- **Resolved — WordPress credential storage (EPIC-11):** parity, i.e. the password lives in the
  plaintext settings JSON exactly as in the Python app. `rivercrossing-wordpress` stores nothing and
  keeps credentials in memory only; the settings layer persists them. Revisit an OS keychain only
  as a separate product decision.
- Tailwind/Node CSS build: keep the Node step as an xtask, or vendor the already-compiled CSS.

## EPIC-12 handoff (from EPIC-10)

- Call `ctx.enable_accesskit()` once in `run`'s `make`; the live tree is empty without it.
- Give every input widget an explicit `.accessible_name`; `hint_text` is only a placeholder.
- Register every §15/§15b id in the `ActionRegistry` and keep the ids-drift check green.
- Add one semantic `Fixture` test and one snapshot golden per screen state; call
  `craft::ui::testing::access::violations` on every settled frame.
- Replace the EPIC-10 probe panel with the real console: the probe is gone, and
  `crates/rivercrossing-ui-egui/tests/snapshots/console_empty.png` plus `setup_open.png` replace
  the retired `shell.png` on the golden host.
- Add real `ui.inspect`/`ui.set` fields and wire `ui.dialog.*` to the dialogs; R-88's dialog clause
  completes here.
- Run Tier C on the Mac at exit and remove `docs/parity/SKIP.md` when the parity run is green.
- Keep exactly the two Tier B smokes (open/quit and control channel); do not add a third real-window
  test.

## EPIC-12 Phase 6 renderer choices

- **Menu renderer: `craft::ui::menu` (`MenuBar`/`MenuTitle`/`MenuItem`), no fallback.** The §15
  table projects onto one `MenuTitle` per section (`File`, `Ride`, `Riders`, `Cards`, `Results`,
  `View`, `Help`), ordered by `action::spec::MenuSection::ALL`. `action::spec::section(id)` is the
  one place that maps the 53 frozen ids to their menu, so the drift snapshot and the rendered bar
  cannot disagree. Each row takes its label and shortcut text from the registry, its enabled state
  from `enable::enabled(id, &state.gate()) && state.implemented(id)`, its check mark from the
  persisted setting (`CheckKind`) and its radio mark from the zoom percentage. The automation-only
  fallback (plain `egui::MenuBar` with `egui::Button` rows) was not needed.
- **The console and the setup form are plain egui widgets**, each wrapped in
  `ui.push_id("<frozen id>", …)` with an explicit `.accessible_name(...)`. Checkable rows and
  radios use egui's own widgets; `value()` and `clock_column()` helpers keep one frozen id per
  widget. The clock dials are hand-painted (a circle plus a needle) because craft ships no
  `RaceClock`; the numeric readout beside each dial carries the value that the tests assert.
- **The setup form writes through `DialogStack`'s field map.** Every field read (`ui.inspect`) and
  write (`ui.set`, `ui.dialog.set`) addresses the same value the widget edits, so the automation
  contract and the form cannot drift. The OK button runs `AppState::submit_setup`, which collects
  `RideConfigParams`, validates through `presenters::setup` and stores the config for the app to
  persist.
- **Goldens:** `shell.png` was retired and replaced by `console_empty.png` (the empty console) and
  `setup_open.png` (the setup dialog over it), both regenerated on this golden host with
  `UPDATE_SNAPSHOTS=force WGPU_BACKEND=vulkan cargo nextest run -P ui -p rivercrossing-ui-egui --locked`.
- **Phase 6 deviations, deliberate:**
  - Native confirms (Stop, Undo, Finish, Clear, Reopen, Exit, the new-location prompts) render as an
    in-app `egui::Modal` until `craft_dialogs` lands in PR C: the modal names the action, focuses
    Cancel by default and only a confirm queues `then`; Esc, Cancel and the backdrop run nothing.
    `ui.dialog.confirm`/`cancel` drive it whenever the `DialogStack` is empty. PR C swaps the modal
    for the native dialog and the `Answer::Unavailable` notice.
  - The sound device adapter (`RodioBackend::open`/`play`, `SoundPlayer::with_default_backend`) is
    the one unexercised path: this headless host has no `/dev/snd`, so the adapter cannot open. The
    cue bytes, the mute/error fallbacks and the silent backend are unit-tested; the workspace
    coverage gate holds at 95.69 % line / 97.07 % branch with the adapter included.
  - The console does not yet request a repaint for the wall clock; it redraws on input. The
    one-second tick and the `RideEngine.closed_elapsed()` freeze arrive with the settings/lifecycle
    PR that owns the clock settings.
  - `app_quit` replaces `probe_quit`; `ui.command` dispatches any action id, so the Tier B smoke
    opens the setup dialog with `mi_new_ride` and types into its name field.
- **Known craft-libs race (recorded, not ours to fix at this rev).**
  `craft_automation::server::serve_connection` calls `WindowHost::repaint_in` on the connection
  thread, which reads the frame clock through the context's pass state and panics with "another
  thread holds this context's pass state" when a request lands inside a pass. The control smoke
  therefore waits two seconds after the listening line, before its first request, so the first
  (window- and device-building) pass has ended. A fix belongs in craft-libs: the server should
  request a repaint without taking the pass state.

## EPIC-12b Phase 2 — editors, CSV preview and the issues report

- **Six windows render through the dialog host:** `rider_editor_dlg` and `team_editor_dlg` (list +
  search/filter + read-only record panel + Add/Edit/Delete-Remove), `add_rider_dlg` and
  `add_team_dlg` (one window in two modes, retitled "Edit Rider"/"Edit Team"; OK says "Save"), the
  `csv_preview_dlg` (summary, conflicts, both off-by-default options, Import disabled while a
  conflict remains) and `rider_issues_dlg` (summary, list, the three fixes gated by
  `presenters::rider_issues::actions`). Every list cell is an egui selectable label; an empty cell
  renders an em dash so the control keeps a non-empty accessible name.
- **The record panels mirror into the dialog field map.** The rider/team editor displays the
  selected record, and each pass writes the frozen `plate_input`/`first_name_input`/… values into
  the dialog's field map, so `ui.inspect` reads what the panel shows; a `ui.set` write is
  overwritten on the next pass, which is exactly a read-only display. The Add forms and the CSV
  options keep their values in the same map, so `ui.set` and the widgets share one source.
- **Roster persistence.** Each mutation queues the roster's audit rows (`AppendRosterEvent`), the
  roster snapshot (`SaveRoster`) and then a `LoadRide`, which rebuilds the engine over the saved
  roster — the UI's roster copy and the engine's `RosterView` cannot drift. The app drains the
  queue at the start of each logic pass and refreshes the editor models when the ride reply lands.
- **Live team moves.** An edited rider whose team changes on a RUNNING or REOPENED ride commits
  through `RideEngine`'s own move (`presenters::riders::commit_live_move`), so the rider's laps and
  cards follow; a live, unstopped ride refuses with "Stop the ride first". The engine event is
  queued with the ride events. The presenter's operator confirmation before a live move is deferred
  to PR C's dialogs; the move is reversible by moving the rider back.
- **Delete and Remove confirm natively.** The editors raise the PR A inline native modal through a
  new `NativeTask::{DeleteRider, RemoveTeam}`; only the confirm runs the presenter and queues the
  save, and Cancel/Esc keeps the record. PR C replaces the inline modal with `craft_dialogs`.
- **`ui.dialog.confirm` on a form submits it**, sharing `AppState::submit_top_form` with the OK
  button, so the Tier B smoke can complete the ride setup through the channel.
- **Deferred to PR D (recorded):** the OS open/save pickers. `mi_import_csv` and `mi_export_csv`
  post "a file picker is not available in this release" as the notice; the preview and the export
  run through `RiverCrossingApp::open_csv_preview(path)` / `export_csv(path)`, which are the seam
  the picker will call. The CSV dialog therefore opens only with a previewed file.
- **Deferred with a reason:** `card_check_lbl` stays hidden — the Python window hides the advisory
  when the ride config and the stored average speed are unavailable, and the estimate's composition
  arrives with the results work. "Pick card" acts on the selected or edited team; staging a card for
  a not-yet-created team in the Add window is deferred.
- **Goldens:** `rider_editor.png`, `team_editor.png`, `csv_preview.png` and `rider_issues.png` join
  `console_empty.png`/`setup_open.png`; regenerate on the golden host with
  `UPDATE_SNAPSHOTS=force WGPU_BACKEND=vulkan cargo nextest run -P ui -p rivercrossing-ui-egui --locked`.
- **Tier B smoke:** the control smoke now completes the ride setup through
  `ui.set` + `ui.dialog.confirm`, waits for the ride in the access tree, then opens the rider editor
  with `ui.command mi_rider_editor` and closes it with `ui.dialog.cancel`. The editor is ride-gated,
  so the round trip necessarily follows the setup step rather than preceding it.

## EPIC-12c Phase 2 — lifecycle, corrections and system dialogs

- **Native dialogs go through `craft_dialogs`.** `ui/native.rs` wraps
  `craft::dialogs::Dialogs<PendingNative>`: every confirm, danger, prompt and error report queues
  the box with its `PendingNative` payload, and `show_pending` asks the box parentless because the
  UI pass has no raw window handle. The app raises the box at the end of a logic pass and reads the
  answers at the start of the next one, so the event loop never blocks. A host that cannot show a
  box answers `Answer::Unavailable`; the app then restores the pending task as the in-app modal
  (`views::dialogs::show_native`) and marks it with `AppState::native_in_app`, so the next logic
  pass does not ask the host again. Cancel and Esc run nothing, and the in-app Cancel button holds
  the focus, so Enter is the safe answer. The control channel forces the unavailable host with
  `set_native_unavailable`, which keeps the Tier B smoke deterministic under Xvfb.
- **Lifecycle.** Launch sends `PreviousSession` and `Rides`; the resume prompt opens only when a
  previous session exists, and `quit_resume::launch_choice` decides. Quit runs the running-ride
  dialog first when a ride is live, the plain native confirm otherwise; `app_quit` closes without a
  box. The hourly backup checks the wall-clock hour bucket (`hour_bucket`/`due_hourly_backup`) on
  each pass and the tick asks for the next hour boundary.
- **Corrections.** Add/Edit Crossing, Manual Deal, DNF, Void Card, Crossing Number and Set Start
  all route through `presenters::corrections`. A crossing's detail opens from the console feed and
  offers Edit, Edit Time, Void Card and Delete; Delete runs through the native danger. Void Card
  resolves the plate from the detail model, because the engine's `void_card` takes a plate, not an
  entry key.
- **Deferred, recorded:** `mi_import_csv` / `mi_export_csv` still post the file-picker notice;
  `card_check_lbl` stays hidden pending the results work; staging a card for a not-yet-created team
  in the Add window is deferred; the correction dialogs have no goldens because a stable fixture
  would need a started ride with a per-dialog form state, and the Crossing Detail golden covers the
  engine-backed card and time cells instead.
- **Goldens (eight new, fourteen total):** `ride_library`, `delete_ride`, `settings`, `audit`,
  `selftest`, `about`, `shortcuts` and `crossing_detail`. The crossing-detail fixture starts at the
  epoch, records one crossing, then stops the ride so the stopped clock reads zero — otherwise the
  live elapsed clock makes the frame differ on every run. The harness also appends egui's Noto Emoji
  face at `FontPriority::Lowest`, because the card suit glyphs (♠♥♦♣) are in no app face; the check
  panics on a missing glyph rather than drawing tofu.
- **Tier B smoke:** the control smoke opens Settings with `ui.command wxID_PREFERENCES`, asserts
  the title and the Appearance section in the access tree, and closes it with `ui.dialog.cancel`.

## EPIC-12d Phase 2 — results, exports, publish and simulator

- **Results board.** `open_results` ranks the live engine through
  `presenters::results::standings_model`, so a finished ride applies the stored tie-break order and
  the DNS toggle while every other state ranks the whole field on the live board. The notebook shows
  the Teams/Solo tabs on a mixed ride and the single `standings_list` on a solo-only ride, with the
  stale-export infobar when a correction event lands at or after the last export's watermark
  (`mark_exported` records the engine's event count). `mi_standings` opens it in every ride state.
- **Exports run on the store worker.** The menu commands raise a `craft_dialogs` save picker whose
  answer composes the `ExportInput` on the UI thread and sends one `StoreCommand::Export` job; the
  worker renders and writes the page, the PDFs, the poster or the CSV and replies with the path or
  the exporter's sentence. `record_export` marks the three preview gates, keeps the last path and
  advances the results watermark. The preview rows open the last file through
  `craft::platform::open::open_external` (the platform feature is enabled on the `craft` dep), and a
  failed handler degrades to a status notice. Tests fake the picker with the native dialogs'
  `set_override`; the app's test seam answers every save request with a chosen path.
- **WordPress publish (R-86).** The form is prefilled from `wp_*` and the live ride
  (`default_title`/`default_slug`), re-seeded when the kind changes. Submitting validates the form
  (the dialog stays open on the messages), persists `wp_*`, and hands a `PublishStart` to a
  dedicated thread. That thread renders `render_wordpress`/`render_poster_wordpress`, checks the
  cancel flag before the lookup and before the publish, then calls `find_page_by_slug` and
  `publish_page` to create or update the page. The progress window's Cancel sets the flag and the
  worker abandons the attempt at its next checkpoint. Success raises the native "Published to
  <link>" box with Open, failure raises Retry/Cancel (Retry re-publishes the kept form), and a
  cancellation posts a notice. The job's orchestration is unit-tested through a `PageWriter` trait.
- **Simulator.** The form restores the `sim_*` settings (the interval falls back to
  `default_interval_minutes` when none is saved), Generate Riders seeds the loaded draft roster
  through `presenters::simulator`, Check fills the infobar with `check_message`, and GO persists the
  fields and hands a `SimStart` to a second thread. That thread replays `run_simulation` with its
  progress channel and cancel flag; the app polls the channel on its tick, restores the ride, appends
  only the new events and closes the progress window. The modal owns the cancel request; the ride is
  absent from the console while the run holds it, which the progress window covers.
- **Deferred to 12e:** the correction dialogs' goldens (the fixture needs a started ride per form),
  the export-preview goldens (the picker path is operator-driven), and the multipart
  parent-page-by-slug resolution (a slug parent currently publishes as a top-level page; the oracle's
  lookup is not wired). `ride_defaults` stays presenter-only: §15 has no menu row that opens a test
  ride, so nothing in this PR calls it.
- **Goldens:** `results`, `publish_form` and `simulation` join the fourteen; all seventeen verify
  without `UPDATE_SNAPSHOTS`. The results and publish fixtures use a stopped ride so the console
  clock reads zero.
- **Tier B smoke:** the control smoke now opens `mi_standings`, asserts the Standings board's
  columns in the access tree, and closes it with `ui.dialog.cancel`.
