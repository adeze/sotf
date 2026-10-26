//! Presentation adapter over canonical IR STFT analysis, reusable by a Rerun exporter.
use math_audio_dsp::rir_waterfall::{
    WATERFALL_HOP_MS, WATERFALL_WINDOW_MS, WaterfallConfig, WaterfallGrid, waterfall_grid_at,
};
use sotf_audio_player::recording_types::RecordingResult;
use std::{
    cell::RefCell,
    collections::{VecDeque, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    sync::Arc,
};

pub(super) struct WaterfallData {
    pub grid: WaterfallGrid,
    pub sample_rate_hz: u32,
    /// Display reference only, not a measured direct-arrival estimate.
    pub peak_sample: usize,
    pub bin_spacing_hz: f64,
}

thread_local! {
    // Only derived arrays are cached. Source impulses remain in the recording.
    static CACHE: RefCell<VecDeque<(u64,Arc<WaterfallData>)>> = const { RefCell::new(VecDeque::new()) };
}

struct Source<'a> {
    sample_rate_hz: u32,
    times: &'a [f32],
    ir: &'a [f32],
    peak_sample: usize,
    rate: f64,
    window_samples: usize,
    supported_tail_ms: f64,
}

fn source(result: &RecordingResult) -> Result<Source<'_>, String> {
    let sample_rate_hz = result
        .sample_rate_hz
        .filter(|rate| (8_000..=192_000).contains(rate))
        .ok_or("Waterfall needs an explicit 8–192 kHz impulse sample rate.")?;
    let (times,ir) = result.measured_impulse()
        .ok_or("Waterfall needs a saved finite impulse and its original time axis; response or spectrogram alone cannot establish decay.")?;
    let step_ms = 1000.0 / f64::from(sample_rate_hz);
    let origin = f64::from(times[0]);
    if times.iter().enumerate().any(|(i, time)| {
        let expected = origin + i as f64 * step_ms;
        (f64::from(*time) - expected).abs()
            > (step_ms * 0.05).max(expected.abs() * f64::from(f32::EPSILON) * 4.0)
    }) {
        return Err("Impulse time spacing disagrees with its saved sample rate.".into());
    }
    let peak_sample = ir
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
        .map(|(i, _)| i)
        .ok_or("Empty impulse.")?;
    if ir[peak_sample] == 0.0 {
        return Err("Silent impulse has no decay reference.".into());
    }
    let rate = f64::from(sample_rate_hz);
    let window_samples = ((WATERFALL_WINDOW_MS * rate / 1000.0).round() as usize).max(16);
    let supported_tail_ms =
        (ir.len() - 1 - peak_sample) as f64 * step_ms - WATERFALL_WINDOW_MS / 2.0;
    if supported_tail_ms < WATERFALL_HOP_MS {
        return Err(
            "Impulse ends before a complete post-peak analysis window is available.".into(),
        );
    }
    Ok(Source {
        sample_rate_hz,
        times,
        ir,
        peak_sample,
        rate,
        window_samples,
        supported_tail_ms,
    })
}

pub(super) fn input_available(result: &RecordingResult) -> Result<(), String> {
    source(result).map(|_| ())
}

pub(super) fn prepare(result: &RecordingResult) -> Result<Arc<WaterfallData>, String> {
    let Source {
        sample_rate_hz,
        times,
        ir,
        peak_sample,
        rate,
        window_samples,
        supported_tail_ms,
    } = source(result)?;
    let fft_length = window_samples.next_power_of_two();
    let mut hasher = DefaultHasher::new();
    sample_rate_hz.hash(&mut hasher);
    // Version/config and actual sample/time bits, never pointer or path identity.
    1_u32.hash(&mut hasher);
    for value in ir.iter().chain(times) {
        value.to_bits().hash(&mut hasher);
    }
    let key = hasher.finish();
    if let Some(data) = CACHE.with(|cache| {
        cache
            .borrow()
            .iter()
            .find(|(id, _)| *id == key)
            .map(|(_, data)| Arc::clone(data))
    }) {
        return Ok(data);
    }
    // Keep genuine neighbouring samples; bound the input copied into the analysis
    // window without padding an absent recording tail into a fictitious decay.
    let start = peak_sample.saturating_sub(window_samples);
    let end = (peak_sample + window_samples + (rate * 0.5).ceil() as usize).min(ir.len());
    let config = WaterfallConfig {
        max_frames: 256,
        max_bins: fft_length / 2 + 1,
        ..Default::default()
    };
    let mut grid = waterfall_grid_at(&ir[start..end], rate, peak_sample - start, &config);
    let keep: Vec<usize> = grid
        .times_ms
        .iter()
        .enumerate()
        .filter_map(|(i, time)| {
            (*time >= 0.0 && *time <= supported_tail_ms.min(500.0)).then_some(i)
        })
        .collect();
    grid.times_ms = keep.iter().map(|&i| grid.times_ms[i]).collect();
    grid.mags_db = keep.iter().map(|&i| grid.mags_db[i].clone()).collect();
    if grid.times_ms.len() < 2
        || grid.freqs_hz.len() < 2
        || grid.times_ms.windows(2).any(|p| p[0] >= p[1])
        || grid
            .mags_db
            .iter()
            .any(|row| row.len() != grid.freqs_hz.len() || row.iter().any(|v| !v.is_finite()))
    {
        return Err("Impulse did not produce a valid supported decay grid.".into());
    }
    let data = Arc::new(WaterfallData {
        grid,
        sample_rate_hz,
        peak_sample,
        bin_spacing_hz: rate / fft_length as f64,
    });
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() == 2 {
            cache.pop_front();
        }
        cache.push_back((key, Arc::clone(&data)));
    });
    Ok(data)
}
