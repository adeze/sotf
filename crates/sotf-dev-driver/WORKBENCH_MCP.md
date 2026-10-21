# Live workbench capabilities

Internal Adeze integration. Chat, ACP sessions and model providers stay disabled.
The MCP server operates the running SOTF Room EQ workspace; it has no separate
project/configuration store and does not expose generic QA actions.

## Build and connect

From the `all_of_sotf` umbrella:

```sh
rustup run stable cargo build --manifest-path sotf/Cargo.toml \
  -p sotf-gpui -p sotf-dev-driver --bin sotf-desktop --bin sotf-workbench-mcp \
  --features sotf-gpui/workbench-api
export SOTF_WORKBENCH_RUN_ID="$(openssl rand -hex 16)"
target/debug/sotf-desktop --workbench-api 9410
```

Configure an MCP host to launch `target/debug/sotf-workbench-mcp` with arguments
`--endpoint 127.0.0.1:9410`, inheriting the same `SOTF_WORKBENCH_RUN_ID`. Use
absolute executable paths in client configuration. Keep the local credential out
of source and chat. The host speaks MCP over stdio; stdout is protocol-only.
The adapter sends bounded authenticated HTTP requests to the app. The listener
requires the opt-in app feature and CLI option; `--qa` is not required.
The separate `workbench-server` transport feature permits this narrow service in
release builds; the original QA `server` release prohibition stays intact.
No global MCP registration or external service configuration is installed.

## Tools and shared commands

| MCP tool | Existing UI service |
| --- | --- |
| `inspect_workbench` | Live optimizer config, input revision, workspace/channel context, progress, result currentness and artifact directory. |
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
and missing study inputs are rejected. Re-inspect after uncertain timeouts;
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
