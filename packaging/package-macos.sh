#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-only
#
# Build the unsigned macOS arm64 app bundle and package it as a versioned DMG.
#
# Usage: packaging/package-macos.sh

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT
VERSION="$(grep -m1 '^version = ' "${ROOT}/Cargo.toml" \
  | sed 's/^version = "\(.*\)"/\1/')"
readonly VERSION
TARGET="aarch64-apple-darwin"
readonly TARGET
OUT="${ROOT}/target/package"
readonly OUT
APP="${OUT}/RiverCrossing.app"
readonly APP
DMG="${OUT}/RiverCrossing-${VERSION}-macos-arm64.dmg"
readonly DMG

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
  if ! mkdir -p "${APP}/Contents/MacOS" \
      "${APP}/Contents/Resources/cards"; then
    err "cannot create the app bundle directories"
    exit 1
  fi

  export MACOSX_DEPLOYMENT_TARGET="11.0"
  if ! cargo build --release --target "${TARGET}" \
      -p rivercrossing --locked; then
    err "cargo build failed"
    exit 1
  fi

  if ! cp "target/${TARGET}/release/rivercrossing" \
      "${APP}/Contents/MacOS/rivercrossing"; then
    err "cannot copy the binary"
    exit 1
  fi
  if ! cp "packaging/branding/RiverCrossing.icns" \
      "${APP}/Contents/Resources/RiverCrossing.icns"; then
    err "cannot copy the app icon"
    exit 1
  fi
  if ! cp design/assets/cards/*-2x.png \
      "${APP}/Contents/Resources/cards/"; then
    err "cannot copy the card bitmaps"
    exit 1
  fi
  if ! cp "LICENSE" "NOTICE" "${APP}/Contents/Resources/"; then
    err "cannot copy the licence texts"
    exit 1
  fi

  if ! write_info_plist "${APP}/Contents/Info.plist"; then
    err "cannot write Info.plist"
    exit 1
  fi

  STAGE="${OUT}/dmg"
  if ! mkdir -p "${STAGE}"; then
    err "cannot create the DMG staging directory"
    exit 1
  fi
  if ! cp -R "${APP}" "${STAGE}/"; then
    err "cannot stage the app bundle"
    exit 1
  fi
  if ! ln -s /Applications "${STAGE}/Applications"; then
    err "cannot stage the Applications symlink"
    exit 1
  fi

  if ! hdiutil create -volname "RiverCrossing" -srcfolder "${STAGE}" \
      -ov -format UDZO "${DMG}"; then
    err "hdiutil failed"
    exit 1
  fi

  printf 'Built %s\n' "${DMG}"
}

write_info_plist() {
  local dest="$1"
  cat > "${dest}" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>
  <string>RiverCrossing</string>
  <key>CFBundleDisplayName</key>
  <string>RiverCrossing</string>
  <key>CFBundleIdentifier</key>
  <string>com.buckaway.rivercrossing</string>
  <key>CFBundleVersion</key>
  <string>${VERSION}</string>
  <key>CFBundleShortVersionString</key>
  <string>${VERSION}</string>
  <key>CFBundleExecutable</key>
  <string>rivercrossing</string>
  <key>CFBundleIconFile</key>
  <string>RiverCrossing.icns</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>LSMinimumSystemVersion</key>
  <string>11.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
EOF
}

main "$@"
