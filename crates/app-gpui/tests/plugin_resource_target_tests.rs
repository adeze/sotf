//! Resource results stay bound to their original instance and editor generation.
use sotf_audio_player::{PluginGraph, PluginType};
use sotf_audio_player_gpui::app::state::PluginState;

fn state() -> PluginState {
    let mut state = PluginState::default();
    state.graph = PluginGraph::with_default_rack();
    state
        .graph
        .add_user_plugin(&PluginType::Convolution)
        .unwrap();
    state
}

#[test]
fn rack_result_rejects_removed_replaced_or_moved_target() {
    let mut state = state();
    let target = state.resource_target(2).unwrap();
    assert!(target.matches(&state));
    let original = state.graph.get_plugin(2).unwrap().id;
    state.graph.remove_plugin_by_index(2).unwrap();
    state
        .graph
        .insert_plugin(2, &PluginType::Convolution)
        .unwrap();
    // Even reused preset instance IDs must not hide a replacement node UUID.
    state.graph.get_plugin_mut(2).unwrap().id = original;
    assert!(!target.matches(&state));
    let target = state.resource_target(2).unwrap();
    state.graph.insert_plugin(2, &PluginType::Gain).unwrap();
    assert!(!target.matches(&state));
}

#[test]
fn graph_result_rejects_new_draft_or_changed_editor() {
    let mut state = state();
    let rack_target = state.resource_target(2).unwrap();
    let instance_id = state.graph.get_plugin(2).unwrap().id;
    let id = state
        .graph
        .nodes
        .values()
        .find(|n| n.plugin.id == instance_id)
        .unwrap()
        .id;
    state.begin_routing_draft();
    state.graph_state.editing_graph_node_uuid = Some(id);
    assert!(!rack_target.matches(&state));
    let target = state.resource_target(2).unwrap();
    assert!(target.matches(&state));
    state.graph_state.draft_generation += 1;
    assert!(!target.matches(&state));
    let target = state.resource_target(2).unwrap();
    state.graph_state.editing_graph_node_uuid = None;
    assert!(!target.matches(&state));
}

#[test]
fn draft_only_target_never_falls_back_to_playback_node() {
    let mut state = state();
    state.begin_routing_draft();
    let id = state
        .routing_controller_mut()
        .graph
        .add_user_plugin(&PluginType::Convolution)
        .unwrap();
    state.graph_state.editing_graph_node_uuid = Some(id);
    let target = state.resource_target(2).unwrap();
    assert!(target.matches(&state));
    assert!(!state.graph.nodes.contains_key(&id));
    state.routing_controller_mut().graph.remove_node(id);
    assert!(!target.matches(&state));
    assert!(state.resource_target(2).is_none());
}

#[test]
fn ab_resource_edit_stays_in_draft_without_publishing_to_player() {
    let mut state = state();
    state.graph = PluginGraph::with_default_rack();
    let id = state.graph.add_user_plugin(&PluginType::ABCompare).unwrap();
    state.update_state = Default::default();
    let active = serde_json::to_value(&state.graph).unwrap();
    state.begin_routing_draft();
    state.graph_state.editing_graph_node_uuid = Some(id);
    let target = state.resource_target(2).unwrap();
    assert!(target.matches(&state));
    let param = sotf_plugins::param_specs::index_of(
        sotf_plugins::param_specs::ab_compare::PARAMS,
        "path_a_config",
    );
    let effect = state
        .routing_controller_mut()
        .set_ab_compare_config_file_by_node_id(
            id,
            param,
            r#"{"type":"Rack","plugins":[{"plugin_type":"gain","parameters":{}}]}"#.into(),
            "ab-path-a.json".into(),
        );
    assert!(matches!(
        effect,
        sotf_audio_player::PluginUpdateEffect::Structural
    ));
    state.record_editor_effect(effect);
    assert_eq!(serde_json::to_value(&state.graph).unwrap(), active);
    assert_ne!(
        serde_json::to_value(&state.routing_controller().graph).unwrap(),
        active
    );
    assert!(state.update_state.pending_plugin_update.is_none());
    assert!(target.matches(&state));
}
