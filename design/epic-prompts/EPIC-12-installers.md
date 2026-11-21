# EPIC-12 — Installers & release

Load into a `shell-code-writer`/main agent (not `tdd-rust-writer` — this is packaging, not `.rs`).

## Goal

Package `apps/rivercrossing` into the platform installers, matching the ArtCraft single-app
methodology, in the platform priority order.

## Context

See `docs/porting-python-to-rust.md` §13 and `architecture.md` §10. Signing/notarization is a later
release EPIC (dev builds unsigned); the Python repo's `installers/` and `.github/workflows` are the
reference for branding assets (`packaging/branding/`).

## Refs

- `docs/porting-python-to-rust.md` §13 · `architecture.md` §10 · `requirements.md` R-01, R-75
- Python `installers/branding/` (SVG + `.icns` + `.ico` + `dmg_background.tiff`)

## Implement

1. macOS ARM64: `cargo build --target aarch64-apple-darwin`, `hdiutil` DMG (`-macos-arm64.dmg`),
   `MACOSX_DEPLOYMENT_TARGET=11.0`.
2. Windows x64 + arm64: `cargo build --target <arch>-pc-windows-msvc` (`+crt-static`), WiX v5 MSI
   (dual-scope) + `Compress-Archive` portable zip.
3. Linux x86_64: `cargo build` on Ubuntu 22.04 (glibc 2.35), `nfpm` `.deb` + `.rpm`.
4. Generate `SHA256SUMS.txt`; copy branding icons into the packages.

## Done when

- Each platform's installer builds and smoke-installs; release assets + `SHA256SUMS.txt` produced.
