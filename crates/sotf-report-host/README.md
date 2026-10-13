# sotf-report-host

Thin offline host for the shared AutoEQ HTML/WASM report.

Generation delegates to AutoEQ's bundled producer; this crate never
duplicates dashboard rendering. Reports are served from an ephemeral
loopback origin so embedded browsers can load the self-contained WASM
assets offline. The server owns one immutable document and stops when
its owner is dropped.
