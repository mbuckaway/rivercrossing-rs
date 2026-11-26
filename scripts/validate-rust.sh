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

step "gitleaks (no committed or working-tree secrets)"
if ! command -v gitleaks >/dev/null 2>&1; then
  fail "gitleaks not installed (brew install gitleaks)" 1
fi
gitleaks git . --no-banner --redact --config .gitleaks.toml || fail "gitleaks git" "$?"
gitleaks dir . --no-banner --redact --config .gitleaks.toml || fail "gitleaks dir" "$?"

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
# No `--no-dev-deps`: it rewrites each member manifest to drop dev-dependencies, which makes
# the committed lock stale, and `--locked` then refuses the update. The test deps resolve fine
# in this check without that flag.
cargo hack check --feature-powerset --depth 2 --workspace --locked || fail "feature-powerset" "$?"

step "deny + machete"
cargo deny --locked --config deny.toml check || fail "deny" "$?"
cargo machete || fail "machete" "$?"

step "layers"
# Nothing below rivercrossing-ui-egui may depend on egui/eframe/winit/rfd/egui_kittest
# (AGENTS.md, CODINGSTANDARDS-RUST.md). The check reads direct deps only.
cargo xtask layers || fail "layers" "$?"

step "ui (egui_kittest snapshots)"
WGPU_BACKEND=vulkan cargo nextest run -P ui -p rivercrossing-ui-egui --locked || fail "ui" "$?"

step "release check (debug-gated control path inert)"
cargo check -p rivercrossing --release --locked || fail "release check" "$?"

step "unit + coverage (line AND branch, 90/90)"
cargo +"$NIGHTLY" llvm-cov clean --workspace
cargo +"$NIGHTLY" llvm-cov nextest --workspace --branch --no-report -P unit --locked || fail "llvm-cov nextest" "$?"
# The egui view layer and thin entry points are excluded, per CODINGSTANDARDS-RUST.md:
# the binaries' main, the shell app (which owns the untestable eframe entry point), the
# xtask entry point, the view/widget-id modules EPIC-12 adds, the control-channel
# glue (Tier B only; its pure helpers are unit-tested), and the WordPress network
# adapter (its agent policy is unit-tested; only the socket call is unreachable offline).
# Everything else must hold 90% line AND 90% branch.
COVERAGE_IGNORE='src/main\.rs|xtask/src/main\.rs|crates/rivercrossing-ui-egui/src/ui/app\.rs|crates/rivercrossing-ui-egui/src/ui/views/|crates/rivercrossing-ui-egui/src/ui/ids\.rs|crates/rivercrossing-ui-egui/src/ui/automation/channel\.rs|crates/rivercrossing-wordpress/src/transport\.rs'
cargo +"$NIGHTLY" llvm-cov report --fail-under-lines 90 \
  --ignore-filename-regex "$COVERAGE_IGNORE" \
  || fail "line coverage" "$?"
# cargo-llvm-cov has no --fail-under-branches, so the branch floor is read from the JSON
# totals. Summing every data entry mirrors upstream's own percentage calculation.
cargo +"$NIGHTLY" llvm-cov report --json --summary-only \
  --ignore-filename-regex "$COVERAGE_IGNORE" > "$OUT/coverage.json" \
  || fail "coverage json" "$?"
python3 scripts/branch-coverage.py "$OUT/coverage.json" 90 || fail "branch coverage" "$?"

printf '\nGATE PASSED\n'
