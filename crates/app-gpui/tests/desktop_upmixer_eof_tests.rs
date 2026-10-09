//! Regression for desktop playback reaching EOF with a surround rack.

use sotf_plugins::GainPlugin;
use sotf_plugins::MatrixPlugin;
use sotf_plugins::plugin::TailLength;
use sotf_plugins::plugin_upmixer::{UpmixerPlugin, UpmixerPluginParams};
use sotf_plugins::{
    DawHost, LoudnessMonitorPlugin, ParametricPluginAdapter, Plugin, ProcessContext,
    SpectrumAnalyzerPlugin,
};

fn upmixer(hr: bool) -> UpmixerPlugin {
    let params: UpmixerPluginParams = serde_json::from_value(serde_json::json!({
        "fft_size": 1024, "speaker_config": "5.1.4", "enable_hr_direct": hr,
        "enable_subharmonic_synth": false, "bypass_decorrelation": true,
        "auto_gain_enabled": false
    }))
    .unwrap();
    UpmixerPlugin::from_params(params)
}

#[test]
fn desktop_surround_rack_eof_preserves_upmixer_tail() {
    for rate in [44_100, 48_000] {
        for (block, hr, spectrum) in [(127, false, false), (1024, true, true)] {
            let mut reference = upmixer(hr);
            reference.initialize(f64::from(rate)).unwrap();
            let channels = reference.output_channels();
            assert_eq!(channels, 10);
            let mut host = DawHost::new(2, rate);
            host.add_plugin(Box::new(LoudnessMonitorPlugin::new(2).unwrap()))
                .unwrap();
            host.add_plugin(Box::new(upmixer(hr))).unwrap();
            host.add_plugin(Box::new(MatrixPlugin::new(channels, channels)))
                .unwrap();
            host.add_plugin(Box::new(ParametricPluginAdapter::new(GainPlugin::new(
                channels, 0.0,
            ))))
            .unwrap();
            if spectrum {
                host.add_plugin(Box::new(SpectrumAnalyzerPlugin::new(channels).unwrap()))
                    .unwrap();
            }
            host.add_plugin(Box::new(LoudnessMonitorPlugin::new(channels).unwrap()))
                .unwrap();
            host.build().unwrap();
            let frames = 3073;
            let mut source = vec![0.0; frames * 2];
            // A final-frame impulse puts audio in the pending WOLA tail.
            source[(frames - 1) * 2] = 0.25;
            source[(frames - 1) * 2 + 1] = 0.125;
            let mut expected = Vec::new();
            let mut actual = Vec::new();
            for input in source.chunks(block * 2) {
                let n = input.len() / 2;
                let mut output = vec![f32::NAN; n * channels];
                assert_eq!(
                    reference
                        .process(input, &mut output, &ProcessContext::new(rate, n))
                        .unwrap(),
                    n
                );
                expected.extend_from_slice(&output);
                assert_eq!(host.process(input, &mut output).unwrap(), n);
                actual.extend_from_slice(&output);
            }
            let tail_bound = match reference.tail_length() {
                TailLength::Finite(n) => n as usize,
                other => panic!("unexpected tail: {other:?}"),
            };
            let mut output = vec![0.0; reference.drain_output_frames_max() * channels];
            let mut complete = false;
            for _ in 0..1000 {
                let drained = reference
                    .drain(&mut output, &ProcessContext::new(rate, 0))
                    .unwrap();
                expected.extend_from_slice(&output[..drained.frames * channels]);
                if drained.complete {
                    complete = true;
                    break;
                }
            }
            assert!(complete, "reference drain did not terminate");
            assert!(
                expected[frames * channels..]
                    .iter()
                    .any(|sample| sample.abs() > 1.0e-5)
            );
            output.resize(host.drain_output_frames_max() * channels, 0.0);
            complete = false;
            for _ in 0..1000 {
                let drained = host
                    .drain(&mut output)
                    .expect("desktop rack must survive EOF");
                actual.extend_from_slice(&output[..drained.frames * channels]);
                if drained.complete {
                    complete = true;
                    break;
                }
            }
            assert!(complete, "desktop rack drain did not terminate");
            assert_eq!(actual.len(), expected.len());
            assert!(actual.len() / channels - frames <= tail_bound);
            let maximum = actual
                .iter()
                .zip(&expected)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0_f32, f32::max);
            assert!(actual.iter().all(|s| s.is_finite()));
            assert!(
                maximum < 1.0e-6,
                "{rate}/{block}: rack changed tail by {maximum}"
            );
            let final_drain = host.drain(&mut output).unwrap();
            assert!(final_drain.complete);
            assert_eq!(final_drain.frames, 0);
        }
    }
}

#[test]
fn subharmonic_render_cap_is_not_a_finite_response_claim() {
    let mut plugin = upmixer(false);
    plugin
        .set_parameter(
            "enable_subharmonic_synth".into(),
            sotf_plugins::ParameterValue::Bool(true),
        )
        .unwrap();
    assert_eq!(plugin.tail_length(), TailLength::Unknown);
}
