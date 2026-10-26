# Live workbench capabilities

Internal Adeze integration. Chat, ACP sessions and model providers stay disabled.
The MCP server operates the running SOTF Room EQ workspace; it has no separate
project/configuration store and does not expose generic QA actions.

## Build and connect

From the `all_of_sotf` umbrella:

```sh
rustup run stable cargo build --manifest-path sotf/Cargo.toml \
  -p sotf-gpui -p sotf-dev-driver --bin sotf-desktop --bin sotf-workbench-mcp \
  --features sotf-gpui/workbench-api,sotf-gpui/calibration-designer
export SOTF_WORKBENCH_RUN_ID="$(openssl rand -hex 16)"
target/debug/sotf-desktop --workbench-api 9410
```

In another terminal, start the MCP adapter with the same `SOTF_WORKBENCH_RUN_ID`:

```sh
target/debug/sotf-workbench-mcp --endpoint 127.0.0.1:9410 --listen 127.0.0.1:9411
```

Configure the MCP host to connect to `http://127.0.0.1:9411/mcp` using
`Authorization: Bearer <SOTF_WORKBENCH_RUN_ID>`. For Codex, use a URL server
with `bearer_token_env_var = "SOTF_WORKBENCH_RUN_ID"`; the client must inherit
the same credential. Keep the credential out of source, command arguments and
chat. This is Streamable HTTP, not the obsolete separate HTTP+SSE transport.
The adapter sends bounded authenticated HTTP requests to the app. The app listener
requires the opt-in app feature and CLI option; `--qa` is not required.
The separate `workbench-server` transport feature permits this narrow service in
release builds; the original QA `server` release prohibition stays intact.
No global MCP registration or external service configuration is installed.

The MCP listener binds only loopback and requires authentication on every request.
The SDK validates exact local Host/Origin authorities and bounds JSON bodies to
16 KiB. Its stateless transport does not own Designer state; every tool delegates
to the same running app handler used previously. Stopping the app makes tools
unavailable even if the adapter remains running. Restart the adapter to apply
Rust/tool-schema changes; MCP transport does not provide Rust hot reload.

For a host that still launches a local child, pass `--stdio --endpoint
127.0.0.1:9410`. This compatibility mode does not open an MCP network listener.
Returning to it is the transport rollback; shared application commands are unchanged.

## Tools and shared commands

| MCP tool | Existing UI service |
| --- | --- |
| `inspect_workbench` | Live optimizer config, input revision, workspace/channel context, current Designer panel/measurement selection, available preview inventory, selected display samples, progress, result currentness and artifact directory. |
| `select_designer_panel` | Open Data, Checks, Goals, Constraints, Advice or Study using the embedded Designer's normal panel selector. Requires `calibration-designer`; Advice leaves chat disabled. |
| `select_measurement` | Select an existing preview by inspected zero-based index through the same UI selector and open Data. Unavailable indices fail without changing the draft. |
| `update_designer_parameters` | Shared `RoomEqScreenModel::update_design_limits`, also used by four Configure controls. Atomic edits select Full mode so wizard defaults cannot replace explicit limits. |
| `run_offline_study` | `PlayerView::start_room_eq_optimization`; inspect to poll completion/failure. |
| `cancel_offline_study` | `PlayerView::cancel_room_eq_optimization`; poll until the backend acknowledges cancellation. |

Patch fields: `min_freq`, `max_freq` in Hz (20–20000, ordered below configured
Nyquist), `max_boost_db`, `max_cut_db` (nonnegative magnitudes, at most 25 dB).
The internal cut floor is negative. Other settings are inspectable but not
agent-editable yet. These are SOTF controls, not Swift's Denon calibration policy.

Inspect first and supply `expected_revision` for edits/run/cancel. Revision hashes
canonical optimizer inputs, including measurements and speaker configuration.
Stale state, unknown fields, invalid/empty/no-op patches, concurrent edits/studies
and missing study inputs are rejected. Panel and measurement selection also
require the current input revision, but are display-only: they do not alter
optimizer inputs, invalidate results or change that revision. The selected
response is the same bounded, unsmoothed 64-log-bin extrema projection used by
the thumbnails, not the full-resolution correction evidence. The capability
inventory distinguishes actual recording spectrogram data and waterfall result
data from Designer controls that are not connected. Re-inspect after uncertain timeouts;
mutations are never automatically retried. Queued commands have deadlines.

Studies use existing Rust offline calculation and normal derived report artifacts.
They do not establish Swift parity, native evidence qualification or acoustic
benefit. Tools do not install DSP, play audio, capture, mutate receivers or
overwrite raw sources. This surface describes the live workspace; it does not
introduce a new cross-language saved-project format.

## Presentation and future adapters

GPUI, SwiftUI and React retain their existing controls. No custom
AppKit/SwiftUI-inside-GPUI adapter is planned. Schemas/snapshots can feed future
JSON-rendered controls without enabling chat. CAD, crossover preview, room fields
and richer solver animation remain separate capabilities; this surface controls
Room EQ only. The Swift/Rust build fixture is not a production workbench ABI.

## Verification

Focused commands, from the SOTF workspace:

```sh
cargo test -p sotf-dev-api workbench_contract
cargo test -p sotf-dev-driver --bin sotf-workbench-mcp
cargo test -p sotf-gpui --test room_eq_config_tests designer_limits
cargo test -p sotf-gpui --test workbench_api --features workbench-api
```

These cover decoding, MCP discovery/authenticated calls, shared validation and
live main-thread/offline calculation paths. Implementation uses the
[official Rust SDK](https://github.com/modelcontextprotocol/rust-sdk), pinned to
`rmcp` 3.6.0.

## Change record

2026-10-22: Streamable HTTP is the default MCP transport; retain explicit stdio
compatibility. Added authenticated HTTP SDK discovery/call coverage and rejection
checks for missing/wrong credentials, duplicate authorization, foreign Host/Origin
and oversized requests. The GPUI shell, offline algorithms and hardware surfaces
are unchanged. No live app, model, receiver or cross-language parity is implied by
the synthetic transport fixture.
