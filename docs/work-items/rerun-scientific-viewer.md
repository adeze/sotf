# Rerun scientific workbench

Status: opt-in integration, focused tests, offline parity and native visual
qualification complete on this macOS host.

Scope: opt-in inline Rerun web viewer inside the existing Room EQ workspace;
unchanged application shell and disabled chat. One native session owns loopback
HTTP/gRPC services and a unique recording. No Python runtime or external viewer
process is required by the Rust application.

Ownership:
- `sotf-rerun-viewer`: SDK/session lifetime, typed scientific logging and
  source-bound geometry import.
- GPUI: bounded child-webview lifetime and existing measurement/workflow state.
- AutoEQ: actual optimizer callback data and existing response calculations.
  The viewer never supplies an objective, candidate or second DSP executor.

Qualification: existing two IR waterfall tests; session/validation tests;
source-digest/mesh importer tests; solver result parity; live room/measurement/
solver inspection and native resize/navigation checks. Use one lean Cargo
target without debug symbols or incremental caches; stop below 12 GiB free.

Unavailable curves and fields stay unavailable. A 32 ms waterfall analysis is
not a long-window bass-decay measurement. Registered room source geometry is
not an approved acoustic boundary shell.

Verified locally (2026-10-22/23): waterfall tests 2/2; viewer navigation policy
1/1; viewer/import/validation tests 3/3; optimizer response-observer parity 1/1.
The native player offline study emitted 78 changing response frames and retained
exact DSP, final curves and score parity with observation disabled/enabled.
The initial QA application rendered real measured responses and the saved
digest-bound VisionPlan mesh inside WKWebView; navigating Home hid the child
view. A real bounded DE run from the application produced callback history and
response lines. These checks do not establish acoustic improvement.

Visual review caught the 1:1 Spatial2D aspect ratio compressing log-frequency
curves. The display now uses `40 * log10(frequency_hz)` and `-level_db`, with
explicit original-unit grid/tick labels. Recorded scientific arrays are unchanged.
The updated executable renders labeled measured response curves and the saved
room mesh inside the native application. Window zoom/resizing was inspected.
WKWebView's WebGPU path failed shader compilation on this host; the macOS child
now selects Rerun's documented WebGL GPU backend. Browser URLs retain the default
renderer. Screenshots are in the private QA directory below. Other platforms,
acoustic performance and long-window bass-decay visualization are not qualified.

Local receipts: umbrella `target/qa/waterfall-tests-20261022/`,
`target/qa/native-rerun-checks-20261022/`,
`target/qa/native-rerun-parity-20261022/parity.json` and
`target/qa/native-rerun-ui-20261022/` (all disposable/private, not source assets).
Builds used one lean target, serialized Cargo processes and a 12 GiB free-space
floor. Standalone AutoEQ and SOTF workspace profiles differ, so their checks
compile different feature/profile graphs despite sharing that target directory.

Deferred candidates:
- `manifold-rust`: solid cabinet/duct construction and Boolean operations;
  non-closed scan geometry still requires explicit boundary interpretation.
- `diffsol`: electromechanical/thermal ODE/DAE transients and sensitivities;
  not a spatial room-wave discretization or replacement for the optimizer.

Neither candidate is added to this integration's dependency graph.

## Additional library screening, 2026-10-22

Documentation screening only; no installation, benchmark or numerical qualification.
Rerun remains the viewer. Existing workspace manifests already declare RustFFT,
RealFFT, Symphonia and the SOTF CoreAudio fork.

| Candidate | Fit / decision |
| --- | --- |
| [mefikit](https://github.com/asonolet/mefikit) | Shortlist for unstructured mesh topology, fields, spatial queries and field transfers; not an acoustic solver by itself. |
| [audionimbus](https://github.com/MaxenceMaire/audionimbus) | Shortlist for Steam Audio propagation/reflections and auralization; introduces a native SDK. Do not treat a rendered preview as measured calibration or a qualified low-frequency wave solution. |
| [simple-si-units](https://github.com/DrPlantabyte/simple-si-units) | Evaluate at calculation boundaries for dimensions and conversions; avoid a second competing unit model. |
| [audioadapter](https://github.com/HEnquist/audioadapter-rs) | Consider if a concrete interleaved/planar or sample-format boundary requires it; custom trait implementations carry buffer-bound safety obligations. |
| [audio_thread_priority](https://github.com/mozilla/audio_thread_priority) | Relevant to owned real-time audio threads, not the offline optimizer or viewer. Inspect existing scheduling before adding. |
| [fundsp](https://github.com/SamiPerttu/fundsp) | Useful composable DSP/synthesis reference; keep the existing SOTF plugin executor. |
| [fft-convolver](https://github.com/neodsp/fft-convolver) | Potential comparison implementation; no replacement without accuracy, latency and allocation benchmarks against existing convolution. |
| [spectrum-analyzer](https://github.com/phip1611/spectrum-analyzer) | FFT convenience layer; current FFT stack already covers the immediate waterfall requirement. |
| [dasp_sample](https://github.com/RustAudio/dasp) | PCM sample conversions; consider only for a missing conversion boundary. |
| [coreaudio-rs](https://github.com/RustAudio/coreaudio-rs) | SOTF already selects its own fork; do not add a parallel device layer. |
| [symphonium](https://docs.rs/symphonium/latest/symphonium/) | Symphonia loading/resampling convenience wrapper; current decoder path already uses Symphonia. Preserve raw measurement sample rates. |
| [mkaudiolibrary](https://github.com/mkaudio-company/mkaudiolibrary) | Another modular audio/plugin stack; no demonstrated missing capability warrants a second plugin architecture. |
| [rust_physics_engine](https://github.com/Magic-Man-us/RustPhysicsEngine) | Broad engineering collection including resonance; function/test counts do not establish acoustic validity. Require equation-level reference cases before reuse. |
| [wellen](https://github.com/ekiwi/wellen) | Digital-logic VCD/FST/GHW waveforms, not acoustic measurement processing. |
| [cfasim](https://github.com/CDCgov/cfa-simulator) | Interactive simulation-app scaffolding, not a room-acoustics solver; duplicates application infrastructure here. |
| [math-iir-fir](https://github.com/pierreaubert/math-audio) | Already part of the local Math Audio workspace; SOTF and SOTF DAW both select the local path dependency. Reuse and contribute there. |
| [lasprs](https://docs.rs/lasprs/0.17.0/lasprs/) | Shortlist for acquisition, calibrated sensor analysis, Welch spectra and transfer-function estimation. Upstream warns of unstable APIs; qualify selected functions before integration. |
| [dasp-rs](https://github.com/dasp-rs/dasp-rs) | Separate project from RustAudio `dasp`; STFT/CQT and music/speech analysis mostly overlap or exceed current scope. Lower priority than LASP for measurement workflows. |
| [rt-fft-convolver](https://docs.rs/rt-fft-convolver/0.2.0/rt_fft_convolver/) | Benchmark candidate: direct-head/FFT-tail convolution, IR crossfades, offline f64. Existing SOTF convolution remains authoritative until accuracy, latency, allocations and filter-switch behavior are compared. Zero algorithmic latency does not remove audio-device buffering. |
| [rack](https://github.com/sinkingsugar/rack) | External AU/VST3 hosting candidate, not the internal workbench plugin protocol. SOTF host already declares optional CLAP/VST3 paths; qualify a missing AU requirement before adding another host. |
| [sofar](https://github.com/andreiltd/sofar) | Pure-Rust SOFA/HRTF reader and renderer; compare supported conventions and interpolation with existing sofa-reader/binaural paths before replacement. Reading HRTF SOFA is not general SOFA export support. |
| [soundscope](https://github.com/bananaofhappiness/soundscope) | Terminal analysis application; useful workflow/reference implementation, not a GPUI/Rerun component. |
| [avaudio](https://github.com/doom-fish/avaudio-rs) | macOS AVFoundation audio graph bindings. Potential platform adapter, not a cross-platform replacement for current capture/playback infrastructure. |
| [auxide](https://docs.rs/auxide/latest/auxide/) | Graph-to-plan-to-runtime audio kernel; useful execution-boundary reference, not a reason to replace SOTF's engine or introduce a node-canvas Designer. |
| [dsp-spectral](https://docs.rs/dsp-spectral/latest/dsp_spectral/) | f64 STFT/ISTFT and spectral-analysis comparison candidate. Independently check window/hop boundaries, normalization and reference spectra; do not denoise measurement authority to make plots look cleaner. |
| [xlsynth-prover](https://docs.rs/xlsynth-prover/latest/xlsynth_prover/) | XLS/DSLX and IR formal equivalence tooling; not a drop-in verifier for arbitrary Rust floating-point DSP or room acoustics. No current integration need. |

Priority after viewer qualification: mesh/field preparation and explicit physical
units, then a bounded auralization comparison. None of these candidates changes
candidate generation or measured-data authority in this integration.
