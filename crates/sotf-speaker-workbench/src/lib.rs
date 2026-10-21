//! Offline workbench: Sonium owns passive circuits; SOTF owns active crossover DSP.
//!
//! Active predictions retain the imported responses' shared voltage/distance reference.
//! They assume ideal voltage-source amplifiers and do not predict amplifier loading,
//! driver protection, room sound, or safe routing to physical drivers.

use num_complex::Complex64;
use serde::{Deserialize, Serialize};
pub use sonium_speaker_designer::{
    Crossover, Curve, CurveKind, DesignerError, Driver, Evaluation, Project, Sample, storage,
};
use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_plugin_crossover::{CrossoverPlugin, CrossoverPluginParams, CrossoverTopology};
use std::f64::consts::TAU;

#[cfg(feature = "desktop")]
pub mod desktop;

/// Separate two-way LR24 design; passive component values never determine this cutoff.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ActiveCrossoverSettings {
    pub sample_rate_hz: u32,
    pub frequency_hz: f64,
}

impl Default for ActiveCrossoverSettings {
    fn default() -> Self {
        Self {
            sample_rate_hz: 48_000,
            frequency_hz: 2_000.0,
        }
    }
}

fn invalid(message: impl Into<String>) -> DesignerError {
    DesignerError::Invalid(message.into())
}

impl ActiveCrossoverSettings {
    /// Validated native plugin parameters; channel 0 is low, channel 1 is high.
    /// The sample rate remains part of these settings, not the plugin parameter schema.
    pub fn plugin_params(&self) -> Result<CrossoverPluginParams, DesignerError> {
        if !(8_000..=192_000).contains(&self.sample_rate_hz)
            || !self.frequency_hz.is_finite()
            || !(20.0..=20_000.0).contains(&self.frequency_hz)
            || self.frequency_hz >= f64::from(self.sample_rate_hz) * 0.495
        {
            return Err(invalid(
                "Active crossover requires 8–192 kHz sample rate and a finite 20–20000 Hz cutoff below 0.495 × sample rate",
            ));
        }
        let params = CrossoverPluginParams {
            crossover_type: "LR24".into(),
            frequency: self.frequency_hz,
            output: "both".into(),
            topology: Some(CrossoverTopology::Bands),
            band_count: Some(2),
            extra_frequencies: Vec::new(),
            fir_taps: None,
            channel_frequencies_hz: Vec::new(),
            channel_modes: None,
        };
        let mut plugin = CrossoverPlugin::from_params(1, &params).map_err(invalid)?;
        plugin
            .initialize(f64::from(self.sample_rate_hz))
            .map_err(invalid)?;
        Ok(params)
    }

    /// Complex electrical transfers measured from the actual plugin's impulse output.
    /// This is an offline finite-impulse approximation, not a second crossover implementation.
    pub fn electrical_response(
        &self,
        frequencies_hz: &[f64],
    ) -> Result<Vec<[Complex64; 2]>, DesignerError> {
        let params = self.plugin_params()?;
        let sample_rate = f64::from(self.sample_rate_hz);
        if frequencies_hz
            .iter()
            .any(|f| !f.is_finite() || *f <= 0.0 || *f >= sample_rate / 2.0)
        {
            return Err(invalid(
                "Response frequencies must be finite, positive, and below Nyquist",
            ));
        }
        let frames = (32.0 * sample_rate / self.frequency_hz).ceil().max(4096.0) as usize;
        let mut plugin = CrossoverPlugin::from_params(1, &params).map_err(invalid)?;
        plugin.initialize(sample_rate).map_err(invalid)?;
        let mut impulse = vec![0.0_f32; frames];
        impulse[0] = 1.0;
        let mut output = vec![0.0_f32; frames * 2];
        for (input, output) in impulse.chunks(256).zip(output.chunks_mut(512)) {
            let written = plugin
                .process(
                    input,
                    output,
                    &ProcessContext::new(self.sample_rate_hz, input.len()),
                )
                .map_err(invalid)?;
            if written != input.len() || output.iter().any(|value| !value.is_finite()) {
                return Err(invalid(
                    "Crossover plugin produced incomplete or non-finite impulse output",
                ));
            }
        }
        if output[output.len() - 512..]
            .iter()
            .any(|sample| sample.abs() > 1e-8)
        {
            return Err(invalid(
                "Crossover impulse has not settled within the preview window",
            ));
        }
        Ok(frequencies_hz
            .iter()
            .map(|frequency| {
                let step = Complex64::from_polar(1.0, -TAU * frequency / sample_rate);
                let mut phase = Complex64::new(1.0, 0.0);
                let mut branches = [Complex64::new(0.0, 0.0); 2];
                for frame in output.as_chunks::<2>().0 {
                    branches[0] += phase * f64::from(frame[0]);
                    branches[1] += phase * f64::from(frame[1]);
                    phase *= step;
                }
                branches
            })
            .collect())
    }

    /// Predict active acoustic summation from imported complex responses only.
    /// Input impedance is unavailable for this mode; its evaluation vectors stay empty.
    pub fn evaluate(&self, project: &Project) -> Result<Evaluation, DesignerError> {
        self.plugin_params()?;
        project.validate_draft()?;
        let woofer = project
            .woofer
            .response
            .as_ref()
            .ok_or_else(|| invalid("Import woofer response before active preview"))?;
        let tweeter = project
            .tweeter
            .response
            .as_ref()
            .ok_or_else(|| invalid("Import tweeter response before active preview"))?;
        let low = woofer.samples[0]
            .frequency_hz
            .max(tweeter.samples[0].frequency_hz);
        let high = woofer
            .samples
            .last()
            .unwrap()
            .frequency_hz
            .min(tweeter.samples.last().unwrap().frequency_hz)
            .min(f64::from(self.sample_rate_hz) * 0.499);
        if low >= high {
            return Err(invalid(
                "Driver responses must share a nonzero interval below Nyquist",
            ));
        }
        let frequencies_hz: Vec<_> = (0..256)
            .map(|index| match index {
                0 => low,
                255 => high,
                _ => (low.ln() + (high.ln() - low.ln()) * f64::from(index) / 255.0)
                    .exp()
                    .clamp(low, high),
            })
            .collect();
        let electrical = self.electrical_response(&frequencies_hz)?;
        let mut result = Evaluation::default();
        for (&frequency, transfer) in frequencies_hz.iter().zip(electrical) {
            let branch = |driver: &Driver,
                          response: &Curve,
                          transfer: Complex64|
             -> Result<Complex64, DesignerError> {
                let phase = -TAU * frequency * driver.delay_ms * 1e-3;
                if !phase.is_finite() {
                    return Err(invalid("Driver delay phase exceeds numerical range"));
                }
                Ok(response.interpolate(frequency, CurveKind::Response)?
                    * transfer
                    * Complex64::from_polar(if driver.invert { -1.0 } else { 1.0 }, phase))
            };
            let low = branch(&project.woofer, woofer, transfer[0])?;
            let high = branch(&project.tweeter, tweeter, transfer[1])?;
            let sum = low + high;
            if [low, high, sum].iter().any(|value| {
                !value.re.is_finite() || !value.im.is_finite() || !value.norm().is_finite()
            }) {
                return Err(invalid(
                    "Active acoustic preview produced non-finite output",
                ));
            }
            let db = |value: Complex64| 20.0 * value.norm().max(1e-15).log10();
            result.woofer_db.push(db(low));
            result.tweeter_db.push(db(high));
            result.sum_db.push(db(sum));
            result.sum_phase_deg.push(sum.arg().to_degrees());
        }
        result.frequencies_hz = frequencies_hz;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measured_project() -> Project {
        let mut project = Project::default();
        for driver in [&mut project.woofer, &mut project.tweeter] {
            driver.response =
                Some(Curve::parse("100 85 0\n10000 85 0", CurveKind::Response, "fixture").unwrap());
        }
        project
    }

    #[test]
    fn rejects_invalid_settings_and_out_of_band_queries() {
        for settings in [
            ActiveCrossoverSettings {
                sample_rate_hz: 0,
                ..Default::default()
            },
            ActiveCrossoverSettings {
                frequency_hz: f64::NAN,
                ..Default::default()
            },
            ActiveCrossoverSettings {
                sample_rate_hz: 8000,
                frequency_hz: 4000.0,
            },
        ] {
            assert!(settings.plugin_params().is_err());
        }
        for frequency in [0.0, 24000.0, f64::NAN] {
            assert!(
                ActiveCrossoverSettings::default()
                    .electrical_response(&[frequency])
                    .is_err()
            );
        }
        let params = ActiveCrossoverSettings::default().plugin_params().unwrap();
        let encoded = serde_json::to_string(&params).unwrap();
        let decoded: CrossoverPluginParams = serde_json::from_str(&encoded).unwrap();
        assert_eq!(
            CrossoverPlugin::from_params(1, &decoded)
                .unwrap()
                .output_channels(),
            2
        );
    }

    #[test]
    fn active_preview_requires_overlap_and_retains_measurement_reference() {
        let settings = ActiveCrossoverSettings::default();
        let mut project = measured_project();
        let result = settings.evaluate(&project).unwrap();
        assert_eq!(result.frequencies_hz[0], 100.0);
        assert_eq!(*result.frequencies_hz.last().unwrap(), 10000.0);
        assert!(result.sum_db.iter().all(|db| (db - 85.0).abs() < 1e-4));
        assert!(result.input_impedance_ohm.is_empty());
        project.reference_voltage_v = 1.0;
        project.reference_distance_m = 2.0;
        assert_eq!(settings.evaluate(&project).unwrap().sum_db, result.sum_db);
        project.tweeter.response = Some(
            Curve::parse("11000 85 0\n12000 85 0", CurveKind::Response, "no overlap").unwrap(),
        );
        assert!(settings.evaluate(&project).is_err());
        assert!(settings.evaluate(&Project::default()).is_err());
    }

    #[test]
    fn electrical_preview_matches_actual_plugin_steady_state() {
        let settings = ActiveCrossoverSettings::default();
        let frequency = 2000.0;
        let transfer = settings.electrical_response(&[frequency]).unwrap()[0];
        let mut plugin =
            CrossoverPlugin::from_params(1, &settings.plugin_params().unwrap()).unwrap();
        plugin.initialize(48000.0).unwrap();
        let input: Vec<_> = (0..8192)
            .map(|n| (TAU * frequency * f64::from(n) / 48000.0).cos() as f32)
            .collect();
        let mut output = vec![0.0; input.len() * 2];
        for (input, output) in input.chunks(256).zip(output.chunks_mut(512)) {
            plugin
                .process(input, output, &ProcessContext::new(48000, input.len()))
                .unwrap();
        }
        for n in 4096..8192 {
            let phase = Complex64::from_polar(1.0, TAU * frequency * n as f64 / 48000.0);
            for band in 0..2 {
                assert!(
                    (f64::from(output[2 * n + band]) - (transfer[band] * phase).re).abs() < 2e-5
                );
            }
        }
    }

    #[test]
    fn driver_delay_and_polarity_remain_complex_operations() {
        let settings = ActiveCrossoverSettings::default();
        let project = measured_project();
        let baseline = settings.evaluate(&project).unwrap();
        let mut inverted = project.clone();
        inverted.tweeter.invert = true;
        let changed = settings.evaluate(&inverted).unwrap();
        assert_eq!(baseline.tweeter_db, changed.tweeter_db);
        assert!(
            baseline
                .sum_db
                .iter()
                .zip(&changed.sum_db)
                .any(|(a, b)| (a - b).abs() > 10.0)
        );
        inverted.tweeter.invert = false;
        inverted.tweeter.delay_ms = 0.25;
        let delayed = settings.evaluate(&inverted).unwrap();
        assert!(
            baseline
                .sum_db
                .iter()
                .zip(delayed.sum_db)
                .any(|(a, b)| (a - b).abs() > 3.0)
        );
        assert!(project.evaluate().is_err()); // Passive mode still requires measured impedance.
    }
}
