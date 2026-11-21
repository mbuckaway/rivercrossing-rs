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
CARGO_BUILD_WARNINGS=deny cargo clippy --workspace --all-targets --locked --keep-going || fail "clippy" "$?"

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
cargo +"$NIGHTLY" llvm-cov report --fail-under-lines 90 --ignore-filename-regex 'src/main\.rs' || fail "line coverage" "$?"

printf '\nGATE PASSED\n'
