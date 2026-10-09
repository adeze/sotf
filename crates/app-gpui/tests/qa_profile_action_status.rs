#![cfg(feature = "dev-api")]

use sotf_audio_player_gpui::app::player_handle::{PlayerHandle, profile_request_matches};
use sotf_plugins::plugin_hiss_reducer::snapshot::ProfileSnapshot;
use std::any::Any;
use std::sync::Arc;

#[test]
fn profile_status_uses_the_published_snapshot_and_rejects_stale_transport() {
    let profile: Arc<dyn Any + Send + Sync> = Arc::new(ProfileSnapshot::new(2));
    let current = PlayerHandle::qa_profile_action_status_from_data(3, 3, 42, 1, &profile);
    assert_eq!(current["kind"], "hiss_reducer");
    assert_eq!(current["engine_index"], 1);
    assert_eq!(current["snapshot_sequence"], 42);
    assert_eq!(current["transport_epoch"], 3);
    assert_eq!(current["capturing"], false);
    assert_eq!(current["profile_present"], false);

    // A source transition increments the requested epoch before a new rack
    // snapshot is published. The old profile must not satisfy a new gesture.
    assert!(PlayerHandle::qa_profile_action_status_from_data(4, 3, 42, 1, &profile).is_null());
}

#[test]
fn profile_reconciliation_requires_fresh_matching_instance_and_idle_commands() {
    let current = Some((5, 8, 3));
    // A pre-edit snapshot, replaced plugin, or pre-rebuild generation must
    // never overwrite the current saved Use Profile setting.
    assert!(!profile_request_matches(Some((5, 7, 3)), current, true));
    assert!(!profile_request_matches(Some((6, 8, 3)), current, true));
    assert!(!profile_request_matches(Some((5, 8, 2)), current, true));
    assert!(!profile_request_matches(current, current, false));
    // Once the actor publishes the post-command request, a saved-state
    // reconciliation may run even after switching away and back.
    assert!(profile_request_matches(current, current, true));
}
