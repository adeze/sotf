//! Bounded display projections. Source responses and calibration inputs are unchanged.
use sotf_audio_player::ui_models::room_eq::RoomEqScreenModel;

pub const FREQUENCY_RANGE: (f64, f64) = (20.0, 20000.0);
const BINS: usize = 64;
const MAX_ROWS: usize = 128;

#[derive(Debug, Clone, PartialEq)]
pub struct ResponsePreview {
    pub label: String,
    /// Finite source samples selected by logarithmic-bin extrema; never smoothed.
    pub points: Vec<(f64, f64)>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MeasurementPreviews {
    pub rows: Vec<ResponsePreview>,
    pub omitted_rows: usize,
    /// One magnitude domain for every thumbnail and the selected response.
    pub magnitude_range_db: Option<(f64, f64)>,
}

impl MeasurementPreviews {
    pub fn from_model(model: &RoomEqScreenModel) -> Self {
        let mut result = Self::default();
        for channel in &model.channel_measurements {
            let takes = std::iter::once(("Primary".to_owned(), &channel.measurement))
                .chain(
                    channel
                        .multi_mic_measurements
                        .iter()
                        .enumerate()
                        .map(|(i, take)| (format!("Additional mic {}", i + 1), take)),
                )
                .chain(
                    channel
                        .group_drivers
                        .iter()
                        .enumerate()
                        .map(|(i, take)| (format!("Driver {}", i + 1), take)),
                );
            for (role, take) in takes {
                if result.rows.len() == MAX_ROWS {
                    result.omitted_rows += 1;
                    continue;
                }
                let row = ResponsePreview::new(
                    format!("{} · {role}", channel.channel_name),
                    &take.frequencies,
                    &take.magnitude_db,
                );
                for &(_, db) in &row.points {
                    result.magnitude_range_db = Some(match result.magnitude_range_db {
                        Some((low, high)) => (low.min(db), high.max(db)),
                        None => (db, db),
                    });
                }
                result.rows.push(row);
            }
        }
        result.magnitude_range_db = result.magnitude_range_db.map(|(low, high)| {
            (
                (low / 5.0).floor() * 5.0 - 5.0,
                (high / 5.0).ceil() * 5.0 + 5.0,
            )
        });
        result
    }
}

impl ResponsePreview {
    fn new(label: String, frequency: &[f32], magnitude: &[f32]) -> Self {
        let mut points = Vec::new();
        if frequency.len() != magnitude.len() {
            return Self { label, points };
        }
        let mut bins: [Option<((f64, f64), (f64, f64))>; BINS] = [None; BINS];
        for (&hz, &db) in frequency.iter().zip(magnitude) {
            let (hz, db) = (f64::from(hz), f64::from(db));
            if !hz.is_finite() || !db.is_finite() || !(20.0..=20000.0).contains(&hz) {
                continue;
            }
            let index =
                (((hz / FREQUENCY_RANGE.0).ln() / 1000.0_f64.ln()) * BINS as f64).floor() as usize;
            let bin = &mut bins[index.min(BINS - 1)];
            match bin {
                None => *bin = Some(((hz, db), (hz, db))),
                Some((min, max)) => {
                    if db < min.1 {
                        *min = (hz, db);
                    }
                    if db > max.1 {
                        *max = (hz, db);
                    }
                }
            }
        }
        for (min, max) in bins.into_iter().flatten() {
            points.push(min);
            if min != max {
                points.push(max);
            }
        }
        points.sort_by(|a, b| a.0.total_cmp(&b.0));
        points.dedup_by(|a, b| a.0 == b.0);
        Self { label, points }
    }
}

/// Native GPUI path rendering, with the same log-Hz/dB transform at both sizes.
#[cfg(feature = "desktop")]
pub fn plot(points: &[(f64, f64)], domain: (f64, f64), cx: &gpui::App) -> gpui::AnyElement {
    use gpui::prelude::*;
    use gpui::*;
    use gpui_ui_kit::ThemeExt;
    let theme = cx.theme();
    let points = points.to_vec();
    let grid_color = theme.border;
    let trace_color = theme.accent;
    canvas(
        move |bounds, _, _| {
            let width = f32::from(bounds.size.width);
            let height = f32::from(bounds.size.height);
            // Insets protect the stroke at the plot boundary, independent of font zoom.
            let position = |x: f32, y: f32| {
                point(
                    bounds.left() + px(1.0 + x * (width - 2.0).max(0.0)),
                    bounds.top() + px(1.0 + y * (height - 2.0).max(0.0)),
                )
            };
            let mut grid = PathBuilder::stroke(px(0.5));
            for x in [0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0] {
                grid.move_to(position(x, 0.0));
                grid.line_to(position(x, 1.0));
            }
            for y in [0.0, 0.5, 1.0] {
                grid.move_to(position(0.0, y));
                grid.line_to(position(1.0, y));
            }
            let mut trace = PathBuilder::stroke(px(1.5));
            for (index, &(hz, db)) in points.iter().enumerate() {
                let p = position(
                    ((hz / FREQUENCY_RANGE.0).ln() / 1000.0_f64.ln()) as f32,
                    (1.0 - (db - domain.0) / (domain.1 - domain.0)) as f32,
                );
                if index == 0 {
                    trace.move_to(p);
                } else {
                    trace.line_to(p);
                }
            }
            (grid.build().ok(), trace.build().ok())
        },
        move |_, (grid, trace), window, _| {
            if let Some(path) = grid {
                window.paint_path(path, grid_color);
            }
            if let Some(path) = trace {
                window.paint_path(path, trace_color);
            }
        },
    )
    .size_full()
    .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_bounds_samples_preserves_extrema_and_rejects_invalid_pairs() {
        let frequencies: Vec<_> = (0..10000)
            .map(|i| 20.0 * 1000.0_f32.powf(i as f32 / 9999.0))
            .collect();
        let mut values = vec![0.0; frequencies.len()];
        values[3000] = -31.0;
        values[5000] = 22.0;
        values[1] = f32::NAN;
        let row = ResponsePreview::new("test".into(), &frequencies, &values);
        assert!(row.points.len() <= BINS * 2);
        assert!(
            row.points
                .iter()
                .all(|(hz, db)| hz.is_finite() && db.is_finite())
        );
        assert!(row.points.windows(2).all(|p| p[0].0 < p[1].0));
        assert!(row.points.iter().any(|p| p.1 == -31.0));
        assert!(row.points.iter().any(|p| p.1 == 22.0));
        assert!(
            ResponsePreview::new("bad".into(), &[100.0], &[])
                .points
                .is_empty()
        );
        assert!(
            ResponsePreview::new("bad".into(), &[0.0, -1.0, f32::INFINITY], &[1.0; 3])
                .points
                .is_empty()
        );
        use sotf_audio_player::{
            recording_types::RecordingResult, room_eq_types::ChannelMeasurement,
        };
        let take = RecordingResult {
            sample_rate_hz: None,
            channel: 0,
            wav_path: None,
            csv_path: None,
            frequencies: vec![20.0, 20000.0],
            magnitude_db: vec![-31.0, 22.0],
            phase_deg: vec![],
            impulse_response: None,
            impulse_time_ms: None,
            thd_percent: None,
            harmonic_distortion_db: None,
            excess_group_delay_ms: None,
            rt60_ms: None,
            clarity_c50_db: None,
            clarity_c80_db: None,
            spectrogram_db: None,
            quality: None,
            stimulus_config: None,
        };
        let mut model = RoomEqScreenModel::default();
        model.channel_measurements.push(ChannelMeasurement {
            acquisition: None,
            driver_measurement_sets: vec![],
            provenance: vec![],
            channel_name: "L".into(),
            measurement: take.clone(),
            is_group: false,
            group_drivers: vec![],
            multi_mic_measurements: vec![take; MAX_ROWS],
        });
        let previews = MeasurementPreviews::from_model(&model);
        assert_eq!(previews.rows.len(), MAX_ROWS);
        assert_eq!(previews.omitted_rows, 1);
        assert_eq!(previews.magnitude_range_db, Some((-40.0, 30.0)));
        assert_eq!(
            model.channel_measurements[0].measurement.magnitude_db,
            [-31.0, 22.0]
        );
    }
}
