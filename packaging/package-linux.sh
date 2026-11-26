#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-only
#
# Build the Linux x86_64 binary and package it as versioned .deb and .rpm files.
#
# Usage: packaging/package-linux.sh

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT
VERSION="$(grep -m1 '^version = ' "${ROOT}/Cargo.toml" \
  | sed 's/^version = "\(.*\)"/\1/')"
readonly VERSION
OUT="${ROOT}/target/package"
readonly OUT
STAGE="${OUT}/stage"
readonly STAGE

err() {
  printf 'ERROR: %s\n' "$*" >&2
}

main() {
  if [[ -z "${VERSION}" ]]; then
    err "cannot read the version from Cargo.toml"
    exit 1
  fi
  if ! cd "${ROOT}"; then
    err "cannot enter ${ROOT}"
    exit 1
  fi

  rm -rf "${OUT}"
  if ! mkdir -p "${STAGE}/usr/bin" \
      "${STAGE}/usr/lib/rivercrossing/cards" \
      "${STAGE}/usr/share/icons/hicolor/scalable/apps" \
      "${STAGE}/usr/share/applications" \
      "${STAGE}/usr/share/doc/rivercrossing"; then
    err "cannot create the staging directories"
    exit 1
  fi

  if ! cargo build --release -p rivercrossing --locked; then
    err "cargo build failed"
    exit 1
  fi

  if ! cp "target/release/rivercrossing" "${STAGE}/usr/bin/rivercrossing"; then
    err "cannot copy the binary"
    exit 1
  fi
  if ! cp design/assets/cards/*-2x.png \
      "${STAGE}/usr/lib/rivercrossing/cards/"; then
    err "cannot copy the card bitmaps"
    exit 1
  fi
  if ! cp "packaging/branding/svg/icon.svg" \
      "${STAGE}/usr/share/icons/hicolor/scalable/apps/rivercrossing.svg"; then
    err "cannot copy the icon"
    exit 1
  fi
  if ! write_desktop "${STAGE}/usr/share/applications/rivercrossing.desktop"; then
    err "cannot write the desktop entry"
    exit 1
  fi
  if ! cp "LICENSE" "NOTICE" "${STAGE}/usr/share/doc/rivercrossing/"; then
    err "cannot copy the licence texts"
    exit 1
  fi

  if ! command -v nfpm >/dev/null 2>&1; then
    err "nfpm is not installed"
    exit 1
  fi

  if ! write_nfpm "${OUT}/nfpm.yaml"; then
    err "cannot write the nfpm configuration"
    exit 1
  fi

  if ! nfpm pkg --config "${OUT}/nfpm.yaml" --packager deb \
      --target "${OUT}/RiverCrossing-${VERSION}-linux-amd64.deb"; then
    err "nfpm deb failed"
    exit 1
  fi
  if ! nfpm pkg --config "${OUT}/nfpm.yaml" --packager rpm \
      --target "${OUT}/RiverCrossing-${VERSION}-linux-amd64.rpm"; then
    err "nfpm rpm failed"
    exit 1
  fi

  printf 'Built %s\n' "${OUT}"
}

write_desktop() {
  local dest="$1"
  cat > "${dest}" <<'EOF'
[Desktop Entry]
Type=Application
Name=RiverCrossing
Comment=Poker-run ride timing and scoring
Exec=/usr/bin/rivercrossing
Icon=rivercrossing
Terminal=false
Categories=Utility;
EOF
}

write_nfpm() {
  local dest="$1"
  cat > "${dest}" <<EOF
name: rivercrossing
arch: amd64
platform: linux
version: ${VERSION}
section: default
priority: optional
maintainer: Mark Buckaway
vendor: RiverCrossing
homepage: https://github.com/mbuckaway/rivercrossing-rs
license: GPL-3.0-only
description: Poker-run ride timing and scoring
contents:
  - src: ${STAGE}/usr/bin/rivercrossing
    dst: /usr/bin/rivercrossing
  - src: ${STAGE}/usr/lib/rivercrossing/cards/*
    dst: /usr/lib/rivercrossing/cards/
  - src: ${STAGE}/usr/share/icons/hicolor/scalable/apps/rivercrossing.svg
    dst: /usr/share/icons/hicolor/scalable/apps/rivercrossing.svg
  - src: ${STAGE}/usr/share/applications/rivercrossing.desktop
    dst: /usr/share/applications/rivercrossing.desktop
  - src: ${STAGE}/usr/share/doc/rivercrossing/LICENSE
    dst: /usr/share/doc/rivercrossing/LICENSE
  - src: ${STAGE}/usr/share/doc/rivercrossing/NOTICE
    dst: /usr/share/doc/rivercrossing/NOTICE
EOF
}

main "$@"
