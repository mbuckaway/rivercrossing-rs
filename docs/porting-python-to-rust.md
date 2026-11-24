Copyright (c) 2026 TrailLensCo. All rights reserved.

# PORTING-PYTHON-TO-RUST.md — runbook for porting a Python app to Rust

A reusable, step-by-step runbook for an agent or engineer who must rewrite a working Python
application in Rust. It captures the decisions, crate choices, test strategy, CI conversion, and
gotchas learned porting `iossim` (a macOS CLI). Read it top to bottom before writing code for a new
port; skip sections that do not apply, but do not skip the pre-flight reads.

The worked example is `iossim`: ~4,200 LOC of async Python (`httpx`/`asyncio`/`tomllib`/`plistlib`)
ported to a single Rust crate with a thin binary.

## 1. Decide whether to port

Port when the Python app is stable, its behavior is understood, and one or more holds: you want a
single self-contained binary (no interpreter, no virtualenv); startup latency or throughput matters;
you want compile-time correctness; or the app is macOS/OS-specific and already shells out to native
tools. Do **not** port a moving target — port a frozen, tested behavior so the Python suite is the
oracle.

## 2. Pre-flight reads (mandatory)

1. `AGENTS.md` and every `CODE_STRUCTURE.md` in the target tree.
2. `${HOME}/.agents/rules/read-patterns.md` — the coding-standards read pattern.
3. `${HOME}/.agents/rules/prefer-library-code.md` — use a vetted crate over hand-rolling.
4. `${HOME}/.agents/rules/coding-guidelines.md` — test categories, mocking, coverage, TDD.
5. `${HOME}/.agents/standards/CODINGSTANDARDS-RUST.md` — the Rust standard (§1–§11, RS-1–RS-24).
6. `CODINGSTANDARDS-COPYRIGHT/SIMPLECODE/SIMPLELANG` — always.

## 3. Toolchain setup (macOS/Apple Silicon confirmed)

```sh
rustup update stable                          # follow stable; do not pin an old version
rustup component add rustfmt clippy llvm-tools # llvm-tools is the current name, not llvm-tools-preview
rustup toolchain install nightly-<date> --profile minimal -c llvm-tools -c miri -c rustfmt -c clippy -c rust-src
cargo install --locked cargo-llvm-cov cargo-nextest cargo-deny cargo-machete cargo-hack
cargo install --locked kani-verifier && cargo kani setup   # formal verification lane
```

- Commit `rust-toolchain.toml` with `channel = "stable"`, `profile = "minimal"`,
  `components = ["rustfmt","clippy","llvm-tools"]`. `rustup toolchain install` (no args) reads it.
- `branch` coverage (`cargo llvm-cov --branch`) and Miri need the dated nightly; pin one date.
- macOS needs Xcode Command Line Tools for the linker (`clang`/`cc`); `ld`/`lld` ad-hoc sign `arm64`
  binaries automatically, so no manual codesigning is needed.

## 4. Idiom mapping (Python → Rust)

| Python | Rust | Notes |
|---|---|---|
| `httpx.AsyncClient` | `reqwest` (`default-features = false, features = ["json"]`) | no TLS stack for plain `http://localhost` |
| `asyncio` | `tokio` (`rt-multi-thread`, `process`, `signal`, `time`, `macros`, `sync`, `io-util`) | avoid `full` |
| `asyncio.create_subprocess_exec` | `tokio::process::Command` | async spawn + piping |
| `os.kill`/`signal.SIGINT` | `nix` (`signal`) via `Child::id()` | tokio has no arbitrary-signal kill; `nix` is safe (no `unsafe`) |
| `tomllib` | `toml` | serde-based read |
| `plistlib` | `plist` | XML/binary only — **not** the ASCII plist `simctl listapps` emits; keep `plutil -convert json` there |
| `dataclasses` | structs + `serde` derive | typed data |
| global `REGISTRY` + import-time `register()` | a `Registry` value built by a constructor | Rust has no import-time side effects |
| `Exception` subclasses | `thiserror` enums (lib) + `anyhow` (bin) | typed errors, `?` propagation |
| `datetime` | `time` (or `jiff`) | **chrono is unmaintained (RUSTSEC-2026-0543)** |
| `uuid.uuid4` | `uuid` (`v4`) | |
| `hashlib.sha256` | `sha2` | |
| `base64` | `base64` (Engine API) | |
| `pathlib` | `std::path::Path` + `tempfile` (`TempDir`) | |
| `Path.rglob` | `walkdir` | |
| `$XDG_*`/`~/.config` | `std::env` + `HOME` | **not** `directories::ProjectDirs` (macOS returns `~/Library/…`) |
| JSONL event log | thin `serde_json` writer | keep the schema byte-faithful; do not force `tracing` |

## 5. Crate-selection checklist

Before writing code, map every Python import to a crate. Prefer the crate over a hand-roll
(prefer-library rule). Confirm the crate is maintained (crates.io/GitHub activity) and runs on the
target. Do **not** add: `async-trait` (banned under the coverage gate), `chrono` (unmaintained),
blocking clients (`ureq`) into an async app, `tracing` when the log is a bespoke artifact, or an
abstraction with one implementation.

## 6. Error handling

- Library: one `thiserror` enum per module boundary; `?` for propagation; no `unwrap`/`expect`/panic.
- Binary: `anyhow` at the top; map to exit codes (`0` ok, `1` runtime/validation, `2` usage).
- `#![forbid(unsafe_code)]` in `lib.rs`. If a dependency needs a signal, use a safe wrapper (`nix`),
  not `libc` + `unsafe`.

## 7. Test strategy

- **Unit test library: `rstest`** (`#[fixture]`, `#[rstest]` + `#[case]`/`#[values]`) — the pytest
  analog for fixtures and table-driven tests; it generates real `#[test]` fns, so `cargo nextest`
  runs them. **`pretty_assertions`** for readable diffs. `mockall` (`#[automock]`) for doubles at
  I/O boundaries. `#[tokio::test(start_paused = true)]` for async (needs tokio `test-util`).
- **Placement:** unit tests in `src/<module>/tests.rs` (declared `#[cfg(test)] mod tests;`),
  functional in `tests/functional/main.rs`, CLI-spawn in `tests/integration/main.rs` (by hand).
- **GUI apps:** use the craft-libs automation tiers (§12.1) instead of the Python wx harness.
  Semantic tests stay in the unit sweep; snapshots are a separate `ui` nextest profile on one golden
  host; only two real-window smokes run, by hand.
- **Coverage:** `cargo llvm-cov` line + branch; branch needs the dated nightly. Target 95/95 if the
  project requires it; exclude `src/main.rs` with `--ignore-filename-regex 'src/main\.rs'`.
- **Formal verification: Kani** on pure leaf logic (parsers, validators, config resolution,
  arithmetic): `#[cfg(kani)] #[kani::proof]` + `kani::any`/`kani::assert`, proving panic-freedom and
  invariants for all inputs up to a bound. Kani does not cover async/FFI; run it as a scheduled lane,
  not the per-PR gate. `cargo-mutants` (advisory) audits whether the tests catch seeded bugs.

### Coverage and TDD (mandatory)

Write the failing test first, then the code — TDD is mandatory, not optional. The trap in a
Python→Rust port is testing only the pure helpers: the async I/O modules (`client`, `simulator`,
`wda`, `runner`, `cli`) stay at 20–50% coverage while the parser and config modules look fine. Mock
every I/O boundary behind a trait (`Http`, `ProcessRunner`) and drive the async paths through those
mocks — including the idempotency predicates, error paths, and the capture pipeline. Measure with
`cargo llvm-cov` on the nightly lane; the floor is **90% line and 90% branch**, raised to 95% where
the project demands it. The original Python tests are a behaviour oracle:
`git show <base>:tests/unit/test_<module>.py`. Use the `tdd-rust-writer` agent per module, and run
those agents **sequentially** — concurrent whole-crate test compiles collide, because one module's
uncompilable test file fails every other module's `cargo test`.

## 8. CI for Forgejo (self-hosted macOS)

- Install the toolchain manually (`rustup update`) on the persistent runner; `dtolnay/rust-toolchain`
  needs a full GitHub URL on Forgejo.
- `actions/upload-artifact@v4` is reported to fail on Forgejo ("GHES not supported") — use v3 or the
  Forgejo-patched v4. `actions/checkout@v4` works (Node must exist on the host).
- Cache only if the runner has a cache service; `Swatinem/rust-cache` with `cache-bin: "false"`.
- The gate: `cargo fmt --check`, `CARGO_BUILD_WARNINGS=deny cargo clippy --all-targets --locked
  --keep-going`, `cargo doc` with `RUSTDOCFLAGS=-D warnings`, `cargo nextest run -P unit`,
  `cargo llvm-cov` (line+branch), `cargo deny check`, `cargo machete`, `cargo hack check
  --rust-version`. A separate scheduled job runs Kani.

## 9. Packaging, release, and easy install

- **Self-contained binary:** `include_str!` bakes data files in; `[profile.release] strip = true,
  lto = "thin", codegen-units = 1, opt-level = 3`; ship one `aarch64-apple-darwin` binary.
- **Store in Forgejo:** attach the binary as a **release asset** via `tea releases create` +
  `tea releases assets create` (the same mechanism a Python wheel used). Optionally also a Generic
  package (raw binary) or a Cargo registry (`cargo install` compiles from source).
- **Easy install:** a one-line `curl -fsSL <release-url>/iossim-aarch64-apple-darwin -o
  /usr/local/bin/iossim && chmod +x /usr/local/bin/iossim`; optionally a Homebrew tap whose formula
  `url` points at the release asset with a pinned `sha256`, or `cargo binstall` with
  `[package.metadata.binstall] pkg-url`.

## 10. Gotchas

- `async-trait` is banned under the coverage gate — use native `async fn` in traits (static) or a fn
  pointer returning `Pin<Box<dyn Future + Send>>` (dynamic); a `dyn` async trait is unverified.
- `chrono` is unmaintained — use `time` or `jiff`.
- `plist` cannot parse `simctl listapps` ASCII output — keep `plutil -convert json`.
- `directories::ProjectDirs` returns `~/Library/…` on macOS; use `std::env` to honour `$XDG_*`.
- `llvm-tools` is the current component name (`llvm-tools-preview` is the legacy alias).
- `tea` release subcommand spelling varies by version — verify with `tea --help`.
- A literal `grep '#\[test\]'` cannot count `rstest` cases; use `cargo nextest list` as the count.

## 11. Execution checklist

1. Freeze the Python behavior; keep its test suite as the oracle.
2. Scaffold the crate: `rust-toolchain.toml`, `Cargo.toml`, `rustfmt.toml`, `clippy.toml`,
   `deny.toml`, `.config/nextest.toml`, `src/lib.rs` + thin `src/main.rs`, `scripts/validate-rust.sh`.
3. Port the leaf modules first (logging, config, spec) with `rstest` tests; prove the pure logic with
   Kani.
4. Port the I/O modules behind mockable traits (`Http`, `ProcessRunner`); mock them in unit tests.
5. Port the orchestrator and CLI; keep `main.rs` a thin shell.
6. Add `tests/functional/main.rs` against the real system.
7. Convert CI; add the release asset + one-line installer.
8. Rewrite the docs and `CODE_STRUCTURE.md`; delete the Python tree; open the PR.
9. GUI apps: before the first screen, add the automation harness (§12.1) — semantic `Fixture`
   tests, the `ui` snapshot profile, the debug control channel, and the two real-window smokes.


---

## 12. GUI apps — egui + craft-libs (ArtCraft)

For a desktop GUI port, do not reuse the CLI crate choices in §4/§5. The ArtCraft suite
(`github.com/storytold`, official site `getartcraft.com`) is the reference: pure Rust, **native egui
on the GPU** (eframe + wgpu + winit), "no Electron, no Tauri". Copy the **WordCraft** app's shape
(`storytold/wordcraft`), not its code. Build the automation harness before the screens — see §12.1.

- **UI stack**: `egui`/`eframe` + `egui_extras` (declared `0.37`, overridden by the storytold egui
  fork), `winit` (storytold winit fork), `wgpu` `30` (inherited via craft-libs). `eframe` features:
  `default_fonts, wgpu, accesskit, clipboard, wayland, x11, links` — no `glow`, no `persistence`.
- **Shared crates**: depend on `storytold/craft-libs` (git-only, MIT/Apache-2.0). Use the `craft`
  facade or individual `craft-ui/settings/quit/menu/dialogs/i18n/keymap/atomic/update/print/pdf`
  crates. Pin **one shared `rev`**, add `version = "=0.1.0"`, commit `Cargo.lock`, build `--locked`.
  Fallback: the released `tag = "v0.1.0"` (egui 0.36, five crates).
- **`[patch.crates-io]` does not propagate** — repeat the egui/winit fork patches in the app's own
  `Cargo.toml` (WordCraft does the same).
- **Layered workspace**: L0 domain crates → L4 engine/command registry → L6 `-ui-egui` → `apps/`.
  Hard rule (`cargo xtask layers`): nothing below `ui-egui` may depend on egui/eframe/winit/rfd.
- **eframe 0.37 bootstrap**: `impl eframe::App` uses `logic()` + `ui()` (not `update()`); build
  `eframe::NativeOptions` + `egui::ViewportBuilder`, `craft::gpu::configure` on `WgpuSetup::CreateNew`,
  `craft::host::Host` + `craft::ui::InstallSpec` + `craft::ui::shell::embed::install`, then
  `eframe::run_native`.

### 12.1 Automation before screens

Build the automation harness before the first screen. A craft app is agent-drivable only if the
widgets carry stable names and the app owns a method table; retrofitting either after the screens
exist is expensive. `design/docs-md/automation.md` and the RiverCrossing EPIC-10 prompt are the
worked example.

- **Two in-process tiers.**
  - **Semantic** — `craft::ui::testing::{Fixture, Frames}` behind the `craft` `ui-testing` feature.
    `Fixture::new` takes a move-only `FnMut(&mut Ui)`, so share the app as `Rc<RefCell<App>>`; run
    it under `cargo nextest -P unit`, with no display or GPU.
  - **Snapshot** — `egui_kittest` `0.37` (`snapshot` + `wgpu`) from the same egui fork patch. Use
    `Harness::new_ui`, install the craft runtime with `embed::install` (build
    `InstallSpec::new(ShellInfo::default(), Arc::new(NoWindows))`; do not reuse `install_spec()`,
    which loads story command sets), then run `embed::before_input`, `embed::logic` and `app.ui(ui)`
    per frame. Put it in `tests/ui/main.rs`, add `[[test]] name = "ui"`, a `[profile.ui]
    default-filter = "kind(test) & binary(ui)"` nextest profile, and a `kittest.toml`.
- **Selector contract.** Action id first (the frozen command id), AccessKit role + accessible name
  second with an optional window scope; never a bare node id. Give every input widget an accessible
  name (`labelled_by`/`accessible_name`); kittest's accessibility check is on by default and should
  stay on. Labels are unique only within a top-level window, so the selector carries the window
  scope.
- **Live control channel.** Enable `craft`'s `automation` feature (the channel half also needs the
  facade's `ui` feature). The app owns the method table and the `ControlChannel`; call `poll_input`
  after injected input and `poll_access_trees` after the UI pass, keep `Listening` alive for the
  window's lifetime, and call `ctx.enable_accesskit()` once at install — do not rely on the eframe
  build to do it. `craft-automation` supplies `ui.click/move/key/type/ime`, the `Captures` queue
  behind `ui.screenshot`, `ui.accessTree` and `ui.dialog.set/confirm/cancel`; the app adds
  `ui.command` (dispatch the Action registry), `ui.resize`, `ui.screenshot` (token plus a bounded
  writer), and any `ui.inspect`/`ui.set` it needs. Gate the whole acquisition (flags **and**
  `<APP>_CONTROL_*` environment) behind `#[cfg(debug_assertions)]`; bind loopback only; token file
  `0600`; confine writes to a canonicalised root. The consented release surface is an `AgentSettings`
  decision, not the debug channel.
- **Snapshots.** One golden host and per-OS thresholds — never per-OS goldens or skips. Keep the
  golden host's OS threshold tight (`0.6`) and the other OSes loose (`2.0`); in the RiverCrossing
  example the golden host is Linux. Keep `tests/snapshots` package-relative, install the app fonts
  (missing glyphs panic), and regenerate only on the golden host with
  `UPDATE_SNAPSHOTS=force cargo nextest -P ui --locked`.
- **Gate.** Add the `ui` profile stage and a release `cargo check` to `scripts/validate-rust.sh`;
  keep exactly two real-window smokes (open/quit and the control channel) and run them by hand under
  `xvfb-run` on Linux. Do not add a second windowed suite.
- **Order.** Land the harness, one probe test/golden, the control smoke and the gate first; then
  build each screen with one semantic test and one golden. For a Python port, share one scenario JSON
  schema between the Rust harness and an in-process Python driver, and run the parity comparison at
  the end of the UI EPIC, on the machine that can launch the Python app.

## 13. Single-app installer methodology (ArtCraft-style)

ArtCraft packages each app via a shared reusable workflow (`storytold/craft-common-actions`) plus
per-app `packaging/` shell scripts. A third-party app cannot `workflow_call` that workflow (its
`git-sources` job rejects non-storytold git deps; its signing steps need storytold secrets), so copy
the trimmed pattern:

| Platform | Artifact | Toolchain |
|---|---|---|
| macOS (ARM64 only; no Intel) | `-macos-arm64.dmg` | `cargo build --target aarch64-apple-darwin`, **`hdiutil`** (not create-dmg), `MACOSX_DEPLOYMENT_TARGET=11.0` |
| Windows x64+arm64 | `.msi` + `-portable.zip` | `cargo build --target <arch>-pc-windows-msvc` (`+crt-static`), **WiX v5** MSI (dual-scope), `Compress-Archive` |
| Linux x86_64 | `.deb` + `.rpm` | `cargo build` on Ubuntu 22.04 (glibc 2.35), **`nfpm`** |

- `SHA256SUMS.txt` from the merged release assets; sign/notarize in a release EPIC (dev builds
  unsigned). Self-update via `craft-update` (Windows `--finish-update` helper).

## 14. Module chunking for a large app

Do not port a large app in one agent session. Split it leaf-first into one crate per module, and
split large modules into context-bounded chunks. Run `tdd-rust-writer` agents **sequentially** — one
uncompilable module test file fails every other module's `cargo test`. The Python test suite is the
behaviour oracle throughout.


## 15. Agent surface — CLI + MCP (ArtCraft pattern)

ArtCraft apps expose every command to agents (one command catalog drives the GUI, the CLI and the MCP
server). The shared pieces, verified in `storytold/wordcraft`:

- **MCP**: the **`craft-mcp-server`** crate (craft-libs) — JSON-RPC 2.0 over stdio (newline-delimited),
  the MCP lifecycle, tools/resources/prompts, tool schemas with argument checks, and text/image
  results. An app implements its `Tools` trait and runs `Server::new(tools).serve(stdin, stdout)`.
  No third-party MCP SDK.
- **CLI**: hand-rolled (no `clap`) — a `USAGE` string plus `OptionSpec`/`validate_options` argument
  parsing that dispatches to the command registry with `serde_json`, exiting 0/1/3.
- **Control channel**: `craft`'s `automation` feature drives a running app over loopback JSON lines
  with a token and a bounded screenshot queue. The app owns the method table that dispatches the
  command registry; `craft-control::agents::Live` resolves the open window so an MCP `Tools` impl can
  forward to it through the shared `craft-mcp-server`. Gate the whole acquisition (flags and
  `<APP>_CONTROL_*` environment) behind `#[cfg(debug_assertions)]`; the consented release surface is
  an `AgentSettings` decision. The in-process harness in §12.1 is the test-side counterpart.
