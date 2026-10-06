//! Regression for a player snapshot that arrives one UI tick late.
#[path = "../ui/spectrum_poll.rs"]
mod spectrum_poll;

#[test]
fn delayed_snapshots_supply_every_throttled_spectrum_update() {
    let mut previous_snapshot_has_spectrum = false;
    let mut published_frames = 0;
    for frame in 0..16 {
        let (request, publish) = spectrum_poll::spectrum_poll_schedule(frame, true);
        if publish && frame > 0 {
            assert!(
                previous_snapshot_has_spectrum,
                "empty delayed snapshot at frame {frame}"
            );
            published_frames += 1;
        }
        previous_snapshot_has_spectrum = request;
    }
    assert_eq!(published_frames, 7);
}

#[test]
fn hidden_spectrum_is_not_requested_or_published() {
    for frame in 0..16 {
        assert_eq!(
            spectrum_poll::spectrum_poll_schedule(frame, false),
            (false, false)
        );
    }
}
