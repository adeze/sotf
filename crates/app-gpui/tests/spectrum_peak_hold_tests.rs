//! Peak envelopes used by the desktop rack and dedicated spectrum views.
#[path = "../../../../gpui-toolkit/crates/gpui-audio-kit/src/spectrum/peak_hold.rs"]
mod peak_hold;

#[test]
fn peaks_hold_then_decay_at_the_same_rate_at_different_refresh_rates() {
    for step in [0.01, 0.1] {
        let mut peaks = peak_hold::SpectrumPeakHold::default();
        peaks.update(&[0.8], 0.0);
        let count = (2.0 / step) as usize;
        for index in 1..=count {
            peaks.update(&[0.2], index as f64 * step);
            if index as f64 * step <= 1.0 {
                assert_eq!(peaks.heights[0], 0.8);
            }
        }
        assert!((peaks.heights[0] - (0.8 - 20.0 / 103.0)).abs() < 0.00001);
        peaks.update(&[0.9], 2.1);
        assert_eq!(peaks.heights[0], 0.9);
    }
}

#[test]
fn hidden_or_reconfigured_spectrum_does_not_keep_old_peaks() {
    let mut peaks = peak_hold::SpectrumPeakHold::default();
    peaks.update(&[0.8], 0.0);
    peaks.update(&[0.2], 2.0);
    assert_eq!(peaks.heights, vec![0.2]);
    peaks.update(&[0.3, 0.4], 2.1);
    assert_eq!(peaks.heights, vec![0.3, 0.4]);
}
