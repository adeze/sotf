//! Real measured single-channel study through the native app's shared pipeline.
use autoeq::roomeq::{CallbackAction, ProcessingMode};
use serde_json::json;
use sotf_audio_player::autoeq::{
    OptimizerConfig, RoomOptimizationProgress, RoomOptimizationResult,
    build_room_config_from_curves, run_room_optimization_with_output_dir,
};
use sotf_rerun_viewer::{SolverResponse, ViewerSession};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

fn scientific_result(result: &RoomOptimizationResult) -> serde_json::Value {
    json!({"channels":result.channels,
        "curves":result.channel_results.iter().map(|(name,r)| (name.clone(),
            json!({"initial":r.initial_curve,"final":r.final_curve,"biquads":r.biquads,
                "pre_score":r.pre_score,"post_score":r.post_score,"fir":r.fir_coeffs})))
            .collect::<std::collections::BTreeMap<_,_>>(),
        "combined_pre_score":result.combined_pre_score,"combined_post_score":result.combined_post_score})
}

fn run(input: &Path, output: &Path) -> Result<(), String> {
    std::fs::create_dir(output).map_err(|e| format!("Use a fresh output directory: {e}"))?;
    let curve =
        autoeq::read::read_curve_from_csv(&input.to_path_buf()).map_err(|e| e.to_string())?;
    let config = build_room_config_from_curves(
        &[("L".into(), curve.clone())],
        OptimizerConfig {
            algorithm: "autoeq:de".into(),
            seed: Some(91827),
            num_filters: 2,
            max_iter: 400,
            population: 12,
            min_freq: 20.,
            max_freq: 300.,
            min_db: -12.,
            max_db: 0.,
            processing_mode: ProcessingMode::LowLatency,
            ..Default::default()
        },
    );
    let baseline = run_room_optimization_with_output_dir(
        &config,
        48000.,
        None,
        Some(&output.join("baseline")),
    )?;
    let session = Arc::new(ViewerSession::new(&output.join("viewer"))?);
    session.log_measurement(
        "L",
        curve.freq.as_slice().ok_or("Noncontiguous Hz grid")?,
        curve.spl.as_slice().ok_or("Noncontiguous dB grid")?,
    )?;
    let observer = session.clone();
    let responses = Arc::new(Mutex::new(Vec::new()));
    let captures = responses.clone();
    let failures = Arc::new(Mutex::new(Vec::new()));
    let observer_failures = failures.clone();
    let observed = run_room_optimization_with_output_dir(
        &config,
        48000.,
        Some(Box::new(move |p: &RoomOptimizationProgress| {
            let response = p.response.as_ref().map(|r| SolverResponse {
                frequency_hz: &r.frequency_hz,
                prepared_measurement_db: &r.prepared_measurement_db,
                target_db: &r.target_db,
                filter_db: &r.filter_db,
                predicted_db: &r.predicted_db,
            });
            if let Some(r) = &p.response {
                captures.lock().unwrap().push(r.predicted_db.clone());
            }
            if let Err(e) = observer.log_solver_progress(
                &p.current_speaker,
                p.iteration,
                (!p.current_speaker.is_empty())
                    .then_some(p.loss)
                    .filter(|x| x.is_finite()),
                p.overall_progress,
                &format!("{:?}/{:?}", p.step_id, p.step_status),
                p.message.as_deref(),
                response,
            ) {
                observer_failures.lock().unwrap().push(e);
            }
            CallbackAction::Continue
        })),
        Some(&output.join("observed")),
    )?;
    let baseline = scientific_result(&baseline);
    let observed = scientific_result(&observed);
    std::fs::write(
        output.join("baseline.json"),
        serde_json::to_vec_pretty(&baseline).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::write(
        output.join("observed.json"),
        serde_json::to_vec_pretty(&observed).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if baseline != observed {
        return Err("Telemetry changed native pipeline DSP/curves/scores".into());
    }
    if !failures.lock().unwrap().is_empty() {
        return Err(format!(
            "Recording failures: {:?}",
            failures.lock().unwrap()
        ));
    }
    let responses = responses.lock().unwrap();
    if responses.len() < 2 || !responses.windows(2).any(|p| p[0] != p[1]) {
        return Err("Study supplied no changing native response snapshots".into());
    }
    session.flush()?;
    let receipt = json!({"input":input,"seed":91827,"response_frames":responses.len(),
        "response_changed":true,"dsp_curve_score_parity":true,"recording":session.recording_path(),
        "viewer_url":session.viewer_url(),"hardware_operations":false});
    std::fs::write(
        output.join("parity.json"),
        serde_json::to_vec_pretty(&receipt).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    println!("{receipt}");
    Ok(())
}

fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let result = if args.len() == 2 {
        run(&PathBuf::from(&args[0]), &PathBuf::from(&args[1]))
    } else {
        Err("Usage: offline_parity <measured.csv> <new-output-directory>".into())
    };
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
