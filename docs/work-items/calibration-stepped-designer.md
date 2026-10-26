# Work item: stepped calibration Designer

Replace the isolated Designer canvas with decision panels. Application shell, engine stage ordering and existing Room EQ wizard remain unchanged.

| Panel | Decision / controls | Existing owner / next consequence |
| --- | --- | --- |
| Data | Review loaded channels; manage measurements | Existing import/Recording routes; no automatic acquisition |
| Checks | Inspect phase availability; open delay/alignment review | Existing preliminary calculations; presence is not qualification |
| Goals | Target slope, bass shelf; link to complete objective options | Shared model updates target configuration; explicit slope selects custom shape |
| Constraints | Correction band, maximum cut/boost | Existing atomic limit validation; edits lock during study |
| Advice | Copy bounded text brief for external discussion | Clipboard only; in-app chat stays disabled; no automatic recommendation acceptance |
| Study | Review inputs, run/cancel, inspect reported stages; open results/export | Existing optimizer and currentness checks; never installs filters |

Dependency order: shared player validation → workbench snapshot/commands → embedded GPUI host → focused model/UI tests and Metal capture. New rendering uses the existing toolkit; no shell migration or new dependencies.

Users may revisit any panel. Snapshot changes clear local review acknowledgement, preserving panel selection. Run requires loaded data, a host-bound input identity, explicit configuration, finite ordered limits, review and an idle engine. Simple-mode users may adopt the displayed settings explicitly before review; study startup must not silently replace the reviewed draft. The host remains authoritative for complete validation and stale results. Historical stage status is distinct from current draft configuration.

Room preview is a channel-role schematic. Current Room EQ data supplies names/roles but no physical coordinates, boundaries or soundstage metric. A future geometry adapter must supply explicit axes, units, coordinate transforms, speaker/listener poses and provenance before the panel can show a physical room. WebGPU shaders may share math/data contracts with native rendering; React/Three.js components cannot be transplanted into GPUI.

Verification: owning-crate tests, Room EQ configuration tests, feature-enabled host check and isolated visual captures. No receiver, audio routing or model calls belong to review.
