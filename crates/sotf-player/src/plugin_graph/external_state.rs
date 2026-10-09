//! Reconcile accepted native state without rebuilding the hosted plugin.

use super::PluginGraph;
use sotf_audio::plugins::PluginSettings;
use sotf_plugins::{ExternalPluginEditorData, PluginDescriptor};

fn same_plugin(left: &PluginDescriptor, right: &PluginDescriptor) -> bool {
    left.id == right.id && left.path == right.path && left.format == right.format
}

impl PluginGraph {
    /// Persist an owned native snapshot without changing routing or hosting configuration.
    ///
    /// Callers must first fence the snapshot against the current engine generation
    /// and edit revision. Missing instances, capture failures and mismatched plugin
    /// identities leave the graph unchanged. Returns whether opaque state changed.
    pub fn reconcile_external_editor_state(&mut self, data: &ExternalPluginEditorData) -> bool {
        let Some(instance_id) = data.plugin_instance_id else {
            return false;
        };
        let Ok(native) = &data.native_state else {
            return false;
        };
        if native.validate_descriptor_consistency().is_err()
            || !same_plugin(&native.descriptor, &data.descriptor)
        {
            return false;
        }
        let Some(node) = self
            .nodes
            .values_mut()
            .find(|node| node.plugin.id == instance_id)
        else {
            return false;
        };
        let PluginSettings::External { state } = &mut node.plugin.settings else {
            return false;
        };
        if !same_plugin(&state.descriptor, &data.descriptor)
            || state.opaque_state == native.opaque_state
        {
            return false;
        }
        state.opaque_state.clone_from(&native.opaque_state);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NodePosition, PluginType};
    use sotf_plugins::{ExternalPluginSandboxMode, ExternalPluginState, PluginFormat};
    use std::collections::HashMap;

    fn fixture() -> (PluginGraph, ExternalPluginEditorData) {
        let mut graph = PluginGraph::new();
        let target = graph
            .add_plugin_node(&PluginType::Gain, NodePosition::default())
            .unwrap();
        graph
            .add_plugin_node(&PluginType::Gain, NodePosition::new(200.0, 0.0))
            .unwrap();
        let descriptor = PluginDescriptor {
            id: "fixture".into(),
            name: "Fixture".into(),
            vendor: "SOTF".into(),
            version: "1".into(),
            format: PluginFormat::Clap,
            path: "/fixture.clap".into(),
            audio_inputs: 2,
            audio_outputs: 2,
            is_instrument: false,
            categories: Vec::new(),
            scan_status: Default::default(),
        };
        let original = ExternalPluginState::new(
            descriptor.clone(),
            ExternalPluginSandboxMode::Isolated,
            vec![1],
        );
        let node = graph.nodes.get_mut(&target).unwrap();
        node.plugin.settings = PluginSettings::External {
            state: original.clone(),
        };
        let instance_id = node.plugin.id;
        let mut native = original;
        native.opaque_state = vec![2, 3, 4];
        native.sandbox_mode = ExternalPluginSandboxMode::InProcess;
        let data = ExternalPluginEditorData {
            plugin_instance_id: Some(instance_id),
            descriptor,
            parameters: Vec::new(),
            parameter_values: HashMap::new(),
            native_state: Ok(native),
        };
        (graph, data)
    }

    #[test]
    fn accepted_external_state_preserves_configuration_and_round_trips() {
        let (mut graph, data) = fixture();
        let mut original = graph.clone();
        assert!(graph.reconcile_external_editor_state(&data));
        assert!(!graph.reconcile_external_editor_state(&data));
        let encoded = serde_json::to_value(&graph).unwrap();
        for node in original.nodes.values_mut() {
            if node.plugin.id == data.plugin_instance_id.unwrap() {
                if let PluginSettings::External { state } = &mut node.plugin.settings {
                    state.opaque_state = vec![2, 3, 4];
                }
            }
        }
        assert_eq!(encoded, serde_json::to_value(&original).unwrap());
        let reloaded: PluginGraph = serde_json::from_value(encoded.clone()).unwrap();
        assert_eq!(encoded, serde_json::to_value(reloaded).unwrap());
    }

    #[test]
    fn unowned_failed_or_inconsistent_external_state_does_not_mutate_graph() {
        let (mut graph, data) = fixture();
        let before = serde_json::to_value(&graph).unwrap();
        let mut cases = vec![];
        let mut changed = data.clone();
        changed.plugin_instance_id = None;
        cases.push(changed);
        let mut changed = data.clone();
        changed.plugin_instance_id = Some(usize::MAX);
        cases.push(changed);
        let mut changed = data.clone();
        changed.plugin_instance_id = graph
            .nodes
            .values()
            .find(|node| !matches!(node.plugin.settings, PluginSettings::External { .. }))
            .map(|node| node.plugin.id);
        cases.push(changed);
        let mut changed = data.clone();
        changed.native_state = Err("capture failed".into());
        cases.push(changed);
        let mut changed = data.clone();
        changed.descriptor.path = "/other.clap".into();
        cases.push(changed);
        let mut changed = data.clone();
        changed.native_state.as_mut().unwrap().plugin_id = "other".into();
        cases.push(changed);
        for changed in cases {
            assert!(!graph.reconcile_external_editor_state(&changed));
            assert_eq!(before, serde_json::to_value(&graph).unwrap());
        }
    }
}
