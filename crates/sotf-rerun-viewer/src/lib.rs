//! Read-only presentation; numerical results remain owned by the solver.
use std::{
    collections::BTreeSet,
    fs::OpenOptions,
    net::TcpListener,
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};

use rerun::{RecordingStream, RecordingStreamBuilder};

mod layout;
pub mod room;

pub struct SolverResponse<'a> {
    pub frequency_hz: &'a [f64],
    pub prepared_measurement_db: &'a [f64],
    pub target_db: &'a [f64],
    pub filter_db: &'a [f64],
    pub predicted_db: &'a [f64],
}

pub struct ViewerSession {
    // Child views must be dropped by the host before this owned origin stops.
    _web_server: re_web_viewer_server::WebViewerServer,
    viewer_url: String,
    recording_path: PathBuf,
    file: RecordingStream,
    live: RecordingStream,
    callback_index: Mutex<u64>,
    measurement_paths: Mutex<BTreeSet<String>>,
    measurement_bounds: Mutex<Option<[f64; 4]>>,
}

impl ViewerSession {
    pub fn new(output_dir: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(output_dir).map_err(|e| e.to_string())?;
        let id = uuid::Uuid::new_v4();
        let recording_path = output_dir.join(format!("scientific-view-{id}.rrd"));
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&recording_path)
            .map_err(|e| format!("Cannot reserve recording: {e}"))?;
        let store = rerun::StoreId::random(rerun::StoreKind::Recording, "sotf_scientific_viewer");
        let builder = || {
            RecordingStreamBuilder::from_store_id(&store)
                .enabled(true)
                .with_default_blueprint(crate::layout::blueprint())
        };
        let file = builder().save(&recording_path).map_err(|e| e.to_string())?;
        // The SDK accepts an address rather than a prebound listener. Limit this
        // local port handoff to loopback and verify startup before exposing it.
        let reservation = TcpListener::bind(("127.0.0.1", 0)).map_err(|e| e.to_string())?;
        let port = reservation.local_addr().map_err(|e| e.to_string())?.port();
        drop(reservation);
        let live = builder()
            .serve_grpc_opts(
                "127.0.0.1",
                port,
                rerun::ServerOptions {
                    memory_limit: rerun::MemoryLimit::from_bytes(128 * 1024 * 1024),
                    ..Default::default()
                },
            )
            .map_err(|e| e.to_string())?;
        let address = std::net::SocketAddr::from(([127, 0, 0, 1], port));
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            if std::net::TcpStream::connect_timeout(&address, Duration::from_millis(50)).is_ok() {
                break;
            }
            if std::time::Instant::now() >= deadline {
                return Err("Rerun loopback server did not start".into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let web_server = rerun::web_viewer::WebViewerConfig {
            bind_ip: "127.0.0.1".into(),
            open_browser: false,
            ..Default::default()
        }
        .host_web_viewer()
        .map_err(|e| e.to_string())?;
        let viewer_url = format!(
            "{}?url=rerun%2Bhttp%3A%2F%2F127.0.0.1%3A{port}%2Fproxy",
            web_server.bound_url()
        );
        let result = Self {
            _web_server: web_server,
            viewer_url,
            recording_path,
            file,
            live,
            callback_index: Mutex::new(0),
            measurement_paths: Mutex::new(BTreeSet::new()),
            measurement_bounds: Mutex::new(None),
        };
        for stream in result.streams() {
            stream.set_log_time_enabled(false);
            stream.log_static("presentation", &rerun::TextDocument::new(
                "Read-only scientific view. Measurements, predictions and geometry diagnostics remain distinct. Live history is bounded; the local recording retains flushed callbacks."
            )).map_err(|e| e.to_string())?;
        }
        result.flush()?;
        Ok(result)
    }

    pub fn viewer_url(&self) -> &str {
        &self.viewer_url
    }
    pub fn recording_path(&self) -> &Path {
        &self.recording_path
    }

    /// Adds read-only canonical project metadata without replacing scientific entities.
    pub fn log_project_entity(
        &self,
        path: &str,
        metadata: &serde_json::Value,
    ) -> Result<(), String> {
        let text = project_entity_text(path, metadata)?;
        for stream in self.streams() {
            stream
                .log_static(path, &rerun::TextDocument::new(text.clone()))
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    fn streams(&self) -> [&RecordingStream; 2] {
        [&self.file, &self.live]
    }

    pub fn clear_inputs(&self) -> Result<(), String> {
        let mut paths = self
            .measurement_paths
            .lock()
            .map_err(|_| "Viewer input lock poisoned")?;
        // Empty each static component explicitly: temporal clears do not define
        // replacement semantics for static data. Keep inventory until both sinks succeed.
        for stream in self.streams() {
            for path in paths.iter() {
                stream
                    .log_static(path.as_str(), &rerun::LineStrips2D::clear_fields())
                    .map_err(|e| e.to_string())?;
                for child in ["frequency_hz", "magnitude_db"] {
                    stream
                        .log_static(format!("{path}/{child}"), &rerun::Tensor::clear_fields())
                        .map_err(|e| e.to_string())?;
                }
                stream
                    .log_static(format!("{path}/axes"), &rerun::TextDocument::clear_fields())
                    .map_err(|e| e.to_string())?;
            }
            stream
                .log_static(
                    "measurements/chart_axes/lines",
                    &rerun::LineStrips2D::clear_fields(),
                )
                .map_err(|e| e.to_string())?;
            stream
                .log_static(
                    "measurements/chart_axes/labels",
                    &rerun::Points2D::clear_fields(),
                )
                .map_err(|e| e.to_string())?;
            stream
                .log_static("room/geometry", &rerun::Mesh3D::clear_fields())
                .map_err(|e| e.to_string())?;
            stream
                .log_static("room/source", &rerun::TextDocument::clear_fields())
                .map_err(|e| e.to_string())?;
            stream
                .log_static("room", &rerun::ViewCoordinates::clear_fields())
                .map_err(|e| e.to_string())?;
        }
        self.flush()?;
        paths.clear();
        *self
            .measurement_bounds
            .lock()
            .map_err(|_| "Viewer bounds lock poisoned")? = None;
        Ok(())
    }

    pub fn log_measurement(
        &self,
        name: &str,
        frequency_hz: &[f64],
        magnitude_db: &[f64],
    ) -> Result<(), String> {
        validate_curve(frequency_hz, magnitude_db)?;
        let path = format!("measurements/{}/response", entity_name(name));
        let mut paths = self
            .measurement_paths
            .lock()
            .map_err(|_| "Viewer input lock poisoned")?;
        paths.insert(path.clone());
        let mut bounds = self
            .measurement_bounds
            .lock()
            .map_err(|_| "Viewer bounds lock poisoned")?;
        let mut extent = curve_bounds(frequency_hz, magnitude_db);
        if let Some(previous) = *bounds {
            extent = [
                previous[0].min(extent[0]),
                previous[1].max(extent[1]),
                previous[2].min(extent[2]),
                previous[3].max(extent[3]),
            ];
        }
        *bounds = Some(extent);
        let frequency_tensor = tensor(frequency_hz);
        let magnitude_tensor = tensor(magnitude_db);
        let points = frequency_hz
            .iter()
            .zip(magnitude_db)
            .map(|(&f, &db)| [display_frequency(f), -db as f32])
            .collect::<Vec<_>>();
        let response = rerun::LineStrips2D::new([points]);
        for stream in self.streams() {
            stream
                .log_static(format!("{path}/frequency_hz"), &frequency_tensor)
                .map_err(|e| e.to_string())?;
            stream
                .log_static(format!("{path}/magnitude_db"), &magnitude_tensor)
                .map_err(|e| e.to_string())?;
            stream
                .log_static(path.as_str(), &response)
                .map_err(|e| e.to_string())?;
            log_chart_axes(stream, "measurements/chart_axes", extent, true)?;
            stream.log_static(format!("{path}/axes"), &rerun::TextDocument::new(
                "Horizontal: 40 × log10 frequency in Hz, with Hz tick labels; vertical display coordinate: -magnitude dB (higher levels appear upward). Tensors retain the supplied float64 samples; no smoothing or correction."
            )).map_err(|e| e.to_string())?;
        }
        self.flush()
    }

    /// Starts another study without leaving previous proposals or final output visible.
    pub fn begin_solver_study(&self, identity: &str) -> Result<(), String> {
        let mut index = self
            .callback_index
            .lock()
            .map_err(|_| "Viewer callback lock poisoned")?;
        *index += 1;
        for stream in self.streams() {
            // Static clears suppress later temporal proposals at every callback time.
            stream.set_time_sequence("callback", *index as i64);
            stream
                .log("solver/channels", &rerun::Clear::recursive())
                .map_err(|e| e.to_string())?;
            stream
                .log_static("solver/final", &rerun::Clear::recursive())
                .map_err(|e| e.to_string())?;
            stream
                .log("solver/study", &rerun::TextDocument::new(identity))
                .map_err(|e| e.to_string())?;
        }
        self.flush()
    }

    /// Canonical post-finalization output, kept separate from optimizer proposals.
    pub fn log_final_response(
        &self,
        channel: &str,
        frequency_hz: &[f64],
        initial_db: &[f64],
        final_db: &[f64],
    ) -> Result<(), String> {
        validate_curve(frequency_hz, initial_db)?;
        validate_curve(frequency_hz, final_db)?;
        let path = format!("solver/final/{}", entity_name(channel));
        for stream in self.streams() {
            stream
                .log_static(format!("{path}/frequency_hz"), &tensor(frequency_hz))
                .map_err(|e| e.to_string())?;
            for (label, samples) in [
                ("measured_input", initial_db),
                ("realized_output", final_db),
            ] {
                let points: Vec<_> = frequency_hz
                    .iter()
                    .zip(samples)
                    .map(|(&f, &db)| [display_frequency(f), -db as f32])
                    .collect();
                stream
                    .log_static(
                        format!("{path}/{label}"),
                        &rerun::LineStrips2D::new([points]),
                    )
                    .map_err(|e| e.to_string())?;
                stream
                    .log_static(format!("{path}/{label}/db"), &tensor(samples))
                    .map_err(|e| e.to_string())?;
            }
            let a = curve_bounds(frequency_hz, initial_db);
            let b = curve_bounds(frequency_hz, final_db);
            log_chart_axes(
                stream,
                &format!("{path}/axes"),
                [
                    a[0].min(b[0]),
                    a[1].max(b[1]),
                    a[2].min(b[2]),
                    a[3].max(b[3]),
                ],
                true,
            )?;
        }
        self.flush()
    }

    pub fn log_room_mesh(
        &self,
        vertices: &[[f32; 3]],
        triangles: &[[u32; 3]],
        provenance: &str,
    ) -> Result<(), String> {
        if vertices.is_empty()
            || triangles.is_empty()
            || vertices.iter().flatten().any(|v| !v.is_finite())
            || triangles
                .iter()
                .flatten()
                .any(|&i| i as usize >= vertices.len())
        {
            return Err("Room mesh requires finite vertices and valid triangle indices".into());
        }
        let _inputs = self
            .measurement_paths
            .lock()
            .map_err(|_| "Viewer input lock poisoned")?;
        for stream in self.streams() {
            stream
                .log_static("room", &rerun::ViewCoordinates::RIGHT_HAND_Z_UP())
                .map_err(|e| e.to_string())?;
            stream
                .log_static(
                    "room/geometry",
                    &rerun::Mesh3D::new(vertices.to_vec())
                        .with_triangle_indices(triangles.to_vec()),
                )
                .map_err(|e| e.to_string())?;
            stream
                .log_static("room/source", &rerun::TextDocument::new(provenance))
                .map_err(|e| e.to_string())?;
        }
        self.flush()
    }

    /// Records every supplied callback before the UI coalesces presentation updates.
    pub fn log_solver_progress(
        &self,
        channel: &str,
        iteration: usize,
        loss: Option<f64>,
        overall: f64,
        stage: &str,
        message: Option<&str>,
        response: Option<SolverResponse<'_>>,
    ) -> Result<(), String> {
        if !overall.is_finite() || loss.is_some_and(|v| !v.is_finite()) {
            return Err("Nonfinite solver progress".into());
        }
        let mut index = self
            .callback_index
            .lock()
            .map_err(|_| "Viewer callback lock poisoned")?;
        let next = *index + 1;
        let path = format!("solver/channels/{}", entity_name(channel));
        if let Some(response) = &response {
            for values in [
                response.prepared_measurement_db,
                response.target_db,
                response.filter_db,
                response.predicted_db,
            ] {
                validate_curve(response.frequency_hz, values)?;
            }
        }
        for stream in self.streams() {
            stream.set_time_sequence("callback", next as i64);
            if let Some(loss) = loss {
                stream
                    .log(format!("{path}/objective"), &rerun::Scalars::new([loss]))
                    .map_err(|e| e.to_string())?;
            }
            let details = serde_json::json!({"callback_index": next, "channel": channel,
                "iteration": iteration, "loss": loss, "overall": overall, "stage": stage,
                "message": message, "response_available":response.is_some(),
                "scope":"actual solver callback; not proof of a generation barrier"});
            stream
                .log(
                    "solver/current",
                    &rerun::TextDocument::new(details.to_string()),
                )
                .map_err(|e| e.to_string())?;
            if let Some(response) = &response {
                stream
                    .log(
                        format!("{path}/response/frequency_hz"),
                        &tensor(response.frequency_hz),
                    )
                    .map_err(|e| e.to_string())?;
                let mut extent =
                    curve_bounds(response.frequency_hz, response.prepared_measurement_db);
                for values in [response.target_db, response.predicted_db] {
                    let other = curve_bounds(response.frequency_hz, values);
                    extent[2] = extent[2].min(other[2]);
                    extent[3] = extent[3].max(other[3]);
                }
                log_chart_axes(stream, "solver/channels/chart_axes", extent, false)?;
                for (name, values) in [
                    ("prepared_measurement", response.prepared_measurement_db),
                    ("target", response.target_db),
                    ("filter", response.filter_db),
                    ("predicted", response.predicted_db),
                ] {
                    stream
                        .log(format!("{path}/arrays/{name}_db"), &tensor(values))
                        .map_err(|e| e.to_string())?;
                    // Electrical filter gain has a different reference from acoustic level.
                    if name == "filter" {
                        continue;
                    }
                    let points = response
                        .frequency_hz
                        .iter()
                        .zip(values)
                        .map(|(&f, &v)| [display_frequency(f), -v as f32])
                        .collect::<Vec<_>>();
                    stream
                        .log(
                            format!("{path}/response/{name}"),
                            &rerun::LineStrips2D::new([points]).with_colors([match name {
                                "prepared_measurement" => 0x8A96A8FFu32,
                                "target" => 0xD99838FF,
                                "filter" => 0xA979CFFF,
                                _ => 0x398FD9FF,
                            }]),
                        )
                        .map_err(|e| e.to_string())?;
                }
            }
            stream.log(
                format!("{path}/response_status"),
                &rerun::TextDocument::new(if response.is_some() {
                    format!("Actual response snapshot at callback {next}; iteration {iteration} ({stage}).")
                } else {
                    format!("Callback {next} supplied no response. Any visible curves are the last actual snapshot, not a new prediction ({stage}).")
                }),
            ).map_err(|e| e.to_string())?;
        }
        self.file
            .flush_with_timeout(Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        *index = next;
        Ok(())
    }

    pub fn flush(&self) -> Result<(), String> {
        self.file
            .flush_with_timeout(Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        self.live
            .flush_with_timeout(Duration::from_secs(2))
            .map_err(|e| e.to_string())
    }
}

// Spatial2D uses isotropic world coordinates, so a raw log10(Hz) span would
// collapse beside a ~100 dB vertical span. This transform is display-only.
fn display_frequency(hz: f64) -> f32 {
    (40.0 * hz.log10()) as f32
}

fn curve_bounds(frequency: &[f64], magnitude: &[f64]) -> [f64; 4] {
    [
        frequency[0],
        frequency[frequency.len() - 1],
        magnitude.iter().copied().fold(f64::INFINITY, f64::min),
        magnitude.iter().copied().fold(f64::NEG_INFINITY, f64::max),
    ]
}

fn log_chart_axes(
    stream: &RecordingStream,
    path: &str,
    bounds: [f64; 4],
    is_static: bool,
) -> Result<(), String> {
    let [min_hz, max_hz, min_db, max_db] = bounds;
    let floor = ((min_db - 3.) / 10.).floor() * 10.;
    let ceiling = ((max_db + 3.) / 10.).ceil() * 10.;
    let left = display_frequency(min_hz);
    let right = display_frequency(max_hz);
    let mut lines = Vec::new();
    let mut positions = Vec::new();
    let mut labels = Vec::new();
    for exponent in min_hz.log10().floor() as i32..=max_hz.log10().ceil() as i32 {
        for multiplier in [1., 2., 5.] {
            let hz = multiplier * 10_f64.powi(exponent);
            if hz < min_hz || hz > max_hz {
                continue;
            }
            let x = display_frequency(hz);
            lines.push(vec![[x, -floor as f32], [x, -ceiling as f32]]);
            positions.push([x, -floor as f32 + 4.]);
            labels.push(if hz >= 1000. {
                format!("{}k Hz", hz / 1000.)
            } else {
                format!("{hz} Hz")
            });
        }
    }
    let step_db = (((ceiling - floor) / 20.).max(10.) / 10.).ceil() * 10.;
    for index in 0..=((ceiling - floor) / step_db) as usize {
        let db = floor + index as f64 * step_db;
        lines.push(vec![[left, -db as f32], [right, -db as f32]]);
        positions.push([left - 4., -db as f32]);
        labels.push(format!("{db} dB"));
    }
    let grid = rerun::LineStrips2D::new(lines).with_colors([0x59657370u32]);
    let ticks = rerun::Points2D::new(positions)
        .with_labels(labels)
        .with_show_labels(true)
        .with_radii([0.0f32])
        .with_colors([0x9CA9B9FFu32]);
    if is_static {
        stream
            .log_static(format!("{path}/lines"), &grid)
            .map_err(|e| e.to_string())?;
        stream
            .log_static(format!("{path}/labels"), &ticks)
            .map_err(|e| e.to_string())?;
    } else {
        stream
            .log(format!("{path}/lines"), &grid)
            .map_err(|e| e.to_string())?;
        stream
            .log(format!("{path}/labels"), &ticks)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn entity_name(name: &str) -> String {
    if name.is_empty() {
        return "%empty".into();
    }
    let mut encoded = String::new();
    for byte in name.bytes() {
        if byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_' {
            encoded.push(byte as char);
        } else {
            use std::fmt::Write;
            write!(&mut encoded, "%{byte:02X}").expect("String formatting cannot fail");
        }
    }
    encoded
}

fn tensor(values: &[f64]) -> rerun::Tensor {
    rerun::Tensor::new(rerun::encodings::TensorData::new(
        vec![values.len() as u64],
        rerun::encodings::TensorBuffer::F64(values.to_vec().into()),
    ))
}

fn project_entity_text(path: &str, metadata: &serde_json::Value) -> Result<String, String> {
    if !path.starts_with("project/")
        || path.len() > 1024
        || path.chars().any(|c| c.is_control() || c == '\\')
        || path.split('/').any(|part| matches!(part, "" | "." | ".."))
    {
        return Err(
            "Project metadata requires a bounded path beneath project/ without traversal".into(),
        );
    }
    let text = serde_json::to_string_pretty(metadata).map_err(|error| error.to_string())?;
    if text.len() > 64 * 1024 {
        return Err("Project metadata exceeds 64 KiB per entity".into());
    }
    Ok(text)
}

fn validate_curve(frequency_hz: &[f64], magnitude_db: &[f64]) -> Result<(), String> {
    if frequency_hz.len() < 2
        || frequency_hz.len() != magnitude_db.len()
        || frequency_hz.iter().any(|f| !f.is_finite() || *f <= 0.)
        || frequency_hz.windows(2).any(|p| p[0] >= p[1])
        || magnitude_db.iter().any(|db| !db.is_finite())
    {
        return Err("Response requires a finite, positive, increasing Hz grid and matching finite dB samples".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn project_metadata_cannot_replace_scientific_entities() {
        let value = serde_json::json!({"role":"speaker"});
        assert!(project_entity_text("project/speakers/4c", &value).is_ok());
        for path in [
            "room/geometry",
            "project/../room",
            "project//x",
            "project/x\n",
            "project/x\\y",
            "project/.",
        ] {
            assert!(project_entity_text(path, &value).is_err());
        }
        assert!(
            project_entity_text("project/config", &serde_json::json!("x".repeat(65537))).is_err()
        );
    }
    #[test]
    fn curve_validation_and_entity_paths_preserve_boundaries() {
        assert!(validate_curve(&[20., 100.], &[-3., 0.]).is_ok());
        assert_eq!(display_frequency(1000.) - display_frequency(100.), 40.);
        assert_eq!(curve_bounds(&[20., 100.], &[-3., 0.]), [20., 100., -3., 0.]);
        for (f, db) in [
            (vec![20., 10.], vec![0., 0.]),
            (vec![20., 100.], vec![0.]),
            (vec![0., 100.], vec![0., 0.]),
            (vec![20., 100.], vec![0., f64::NAN]),
        ] {
            assert!(validate_curve(&f, &db).is_err());
        }
        assert_eq!(entity_name("L/../curve"), "L%2F%2E%2E%2Fcurve");
        assert_ne!(entity_name("L/curve"), entity_name("L_curve"));
        assert_ne!(entity_name(""), entity_name("%empty"));
        assert_eq!(entity_name(""), "%empty");
    }
}
