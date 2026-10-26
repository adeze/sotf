//! Synthetic known IRs validate the adapter; these are not room measurements.
#[path = "../components/recording/evaluating/waterfall_data.rs"]
mod waterfall_data;
use sotf_audio_player::recording_types::RecordingResult;
use std::sync::Arc;

fn fixture(samples: usize, peak: usize) -> RecordingResult {
    let mut impulse = vec![0.0_f32; samples];
    impulse[peak] = 1.0;
    for (i, value) in impulse.iter_mut().enumerate().skip(peak + 1) {
        let t = (i - peak) as f32 / 8000.0;
        *value = 0.2 * (-t / 0.1).exp() * (std::f32::consts::TAU * 125.0 * t).sin();
    }
    serde_json::from_value(serde_json::json!({"channel":0,"sample_rate_hz":8000,
        "wav_path":null,"csv_path":null,"frequencies":[100.0,1000.0],
        "magnitude_db":[0.0,0.0],"phase_deg":[0.0,0.0],
        "impulse_response":impulse,"impulse_time_ms":(0..samples).map(|i|i as f32*0.125).collect::<Vec<_>>(),
        "thd_percent":null,"harmonic_distortion_db":null,"excess_group_delay_ms":null,
        "rt60_ms":null,"clarity_c50_db":null,"clarity_c80_db":null,"spectrogram_db":null})).unwrap()
}

#[test]
fn canonical_waterfall_keeps_real_axes_cached_data_and_supported_tail() {
    let recording = fixture(8000, 200);
    let original = recording.impulse_response.clone();
    assert!(waterfall_data::input_available(&recording).is_ok());
    let data = waterfall_data::prepare(&recording).unwrap();
    assert_eq!(data.peak_sample, 200);
    assert_eq!(data.sample_rate_hz, 8000);
    assert_eq!(data.bin_spacing_hz, 31.25);
    assert!(data.grid.times_ms.len() > 20);
    assert!(data.grid.times_ms.iter().all(|t| *t >= 0.0 && *t <= 500.0));
    assert!(data.grid.freqs_hz.windows(2).all(|p| p[0] < p[1]));
    assert!(
        data.grid
            .mags_db
            .iter()
            .flatten()
            .all(|v| v.is_finite() && *v <= 0.0)
    );
    let bin = data.grid.freqs_hz.iter().position(|f| *f == 125.0).unwrap();
    let nearest_frame = |time_ms: f64| {
        data.grid
            .times_ms
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| (**a - time_ms).abs().total_cmp(&(**b - time_ms).abs()))
            .unwrap()
            .0
    };
    let (early, late) = (nearest_frame(100.0), nearest_frame(300.0));
    let expected_drop_db = -20.0 * (data.grid.times_ms[late] - data.grid.times_ms[early])
        / (1000.0 * 0.1 * std::f64::consts::LN_10);
    let observed_drop_db = f64::from(data.grid.mags_db[late][bin] - data.grid.mags_db[early][bin]);
    assert!((observed_drop_db - expected_drop_db).abs() < 0.1);
    assert!(Arc::ptr_eq(
        &data,
        &waterfall_data::prepare(&recording).unwrap()
    ));
    assert_eq!(original, recording.impulse_response);
    let mut changed = recording.clone();
    changed.impulse_response.as_mut().unwrap()[7000] *= 0.5;
    assert!(!Arc::ptr_eq(
        &data,
        &waterfall_data::prepare(&changed).unwrap()
    ));
    let short = fixture(800, 80);
    let short_data = waterfall_data::prepare(&short).unwrap();
    let valid_tail_ms = (800.0 - 1.0 - 80.0) / 8.0 - 16.0;
    assert!(
        short_data
            .grid
            .times_ms
            .last()
            .is_some_and(|t| *t <= valid_tail_ms)
    );
}

#[test]
fn response_only_invalid_rate_time_axis_silence_and_truncated_ir_are_unavailable() {
    let original = fixture(8000, 200);
    let mut response_only = original.clone();
    response_only.impulse_response = None;
    response_only.impulse_time_ms = None;
    assert!(response_only.can_reconstruct_impulse());
    assert!(waterfall_data::prepare(&response_only).is_err());
    let mut missing_rate = original.clone();
    missing_rate.sample_rate_hz = None;
    assert!(waterfall_data::prepare(&missing_rate).is_err());
    let mut wrong_axis = original.clone();
    wrong_axis.sample_rate_hz = Some(48000);
    assert!(waterfall_data::prepare(&wrong_axis).is_err());
    let mut invalid = original.clone();
    invalid.impulse_response.as_mut().unwrap()[0] = f32::NAN;
    assert!(waterfall_data::prepare(&invalid).is_err());
    let mut silence = original.clone();
    silence.impulse_response.as_mut().unwrap().fill(0.0);
    assert!(waterfall_data::prepare(&silence).is_err());
    assert!(waterfall_data::prepare(&fixture(400, 390)).is_err());
    let mut invalid_axes = [original.clone(), original.clone(), original.clone()];
    invalid_axes[0].impulse_time_ms.as_mut().unwrap().pop();
    invalid_axes[1].impulse_time_ms.as_mut().unwrap()[0] = f32::NAN;
    invalid_axes[2].impulse_time_ms.as_mut().unwrap()[10] = 0.0;
    for invalid in invalid_axes {
        assert!(waterfall_data::input_available(&invalid).is_err());
        assert!(waterfall_data::prepare(&invalid).is_err());
    }
    let mut invalid_rate = original.clone();
    invalid_rate.sample_rate_hz = Some(7999);
    assert!(waterfall_data::input_available(&invalid_rate).is_err());
    assert!(waterfall_data::prepare(&invalid_rate).is_err());
}
