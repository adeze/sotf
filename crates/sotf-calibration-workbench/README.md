# Calibration Designer workbench

Enable `calibration-designer` in `sotf-gpui` for the isolated Designer inside Room EQ Configure. Parameters remains the default. The application shell, menus, playback and receiver state are unchanged.

The Designer uses six decision panels: **Data → Checks → Goals → Constraints → Advice → Study**. They are not engine nodes. Users can revisit panels without losing the selected step. [Workflow and verification](../../docs/work-items/calibration-stepped-designer.md) and [design rules](DESIGN.md).

`DesignerSnapshot::from_model` projects bounded configuration and lifecycle metadata, not measurement arrays. Snapshot changes clear the local review acknowledgement. `DesignerCommand` delegates to the host's existing data, timing, parameter, study and result routes. Target edits use `RoomEqScreenModel::update_design_target`; limit edits use `update_design_limits`. Both reject edits while running. Explicit target changes select Full mode so Simple presets cannot replace them at study start.

Study requires loaded data, finite ordered limits and local review. Host validation and input-bound result currentness remain authoritative. Prior engine statuses stay separate from draft settings. Engine stage order and conditional execution remain unchanged.

The channel-role preview is a **schematic**, not measured room geometry or a soundstage model. In-app chat is disabled. Copy draft brief writes only text to the clipboard; no data is sent, no model called and no recommendation accepted automatically. Neither Designer nor its diagram acquires measurements, installs filters or mutates a receiver.

For isolated Metal screenshots, the existing `plugin_visual_review` tool accepts `--surface calibration-designer --designer-step 1` through `6`, plus `--light` and `--zoom`. Example:

```sh
cargo run -p sotf-gpui --bin plugin_visual_review --features calibration-designer,visual-qa --locked -- --output target/designer-review --surface calibration-designer --designer-step 4 --plugin Gain --light
```

The local Progress API takes a fraction in 0..=1, unlike the upstream GPUI Kit example's 0..=100. Keep that unit distinction at the presentation boundary.
