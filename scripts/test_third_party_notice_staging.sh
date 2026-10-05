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
    "$FIXTURE_ROOT/gpui-toolkit/crates/gpui-d3rs/assets" \
    "$FIXTURE_ROOT/sotf-daw/assets/third-party/nnnoiseless"
printf 'Font notice bytes\n\000end\n' > "$FIXTURE_ROOT/sotf/crates/app-gpui/assets/fonts/THIRD_PARTY_NOTICES.txt"
printf 'DejaVu license bytes\n' > "$FIXTURE_ROOT/gpui-toolkit/crates/gpui-d3rs/assets/DejaVu-LICENSE"
cp "$SCRIPT_ROOT/sotf-daw/assets/third-party/nnnoiseless/MODEL_NOTICES.txt" \
    "$FIXTURE_ROOT/sotf-daw/assets/third-party/nnnoiseless/MODEL_NOTICES.txt"
cp "$SCRIPT_ROOT/sotf-daw/assets/third-party/nnnoiseless/nnnoiseless-COPYING.txt" \
    "$FIXTURE_ROOT/sotf-daw/assets/third-party/nnnoiseless/nnnoiseless-COPYING.txt"

source "$SCRIPT_ROOT/sotf/scripts/build-dmg-sotf.sh"
PROJECT_ROOT="$FIXTURE_ROOT/sotf"
mac_resources="$TEMP_ROOT/macos-app/Contents/Resources"
mkdir -p "$mac_resources"
printf 'pre-existing app resource\n' > "$mac_resources/CustomResource.txt"
stage_macos_third_party_notices "$mac_resources"
test "$(cat "$mac_resources/CustomResource.txt")" = "pre-existing app resource"
cmp "$FIXTURE_ROOT/sotf/crates/app-gpui/assets/fonts/THIRD_PARTY_NOTICES.txt" \
    "$mac_resources/THIRD_PARTY_NOTICES.txt"
cmp "$FIXTURE_ROOT/gpui-toolkit/crates/gpui-d3rs/assets/DejaVu-LICENSE" \
    "$mac_resources/DejaVu-LICENSE"
cmp "$FIXTURE_ROOT/sotf-daw/assets/third-party/nnnoiseless/MODEL_NOTICES.txt" \
    "$mac_resources/MODEL_NOTICES.txt"
cmp "$FIXTURE_ROOT/sotf-daw/assets/third-party/nnnoiseless/nnnoiseless-COPYING.txt" \
    "$mac_resources/nnnoiseless-COPYING.txt"

# Re-staging byte-identical files must not require writing already-signed
# Resources; a read-only identical destination proves the helper skips copies.
chmod 444 "$mac_resources/MODEL_NOTICES.txt" \
    "$mac_resources/nnnoiseless-COPYING.txt"
stage_macos_third_party_notices "$mac_resources"

# Ensure a failed first model copy remains a failure in conditional context,
# even if a later copy would succeed.
copy_stub_dir="$TEMP_ROOT/cp-failure-bin"
mkdir -p "$copy_stub_dir"
cat > "$copy_stub_dir/cp" << 'CP_STUB'
#!/bin/sh
case "$1" in
    */MODEL_NOTICES.txt)
        exit 1
        ;;
    *)
        exec /bin/cp "$@"
        ;;
esac
CP_STUB
chmod +x "$copy_stub_dir/cp"
chmod u+w "$mac_resources/MODEL_NOTICES.txt"
printf 'stale model notice\n' > "$mac_resources/MODEL_NOTICES.txt"
chmod 444 "$mac_resources/MODEL_NOTICES.txt"
if PATH="$copy_stub_dir:$PATH" stage_nnnoiseless_model_notices \
    "$FIXTURE_ROOT/sotf-daw" "$mac_resources"; then
    echo "expected a failed model notice copy to fail staging" >&2
    exit 1
fi

source "$SCRIPT_ROOT/sotf/scripts/build-linux.sh"
PROJECT_ROOT="$FIXTURE_ROOT/sotf"
linux_tarball="$TEMP_ROOT/linux-tarball/share/doc/sotf"
linux_appdir="$TEMP_ROOT/linux.AppDir/usr/share/doc/sotf"
mkdir -p "$linux_tarball" "$linux_appdir"
printf 'pre-existing tarball file\n' > "$linux_tarball/CustomNotice.txt"
printf 'pre-existing AppDir file\n' > "$linux_appdir/CustomNotice.txt"
stage_linux_third_party_notices "$linux_tarball"
stage_linux_third_party_notices "$linux_appdir"
test "$(cat "$linux_tarball/CustomNotice.txt")" = "pre-existing tarball file"
test "$(cat "$linux_appdir/CustomNotice.txt")" = "pre-existing AppDir file"
for destination in "$linux_tarball" "$linux_appdir"; do
    cmp "$FIXTURE_ROOT/sotf/crates/app-gpui/assets/fonts/THIRD_PARTY_NOTICES.txt" \
        "$destination/THIRD_PARTY_NOTICES.txt"
    cmp "$FIXTURE_ROOT/gpui-toolkit/crates/gpui-d3rs/assets/DejaVu-LICENSE" \
        "$destination/DejaVu-LICENSE"
    cmp "$FIXTURE_ROOT/sotf-daw/assets/third-party/nnnoiseless/MODEL_NOTICES.txt" \
        "$destination/MODEL_NOTICES.txt"
    cmp "$FIXTURE_ROOT/sotf-daw/assets/third-party/nnnoiseless/nnnoiseless-COPYING.txt" \
        "$destination/nnnoiseless-COPYING.txt"
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

printf 'DejaVu license bytes\n' > "$FIXTURE_ROOT/gpui-toolkit/crates/gpui-d3rs/assets/DejaVu-LICENSE"
rm "$FIXTURE_ROOT/sotf-daw/assets/third-party/nnnoiseless/MODEL_NOTICES.txt"
if validate_macos_third_party_notice_sources >/dev/null 2>&1; then
    echo "expected missing model notice to fail macOS validation" >&2
    exit 1
fi
if validate_linux_third_party_notice_sources >/dev/null 2>&1; then
    echo "expected missing model notice to fail Linux validation" >&2
    exit 1
fi

cp "$SCRIPT_ROOT/sotf-daw/assets/third-party/nnnoiseless/MODEL_NOTICES.txt" \
    "$FIXTURE_ROOT/sotf-daw/assets/third-party/nnnoiseless/MODEL_NOTICES.txt"
rm "$FIXTURE_ROOT/sotf-daw/assets/third-party/nnnoiseless/nnnoiseless-COPYING.txt"
if validate_macos_third_party_notice_sources >/dev/null 2>&1; then
    echo "expected missing model license to fail macOS validation" >&2
    exit 1
fi
if validate_linux_third_party_notice_sources >/dev/null 2>&1; then
    echo "expected missing model license to fail Linux validation" >&2
    exit 1
fi

cp "$SCRIPT_ROOT/sotf-daw/assets/third-party/nnnoiseless/nnnoiseless-COPYING.txt" \
    "$FIXTURE_ROOT/sotf-daw/assets/third-party/nnnoiseless/nnnoiseless-COPYING.txt"
: > "$FIXTURE_ROOT/sotf-daw/assets/third-party/nnnoiseless/MODEL_NOTICES.txt"
if validate_macos_third_party_notice_sources >/dev/null 2>&1; then
    echo "expected empty model notice to fail macOS validation" >&2
    exit 1
fi
if validate_linux_third_party_notice_sources >/dev/null 2>&1; then
    echo "expected empty model notice to fail Linux validation" >&2
    exit 1
fi

echo "third-party notice staging tests passed"
