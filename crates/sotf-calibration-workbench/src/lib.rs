//! Presentation contracts only. The host owns inputs, validation and study execution.
use sotf_audio_player::autoeq::{PipelineStepId, PipelineStepStatus};
use sotf_audio_player::room_eq_types::RoomEqOptimizerConfig;
use sotf_audio_player::ui_models::room_eq::RoomEqScreenModel;

#[cfg(feature = "desktop")]
pub mod desktop;
pub mod measurement_preview;
#[cfg(feature = "desktop")]
pub mod room_layout;

/// User decisions, separate from the engine's conditional execution stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DesignerStep {
    #[default]
    Data,
    Checks,
    Goals,
    Constraints,
    Advice,
    Study,
}

impl DesignerStep {
    pub const ALL: [Self; 6] = [
        Self::Data,
        Self::Checks,
        Self::Goals,
        Self::Constraints,
        Self::Advice,
        Self::Study,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Data => "Data",
            Self::Checks => "Checks",
            Self::Goals => "Goals",
            Self::Constraints => "Constraints",
            Self::Advice => "Advice",
            Self::Study => "Study",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DesignerCommand {
    MinimumFrequency(f64),
    MaximumFrequency(f64),
    MaximumCut(f64),
    MaximumBoost(f64),
    TargetSlope(f64),
    BassShelf(f64),
    OpenData,
    OpenChecks,
    OpenParameters,
    OpenResults,
    UseExplicitSettings,
    RunStudy,
    CancelStudy,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StageSnapshot {
    pub step: PipelineStepId,
    pub status: &'static str,
    pub summary: String,
}

/// Bounded presentation data; response previews retain only display samples.
#[derive(Debug, Clone, PartialEq)]
pub struct DesignerSnapshot {
    pub stages: Vec<StageSnapshot>,
    pub current_stage: Option<PipelineStepId>,
    pub minimum_frequency_hz: f64,
    pub maximum_frequency_hz: f64,
    pub maximum_cut_db: f64,
    pub maximum_boost_db: f64,
    pub running: bool,
    pub has_measurements: bool,
    pub error: Option<String>,
    pub channel_names: Vec<String>,
    pub measurement_previews: measurement_preview::MeasurementPreviews,
    pub has_phase: bool,
    pub target_enabled: bool,
    pub target_shape: String,
    pub target_slope: f64,
    pub bass_shelf_db: f64,
    pub result_count: usize,
    /// Covers advanced settings too, so edits outside these panels clear review.
    pub configuration_identity: String,
    pub explicit_configuration: bool,
    /// Supplied by the host from its complete canonical study-input snapshot.
    pub input_identity: String,
    /// Solver-reported fraction in 0..=1; absent when idle or unavailable.
    pub reported_progress: Option<f32>,
}

impl DesignerSnapshot {
    pub fn from_model(room: &RoomEqScreenModel) -> Self {
        let config = &room.optimizer_config;
        Self {
            stages: PipelineStepId::ALL
                .iter()
                .map(|&step| StageSnapshot {
                    step,
                    status: match room.step_history.get(&step) {
                        Some(PipelineStepStatus::Started | PipelineStepStatus::InProgress)
                            if room.is_optimizing() =>
                        {
                            "Running"
                        }
                        Some(PipelineStepStatus::Started | PipelineStepStatus::InProgress) => {
                            "Stopped before completion"
                        }
                        Some(PipelineStepStatus::Completed) => "Completed in last study",
                        Some(PipelineStepStatus::Skipped) => "Skipped in last study",
                        None => "Not observed in a study",
                    },
                    summary: stage_summary(step, config, room.channel_measurements.len()),
                })
                .collect(),
            current_stage: room.current_pipeline_step,
            minimum_frequency_hz: config.min_freq,
            maximum_frequency_hz: config.max_freq,
            maximum_cut_db: -config.min_db,
            maximum_boost_db: config.max_db,
            running: room.is_optimizing(),
            has_measurements: !room.channel_measurements.is_empty(),
            error: room.error_message.clone(),
            channel_names: room
                .channel_measurements
                .iter()
                .map(|m| m.channel_name.clone())
                .collect(),
            measurement_previews: measurement_preview::MeasurementPreviews::from_model(room),
            has_phase: room.has_phase_data(),
            target_enabled: config.target_response.enabled,
            target_shape: config.target_response.shape.clone(),
            target_slope: config.target_response.slope_db_per_octave,
            bass_shelf_db: config.target_response.bass_shelf_db,
            result_count: room.channel_results.len(),
            configuration_identity: format!("{config:?}"),
            explicit_configuration: room.wizard_mode
                == sotf_audio_player::room_eq_types::RoomEqWizardMode::Full,
            input_identity: String::new(),
            reported_progress: room
                .is_optimizing()
                .then_some(room.overall_progress)
                .filter(|v| v.is_finite())
                .map(|v| v.clamp(0.0, 1.0)),
        }
    }

    pub fn can_run(&self) -> bool {
        self.has_measurements
            && self.explicit_configuration
            && !self.input_identity.is_empty()
            && !self.running
            && [
                self.minimum_frequency_hz,
                self.maximum_frequency_hz,
                self.maximum_cut_db,
                self.maximum_boost_db,
            ]
            .iter()
            .all(|n| n.is_finite())
            && self.minimum_frequency_hz < self.maximum_frequency_hz
    }

    /// Text-only handoff; no measurement samples or project paths leave the app.
    pub fn advice_brief(&self) -> String {
        format!(
            "Calibration draft\nChannels: {}\nPhase data present: {}\nTarget: {} (enabled: {}), stored slope {:.2} dB/oct (custom shape only), bass shelf {:.1} dB\nCorrection: {:.0}–{:.0} Hz; maximum cut {:.1} dB, boost {:.1} dB\nPlease explain tradeoffs before proposing supported parameter changes. Soundstage/spaciousness are not scored by this panel. These are draft settings, not measured improvements. Channel layout is schematic; no room coordinates supplied.\n",
            self.channel_names.join(", "),
            self.has_phase,
            self.target_shape,
            self.target_enabled,
            self.target_slope,
            self.bass_shelf_db,
            self.minimum_frequency_hz,
            self.maximum_frequency_hz,
            self.maximum_cut_db,
            self.maximum_boost_db
        )
    }
}

#[cfg(feature = "desktop")]
fn stage_description(step: PipelineStepId) -> &'static str {
    match step {
        PipelineStepId::ConfigPreparation => {
            "Prepare effective configuration and measurement inputs. Simple presets are resolved when the study starts."
        }
        PipelineStepId::Validation => {
            "Validate the prepared request before optimization. Missing or incompatible inputs remain errors."
        }
        PipelineStepId::TopologyRouteSelection => {
            "Choose the topology-specific or generic channel workflow from the prepared system configuration."
        }
        PipelineStepId::TopologyWorkflowExecution => {
            "Run the selected topology workflow. This is conditional; the generic route is an alternative."
        }
        PipelineStepId::GenericChannelOptimization => {
            "Fit channel correction using the selected objective, filter model, and optimizer limits."
        }
        PipelineStepId::FirGeneration => {
            "Realize full FIR coefficients when the chosen processing mode requires them."
        }
        PipelineStepId::MixedPhaseFirGeneration => {
            "Realize the short mixed-phase FIR stage when that mode is selected."
        }
        PipelineStepId::PhaseCorrection => {
            "Apply standalone phase correction when supported and requested by the effective configuration."
        }
        PipelineStepId::TimeAlignment => {
            "Align channels using arrival information available to this Rust workflow."
        }
        PipelineStepId::SpectralAlignment => {
            "Align broad spectral balance when enabled in the effective configuration."
        }
        PipelineStepId::InterChannelTimbreMatching => {
            "Apply inter-channel timbre matching when requested by the selected workflow."
        }
        PipelineStepId::HeightChannelAlignment => {
            "Align overhead channels with role-appropriate bed references when available."
        }
        PipelineStepId::PhaseAlignment => {
            "Optimize subwoofer/main phase alignment when enabled and supported by input data."
        }
        PipelineStepId::GroupDelayOptimization => {
            "Run optional group-delay optimization when enabled in the effective configuration."
        }
        PipelineStepId::ImpulseResponseComputation => {
            "Compute pre/post impulse responses from optimization results."
        }
        PipelineStepId::ChannelMatching => {
            "Analyze and optionally correct inter-channel response deviation."
        }
        PipelineStepId::MetadataRefresh => {
            "Refresh result metadata and derived reports after processing."
        }
        PipelineStepId::SanityCheck => {
            "Check final result invariants before returning the completed study."
        }
    }
}

fn stage_summary(
    step: PipelineStepId,
    config: &RoomEqOptimizerConfig,
    channel_count: usize,
) -> String {
    match step {
        PipelineStepId::GenericChannelOptimization => format!(
            "Draft: {} · {} filters · {:.0}–{:.0} Hz · {:.1} to +{:.1} dB",
            config.algorithm,
            config.num_filters,
            config.min_freq,
            config.max_freq,
            config.min_db,
            config.max_db
        ),
        PipelineStepId::FirGeneration => format!(
            "Draft: {} · {} FIR taps · {} Hz",
            config.mode.as_str(),
            config.fir.taps,
            config.sample_rate
        ),
        PipelineStepId::MixedPhaseFirGeneration => format!(
            "Draft: {} · maximum FIR length {:.1} ms · {} Hz",
            config.mode.as_str(),
            config.mixed_phase.max_fir_length_ms,
            config.sample_rate
        ),
        PipelineStepId::PhaseAlignment => format!(
            "Draft: phase alignment {} · {:.0}–{:.0} Hz",
            if config.phase_alignment.enabled {
                "enabled"
            } else {
                "disabled"
            },
            config.phase_alignment.min_freq,
            config.phase_alignment.max_freq
        ),
        PipelineStepId::ChannelMatching => format!(
            "Draft: channel matching {} · threshold {:.1} dB",
            if config.channel_matching.enabled {
                "enabled"
            } else {
                "disabled"
            },
            config.channel_matching.threshold_db
        ),
        _ => format!(
            "{} measurement channels · draft mode {}",
            channel_count,
            config.mode.as_str()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_preserves_engine_order_limits_and_observed_status() {
        let mut model = RoomEqScreenModel::default();
        model.optimizer_config.min_freq = 35.0;
        model.optimizer_config.max_freq = 18000.0;
        model.optimizer_config.min_db = -9.0;
        model.optimizer_config.max_db = 2.0;
        model
            .step_history
            .insert(PipelineStepId::Validation, PipelineStepStatus::Completed);
        let snapshot = DesignerSnapshot::from_model(&model);
        assert_eq!(
            snapshot.stages.iter().map(|s| s.step).collect::<Vec<_>>(),
            PipelineStepId::ALL
        );
        assert_eq!(snapshot.maximum_cut_db, 9.0);
        assert_eq!(snapshot.minimum_frequency_hz, 35.0);
        assert_eq!(snapshot.maximum_frequency_hz, 18000.0);
        assert_eq!(snapshot.maximum_boost_db, 2.0);
        assert!(!snapshot.has_measurements);
        assert!(!snapshot.running);
        assert_eq!(
            snapshot
                .stages
                .iter()
                .find(|s| s.step == PipelineStepId::Validation)
                .unwrap()
                .status,
            "Completed in last study"
        );
        assert!(
            snapshot
                .stages
                .iter()
                .any(|s| s.status == "Not observed in a study")
        );
        model.optimization_status = sotf_audio_player::room_eq_types::OptimizationStatus::Running;
        model.overall_progress = 0.42;
        assert_eq!(
            DesignerSnapshot::from_model(&model).reported_progress,
            Some(0.42)
        );
        model.overall_progress = f32::NAN;
        assert_eq!(DesignerSnapshot::from_model(&model).reported_progress, None);
    }
}
