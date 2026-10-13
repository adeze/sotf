//! Delegates dashboard data and document assembly to shared AutoEQ APIs.

// Rust guideline compliant 2026-02-21

use anyhow::Context;
use rust_embed::RustEmbed;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

// Compile the same producer/assets used by standalone AutoEQ reports. This
// embeds upstream sources; it does not maintain a second copy or renderer.
#[derive(RustEmbed)]
#[folder = "../../../autoeq/scripts/src/"]
#[include = "*.py"]
struct AutoEqProducer;

/// Generated document and its private working directory.
#[derive(Debug)]
pub struct GeneratedReport {
    directory: tempfile::TempDir,
    html_path: PathBuf,
}

impl GeneratedReport {
    /// Read the self-contained HTML/WASM document.
    ///
    /// # Errors
    /// Returns an error if the generated file cannot be read.
    pub fn read_html(&self) -> anyhow::Result<String> {
        std::fs::read_to_string(&self.html_path).context("Reading shared AutoEQ report")
    }

    /// Generated report path, valid while this object lives.
    pub fn path(&self) -> &Path {
        &self.html_path
    }

    /// Save the same document for standalone viewing.
    ///
    /// # Errors
    /// Returns an error if the destination cannot be written.
    pub fn save(&self, destination: &Path) -> anyhow::Result<()> {
        std::fs::copy(&self.html_path, destination).context("Saving shared AutoEQ report")?;
        Ok(())
    }

    /// Directory retained for diagnostics during this report's lifetime.
    pub fn working_directory(&self) -> &Path {
        self.directory.path()
    }
}

/// Generate every dashboard using AutoEQ's bundled report producer.
///
/// Run this function on a background worker. `artifact_root` resolves relative
/// response/IR sidecars without rewriting their identities. Python with NumPy
/// is required by the existing AutoEQ producer; `python` selects that runtime.
/// The native dataset producer resolves to `AUTOEQ_REPORT_PRODUCER` when set,
/// else to Cargo's cached build in the sibling AutoEQ checkout when present.
/// No packages are installed and no network access is performed.
///
/// # Errors
/// Returns an error for missing Python/NumPy, invalid results, missing artifacts,
/// producer failures, or generation exceeding its one-minute timeout.
pub fn generate_report(
    result: &serde_json::Value,
    artifact_root: Option<&Path>,
    python: Option<&Path>,
) -> anyhow::Result<GeneratedReport> {
    generate_report_with_quality(result, artifact_root, python, None)
}

/// Generate the shared dashboards with source-bound capture evidence.
///
/// The additional section is assembled by the upstream native/WASM report
/// library. The host supplies evidence and does not render a dashboard.
///
/// # Errors
/// Returns generation errors or invalid capture-quality evidence.
pub fn generate_report_with_quality(
    result: &serde_json::Value,
    artifact_root: Option<&Path>,
    python: Option<&Path>,
    quality: Option<&autoeq_report_wasm::capture_quality::CaptureQualityReport>,
) -> anyhow::Result<GeneratedReport> {
    let directory = tempfile::Builder::new().prefix("sotf-report-").tempdir()?;
    for name in AutoEqProducer::iter() {
        let bytes = AutoEqProducer::get(&name).context("Missing embedded AutoEQ producer")?;
        let target = directory
            .path()
            .join("autoeq/scripts/src")
            .join(name.as_ref());
        std::fs::create_dir_all(target.parent().context("Report asset has no directory")?)?;
        std::fs::write(target, bytes.data.as_ref())?;
    }
    // The canonical payload producer reads the workspace version for its
    // provenance header; retain that context without staging renderer assets.
    std::fs::write(
        directory.path().join("autoeq/Cargo.toml"),
        include_bytes!("../../../../autoeq/Cargo.toml"),
    )?;
    let json_path = directory.path().join("result.json");
    let payload_path = directory.path().join("report.json");
    let html_path = directory.path().join("report.html");
    std::fs::write(&json_path, serde_json::to_vec(result)?)?;
    if let Some(root) = artifact_root {
        let root = root
            .canonicalize()
            .context("Resolving AutoEQ report artifacts")?;
        anyhow::ensure!(
            root.is_dir(),
            "AutoEQ report artifact root is not a directory"
        );
        let assets = directory.path().join("result_files");
        // The canonical loader requires a regular support directory and
        // rejects symlinked bundle roots. Stage assets in that same layout;
        // the upstream loader retains responsibility for their validation.
        copy_artifacts(&root, &assets)?;
    }
    let producer = directory.path().join("autoeq/scripts");
    let stderr_path = directory.path().join("producer.stderr");
    let stderr = std::fs::File::create(&stderr_path)?;
    let stdout = std::fs::File::create(directory.path().join("producer.stdout"))?;
    // Arguments are passed directly; paths and result content are never shell code.
    let configured_python = std::env::var_os("SOTF_REPORT_PYTHON").map(PathBuf::from);
    let interpreter = python
        .or(configured_python.as_deref())
        .unwrap_or(Path::new("python3"));
    let mut command = Command::new(interpreter);
    // Staged scripts are not a runnable cargo workspace, so the native
    // producer fallback needs the real sibling checkout. Installed hosts
    // without a checkout keep the previous behavior and must provide
    // AUTOEQ_REPORT_PRODUCER (a prebuilt report-producer executable).
    let sibling = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../autoeq");
    if sibling
        .join("crates/autoeq-report-wasm/Cargo.toml")
        .is_file()
    {
        command.env("AUTOEQ_WORKSPACE_ROOT", &sibling);
    }
    let mut child = command
        .arg("-I").arg("-c")
        .arg("import sys; from pathlib import Path; sys.path.insert(0,sys.argv[1]); from src.loaders import load_roomeq_json; from src.report import build_report_payload; import json; Path(sys.argv[3]).write_text(json.dumps(build_report_payload(load_roomeq_json(Path(sys.argv[2]))),ensure_ascii=False,allow_nan=False),encoding=\"utf-8\")")
        .arg(producer).arg(&json_path).arg(&payload_path)
        .stdin(Stdio::null()).stdout(stdout).stderr(stderr)
        .spawn().context("Starting AutoEQ report producer; install/configure Python with NumPy")?;
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            if !status.success() {
                let mut error = std::fs::read_to_string(&stderr_path).unwrap_or_default();
                // Keep errors actionable without flooding the application's status UI.
                error.truncate(error.floor_char_boundary(4096));
                anyhow::bail!("AutoEQ report generation failed: {error}");
            }
            break;
        }
        if started.elapsed() >= Duration::from_secs(60) {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("AutoEQ report generation timed out after one minute");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    anyhow::ensure!(
        payload_path.is_file(),
        "AutoEQ producer did not emit a payload"
    );
    let mut payload: serde_json::Value = serde_json::from_slice(&std::fs::read(payload_path)?)?;
    // Validation and dashboard grouping belong to the same upstream library
    // used by standalone Rust producers and WASM consumers.
    let empty_quality = autoeq_report_wasm::capture_quality::CaptureQualityReport::default();
    autoeq_report_wasm::capture_quality::append_capture_quality_section_to_value(
        &mut payload,
        quality.unwrap_or(&empty_quality),
    )?;
    let title = payload["title"]
        .as_str()
        .context("Report title is missing")?;
    let assets = autoeq_report_wasm::assemble::checked_in_assets().map_err(anyhow::Error::msg)?;
    let html = autoeq_report_wasm::assemble::assemble_html(
        title,
        &serde_json::to_string(&payload)?,
        &assets,
    );
    std::fs::write(&html_path, html)?;
    Ok(GeneratedReport {
        directory,
        html_path,
    })
}

fn copy_artifacts(source: &Path, target: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(target)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            copy_artifacts(&entry.path(), &target.join(entry.file_name()))?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), target.join(entry.file_name()))?;
        } else {
            return Err(std::io::Error::other(
                "Report artifacts must be regular files or directories",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_delegates_all_dashboard_groups_to_autoeq() {
        let result = serde_json::json!({"version":"1.0", "channels":{}, "metadata":{}});
        let python = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../autoeq/.venv/bin/python");
        let report =
            generate_report(&result, None, python.is_file().then_some(python.as_path())).unwrap();
        let html = report.read_html().unwrap();
        for group in [
            "DSP analysis",
            "Acoustics analysis",
            "Psychoacoustic report",
            "Why this correction?",
            "Time domain analysis",
            "DSP signal flow",
        ] {
            assert!(html.contains(group), "Missing shared dashboard {group}");
        }
        assert!(html.contains("autoeq-report-data-v1"));
        assert!(!html.contains("{{WASM_2D_B64}}"));
        assert!(!html.contains("{{PAYLOAD_JSON}}"));
        let exported = report.working_directory().join("standalone.html");
        report.save(&exported).unwrap();
        assert_eq!(std::fs::read_to_string(exported).unwrap(), html);
    }

    #[test]
    fn missing_runtime_is_reported_without_an_empty_success_document() {
        let result = serde_json::json!({"channels":{}});
        let error =
            generate_report(&result, None, Some(Path::new("/missing/sotf-python"))).unwrap_err();
        assert!(error.to_string().contains("Python with NumPy"));
    }

    #[test]
    fn shared_quality_dashboard_preserves_the_existing_payload() {
        use autoeq_report_wasm::capture_quality::CaptureQualityReport;
        let quality: CaptureQualityReport = serde_json::from_value(serde_json::json!({"sources":[{
            "source_id":"take-L-mic-2-position-3", "channel":"L", "mic":"2", "position":"3",
            "stimulus":{"signal_type":"sweep","sample_rate_hz":48000,"duration_secs":4.25,"level_dbfs":-30.0},
            "findings":[
                {"metric":{"kind":"thd"},"status":"warning","value":25.76,"unit":"%",
                 "policy":"SingleTake","method":"Farina harmonic IR","threshold":{"comparison":"less_than_or_equal","value":10.0,"unit":"%"},
                 "recommendation":"Lower playback level and retake"},
                {"metric":{"kind":"broadband_snr"},"status":"unknown","unit":"dB",
                 "unknown_reason":"Digital silence does not estimate ambient noise"}
            ]
        }]})).unwrap();
        let result = serde_json::json!({"version":"1.0","channels":{},"metadata":{}});
        let python = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../autoeq/.venv/bin/python");
        let report =
            generate_report(&result, None, python.is_file().then_some(python.as_path())).unwrap();
        let read_payload = |html: &str| -> serde_json::Value {
            serde_json::from_str(
                html.split_once("<script id=\"report-payload\" type=\"application/json\">")
                    .unwrap()
                    .1
                    .split_once("</script>")
                    .unwrap()
                    .0,
            )
            .unwrap()
        };
        let original = read_payload(&report.read_html().unwrap());
        let report = generate_report_with_quality(
            &result,
            None,
            python.is_file().then_some(python.as_path()),
            Some(&quality),
        )
        .unwrap();
        let enriched = read_payload(&report.read_html().unwrap());
        let old_sections = original["sections"].as_array().unwrap();
        let new_sections = enriched["sections"].as_array().unwrap();
        assert_eq!(&new_sections[..old_sections.len()], old_sections.as_slice());
        assert_eq!(new_sections.len(), old_sections.len() + 1);
        let evidence = new_sections.last().unwrap()["html"].as_str().unwrap();
        for label in [
            "Capture quality",
            "Warning",
            "Unknown",
            "25.76",
            "SingleTake",
            "Farina harmonic IR",
            "Digital silence",
            "-30",
        ] {
            assert!(
                evidence.contains(label),
                "Missing retained evidence {label}"
            );
        }
        assert_eq!(new_sections.last().unwrap()["bucket"], "DSP analysis");
        if let Some(destination) = std::env::var_os("SOTF_REPORT_QA_HTML") {
            report.save(Path::new(&destination)).unwrap();
        }
    }

    #[test]
    fn upstream_sti_dashboard_retains_measured_basis_and_missing_evidence() {
        let result = serde_json::json!({
            "version":"1.0", "metadata":{}, "channels":{
                "L":{"plugins":[], "speech_transmission":{
                    "method":"iec_60268_16_2020_indirect_ir_only_v1",
                    "basis":"measured_room_ir", "sti":0.625,
                    "sample_rate_hz":48000, "duration_s":2.0,
                    "octave_centers_hz":[125,250,500,1000,2000,4000,8000],
                    "modulation_frequencies_hz":[0.63,0.8,1.0,1.25,1.6,2.0,2.5,3.15,4.0,5.0,6.3,8.0,10.0,12.5],
                    "mti":vec![0.625; 7],
                    "modulation_transfer":vec![vec![0.625; 7]; 14]
                }},
                "R":{"plugins":[]}
            }
        });
        let python = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../autoeq/.venv/bin/python");
        let report =
            generate_report(&result, None, python.is_file().then_some(python.as_path())).unwrap();
        let html = report.read_html().unwrap();
        assert!(html.contains("Full STI (IR-only): 0.625"));
        assert!(html.contains("STI unavailable"));
        assert!(html.contains("not a post-EQ prediction"));
    }

    #[test]
    fn slim_result_curves_are_loaded_by_the_upstream_bundle_loader() {
        let artifacts = tempfile::tempdir().unwrap();
        std::fs::write(
            artifacts.path().join("initial.csv"),
            "freq,spl\n20,0\n100,2\n1833,1.5\n10000,-1\n20000,-2\n",
        )
        .unwrap();
        std::fs::write(
            artifacts.path().join("eq.csv"),
            "freq,spl\n20,0\n100,-2\n1833,-1.5\n10000,1\n20000,2\n",
        )
        .unwrap();
        std::fs::write(
            artifacts.path().join("measurements_index.json"),
            r#"{"channels":{"L":{"initial_curve":"initial.csv","eq_response":"eq.csv"}}}"#,
        )
        .unwrap();
        let result = serde_json::json!({"version":"1.0","channels":{"L":{"plugins":[],"initial_curve":null,"eq_response":null}},"metadata":{}});
        let python = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../autoeq/.venv/bin/python");
        let report = generate_report(
            &result,
            Some(artifacts.path()),
            python.is_file().then_some(python.as_path()),
        )
        .unwrap();
        let html = report.read_html().unwrap();
        let payload = html
            .split("<script id=\"report-payload\" type=\"application/json\">")
            .nth(1)
            .unwrap()
            .split("</script>")
            .next()
            .unwrap();
        let payload: serde_json::Value = serde_json::from_str(payload).unwrap();
        assert!(
            payload.to_string().contains("1833"),
            "Sidecar frequency must reach the shared dashboard payload"
        );
        assert_eq!(
            result["channels"]["L"]["initial_curve"],
            serde_json::Value::Null
        );
    }
}
