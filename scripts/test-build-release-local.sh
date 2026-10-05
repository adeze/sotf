#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCRIPT="$SCRIPT_DIR/build-release-local.sh"
TEST_ROOT="$(mktemp -d "${TMPDIR:-/private/tmp}/sotf-release-test.XXXXXX")"
trap 'rm -rf "$TEST_ROOT"' EXIT

mkdir -p "$TEST_ROOT/home" "$TEST_ROOT/bin" "$TEST_ROOT/project/scripts"
: > "$TEST_ROOT/home/.sotf-release.conf"
HOME="$TEST_ROOT/home"
export HOME

# Source the orchestrator without running main so the checks exercise its real
# helpers while every external build command remains a local mock.
source "$SCRIPT"
set +e

PROJECT_ROOT="$TEST_ROOT/project"
DIST_DIR="$PROJECT_ROOT/dist"
VERSION=9.8.7
AGGREGATE_ROOT="$TEST_ROOT/aggregate"
mkdir -p "$DIST_DIR" "$AGGREGATE_ROOT/sotf" "$AGGREGATE_ROOT/sotf-daw"
for sibling in sotf-systemwide sotf-capture autoeq math-audio gpui-toolkit sofa-reader symphonia-add-ons; do
    mkdir -p "$AGGREGATE_ROOT/$sibling"
done
export PROJECT_ROOT VERSION

cat > "$TEST_ROOT/bin/rustup" <<'MOCK'
#!/usr/bin/env bash
exit 0
MOCK
cat > "$TEST_ROOT/bin/cargo" <<'MOCK'
#!/usr/bin/env bash
if [ "${MOCK_CARGO_FAIL:-false}" = true ]; then
    exit 23
fi
target=""
while [ "$#" -gt 0 ]; do
    if [ "$1" = --target ]; then
        target="$2"
        shift 2
    else
        shift
    fi
done
mkdir -p "$PROJECT_ROOT/target/$target/dist"
printf 'fresh tui %s\n' "$target" > "$PROJECT_ROOT/target/$target/dist/sotf-tui"
printf 'fresh desktop %s\n' "$target" > "$PROJECT_ROOT/target/$target/dist/sotf-desktop"
MOCK
cat > "$TEST_ROOT/bin/lipo" <<'MOCK'
#!/usr/bin/env bash
exit 0
MOCK
chmod +x "$TEST_ROOT/bin/rustup" "$TEST_ROOT/bin/cargo" "$TEST_ROOT/bin/lipo"
PATH="$TEST_ROOT/bin:$PATH"
export PATH

cat > "$PROJECT_ROOT/scripts/build-dmg-sotf.sh" <<'MOCK'
#!/usr/bin/env bash
set -euo pipefail
arch=""
while [ "$#" -gt 0 ]; do
    if [ "$1" = --arch ]; then
        arch="$2"
        shift 2
    else
        shift
    fi
done
mkdir -p "$PROJECT_ROOT/dist"
printf 'fresh image\n' > "$PROJECT_ROOT/dist/sotf-desktop-${VERSION}-macos-${arch}.dmg"
MOCK
chmod +x "$PROJECT_ROOT/scripts/build-dmg-sotf.sh"

cd "$PROJECT_ROOT"
BUILD_RESULTS=()
RELEASE_ARTIFACTS=()
BUILD_FAILED=false
if ! build_macos_target arm64 aarch64-apple-darwin hal; then
    echo "expected mocked macOS target build to pass" >&2
    exit 1
fi
for artifact in \
    "$DIST_DIR/sotf-tui-${VERSION}-macos-arm64" \
    "$DIST_DIR/sotf-desktop-${VERSION}-macos-arm64" \
    "$DIST_DIR/sotf-desktop-${VERSION}-macos-arm64.dmg"; do
    [ -s "$artifact" ] || { echo "missing fresh artifact: $artifact" >&2; exit 1; }
done

# A failed cargo build must not stage a stale target binary into dist.
rm -rf "$DIST_DIR" "$PROJECT_ROOT/target"
mkdir -p "$DIST_DIR" "$PROJECT_ROOT/target/aarch64-apple-darwin/dist"
printf 'stale\n' > "$DIST_DIR/sotf-tui-${VERSION}-macos-arm64"
printf 'stale\n' > "$PROJECT_ROOT/target/aarch64-apple-darwin/dist/sotf-tui"
clear_versioned_artifacts "$DIST_DIR"
BUILD_RESULTS=()
RELEASE_ARTIFACTS=()
BUILD_FAILED=false
if MOCK_CARGO_FAIL=true build_macos_target arm64 aarch64-apple-darwin hal; then
    echo "expected mocked cargo failure" >&2
    exit 1
fi
[ ! -e "$DIST_DIR/sotf-tui-${VERSION}-macos-arm64" ] || {
    echo "stale artifact survived the fresh-build cleanup" >&2
    exit 1
}
[ "$BUILD_FAILED" = true ] || { echo "cargo failure was not aggregated" >&2; exit 1; }

# A copy failure and a signing failure must also prevent an apparent success.
BUILD_RESULTS=()
RELEASE_ARTIFACTS=()
BUILD_FAILED=false
cp() { return 44; }
if build_macos_target arm64 aarch64-apple-darwin hal; then
    echo "expected mocked artifact copy failure" >&2
    exit 1
fi
unset -f cp
[ "$BUILD_FAILED" = true ] || { echo "copy failure was not aggregated" >&2; exit 1; }

cat > "$PROJECT_ROOT/scripts/sign-macos.sh" <<'MOCK'
#!/usr/bin/env bash
exit 45
MOCK
chmod +x "$PROJECT_ROOT/scripts/sign-macos.sh"
BUILD_RESULTS=()
RELEASE_ARTIFACTS=()
BUILD_FAILED=false
SKIP_MACOS=false
SKIP_SIGN=false
DEVELOPER_ID=mock-identity
APPLE_ID=""
if build_macos; then
    echo "expected mocked macOS signing failure" >&2
    exit 1
fi
[[ "$BUILD_FAILED" = true ]] || { echo "signing failure was not aggregated" >&2; exit 1; }
unset DEVELOPER_ID APPLE_ID

# The source manifest must cover sibling source paths and detect changed bytes.
printf 'root source\n' > "$AGGREGATE_ROOT/Justfile"
printf 'sibling source\n' > "$AGGREGATE_ROOT/sotf-daw/Cargo.toml"
manifest="$TEST_ROOT/source-manifest.sha256"
write_linux_source_manifest "$AGGREGATE_ROOT" "$manifest"
grep -q '^# source sotf-daw head ' "$manifest"
grep -q 'sotf-daw/Cargo.toml' "$manifest"
(cd "$AGGREGATE_ROOT" && grep -v '^#' "$manifest" | shasum -a 256 -c - >/dev/null)

# Exercise the Linux orchestration without a network connection. These shell
# mocks materialize the remote snapshot under the test directory, perform the
# same hash check there, and create the two required architecture outputs.
LINUX_HOST=mock-builder
LINUX_PORT=22
LINUX_SSH_KEY=""
LINUX_PROJECT_DIR="$TEST_ROOT/remote/sotf"
mkdir -p "$TEST_ROOT/remote"
remote_sotf() {
    local snapshot
    for snapshot in "$TEST_ROOT"/remote/.sotf-release-*; do
        [ -d "$snapshot" ] && { printf '%s/sotf\n' "$snapshot"; return; }
    done
    return 1
}
ssh_cmd() {
    local cmd="$4"
    local remote_root manifest
    case "$cmd" in
        mkdir*)
            local dir
            dir="$(printf '%s\n' "$cmd" | sed -n "s/.*mkdir '\\([^']*\\)'$/\\1/p")"
            mkdir -p "$dir"
            ;;
        *"shasum -a 256 -c"*)
            remote_root="$(printf '%s\n' "$cmd" | sed -n "s/^cd '\\([^']*\\)'.*/\\1/p")"
            manifest="$(printf '%s\n' "$cmd" | sed -n "s/.*grep -v '\\^#' '\\([^']*\\)'.*/\\1/p")"
            (cd "$remote_root" && grep -v '^#' "$manifest" | shasum -a 256 -c - >/dev/null)
            ;;
        *"build-linux.sh --appimage"*)
            local dir
            dir="$(remote_sotf)"
            mkdir -p "$dir/dist"
            printf 'x86 tar\n' > "$dir/dist/sotf-desktop-${VERSION}-linux-x86_64.tar.gz"
            printf 'x86 image\n' > "$dir/dist/sotf-desktop-${VERSION}-linux-x86_64.AppImage"
            ;;
        *"just docker-linux-arm64"*)
            local dir
            dir="$(remote_sotf)"
            mkdir -p "$dir/dist"
            printf 'arm tar\n' > "$dir/dist/sotf-desktop-${VERSION}-linux-arm64.tar.gz"
            printf 'arm image\n' > "$dir/dist/sotf-desktop-${VERSION}-linux-arm64.AppImage"
            ;;
        *)
            echo "unexpected mocked ssh command: $cmd" >&2
            return 97
            ;;
    esac
}
rsync() {
    local last="" previous="" arg destination
    for arg in "$@"; do previous="$last"; last="$arg"; done
    destination="${last#*:}"
    destination="${destination%/}"
    [ -d "$AGGREGATE_ROOT/sotf-daw" ] || return 98
    cp -R "$AGGREGATE_ROOT/." "$destination/"
}
rsync_from() {
    local remote_sotf_dir
    remote_sotf_dir="$(remote_sotf)"
    cp "$remote_sotf_dir/dist/"*"$VERSION"* "$DIST_DIR/"
}
file() {
    case "$*" in
        *linux-x86_64*) printf 'ELF 64-bit LSB pie executable, x86-64\n' ;;
        *linux-arm64*) printf 'ELF 64-bit LSB pie executable, ARM aarch64\n' ;;
        *) return 1 ;;
    esac
}
BUILD_RESULTS=()
RELEASE_ARTIFACTS=()
BUILD_FAILED=false
SKIP_LINUX=false
DRY_RUN=false
build_linux
for artifact in \
    "$DIST_DIR/sotf-desktop-${VERSION}-linux-x86_64.tar.gz" \
    "$DIST_DIR/sotf-desktop-${VERSION}-linux-x86_64.AppImage" \
    "$DIST_DIR/sotf-desktop-${VERSION}-linux-arm64.tar.gz" \
    "$DIST_DIR/sotf-desktop-${VERSION}-linux-arm64.AppImage"; do
    [ -s "$artifact" ] || { echo "missing mocked Linux artifact: $artifact" >&2; exit 1; }
done
remote_root="$(dirname "$(remote_sotf)")"
remote_manifest="$remote_root/.release-source-manifest-${VERSION}-"*.sha256
[ -s $remote_manifest ] || {
    echo "Linux snapshot omitted its source manifest" >&2
    exit 1
}
grep -q 'sotf-daw/Cargo.toml' $remote_manifest

printf 'changed source\n' >> "$AGGREGATE_ROOT/sotf-daw/Cargo.toml"
if (cd "$AGGREGATE_ROOT" && grep -v '^#' "$manifest" | shasum -a 256 -c - >/dev/null 2>&1); then
    echo "source hash manifest accepted modified sibling source" >&2
    exit 1
fi

# Skip-windows contributes no build result, while any recorded target failure
# makes main fail and prevents post-build signing/metadata.
BUILD_RESULTS=()
BUILD_FAILED=false
SKIP_WINDOWS=true
build_windows
[ "$BUILD_FAILED" = false ] || { echo "skipped Windows target marked the run failed" >&2; exit 1; }
preflight() { return 0; }
build_macos() { record_result 'macOS ARM64: FAILED (mock)'; return 1; }
build_linux() { return 0; }
post_build() { echo "post-build must not run after a required-target failure" >&2; return 90; }
print_summary() { :; }
DRY_RUN=false
POST_ONLY=false
SKIP_MACOS=false
SKIP_LINUX=false
SKIP_WINDOWS=true
if main; then
    echo "main reported success after a required-target failure" >&2
    exit 1
fi

echo "build-release-local mocked regressions passed"
