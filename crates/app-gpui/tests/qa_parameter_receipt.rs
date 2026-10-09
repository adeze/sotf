#![cfg(feature = "dev-api")]

use sotf_audio_player::Player;
use sotf_audio_player::PluginUpdateEffect;
use sotf_audio_player::PluginType;
use sotf_audio_player_gpui::app::player_handle::{PlayerCommandReceipt, PlayerHandle};
use sotf_audio_player_gpui::app::state::plugin::{
    DevParameterOutcome, DevParameterReceipt, PluginState,
};
use sotf_audio_player_gpui::app::types::PluginUpdateType;
use std::time::{Duration, Instant};

fn await_receipt(receipt: &PlayerCommandReceipt) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(result) = receipt.poll() {
            return result.map_err(|failure| failure.error);
        }
        assert!(Instant::now() < deadline, "player command did not complete");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn offline_parameter_and_structural_receipts_are_deferred_without_user_error() {
    let handle = PlayerHandle::new(Player::new());
    let parameter = handle
        .set_plugin_parameter_with_receipt(0, "gain".into(), "-3".into())
        .unwrap();
    let structural = handle.update_plugins_qa_receipt(Vec::new()).unwrap();
    assert_eq!(await_receipt(&parameter), Err("No engine running".into()));
    assert_eq!(await_receipt(&structural), Err("No engine running".into()));
    assert!(handle.drain_failures().is_empty());
}

#[test]
fn offline_momentary_action_reports_failure_instead_of_silent_deferral() {
    let handle = PlayerHandle::new(Player::new());
    handle
        .set_plugin_action(0, "learn_noise".into(), "true".into())
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(failure) = handle.drain_failures().into_iter().next() {
            assert_eq!(failure.label, "set_plugin_action");
            assert_eq!(failure.error, "No engine running");
            break;
        }
        assert!(Instant::now() < deadline, "action failure was swallowed");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn offline_action_receipt_also_routes_failure_to_user_feedback() {
    let handle = PlayerHandle::new(Player::new());
    let receipt = handle
        .set_plugin_action_with_receipt(0, "learn_noise".into(), "true".into())
        .unwrap();
    assert_eq!(await_receipt(&receipt), Err("No engine running".into()));
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let failures = handle.drain_failures();
        if let Some(failure) = failures.first() {
            assert_eq!(failures.len(), 1);
            assert_eq!(failure.label, "set_plugin_action");
            assert_eq!(failure.error, "No engine running");
            break;
        }
        assert!(Instant::now() < deadline, "action failure feedback was lost");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn direct_use_guards_are_scoped_to_stable_denoiser_instances() {
    let mut state = PluginState::default();
    let first = state.graph.add_user_plugin(&PluginType::Denoiser).unwrap();
    let second = state.graph.add_user_plugin(&PluginType::Denoiser).unwrap();
    for node_id in [first, second] {
        state
            .graph
            .nodes
            .get_mut(&node_id)
            .unwrap()
            .plugin
            .settings
            .set_param_value(21, 1.0);
        state.record_editor_effect(PluginUpdateEffect::ParameterByNodeId {
            node_id,
            param_index: 21,
        });
    }
    let first_id = state.graph.nodes[&first].plugin.id;
    let second_id = state.graph.nodes[&second].plugin.id;
    assert_eq!(state.update_state.denoiser_direct_use_guard.get(&first_id), Some(&true));
    assert_eq!(state.update_state.denoiser_direct_use_guard.get(&second_id), Some(&true));

    state.record_editor_effect(PluginUpdateEffect::Action {
        plugin_index: 0,
        node_id: Some(second),
        param_index: 22,
        key: "clear_profile",
        value: true,
    });
    assert_eq!(state.update_state.denoiser_direct_use_guard.get(&first_id), Some(&true));
    assert!(!state.update_state.denoiser_direct_use_guard.contains_key(&second_id));
}

#[test]
fn disconnected_worker_rejects_a_parameter_without_returning_a_pending_receipt() {
    let mut handle = PlayerHandle::new(Player::new());
    handle.qa_disconnect_worker_for_test();
    assert!(
        handle
            .set_plugin_parameter_with_receipt(0, "gain".into(), "-3".into())
            .is_err()
    );
    assert!(handle.update_plugins_qa_receipt(Vec::new()).is_err());
}

#[test]
fn no_change_edit_invalidates_an_earlier_parameter_receipt() {
    let mut state = PluginState::default();
    state.update_state.parameter_request_id = 7;
    state.update_state.last_parameter_receipt = Some(DevParameterReceipt {
        request_id: 7,
        transport_epoch: 0,
        instance_id: 5,
        engine_generation: 0,
        accepted_generation: None,
        engine_index: 1,
        param_index: 0,
        key: "gain_db".into(),
        requested_value: "-6".into(),
        outcome: DevParameterOutcome::Deferred("fixture"),
    });
    state.record_editor_effect(PluginUpdateEffect::None);
    assert_eq!(state.update_state.parameter_request_id, 8);
    assert!(state.update_state.last_parameter_receipt.is_none());
    assert_eq!(
        state.update_state.last_edit_rejection,
        Some("edit produced no settings change")
    );
}

#[test]
fn routing_draft_edit_invalidates_live_receipt_without_submitting_engine_work() {
    let mut state = PluginState::default();
    let node_id = *state
        .graph
        .nodes
        .keys()
        .next()
        .expect("default graph has nodes");
    state.begin_routing_draft();
    state.graph_state.editing_graph_node_uuid = Some(node_id);
    state.update_state.parameter_request_id = 3;
    state.update_state.last_parameter_receipt = Some(DevParameterReceipt {
        request_id: 3,
        transport_epoch: 0,
        instance_id: 5,
        engine_generation: 0,
        accepted_generation: None,
        engine_index: 1,
        param_index: 0,
        key: "gain_db".into(),
        requested_value: "-6".into(),
        outcome: DevParameterOutcome::Deferred("fixture"),
    });
    state.record_editor_effect(PluginUpdateEffect::Structural);
    assert_eq!(state.update_state.parameter_request_id, 4);
    assert!(state.update_state.last_parameter_receipt.is_none());
    assert_eq!(
        state.update_state.last_edit_rejection,
        Some("routing draft has not been applied")
    );
    assert!(state.update_state.pending_plugin_update.is_none());
}

#[test]
fn momentary_start_and_cancel_survive_a_pending_structural_update_in_order() {
    let mut state = PluginState::default();
    let node_id = *state.graph.nodes.keys().next().unwrap();
    state.update_state.pending_plugin_update = Some(PluginUpdateType::Structural);
    for value in [true, false] {
        state.record_editor_effect(PluginUpdateEffect::Action {
            plugin_index: 0,
            node_id: Some(node_id),
            param_index: 5,
            key: "learn_noise",
            value,
        });
    }
    assert!(matches!(state.update_state.pending_plugin_update, Some(PluginUpdateType::Structural)));
    let first = state.update_state.pending_actions.pop_front().unwrap();
    let second = state.update_state.pending_actions.pop_front().unwrap();
    assert!(matches!(first, PluginUpdateType::Action { value: true, request_id: 1, node_id: Some(id), .. } if id == node_id));
    assert!(matches!(second, PluginUpdateType::Action { value: false, request_id: 2, node_id: Some(id), .. } if id == node_id));
    assert!(state.update_state.pending_actions.is_empty());
}

#[test]
fn failed_rebuild_cancels_actions_before_a_newer_graph_can_retry() {
    let mut state = PluginState::default();
    let node_id = *state.graph.nodes.keys().next().unwrap();
    for value in [true, false] {
        state.record_editor_effect(PluginUpdateEffect::Action {
            plugin_index: 0,
            node_id: Some(node_id),
            param_index: 5,
            key: "learn_noise",
            value,
        });
    }
    let rejected = state.update_state.settle_action_topology(false, false);
    assert!(matches!(rejected, Some(PluginUpdateType::Action { value: false, request_id: 2, .. })));
    assert!(state.update_state.pending_actions.is_empty());
    assert!(state.update_state.action_topology_unavailable);
    state.update_state.settle_action_topology(true, false);
    assert!(!state.update_state.action_topology_unavailable);
    state.update_state.settle_action_topology(false, true);
    assert!(!state.update_state.action_topology_unavailable);
}
