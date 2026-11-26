# Changelog

All notable changes to RiverCrossing are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versioning follows
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

The version is single-sourced from `[workspace.package] version` in the root
`Cargo.toml`; a release bumps it on a `release/v<X.Y.Z>` branch together with a
new `## [<version>]` section here.

## [Unreleased]

## [2.0.0] - 2026-11-26

### Added

- **The Rust/egui port of RiverCrossing** — the full desktop app: ride setup and
  the keyboard-first console, the roster and teams editors, CSV import/export,
  standings and results, the audit trail, and the rider simulator.
- **Exports** — self-contained HTML results and podium pages, deterministic PDF
  reports and posters, the standings CSV, and WordPress publishing.
- **Persistence** — an SQLite store (WAL, migrations, hourly backups) so a crash
  costs at most the keystroke in flight.
- **Platform installers** — macOS arm64 DMG, Windows x64 + arm64 WiX v5 MSI and
  portable zip, and Linux amd64 `.deb` + `.rpm`, with a `SHA256SUMS.txt`.
- **CI** — a GitHub Actions gate (fmt, clippy, docs, tests, coverage, snapshots)
  and a tag-driven installer/release workflow that publishes the versioned
  installers with this changelog as the release notes.

### Notes

- Dev builds are unsigned: macOS signing/notarization and the Windows installer
  signature are deferred to a later release epic.
