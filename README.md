# RiverCrossing-rs

**Poker-run ride timing and scoring for macOS, Windows and Linux — the Rust/egui port of
[RiverCrossing](https://github.com/mbuckaway/rivercrossing).**

RiverCrossing-rs is a keyboard-first desktop application for running poker-run bike rides. The
operator types a rider's plate number and presses Enter at each lap crossing; the app records the lap,
deals a card from a seeded virtual multi-deck shoe, and at the finish scores every entry's best
five-card poker hand (jokers wild, five of a kind above a royal flush), applies the configured
tie-breaks, and publishes results as a self-contained HTML page, a PDF report, a podium poster and a
CSV.

It is built to survive a bad day at the scorer's table: every crossing commits to SQLite as it
happens, elapsed time comes from the wall clock rather than an in-memory timer, and a crash, an
accidental stop or a full restart costs at most the keystroke in flight.

## Status

**Early — being rebuilt in Rust.** The Python application
([`mbuckaway/rivercrossing`](https://github.com/mbuckaway/rivercrossing)) is complete and remains the
behaviour oracle: the Rust port reproduces its screens and functionality, and its unit tests are the
reference for the Rust fixtures.

The build contract lives in [`design/`](design/):

- [`design/docs-md/architecture.md`](design/docs-md/architecture.md) — the Rust architecture
- [`design/docs-md/project-plan.md`](design/docs-md/project-plan.md) — the EPIC plan (EPIC-0…14)
- [`design/epic-prompts/`](design/epic-prompts/) — agent-ready prompts for each EPIC
- [`design/docs-md/requirements.md`](design/docs-md/requirements.md) — the numbered R-ids
- [`CODE_STRUCTURE.md`](CODE_STRUCTURE.md) — the crate layout and build order

## Stack

- **Rust** (edition 2024) — a layered Cargo workspace: UI-agnostic domain crates under
  `crates/rivercrossing-*`, an egui front end in `crates/rivercrossing-ui-egui`, and thin binaries in
  `apps/`.
- **egui + eframe + wgpu** — a native, GPU-drawn UI (the ArtCraft stack), built on the shared
  [`craft-libs`](https://github.com/storytold/craft-libs) crates (`craft`, `craft-ui`,
  `craft-settings`, `craft-quit`, `craft-dialogs`, `craft-mcp-server`, …).
- **SQLite** via `rusqlite` (bundled) — WAL, event-sourced crossings, hourly backups.
- **TDD** — all code is test-first, with 90% line and 90% branch coverage on the domain crates.

## Platforms

| Platform | Build |
|---|---|
| macOS | ARM64 only (Apple Silicon) — `.dmg` |
| Windows | x64 + ARM64 — `.msi` + portable `.zip` |
| Linux | x86_64 — `.deb` (Ubuntu) + `.rpm` (Red Hat/Fedora) |

## Building

```bash
cargo build --locked
cargo nextest run -P unit --locked
./scripts/validate-rust.sh     # the per-PR gate
```

The egui shell and the domain crates land EPIC by EPIC; see
[`design/docs-md/project-plan.md`](design/docs-md/project-plan.md).

## Contributing

- Never commit to `main` — use `topic/*` branches and a pull request.
- All Rust is written test-first by the `tdd-rust-writer` agent, per [`AGENTS.md`](AGENTS.md) and
  [`CODINGSTANDARDS-RUST.md`](CODINGSTANDARDS-RUST.md).
- No AI advertising in commits, pull requests or code.

## Licence

[GNU General Public License v3.0 only](LICENSE). Bundled fonts keep their own licences — see
[`NOTICE`](NOTICE).
