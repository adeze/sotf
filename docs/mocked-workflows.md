# Mocked workflow testing

Tracked in private Gitea issue [#184](http://192.168.1.32:3001/pierre/sotf/issues/184).

## Boundaries

Tests replace the audio device boundary with `sotf-capture`'s `VirtualWire`.
They retain the production sweep generation, recording, deconvolution,
harmonic analysis, quality evaluation, review, persistence and RoomEQ import.
Do not mark a capture complete or inject optimizer results to satisfy a test.
The virtual backend is feature-gated by `test-util`, propagated through the
apps' `dev-api` feature. Configuration belongs to each app instance.

The GPUI suite uses its capture fixture endpoint; the TUI suite uses
`/qa/capture/virtual`. The driver selects the route from the configured app
binary. Fixture arrangement may choose input data, acoustic profile and
output location; subsequent steps must use normal UI input. One-shot device
loss and file I/O errors are distinct from measured clipping.

Learning and ABX playback use a separate per-player output selector:
`POST /qa/playback/output` with `{"enabled":true}`, arranged by the driver's
`mock_playback` scenario option before playback starts. The engine's
feature-gated file-test sink reuses its existing clocked null-output runtime.
It retains real file decoding, DSP, transport, processing counters and meters.
It bypasses hardware output selection; it does not inject playback state.
Changing the selector while an engine is active returns an error.
The current test output accepts 44.1, 48, 88.2, 96, 176.4 and 192 kHz sources;
32 and 22.05 kHz files are outside its present validation policy. Keep fixture
rates explicit rather than treating such a rejected source as a hardware skip.

## Quality evidence

| Profile | Expected measured behavior | Intended review |
|---|---|---|
| `clean` | Linear response with deterministic small noise | Accepted when the applicable checks pass |
| `high-thd` | Nonlinear response, Farina harmonic THD above 10% | Warning and retake |
| `low-snr` | Measured quiet-window noise, SNR below 10 dB | Warning and quieter retake |
| `unknown-evidence` | Digital silence does not supply a noise-floor estimate | Unknown evidence, never an SNR of zero or a pass |
| `clipping` | Full-scale sample count greater than zero | Lower gain and retake |

These thresholds preserve the capture library's existing policies; they are
not universal acoustic acceptability limits. Single-take advisory SNR uses
10 dB, while coherent multi-channel capture uses 30 dB. Farina THD is a
harmonic-only estimate from separated harmonic impulse responses, not THD+N.

Typed findings retain policy, metric, status, measured value, units, frequency
band, method, threshold, unknown reason and recommendation. Legacy issue
strings remain for older consumers. New serialized fields use defaults so
older reports continue to load. Retaking one source must preserve unaffected
sources, microphone identity, position and take provenance.

## Shared AutoEQ dashboards

`sotf-report-host` packages the upstream AutoEQ producer and the same report
shell/WASM bundles used by standalone reports. It does not recreate the DSP,
acoustics or psychoacoustic dashboards in GPUI. The existing compact review
charts remain previews; the full report button opens the shared viewer.
The save button writes the same self-contained HTML document.

The AutoEQ checkout owns the tracked `dist/` bundles. After changing the
upstream renderer, run `just refresh-shared-report` and rebuild SOTF to embed
the refreshed bundle. This delegates to AutoEQ's stable 2D WASM build; it does
not introduce a second dashboard source tree.

Generation runs on a background worker and uses the optimizer's artifact
directory to resolve response and impulse-response sidecars. If inputs change
while generation runs, GPUI rejects the obsolete result. The native browser
loads one immutable document from an ephemeral loopback origin. Closing the
window drops the webview, stops its server and releases temporary files.
Navigation is scoped to that document. IPC messages are bounded and bound to
the report content identity.

Current integration limitations:

- The upstream full-report producer requires Python and NumPy. Set
  `SOTF_REPORT_PYTHON` to an existing compatible interpreter. No package
  installation or network operation occurs during report generation.
- Embedded native viewing is currently supported on macOS and Windows.
  Standalone HTML export is independent of that platform feature.
- The host protocol reserves selection/retake commands, but the upstream
  viewer and app have not yet connected those interactions. Do not count
  protocol parsing tests as a retake E2E test.
- The shared shell currently owns its chart colors and language. Host
  controls are localized; a complete report theme/locale contract belongs
  upstream so the renderer remains shared.
- Capture-quality sidecars are shown in recording review and RoomEQ import.
  GPUI adapts the imported measurement snapshots to AutoEQ's version-1
  `CaptureQualityReport` input. The shared Rust/WASM library builds the full
  report's Capture quality dashboard, retaining policy, method, band, threshold,
  Unknown reason, corrective action and per-source stimulus settings. The host
  preserves the other upstream sections and their additive layout metadata.
  CSV columns alone do not establish quality evidence: an import without typed
  findings is shown as Unknown, rather than zero THD or a quality pass.

## Migration toward a mostly WASM report

AutoEQ already owns `autoeq-report-wasm`'s versioned schema, assembly and 2D
renderer. `autoeq-report-gpui` consumes that schema and adds the optional
WebGPU explorer. The Python producer currently resolves artifacts, calculates
report summaries and builds the section payload. Embedding its existing output
is the first integration step; recreating those calculations in app-gpui would
create a second implementation.

The capture-quality input and section builder are implemented upstream in an
isolated AutoEQ branch; the concurrent STI worktree remains untouched. The
remaining migration keeps the complete optimizer report and derived summaries
under that same upstream ownership:

1. Define an upstream report-input contract containing optimization results,
   resolved measurement/IR arrays, capture quality and source identities. The
   native host resolves files; the renderer receives data rather than filesystem
   paths. Keep unavailable values explicit and reject unsupported schema versions.
2. Move payload assembly and report-specific derived summaries into an upstream
   Rust library, with the same entry point available natively and through WASM.
   Reuse existing acoustics/DSP crates for calculations; retain one owner for
   every dashboard. Compare its populated payload against the Python producer
   before removing the Python dependency.
3. Ship one shared shell and WASM bundle for browser reports and native webviews.
   Add theme, locale, resize and versioned selection/retake/export messages to
   that upstream contract. Keep app-specific recording actions in the host.
4. Use the same fixtures for standalone HTML, the default 2D renderer, optional
   WebGPU explorer and native embedding. Test missing evidence, non-finite data,
   large multi-position sessions and compatibility with older saved results.
5. Remove the packaged Python requirement once payload parity and standalone
   export pass. Extend the thin webview host to Linux without introducing native
   copies of the dashboard components.

Each captured take snapshots the effective signal settings and generated/padded
frame durations. Version-1 quality sidecars additively retain that stimulus
provenance. GPUI explains settings changes before editing: structural changes
invalidate takes, while gain-only changes can preserve neighboring takes when
every take has provenance. Sweep response magnitudes are reference-normalized;
THD and SNR retain the actual capture level rather than implying one common
measurement level. TUI shows effective duration and context-specific focus hints.
Quality sidecars preserve review evidence across save/reload and RoomEQ import,
so the import screen can show the original Warning or Unknown decision.

## Test tiers

Run from the SOTF workspace:

```sh
just test-workflow-contracts
SOTF_REPORT_PYTHON=../autoeq/.venv/bin/python just test-shared-report
just test-recording-gpui
just test-listening-gpui
just test-mocked-tui
```

The contract tier checks real emulated capture analysis, shared learning and
ABX controller behavior, and driver fixture routing. Controller checks include
deterministic questions, scoring, history, real plugin level matching and
reported-path latency handling. They complement UI tests; they do not prove
that a screen is usable.

The report tier checks upstream dashboard production, export fidelity, stale
protocol identities, scoped navigation, server shutdown, the STI dashboard and
shared capture-quality augmentation without modifying existing sections. It needs local
socket permission, but no audio hardware. Build both apps with `dev-api` for
UI suites. UI runs also need a graphical session for GPUI or a terminal for
TUI. Keep numerical analysis checks in the fast tier and screenshots/keyboard
flows in the UI tier.

The dedicated `Mocked workflow contracts` CI workflow checks out sibling
repositories before running the contract and report tiers; it does not assume
that a standalone SOTF checkout contains them. Configure
`WORKSPACE_READ_TOKEN` if the runner's normal token cannot read the private
siblings. `SOTF_CAPTURE_REF`, `SOTF_DAW_REF`, `AUTOEQ_REF` and
`GPUI_TOOLKIT_REF` repository variables can select compatible companion
revisions until those changes merge. This job
installs NumPy on the runner and preserves both contract logs on failure.
Native UI suites remain a separate desktop/terminal tier and retain their
own screenshots, state snapshots and app logs.

For each failing UI scenario preserve the scenario log, application log,
query snapshot, screen capture and measurement/output artifacts. A scenario
that cannot start must fail visibly; it must not silently become a passing
hardware skip.

## Completion checks

Before considering the full work complete, verify both apps through normal
input for learning, ABX, recording and RoomEQ. Verify clean capture, measured
warnings, missing evidence, cancellation, error recovery, selective retake,
save/reload, and a capture-to-RoomEQ handoff. RoomEQ must run the real optimizer
and validate exported correction and application state. Verify the shared
report in a native window, including resizing and closing, with populated
results rather than only an empty dashboard fixture.

Release work still needs a portable report-production runtime or an upstream
reusable producer API, Linux viewer integration, shared theme/locale support,
and the viewer-to-app selection/retake contract. These should reuse the
AutoEQ implementation rather than adding native dashboard copies.

## Recording and RoomEQ improvement sequence

| Deficiency found | Change or next step | Evidence required |
|---|---|---|
| A completed capture could enter ReviewNeeded without refreshing the GPUI view | Notify the view after every completion path, including parked review and errors | The actual Accept/Retake controls appear after worker completion without another user gesture |
| Capture work blocked the GPUI foreground executor | Run acquisition and analysis on a background executor, deliver completion through the app context | Cancel and query the UI while a real virtual sweep is running |
| A scalar THD/SNR or issue string lacked measurement context | Carry typed findings through take review; display value, unit, policy threshold, method, unknown reason and corrective action | High distortion, low SNR, clipping and unavailable evidence produce different review messages |
| Existing fake capture marked recording complete without exercising analysis | Mock only the device boundary with deterministic VirtualWire profiles | Saved WAV/CSV and quality findings arise from real capture and deconvolution |
| Learning and ABX tests depended on a physical output | Use a per-player, feature-gated clocked file sink | Real decode/DSP and normal start, switch, answer, pause and persistence actions work without hardware |
| Saving a capture did not offer a direct GPUI RoomEQ handoff | Add a visible action that imports the saved `recordings.json` through the normal RoomEQ loader | Same-session capture, review, save, import, optimize, export and apply |
| Full report information was unavailable in the app | Embed the shared AutoEQ WASM report in a thin native host and export the same document | Populated dashboards render, resize, close and retain standalone export fidelity |
| Sweep duration controls did not describe the generated logarithmic sweep duration | Make the capture model expose effective duration and bind or disable each duration control by signal type | Changing a visible acquisition setting changes the real saved capture as described; estimates match progress |
| Changing session-wide settings silently discarded other take statuses | Explain invalidation and preserve per-take settings before allowing compatible selective changes | Edited gain can be traced per take; unchanged neighbors retain evidence when compatibility permits |
| Quality findings disappeared between recording review and RoomEQ import | Persist source-bound evidence in a versioned sidecar and show it in the import UI | Warning and Unknown decisions survive save/reload without attaching to another source |
| Recording save and review actions could fall below smaller viewports | Keep primary Save/Retake/Continue actions in a stable wizard footer; make dense evidence panels scroll independently | Complete the same flow at narrow and short window sizes without losing access to the primary action |
| TUI step-tab focus and file editing were easy to confuse | Show the current focus level and context-specific key hints from the existing keybinding catalog | Keyboard-only import and export tests assert focus transitions before typing or navigation |
| Old RoomEQ fixtures used a retired schema | Keep fixtures in the current RoomConfig format and drive the current import UI | Real optimization succeeds and exported correction is usable |

Follow-up work should retain these boundaries. First make report generation
self-contained for packaged apps through an upstream reusable producer API
or a shipped runtime. Then extend the same host to Linux and negotiate theme,
locale and report selection messages with the shared viewer. Keep quality
policies explicit and configurable by measurement purpose rather than
hard-coding a universal THD/SNR acceptance standard into either UI.
