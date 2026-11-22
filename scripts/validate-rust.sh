#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-only
#
# rivercrossing-rs per-PR validation gate (CODINGSTANDARDS-RUST.md).
# Run from the workspace root. No `set -e` — check every exit code explicitly
# (CODINGSTANDARDS-SHELL.md).

set -u

NIGHTLY="nightly-2026-11-05"
OUT="target/gate"

step() { printf '\n== %s ==\n' "$1"; }
fail() { printf 'GATE FAILED: %s (exit %s)\n' "$1" "$2"; exit 1; }

mkdir -p "$OUT"

step "fmt"
cargo fmt --all -- --check || fail "fmt" "$?"

step "clippy"
# `-D warnings`, not CARGO_BUILD_WARNINGS: that variable only exists on Cargo >= 1.97,
# so it silently no-ops on the 1.95 MSRV leg below.
cargo clippy --workspace --all-targets --locked --keep-going -- -D warnings || fail "clippy" "$?"

step "doc"
cargo doc --workspace --no-deps --locked || fail "doc" "$?"

step "nextest list (category/layout)"
cargo nextest list --workspace --locked || fail "nextest list" "$?"

step "msrv + feature-powerset"
cargo hack check --rust-version --workspace --all-targets --locked || fail "msrv" "$?"
cargo hack check --feature-powerset --depth 2 --no-dev-deps --workspace --locked || fail "feature-powerset" "$?"

step "deny + machete"
cargo deny --locked --config deny.toml check || fail "deny" "$?"
cargo machete || fail "machete" "$?"

step "unit + coverage (line AND branch, 90/90)"
cargo +"$NIGHTLY" llvm-cov clean --workspace
cargo +"$NIGHTLY" llvm-cov nextest --workspace --branch --no-report -P unit --locked || fail "llvm-cov nextest" "$?"
# The egui view layer and thin entry points are excluded, per CODINGSTANDARDS-RUST.md:
# the binary's main, the shell app (which owns the untestable eframe entry point), and
# the view/widget-id modules EPIC-11 adds. Everything else must hold 90% line coverage.
cargo +"$NIGHTLY" llvm-cov report --fail-under-lines 90 \
  --ignore-filename-regex 'src/main\.rs|crates/rivercrossing-ui-egui/src/ui/app\.rs|crates/rivercrossing-ui-egui/src/ui/views/|crates/rivercrossing-ui-egui/src/ui/ids\.rs' \
  || fail "line coverage" "$?"

printf '\nGATE PASSED\n'
