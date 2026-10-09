use sotf_plugins::ChannelState;

pub(super) fn replace_matrix_channel_states(
    current: &mut Vec<ChannelState>,
    desired: &[(usize, ChannelState)],
    output_channels: usize,
) -> bool {
    // Meter groups can briefly reflect a previous source width while the
    // graph already routes a new output width. Never let that transient UI
    // view truncate the persisted M/S/D vector for the Matrix output bus.
    let mut next = current.clone();
    next.resize(output_channels, ChannelState::default());
    for (index, state) in desired {
        if let Some(channel) = next.get_mut(*index) {
            *channel = state.clone();
        }
    }
    if *current == next {
        return false;
    }
    *current = next;
    true
}
