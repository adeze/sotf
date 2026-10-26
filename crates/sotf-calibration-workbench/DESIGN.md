# Calibration Designer

## Scope

An embedded workbench, not an application shell or audio-plugin graph. Six accessible tab panels express user decisions; the engine's internal stages appear only as observed study details. No node canvas, draggable wiring, fake status badges or unsupported spatial controls.

## Layout

Header → wrapping numbered tabs → one active content panel → Previous/Next. Keep navigation reachable at narrow widths. Use the host's existing outer scrolling, typography, font zoom and light/dark theme. Do not create nested card hierarchies or a second theme. Stable element IDs preserve focus and numeric editing.

Data shows selected measurement channels and a role schematic. Checks opens existing alignment tools. Goals contains supported target preferences. Constraints contains bounded correction controls. Advice provides a deliberate text handoff. Study summarizes the draft and requires review before calculation; it links to existing result/export views.

## Decisions and feedback

Allow revisiting panels. Loaded data gates study, not browsing. Disable parameter edits during calculation; keep Cancel available. Show actual host validation errors. A changed snapshot clears review without moving the selected tab. The host binds snapshot identity to its complete canonical study inputs, including measurement records, speaker configurations, system and probe delays. Advanced configuration changes participate too. Local review is a usability acknowledgement, not scientific evidence qualification.

Target slope explicitly chooses the existing custom shape; shelf preference layers on the current shape. Explain headroom and tonal tradeoffs. Keep soundstage, seat consistency, timing and bass decay as distinct questions; do not imply that a scalar tonal setting optimizes them all.

## Scientific graphics

Data includes selectable response thumbnails and a larger selected response from saved primary, additional-microphone and driver takes. Selection is retained presentation state only: it does not edit the draft, invalidate study review or change candidate identity. All visible responses share one padded magnitude domain and the same logarithmic 20 Hz–20 kHz frequency domain. At most 128 rows and 128 finite logarithmic-bin extrema per response enter the presentation snapshot; source arrays remain intact. Missing/mismatched responses show an unavailable message. Magnitude is labeled dB without asserting SPL calibration. This display projection is neither evidence qualification nor input smoothing.

Use native toolkit chart/rendering controls and typed source data. Channel names establish role labels only; draw no physical room boundary, distance, acoustic ray, pressure field or predicted stage width without explicit geometry and provenance. Heights remain distinct from horizontal placement. Unknown roles and subwoofer positions stay unspecified.

Reuse data schemas and qualified WGSL mathematics across web/native where practical. React/Three.js scene graphs and event handling require a native adapter; sharing WebGPU does not make them portable widgets.

## Component mapping and state ownership

GPUI Kit documentation is a design reference, not a binary-compatible dependency for the local `gpui-ui-kit` fork.

| Pattern | Designer use / local choice |
| --- | --- |
| Questionnaire | Guided decisions and validation; six retained tab panels with Previous/Next for this implementation |
| Form / InputGroup | One phase owns related labelled controls, units, limits and validation; existing NumberInput widgets |
| DescriptionList | Structured study summary using local MetadataList |
| Empty | Missing measurements with a concrete next action using local EmptyState |
| Collapsible | Advanced options and observed stage details; full existing parameter view remains reachable |
| DataTable / List | Measurement/channel/result browsing stays in existing owner views |
| Command | Existing SOTF palette and typed command handlers; MCP is a separate transport to shared operations |
| Bubble | Future advisory conversation only; no chat runtime activated |
| Carousel | Optional presentation inspiration, not the dependency or validation model |
| Dock | Not used by the current SOTF shell; introducing docking would be separate shell work |

MVVM-style ownership already exists: `RoomEqScreenModel` owns measurement/configuration and validation; `DesignerSnapshot` is a derived bounded projection; the retained `Designer` entity owns selected phase, focus-dependent controls and local review acknowledgement; typed commands reach host side effects. Do not add a second form store, event bus or solver executor. Presentation and MCP may invoke the same model operations without representing command-palette UI as an agent protocol.

References: [Questionnaire](https://gpui-kit.com/component/questionnaire/), [Form](https://gpui-kit.com/component/form/), [Command](https://gpui-kit.com/component/command/), [Dock](https://gpui-kit.com/component/dock/).

## Preferences, inspectors and operation feedback

- Settings patterns belong to global device, appearance and integration preferences. Project-specific acoustic settings stay in the project workflow.
- Sheets suit temporary channel/parameter inspectors that keep the active phase visible. They must preserve drafts on dismissal; no new sheet or docking shell is introduced in this change.
- Pagination suits measurement/result catalogs. Phase navigation uses named steps, not unlabeled page numbers.
- Use the existing Progress control only with backend-reported completion. The Study panel reads `overall_progress` while the solver is running; it does not invent a percentage from elapsed time or selected phase.
- For future measurement loading, show an indeterminate indicator when total work is unknown; show completed/expected takes only when acquisition reports those counts. Shimmer is optional short-lived label/placeholder polish, never a measurement curve, progress authority or substitute for accessible status. Respect reduced-motion preferences before introducing animation.

References: [Settings](https://gpui-kit.com/component/settings/), [Sheet](https://gpui-kit.com/component/sheet/), [Pagination](https://gpui-kit.com/component/pagination/), [Progress](https://gpui-kit.com/component/progress/), [Shimmer](https://gpui-kit.com/component/shimmer/).

## Catalog and comparison extensions

Use tags for channel/seat/role/filter metadata, with human-readable labels independent of color. Use a virtualized list or tree for large measurement catalogs; a tree may group channel → seat → take and phase → nested calculation. Grouping is presentation, not permission to reorder the engine or imply that optional stages executed.

Radar charts are secondary comparison summaries only. Every spoke must declare a measured/predicted metric, unit, direction, reference and fixed normalization; missing metrics remain missing. Do not invent a soundstage score or compare traces with inconsistent scales. Keep response, impulse, decay and directivity plots available as the underlying evidence.

The host's existing theme and design tokens remain authoritative. Stitch may explore composition; generated colors, font sizes and widget APIs must be translated to the local toolkit rather than replace the shell or introduce another theme runtime.

## Native Shell evaluation boundary

Native Shell is a candidate for a separate dynamic-panel pilot, not part of this implementation. Its TSX format uses its own elements, signals and runtime, rather than React hooks/DOM. Existing Rust logic could be called through an explicit host capability; Swift logic needs the established ABI or service boundary. A json-render catalog would need an NSH component/action renderer. React, Three.js, Plotly and WGSL assets do not become native widgets simply by packaging the existing Spatial Studio app.

NSH documents its GPUI backend against GPUI Fast, while SOTF uses a different fork. Before adoption, qualify that compatibility, one form's state/action parity, theme/accessibility behavior and a real scientific plot. Keep the deterministic model and shared commands authoritative; dynamic UI never becomes a second solver. No NSH dependency, runtime, bridge or generated-code execution is introduced here.

References: [Tabs](https://gpui-kit.com/component/tabs/), [Tags](https://gpui-kit.com/component/tag/), [Theme](https://gpui-kit.com/component/theme/), [VirtualList](https://gpui-kit.com/component/virtual-list/), [Charts](https://gpui-kit.com/component/chart/), [Tree](https://gpui-kit.com/component/tree/), [Native Shell](https://nsh.dev/docs/).
