#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-only
#
# Windows cross-check: type-check the UI crate (and the domain crates it pulls in)
# for x86_64-pc-windows-gnu from the Linux build host. This is not part of the
# per-PR gate; the authoritative Windows run is at EPIC-13 on a Windows host.
#
# Prerequisites (one-time): rustup target add x86_64-pc-windows-gnu and
# gcc-mingw-w64-x86-64 (the bundled SQLite C build needs the cross compiler).

set -u

export PATH="$HOME/.cargo/bin:$PATH"

cargo check -p rivercrossing-ui-egui --target x86_64-pc-windows-gnu --locked || {
  printf 'windows cross-check FAILED\n'
  exit 1
}
printf '\nwindows cross-check PASSED\n'
