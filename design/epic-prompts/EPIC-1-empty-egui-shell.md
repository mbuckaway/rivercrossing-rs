# EPIC-1 — Empty egui shell (de-risk the toolchain)

Load into a `tdd-rust-writer` agent. Read `AGENTS.md`, `CODE_STRUCTURE.md`, and
`CODINGSTANDARDS-RUST.md` first. TDD (RS-14).

## Goal

Stand up `crates/rivercrossing-ui-egui` + `apps/rivercrossing` so `cargo run --release -p
rivercrossing` opens an empty, titled, resizable egui window on macOS, proving the egui/craft-libs
toolchain before any domain code lands.

## Context

RiverCrossing-rs is a layered Cargo workspace on egui + eframe + wgpu via the ArtCraft `craft-libs`
crates (see `architecture.md`). The `craft` facade + `craft-ui/settings/quit` crates, and the egui/
winit `[patch.crates-io]` forks, are added now and locked. Known-good pin (WordCraft's): craft-libs
rev `243d68e12d05e89122db6d154924bf1992e10d38`; egui/eframe `0.37` via storytold egui fork
`7e1c30d142bc7879d043e74b5ceb3ecfe7f51329`; winit fork `a517b3846ee8457d84ee4975110bfda6797c44dc`;
wgpu `30` (inherited). eframe 0.37 uses `logic()` + `ui()` (not `update()`).

## Refs

- `Cargo.toml` (the commented `[patch.crates-io]` block to enable) · `CODE_STRUCTURE.md`
- `architecture.md` §1, §4, §5 · `requirements.md` R-01, R-05

## Tests-first (Red)

- One open/quit smoke test (the only functional test, R-74 analog) that builds the app, shows the
  window, and drives quit — the regression gate for the launch path.

## Implement (Green → Refactor)

1. Enable the craft-libs rev + the egui/winit `[patch.crates-io]` block in `Cargo.toml`; add `craft`
   (`ui`, `ui-app`) and `eframe` to `[workspace.dependencies]`, then grow `members`.
2. `crates/rivercrossing-ui-egui`: the window identity (`AppSpec::new(APP_ID, APP_NAME,
   Defaults::new([1100.0, 780.0], [1100.0, 780.0]))`) and an empty `impl ShellApp` (`frame_input`,
   `ui`, `logic`), launched with **`craft::ui::shell::app::run(spec, make)`**.
3. `apps/rivercrossing/src/main.rs`: `rivercrossing_ui_egui::run(smoke_quit_after())` — nothing else.
4. SPDX line on every `.rs`.

> Correction (EPIC-1 execution): the original steps named `craft::gpu::configure` (which takes an
> `egui_wgpu::WgpuSetupCreateNew`, not `NativeOptions`), a pre-`run_native` `embed::install` (which can
> only run inside eframe's creator closure) and a hand-built `NativeOptions`. `shell::app::run` performs
> all of that: it builds the options, installs the runtime in the creator closure, and calls
> `eframe::run_native`. There is no logger to install — craft-libs has none.

## Done when

- `cargo run --release -p rivercrossing` opens an empty, titled, resizable window on macOS.
- The open/quit smoke test, `cargo fmt --check`, `cargo check` and `cargo clippy -D warnings` pass.
- The pinned rev is recorded in `AGENTS.md`, with the build-location rule (forge box + Mac over SSH;
  never a fleet runner).
