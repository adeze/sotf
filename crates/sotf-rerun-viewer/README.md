# Scientific viewer

Read-only Rerun 0.40.0 SDK integration. The app owns the session, local HTTP web
assets, loopback gRPC endpoint and uniquely named `.rrd` recording. No Python,
external Rerun executable, chat or receiver control is required. The live server
retains at most 128 MiB of temporal data; the recording retains flushed callbacks.

Measurements preserve supplied float64 Hz/dB arrays. Display lines use float32
`(40 × log10(Hz), -dB)` coordinates with original Hz/dB tick labels; this is a
rendering transform, not smoothing or a numerical solver input.
Room models retain model-declared units, coordinates and
source digests. Empty static archetype fields replace prior input components when
projects reload; input names use reversible byte escaping to prevent collisions.

## Native application

From the SOTF checkout, keep the same lean target/profile for subsequent commands:

```sh
CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
  cargo run --target-dir ../target/lean-rerun -j2 -p sotf-gpui \
  --features calibration-designer,rerun-viewer --bin sotf-desktop
```

Room EQ's **Rerun view** opens the inline pane. **Refresh measurements** uses the
current workflow's channel arrays. **Choose room…** selects a room-model JSON
with explicit metre/+Z-up coordinates and a digest-bound relative OBJ reference;
paths outside the model directory are rejected. Loading a model does not approve
its mesh for acoustic solving or infer materials or speaker coordinates.

The macOS embedded WKWebView uses Rerun's documented `renderer=webgl` GPU backend
because its WebGPU path produced a Metal shader compilation error on this host.
The SDK's browser URL keeps the default renderer. The child view hides on
navigation away and remains constrained to the Room EQ content rectangle.

Recordings use the workflow artifact directory, or the platform's `sotf/rerun`
cache directory before a study has an artifact directory. `--qa /absolute/path`
selects a disposable application profile; a temporary macOS bundle needs a unique
bundle identifier so app-control tools do not select an installed SOTF version.

The separate native IR waterfall uses a 32 ms analysis window. Its FFT-bin spacing
does not establish long-window bass-decay resolution; response-only input cannot
produce measured decay.

## Callback contract

`OptimProgressCallback` now accepts `(iteration, loss, epa_preference, response)`.
The additive fourth argument is `Option<Arc<OptimizerResponseSnapshot>>`; adapters
must forward it, or explicitly pass `None` when a backend cannot supply a response.
The standard single-channel DE and EPA objective callbacks derive the response
from their actual candidate vector with existing `x2spl`. They do not rescore the
objective or modify optimization state. Unsupported multi-objective, driver-model
and other backend callbacks explicitly omit the response.

The native Room EQ callback records events before UI progress coalescing. The
`callback` timeline counts actual supplied events, not inferred generations.
Missing response payloads retain the last actual lines, with an explicit status
explaining their age rather than inventing a new curve.
Prepared measurement, target, filter and predicted response remain distinct;
prediction is the prepared measurement plus candidate filter response. Displayed
predictions are not post-calibration acoustic measurements. Recording failures
stop telemetry, preserve solver execution and mark terminal telemetry incomplete.
Open the session before starting a study; a running study retains its captured
session even if another viewer is opened.

## Offline parity study

From the SOTF checkout, using the same lean Cargo target:

```sh
CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
  cargo run --target-dir ../target/lean-rerun -j2 -p sotf-rerun-viewer \
  --example offline_parity -- \
  ../autoeq/data_tests/roomeq/measured/2.0_8361a/L.csv \
  ../target/qa/native-rerun-parity-fresh
```

The output directory must not exist. This invokes the shared native optimization
pipeline twice with identical measured input, explicit DE seed and parameters,
compares scientific results exactly, requires changing callback response frames,
and saves the observed RRD plus a compact parity summary. Hardware is untouched.
