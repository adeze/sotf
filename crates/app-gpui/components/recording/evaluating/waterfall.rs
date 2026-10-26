// intentional-file: normalized scientific chart projection and axis geometry.
//! Minimal native preview. Rerun remains the primary scientific viewer.
use super::waterfall_data::{WaterfallData, prepare};
use crate::{components::design::Ds, theme::Theme, ui::PlayerView};
use gpui::{prelude::*, *};
use gpui_px::ColorScale;
use gpui_ui_kit::{Card, StackSpacing, Text, VStack};
use std::sync::Arc;

const DISPLAY_FLOOR_DB: f64 = -60.0;
const COLOR_BANDS: usize = 16;

// Fixed orthographic 3D projection: frequency right, later slices back/right,
// larger level up. All normalizations use fixed, labeled physical domains.
fn project(frequency: f64, delay: f64, level: f64) -> [f32; 2] {
    [
        (0.11 + 0.65 * frequency + 0.13 * delay) as f32,
        (0.82 - 0.13 * delay - 0.55 * level) as f32,
    ]
}

impl PlayerView {
    /// Source eligibility only; no STFT, renderer or cache population during inspection.
    pub(crate) fn recording_waterfall_availability(
        result: &crate::app::types::RecordingResult,
    ) -> Result<(), String> {
        super::waterfall_data::input_available(result)
    }
    pub(super) fn render_waterfall_plot(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.state.read(cx).app.ui_state.theme.clone();
        let d = Ds::from_cx(cx);
        let results = self.get_filtered_results(cx);
        let (title, body) = match results.first() {
            Some((name, _, result)) => match prepare(result) {
                Ok(data) => (format!("Waterfall · {name}"), preview(data, &theme, d)),
                Err(reason) => (
                    "Waterfall".to_owned(),
                    div()
                        .py(d.pad_y)
                        .child(Text::caption(reason).color(theme.text_secondary))
                        .into_any_element(),
                ),
            },
            None => (
                "Waterfall".to_owned(),
                div()
                    .py(d.pad_y)
                    .child(Text::caption(
                        "Select an impulse measurement to inspect its decay.",
                    ))
                    .into_any_element(),
            ),
        };
        Card::new().content(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(Text::section_header(title))
                .child(body),
        )
    }
}

fn preview(data: Arc<WaterfallData>, theme: &Theme, d: Ds) -> AnyElement {
    let max_hz = (f64::from(data.sample_rate_hz) / 2.0).min(20_000.0);
    let max_ms = data.grid.times_ms.last().copied().unwrap_or(1.0).max(1.0);
    let ridge_stride = (data.grid.times_ms.len() - 1).div_ceil(32).max(1);
    let displayed_ridges = (data.grid.times_ms.len() - 1).div_ceil(ridge_stride) + 1;
    let log_span = (max_hz / 20.0).log10();
    let bins: Vec<usize> = data
        .grid
        .freqs_hz
        .iter()
        .enumerate()
        .filter_map(|(i, hz)| (*hz >= 20.0 && *hz <= max_hz).then_some(i))
        .collect();
    let color_scale = ColorScale::Viridis;
    let fill = theme.surface;
    let border = theme.border;
    let mut labels: Vec<(String, [f32; 2])> = Vec::new();
    for (hz, label) in [
        (20.0, "20"),
        (100.0, "100"),
        (1000.0, "1k"),
        (10000.0, "10k"),
        (20000.0, "20k"),
    ] {
        if hz <= max_hz {
            let mut p = project((hz / 20.0).log10() / log_span, 0.0, 0.0);
            p[1] += 0.035;
            labels.push((label.to_owned(), p));
        }
    }
    for db in [-60.0, -30.0, 0.0] {
        let mut p = project(0.0, 0.0, (db - DISPLAY_FLOOR_DB) / -DISPLAY_FLOOR_DB);
        p[0] -= 0.07;
        labels.push((format!("{db:.0}"), p));
    }
    for fraction in [0.0, 0.5, 1.0] {
        let mut p = project(1.0, fraction, 0.0);
        p[0] += 0.025;
        labels.push((format!("{:.0} ms", fraction * max_ms), p));
    }
    let grid_data = Arc::clone(&data);
    let chart = div()
        .relative()
        .flex_1()
        .min_w_0()
        .h(px(360.0))
        .child(
            canvas(
                move |bounds, _, _| {
                    let mut paths = Vec::new();
                    let pixel = |p: [f32; 2]| {
                        point(
                            bounds.origin.x + bounds.size.width * p[0],
                            bounds.origin.y + bounds.size.height * p[1],
                        )
                    };
                    let mut axes = PathBuilder::stroke(px(1.0));
                    for (a, b) in [
                        ([0.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
                        ([1.0, 0.0, 0.0], [1.0, 1.0, 0.0]),
                        ([0.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
                        ([0.0, 1.0, 0.0], [1.0, 1.0, 0.0]),
                    ] {
                        axes.move_to(pixel(project(a[0], a[1], a[2])));
                        axes.line_to(pixel(project(b[0], b[1], b[2])));
                    }
                    if let Ok(path) = axes.build() {
                        paths.push((path, border));
                    }
                    let stride = (grid_data.grid.times_ms.len() - 1).div_ceil(32).max(1);
                    let mut rows: Vec<usize> =
                        (0..grid_data.grid.times_ms.len()).step_by(stride).collect();
                    let last = grid_data.grid.times_ms.len() - 1;
                    if rows.last() != Some(&last) {
                        rows.push(last);
                    }
                    for row_index in rows.into_iter().rev() {
                        let delay = grid_data.grid.times_ms[row_index] / max_ms;
                        let row = &grid_data.grid.mags_db[row_index];
                        let position = |i: usize| {
                            let frequency = (grid_data.grid.freqs_hz[i] / 20.0).log10() / log_span;
                            let level = ((f64::from(row[i]) - DISPLAY_FLOOR_DB)
                                / -DISPLAY_FLOOR_DB)
                                .clamp(0.0, 1.0);
                            pixel(project(frequency, delay, level))
                        };
                        // Opaque slice silhouettes occlude farther slices; colored
                        // ridge segments encode magnitude, never animation/time hue.
                        let mut silhouette = PathBuilder::fill();
                        if let Some(&first) = bins.first() {
                            let x = (grid_data.grid.freqs_hz[first] / 20.0).log10() / log_span;
                            silhouette.move_to(pixel(project(x, delay, 0.0)));
                            for &i in &bins {
                                silhouette.line_to(position(i));
                            }
                            if let Some(&last) = bins.last() {
                                let x = (grid_data.grid.freqs_hz[last] / 20.0).log10() / log_span;
                                silhouette.line_to(pixel(project(x, delay, 0.0)));
                            }
                            silhouette.close();
                            if let Ok(path) = silhouette.build() {
                                paths.push((path, fill));
                            }
                        }
                        let mut bands: [PathBuilder; COLOR_BANDS] =
                            std::array::from_fn(|_| PathBuilder::stroke(px(1.25)));
                        for pair in bins.windows(2) {
                            let value = f64::from(row[pair[0]].max(row[pair[1]]));
                            let band = (((value - DISPLAY_FLOOR_DB) / -DISPLAY_FLOOR_DB)
                                .clamp(0.0, 1.0)
                                * (COLOR_BANDS - 1) as f64)
                                .round() as usize;
                            bands[band].move_to(position(pair[0]));
                            bands[band].line_to(position(pair[1]));
                        }
                        for (i, band) in bands.into_iter().enumerate() {
                            if let Ok(path) = band.build() {
                                paths.push((
                                    path,
                                    color_scale
                                        .map(i as f64 / (COLOR_BANDS - 1) as f64)
                                        .to_rgba(),
                                ));
                            }
                        }
                    }
                    paths
                },
                move |_, paths, window, _| {
                    for (path, color) in paths {
                        window.paint_path(path, color);
                    }
                },
            )
            .size_full(),
        )
        .children(labels.into_iter().map(|(label, p)| {
            div()
                .absolute()
                .left(relative(p[0]))
                .top(relative(p[1]))
                .text_size(d.text_xs)
                .text_color(theme.text_secondary)
                .child(label)
        }))
        .child(
            div()
                .absolute()
                .left(relative(0.30))
                .top(relative(0.93))
                .child(Text::caption("Frequency (Hz, log)")),
        )
        .child(
            div()
                .absolute()
                .left(relative(0.02))
                .top(relative(0.08))
                .child(Text::caption("Level (dB)")),
        );
    let bar = div()
        .flex()
        .flex_col()
        .gap(d.grid)
        .w(px(75.0))
        .child(Text::caption("dB rel. peak"))
        .child(
            div()
                .flex()
                .gap(d.grid)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .children((0..COLOR_BANDS).rev().map(|i| {
                            div().w(px(16.0)).h(px(13.0)).bg(ColorScale::Viridis
                                .map(i as f64 / (COLOR_BANDS - 1) as f64)
                                .to_rgba())
                        })),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .justify_between()
                        .h(px(208.0))
                        .child(Text::caption("0"))
                        .child(Text::caption("−30"))
                        .child(Text::caption("−60")),
                ),
        );
    div().flex().flex_col().gap(d.gap)
        .child(Text::caption("Measured impulse decay · frequency × delay × relative level"))
        .child(div().flex().w_full().min_w_0().gap(d.gap).child(chart).child(bar))
        .child(Text::caption(format!("32 ms Hann · 2 ms hop · {:.1} Hz FFT bin spacing · {:.0}–{:.0} ms available post-peak",
            data.bin_spacing_hz,data.grid.times_ms[0],max_ms)))
        .child(Text::caption("Short windows show timing detail; FFT bin spacing is not independent resolving power for fine bass modes."))
        .child(Text::caption(format!("Fixed 3D preview · {displayed_ridges}/{} slices shown · display floor −60 dB · largest IR sample {} (not physical arrival)",data.grid.times_ms.len(),data.peak_sample)))
        .into_any_element()
}
