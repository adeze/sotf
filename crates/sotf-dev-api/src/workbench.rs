//! Narrow workbench commands shared by UI services and agent adapters.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Actual panels of the embedded calibration Designer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DesignerPanel {
    Data,
    Checks,
    Goals,
    Constraints,
    Advice,
    Study,
}

/// Only supported offline Room EQ configuration controls are exposed.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DesignerPatch {
    /// Lower correction frequency in Hz.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 20.0, max = 20000.0))]
    pub min_freq: Option<f64>,
    /// Upper correction frequency in Hz.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 20.0, max = 20000.0))]
    pub max_freq: Option<f64>,
    /// Nonnegative maximum boost in dB.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0, max = 25.0))]
    pub max_boost_db: Option<f64>,
    /// Nonnegative maximum cut magnitude in dB; stored internally as a negative floor.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0, max = 25.0))]
    pub max_cut_db: Option<f64>,
}

/// Mutations must bind the exact input revision returned by inspection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkbenchRequest {
    Inspect {},
    SelectDesignerPanel {
        expected_revision: String,
        panel: DesignerPanel,
    },
    SelectMeasurement {
        expected_revision: String,
        index: usize,
    },
    UpdateDesigner {
        expected_revision: String,
        patch: DesignerPatch,
    },
    RunStudy {
        expected_revision: String,
    },
    CancelStudy {
        expected_revision: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn workbench_contract_rejects_unknown_operations_and_parameters() {
        for value in [
            json!({"operation":"transfer"}),
            json!({"operation":"inspect","path":"arbitrary"}),
            json!({"operation":"run_study"}),
            json!({"operation":"update_designer","expected_revision":"r1","patch":{"gain":2}}),
            json!({"operation":"select_designer_panel","expected_revision":"r1","panel":"waterfall"}),
            json!({"operation":"select_measurement","expected_revision":"r1","index":-1}),
        ] {
            assert!(serde_json::from_value::<WorkbenchRequest>(value).is_err());
        }
        let request = WorkbenchRequest::UpdateDesigner {
            expected_revision: "r1".into(),
            patch: DesignerPatch {
                max_cut_db: Some(6.0),
                ..Default::default()
            },
        };
        let encoded = serde_json::to_value(&request).unwrap();
        assert_eq!(
            serde_json::from_value::<WorkbenchRequest>(encoded).unwrap(),
            request
        );
    }
}
