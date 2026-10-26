//! Real authenticated HTTP -> GPUI main thread -> current RoomEQ state.
#![cfg(feature = "workbench-api")]

use gpui::{AppContext, TestAppContext, VisualTestContext};
use serde_json::{Value, json};
use sotf_audio_player::Player;
use sotf_audio_player_gpui::{
    app::{App, AppState, player_handle::PlayerHandle, state::ui::LayoutState, workbench_api},
    ui::PlayerView,
};
use sotf_dev_api::RunId;
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    time::{Duration, Instant},
};

const RUN_ID: &str = "0123456789abcdef0123456789abcdef";

fn request(
    endpoint: SocketAddr,
    cx: &mut VisualTestContext,
    body: Value,
    authenticated: bool,
) -> Value {
    let response = std::thread::spawn(move || {
        let mut stream = TcpStream::connect(endpoint).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(4)))
            .unwrap();
        let body = body.to_string();
        let header = if authenticated {
            format!("X-SOTF-Dev-Run-ID: {RUN_ID}\r\n")
        } else {
            String::new()
        };
        write!(stream, "POST /workbench HTTP/1.1\r\nHost: localhost\r\n{header}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        let mut text = String::new();
        stream.read_to_string(&mut text).unwrap();
        let (_, body) = text.split_once("\r\n\r\n").unwrap();
        serde_json::from_str::<Value>(body).unwrap()
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !response.is_finished() {
        cx.executor().advance_clock(Duration::from_millis(10));
        cx.run_until_parked();
        std::thread::yield_now();
        assert!(Instant::now() < deadline, "workbench route timed out");
    }
    response.join().unwrap()
}

#[gpui::test]
async fn live_workbench_commands_share_ui_state_and_enforce_boundaries(cx: &mut TestAppContext) {
    // HTTP and the existing optimizer use real worker threads outside the test scheduler.
    cx.executor().allow_parking();
    let directory = tempfile::tempdir().unwrap();
    sotf_audio_player::config::set_config_dir_override(directory.path().to_path_buf());
    let window = cx.add_window(|_, cx| {
        let state = cx.new(|cx| AppState {
            app: App::new(),
            layout: cx.new(|_| LayoutState::default()),
            player: PlayerHandle::new(Player::new()),
        });
        PlayerView::new(state, cx)
    });
    let service = cx
        .update(|cx| {
            workbench_api::start_for_testing(cx, 0, window.into(), RunId::parse(RUN_ID).unwrap())
        })
        .unwrap();
    let endpoint = service.endpoint();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();
    #[cfg(feature = "calibration-designer")]
    {
        use sotf_audio_player_gpui::{RoomEqStep, Screen};
        use sotf_calibration_workbench::DesignerCommand;
        let designer = window
            .update(&mut visual, |view, _, cx| {
                view.state.update(cx, |state, _| {
                    let room = &mut state.app.measurement_state.room_eq_state;
                    assert!(!room.configure_designer_selected);
                    room.configure_designer_selected = true;
                    room.step = RoomEqStep::Configure;
                    state.app.set_screen(Screen::RoomEq, "test-designer");
                    assert!(state.app.is_screen_available(&Screen::Library));
                });
                view.calibration_designer(cx)
            })
            .unwrap();
        for command in [DesignerCommand::RunStudy, DesignerCommand::MaximumCut(-2.0)] {
            designer.update(&mut visual, |_, cx| cx.emit(command));
            visual.run_until_parked();
            window
                .update(&mut visual, |view, _, cx| {
                    let room = &view.state.read(cx).app.measurement_state.room_eq_state;
                    assert!(room.error_message.is_some());
                    assert!(!room.is_optimizing());
                    assert!(room.optimizer_config.min_db <= 0.0);
                })
                .unwrap();
        }
        designer.update(&mut visual, |_, cx| {
            cx.emit(DesignerCommand::MinimumFrequency(77.0))
        });
        visual.run_until_parked();
        designer.update(&mut visual, |_, cx| {
            cx.emit(DesignerCommand::TargetSlope(-0.6));
            cx.emit(DesignerCommand::BassShelf(2.0));
        });
        visual.run_until_parked();
        window
            .update(&mut visual, |view, _, cx| {
                let room = &view.state.read(cx).app.measurement_state.room_eq_state;
                assert_eq!(room.optimizer_config.target_response.shape, "custom");
                assert_eq!(
                    room.optimizer_config.target_response.slope_db_per_octave,
                    -0.6
                );
                assert_eq!(room.optimizer_config.target_response.bass_shelf_db, 2.0);
                assert!(room.error_message.is_none());
            })
            .unwrap();
        window
            .update(&mut visual, |view, _, cx| {
                view.state.update(cx, |state, _| {
                    let room = &mut state.app.measurement_state.room_eq_state;
                    assert_eq!(room.optimizer_config.min_freq, 77.0);
                    assert!(room.error_message.is_none());
                    room.configure_designer_selected = false;
                    room.calibration_designer = None;
                    room.step = RoomEqStep::LoadData;
                    state.app.set_screen(Screen::Home, "test-designer-return");
                });
            })
            .unwrap();
        drop(designer);
        visual.run_until_parked();
    }
    let unauthenticated = request(endpoint, &mut visual, json!({"operation":"inspect"}), false);
    assert_ne!(unauthenticated["ok"], true);
    let inspection = request(endpoint, &mut visual, json!({"operation":"inspect"}), true);
    assert_eq!(inspection["ok"], true);
    assert_eq!(inspection["value"]["chat_enabled"], false);
    assert_eq!(inspection["value"]["project"]["measurement_count"], 0);
    #[cfg(not(feature = "calibration-designer"))]
    {
        assert_eq!(
            inspection["value"]["presentation"]["designer_available"],
            false
        );
        let unavailable = request(
            endpoint,
            &mut visual,
            json!({
                "operation":"select_designer_panel", "expected_revision":inspection["value"]["revision"],
                "panel":"goals"
            }),
            true,
        );
        assert_eq!(unavailable["ok"], false);
    }
    assert_eq!(
        inspection["value"]["capabilities"]["recording_spectrogram"]["selected_result_available"],
        false
    );
    #[cfg(feature = "calibration-designer")]
    {
        let selected = request(
            endpoint,
            &mut visual,
            json!({
                "operation":"select_designer_panel", "expected_revision":inspection["value"]["revision"],
                "panel":"goals"
            }),
            true,
        );
        assert_eq!(selected["ok"], true, "{selected}");
        assert_eq!(selected["value"]["presentation"]["selected_panel"], "goals");
        assert_eq!(
            selected["value"]["revision"],
            inspection["value"]["revision"]
        );
        assert_eq!(selected["value"]["config"], inspection["value"]["config"]);
        let stale = request(
            endpoint,
            &mut visual,
            json!({
                "operation":"select_designer_panel", "expected_revision":"stale",
                "panel":"study"
            }),
            true,
        );
        assert_eq!(stale["ok"], false);
        let unavailable = request(
            endpoint,
            &mut visual,
            json!({
                "operation":"select_measurement", "expected_revision":inspection["value"]["revision"],
                "index":0
            }),
            true,
        );
        assert_eq!(unavailable["ok"], false);
    }
    let revision = inspection["value"]["revision"]
        .as_str()
        .unwrap()
        .to_string();
    let patched = request(
        endpoint,
        &mut visual,
        json!({
            "operation":"update_designer", "expected_revision":revision,
            "patch":{"min_freq":35.0,"max_freq":15000.0,"max_cut_db":12.0,"max_boost_db":1.0}
        }),
        true,
    );
    assert_eq!(patched["ok"], true, "{patched}");
    assert_ne!(patched["value"]["revision"], revision);
    window
        .update(&mut visual, |view, _, cx| {
            let room = &view.state.read(cx).app.measurement_state.room_eq_state;
            assert_eq!(room.optimizer_config.min_freq, 35.0);
            assert_eq!(room.optimizer_config.max_freq, 15000.0);
            assert_eq!(room.optimizer_config.min_db, -12.0);
            assert_eq!(room.optimizer_config.max_db, 1.0);
        })
        .unwrap();
    let stale = request(
        endpoint,
        &mut visual,
        json!({
            "operation":"update_designer", "expected_revision":revision,
            "patch":{"min_freq":40.0}
        }),
        true,
    );
    assert_eq!(stale["ok"], false);
    let current = patched["value"]["revision"].as_str().unwrap();
    let invalid = request(
        endpoint,
        &mut visual,
        json!({
            "operation":"update_designer", "expected_revision":current,
            "patch":{"min_freq":1000.0,"max_freq":100.0}
        }),
        true,
    );
    assert_eq!(invalid["ok"], false);
    let missing_measurements = request(
        endpoint,
        &mut visual,
        json!({
            "operation":"run_study", "expected_revision":current
        }),
        true,
    );
    assert_eq!(missing_measurements["ok"], false);
    let after = request(endpoint, &mut visual, json!({"operation":"inspect"}), true);
    assert_eq!(after["value"]["revision"], current);
    assert_eq!(after["value"]["designer"], patched["value"]["designer"]);
    // A small, explicitly synthetic fixture exercises the actual offline UI solver.
    // Test cancellation routing without racing a deliberately tiny backend run.
    // This checks the shared UI signal, not backend acknowledgement timing.
    window
        .update(&mut visual, |view, _, cx| {
            view.state.update(cx, |state, _| {
                let room = &mut state.app.measurement_state.room_eq_state;
                room.optimization_status =
                    sotf_audio_player_gpui::app::types::OptimizationStatus::Running;
                room.cancel_requested
                    .store(false, std::sync::atomic::Ordering::Relaxed);
            });
        })
        .unwrap();
    let cancelled = request(
        endpoint,
        &mut visual,
        json!({
            "operation":"cancel_study", "expected_revision":current
        }),
        true,
    );
    assert_eq!(cancelled["ok"], true, "{cancelled}");
    window
        .update(&mut visual, |view, _, cx| {
            view.state.update(cx, |state, _| {
                let room = &mut state.app.measurement_state.room_eq_state;
                assert!(
                    room.cancel_requested
                        .load(std::sync::atomic::Ordering::Relaxed)
                );
                room.optimization_status =
                    sotf_audio_player_gpui::app::types::OptimizationStatus::Idle;
            });
        })
        .unwrap();
    window
        .update(&mut visual, |view, _, cx| {
            view.state.update(cx, |state, _| {
                let room = &mut state.app.measurement_state.room_eq_state;
                let frequencies: Vec<f32> = (0..64)
                    .map(|i| 20.0 * 1000.0_f32.powf(i as f32 / 63.0))
                    .collect();
                room.channel_measurements = vec![sotf_audio_player_gpui::ChannelMeasurement {
                    acquisition: None,
                    driver_measurement_sets: Vec::new(),
                    provenance: Vec::new(),
                    channel_name: "L".into(),
                    measurement: sotf_audio_player_gpui::RecordingResult {
                        sample_rate_hz: Some(48_000),
                        channel: 0,
                        magnitude_db: frequencies
                            .iter()
                            .map(|f| 70.0 + 3.0 * (f / 300.0).ln().sin())
                            .collect(),
                        frequencies,
                        phase_deg: Vec::new(),
                        wav_path: None,
                        csv_path: None,
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
                    },
                    is_group: false,
                    group_drivers: Vec::new(),
                    multi_mic_measurements: Vec::new(),
                }];
                room.init_speaker_configs();
                room.optimizer_config.algorithm = "autoeq:cmaes".into();
                room.optimizer_config.num_filters = 1;
                room.optimizer_config.max_iter = 2;
                room.optimizer_config.population = 12;
                room.optimizer_config.refine = false;
            });
        })
        .unwrap();
    let loaded = request(endpoint, &mut visual, json!({"operation":"inspect"}), true);
    #[cfg(feature = "calibration-designer")]
    {
        let selected = request(
            endpoint,
            &mut visual,
            json!({
                "operation":"select_measurement", "expected_revision":loaded["value"]["revision"],
                "index":0
            }),
            true,
        );
        assert_eq!(selected["ok"], true, "{selected}");
        assert_eq!(selected["value"]["presentation"]["selected_panel"], "data");
        assert_eq!(selected["value"]["presentation"]["selected_measurement"], 0);
        assert!(
            selected["value"]["presentation"]["selected_response"]["samples_hz_db"]
                .as_array()
                .unwrap()
                .len()
                > 2
        );
        assert_eq!(selected["value"]["config"], loaded["value"]["config"]);
        assert_eq!(selected["value"]["revision"], loaded["value"]["revision"]);
        let invalid = request(
            endpoint,
            &mut visual,
            json!({
                "operation":"select_measurement", "expected_revision":loaded["value"]["revision"],
                "index":127
            }),
            true,
        );
        assert_eq!(invalid["ok"], false);
    }
    let started = request(
        endpoint,
        &mut visual,
        json!({
            "operation":"run_study", "expected_revision":loaded["value"]["revision"]
        }),
        true,
    );
    assert_eq!(started["ok"], true, "{started}");
    assert_eq!(started["value"]["study"]["status"], "Running");
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let status = request(endpoint, &mut visual, json!({"operation":"inspect"}), true);
        if status["value"]["study"]["status"] != "Running" {
            assert_eq!(status["value"]["study"]["status"], "Completed", "{status}");
            assert_eq!(status["value"]["study"]["results_current"], true);
            assert_eq!(status["value"]["study"]["result_count"], 1);
            break;
        }
        assert!(Instant::now() < deadline, "offline study did not finish");
        std::thread::yield_now();
    }
    drop(service);
    visual.run_until_parked();
}
