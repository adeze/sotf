#!/bin/bash
# Build the Rust static library for tvOS simulator or device.
#
# This script is called from Xcode's pre-build phase and also works standalone.
# It detects the target architecture from PLATFORM_NAME/ARCHS (set by Xcode)
# or defaults to the tvOS simulator.
#
# ARM64 tvOS targets ship standard libraries through stable rustup.
#
# Output: $SOTF_TVOS_RUST_LIB_DIR/libsotf_tvos.a, or Xcode's DERIVED_FILE_DIR/rust.

set -euo pipefail

# Navigate to the workspace root (3 levels up from tvos/)
WORKSPACE_ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
TVOS_DIR="$(cd "$(dirname "$0")" && pwd)"

echo "=== Building SotF tvOS static library ==="
echo "  Workspace: $WORKSPACE_ROOT"
echo "  tvOS dir:  $TVOS_DIR"

RUSTUP_BIN="${RUSTUP:-}"
if [ -z "$RUSTUP_BIN" ]; then
    if command -v rustup >/dev/null 2>&1; then
        RUSTUP_BIN="$(command -v rustup)"
    elif [ -x "$HOME/.cargo/bin/rustup" ]; then
        RUSTUP_BIN="$HOME/.cargo/bin/rustup"
    else
        echo "error: rustup not found on PATH or at \$HOME/.cargo/bin/rustup" >&2
        echo "       Install Rust or set RUSTUP=/path/to/rustup before running this script." >&2
        exit 127
    fi
fi
CARGO_BIN="${CARGO:-$($RUSTUP_BIN which --toolchain stable cargo)}"
# Keep Cargo and rustc together even when Homebrew's rustc precedes rustup on PATH.
RUSTC="$($RUSTUP_BIN which --toolchain stable rustc)"
export RUSTC
export RUSTUP_TOOLCHAIN=stable

# Determine Rust target from Xcode environment or default to simulator
if [ "${PLATFORM_NAME:-}" = "appletvos" ]; then
    RUST_TARGET="aarch64-apple-tvos"
    PROFILE="release"
elif [ "${PLATFORM_NAME:-}" = "appletvsimulator" ]; then
    RUST_TARGET="aarch64-apple-tvos-sim"
    PROFILE="release"
else
    # Default: simulator (for standalone builds)
    RUST_TARGET="aarch64-apple-tvos-sim"
    PROFILE="release"
fi

echo "  Target:    $RUST_TARGET"
echo "  Profile:   $PROFILE"
echo "  Cargo:     $CARGO_BIN"

# Build against the installed stable target standard library.
cd "$WORKSPACE_ROOT"

if [ "$PROFILE" = "release" ]; then
    "$CARGO_BIN" build --locked -p sotf-tvos --target "$RUST_TARGET" --release
    LIB_DIR="target/$RUST_TARGET/release"
else
    "$CARGO_BIN" build --locked -p sotf-tvos --target "$RUST_TARGET"
    LIB_DIR="target/$RUST_TARGET/debug"
fi

# Copy the .a file to Xcode's derived-file area so build outputs never land in
# the source tree. Standalone invocations fall back to ignored tvos/build/rust/.
OUTPUT_LIB_DIR="${SOTF_TVOS_RUST_LIB_DIR:-${DERIVED_FILE_DIR:-$TVOS_DIR/build}/rust}"
mkdir -p "$OUTPUT_LIB_DIR"
cp "$LIB_DIR/libsotf_tvos.a" "$OUTPUT_LIB_DIR/"

echo "=== SotF tvOS static library built: $OUTPUT_LIB_DIR/libsotf_tvos.a ==="
ls -lh "$OUTPUT_LIB_DIR/libsotf_tvos.a"
