#!/usr/bin/env bash
# Exercise notice staging helpers with temporary aggregate-layout fixtures.
# This sources packaging scripts but never calls their build/package entrypoint.

set -euo pipefail

SCRIPT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TEMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/sotf-notice-stage.XXXXXX")"
trap 'rm -rf "$TEMP_ROOT"' EXIT

FIXTURE_ROOT="$TEMP_ROOT/all_of_sotf"
mkdir -p \
    "$FIXTURE_ROOT/sotf/crates/app-gpui/assets/fonts" \
    "$FIXTURE_ROOT/gpui-toolkit/crates/gpui-d3rs/assets"
printf 'Font notice bytes\n\000end\n' > "$FIXTURE_ROOT/sotf/crates/app-gpui/assets/fonts/THIRD_PARTY_NOTICES.txt"
printf 'DejaVu license bytes\n' > "$FIXTURE_ROOT/gpui-toolkit/crates/gpui-d3rs/assets/DejaVu-LICENSE"

source "$SCRIPT_ROOT/sotf/scripts/build-dmg-sotf.sh"
PROJECT_ROOT="$FIXTURE_ROOT/sotf"
mac_resources="$TEMP_ROOT/macos-app/Contents/Resources"
stage_macos_third_party_notices "$mac_resources"
cmp "$FIXTURE_ROOT/sotf/crates/app-gpui/assets/fonts/THIRD_PARTY_NOTICES.txt" \
    "$mac_resources/THIRD_PARTY_NOTICES.txt"
cmp "$FIXTURE_ROOT/gpui-toolkit/crates/gpui-d3rs/assets/DejaVu-LICENSE" \
    "$mac_resources/DejaVu-LICENSE"

source "$SCRIPT_ROOT/sotf/scripts/build-linux.sh"
PROJECT_ROOT="$FIXTURE_ROOT/sotf"
linux_tarball="$TEMP_ROOT/linux-tarball/share/doc/sotf"
linux_appdir="$TEMP_ROOT/linux.AppDir/usr/share/doc/sotf"
stage_linux_third_party_notices "$linux_tarball"
stage_linux_third_party_notices "$linux_appdir"
for destination in "$linux_tarball" "$linux_appdir"; do
    cmp "$FIXTURE_ROOT/sotf/crates/app-gpui/assets/fonts/THIRD_PARTY_NOTICES.txt" \
        "$destination/THIRD_PARTY_NOTICES.txt"
    cmp "$FIXTURE_ROOT/gpui-toolkit/crates/gpui-d3rs/assets/DejaVu-LICENSE" \
        "$destination/DejaVu-LICENSE"
done

rm "$FIXTURE_ROOT/gpui-toolkit/crates/gpui-d3rs/assets/DejaVu-LICENSE"
if validate_macos_third_party_notice_sources >/dev/null 2>&1; then
    echo "expected missing sibling license to fail macOS validation" >&2
    exit 1
fi
if validate_linux_third_party_notice_sources >/dev/null 2>&1; then
    echo "expected missing sibling license to fail validation" >&2
    exit 1
fi

echo "third-party notice staging tests passed"
