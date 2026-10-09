#[path = "../components/plugins/level_meters/matrix_channel_states.rs"]
mod matrix_channel_states;

use matrix_channel_states::replace_matrix_channel_states;
use sotf_plugins::ChannelState;

#[test]
fn transient_meter_width_preserves_matrix_output_flags() {
    let mut states = vec![ChannelState {
        muted: true,
        soloed: false,
        dimmed: false,
    }];
    assert!(replace_matrix_channel_states(
        &mut states,
        &[(0, ChannelState::default())],
        2,
    ));
    assert_eq!(states, vec![ChannelState::default(); 2]);

    states[1].soloed = true;
    assert!(!replace_matrix_channel_states(
        &mut states,
        &[(0, ChannelState::default())],
        2,
    ));
    assert!(states[1].soloed);

    states.push(ChannelState {
        muted: false,
        soloed: false,
        dimmed: true,
    });
    assert!(replace_matrix_channel_states(
        &mut states,
        &[(0, ChannelState::default())],
        2,
    ));
    assert_eq!(states.len(), 2);
    assert!(states[1].soloed);

    assert!(replace_matrix_channel_states(
        &mut states,
        &[(1, ChannelState::default())],
        2,
    ));
    assert!(!states[1].soloed);
    states[0].muted = true;
    assert!(!replace_matrix_channel_states(
        &mut states,
        &[(1, ChannelState::default())],
        2,
    ));
    assert!(states[0].muted, "an unobserved channel keeps its flag");
}
