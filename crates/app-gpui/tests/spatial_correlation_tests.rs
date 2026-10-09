//! Exercise the actual desktop graph configuration and monitor factory.

use sotf_audio_player::{PluginGraph, PluginType};
use sotf_plugins::{LoudnessData, ProcessContext, create_plugin};

#[test]
fn desktop_output_monitor_publishes_live_spatial_correlation() {
    let mut graph = PluginGraph::with_default_rack();
    graph.add_user_plugin(&PluginType::Upmixer).unwrap();
    let configs = graph.to_plugin_configs(48_000.0);
    assert!(configs[0].parameters.get("spatial_enabled").is_none());
    let output = configs.last().unwrap();
    assert_eq!(output.parameters["spatial_enabled"], true);
    let dag = graph.to_plugin_graph_config(48_000.0);
    assert_eq!(
        dag.nodes
            .iter()
            .filter(|n| n.parameters["spatial_enabled"] == true)
            .count(),
        1
    );

    let channels = 6;
    let mut monitor =
        create_plugin("loudness_monitor", &output.parameters, channels, 48_000.0).unwrap();
    monitor.initialize(48_000.0).unwrap();
    let frames = 8192;
    let mut input = vec![0.0; frames * channels];
    for (i, frame) in input.chunks_exact_mut(channels).enumerate() {
        let signal = ((i * 17 % 101) as f32 - 50.0) / 128.0;
        frame[0] = signal;
        frame[1] = -signal;
        frame[2] = signal;
    }
    let mut passthrough = vec![f32::NAN; input.len()];
    assert_eq!(
        monitor
            .process(
                &input,
                &mut passthrough,
                &ProcessContext::new(48_000, frames)
            )
            .unwrap(),
        frames
    );
    assert_eq!(input, passthrough);
    let data = monitor
        .get_data()
        .unwrap()
        .downcast::<LoudnessData>()
        .unwrap();
    assert!(data.correlation_samples_seen > 0);
    assert_eq!(data.correlation_matrix.len(), channels * channels);
    assert!((data.correlation_matrix[1] + 1.0).abs() < 0.001);
    assert!((data.correlation_matrix[2] - 1.0).abs() < 0.001);
    // The spider observes chain output, even when a later node changes width.
    let snapshot =
        sotf_audio_player_gpui::components::plugins::spatial_spider::SpatialSpiderSnapshot {
            loudness: Some(data.clone()),
            ui: Default::default(),
        };
    let layout =
        sotf_audio_player_gpui::components::plugins::spatial_spider::resolve_speaker_config(
            &snapshot,
            Some("7.1.4"),
        )
        .unwrap();
    assert_eq!(layout.total_channels, channels);

    let mut input_monitor =
        create_plugin("loudness_monitor", &configs[0].parameters, 2, 48_000.0).unwrap();
    input_monitor.initialize(48_000.0).unwrap();
    let stereo = vec![0.125; frames * 2];
    let mut output = stereo.clone();
    input_monitor
        .process(&stereo, &mut output, &ProcessContext::new(48_000, frames))
        .unwrap();
    let data = input_monitor
        .get_data()
        .unwrap()
        .downcast::<LoudnessData>()
        .unwrap();
    assert_eq!(data.correlation_samples_seen, 0);
}
