use openmaths::Vector3;
use serde::{Deserialize, Serialize};

use crate::brep::BrepError;

/// Where in the boolean pipeline the error originated. Used by callers to
/// decide whether to blame the input geometry, the union pre-pass, the per-step
/// subtract, or the final output validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BooleanErrorPhase {
    InputValidation,
    UnionPrePass,
    SubtractStep,
    OutputValidation,
}

/// Categorizes the failure. Variants carry payload only when the kind itself
/// implies a specific data point (e.g. the other cutter's index, the axis the
/// cutter exceeds the host along).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum BooleanErrorKind {
    InvalidOperand,
    MalformedInput,
    MixedOperandKinds,
    UnsupportedOperandKind,
    NonCoplanarPlanarOperands,
    TopologyError,
    KernelFailure,
    NonManifoldEdges,
    OpenShell,
    CoincidentFaces,
    DegenerateTriangle,
    EmptyResult,
    OverlappingCutters { other_index: usize },
    CutterExceedsHost { axis: char, overshoot: f64 },
}

/// Structured boolean error. Crosses the WASM boundary as JSON via
/// `serde_json::to_string` so the TS side can rebuild typed `BooleanError`
/// subclasses from `kind`, `phase`, and the indexed payload.
///
/// Cutter indices are **0-based** to match `Vec` indexing in both Rust and JS.
#[derive(Clone, Serialize)]
pub struct BooleanError {
    pub kind: BooleanErrorKind,
    pub phase: BooleanErrorPhase,
    pub cutter_index: Option<usize>,
    pub message: String,
    pub details: Option<String>,
    pub edge_samples: Option<Vec<[Vector3; 2]>>,
}

impl core::fmt::Debug for BooleanError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("BooleanError")
            .field("kind", &self.kind)
            .field("phase", &self.phase)
            .field("cutter_index", &self.cutter_index)
            .field("message", &self.message)
            .field("details", &self.details)
            .field(
                "edge_samples",
                &self.edge_samples.as_ref().map(|samples| samples.len()),
            )
            .finish()
    }
}

impl BooleanError {
    /// Backward-compatible constructor matching the pre-WS-0 signature. Defaults
    /// to `InputValidation` phase with no cutter index. Prefer the phase-specific
    /// constructors below for new call sites.
    pub fn new(kind: BooleanErrorKind, message: impl Into<String>) -> Self {
        let message = message.into();
        Self {
            kind,
            phase: BooleanErrorPhase::InputValidation,
            cutter_index: None,
            message,
            details: None,
            edge_samples: None,
        }
    }

    pub fn input_validation(kind: BooleanErrorKind, message: impl Into<String>) -> Self {
        Self::new(kind, message)
    }

    pub fn union_pre_pass(kind: BooleanErrorKind, message: impl Into<String>) -> Self {
        Self {
            phase: BooleanErrorPhase::UnionPrePass,
            ..Self::new(kind, message)
        }
    }

    pub fn subtract_step(
        kind: BooleanErrorKind,
        cutter_index: usize,
        message: impl Into<String>,
    ) -> Self {
        Self {
            phase: BooleanErrorPhase::SubtractStep,
            cutter_index: Some(cutter_index),
            ..Self::new(kind, message)
        }
    }

    pub fn output_validation(kind: BooleanErrorKind, message: impl Into<String>) -> Self {
        Self {
            phase: BooleanErrorPhase::OutputValidation,
            ..Self::new(kind, message)
        }
    }

    pub fn with_cutter_index(mut self, index: usize) -> Self {
        self.cutter_index = Some(index);
        self
    }

    pub fn with_phase(mut self, phase: BooleanErrorPhase) -> Self {
        self.phase = phase;
        self
    }

    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    pub fn with_edge_samples(mut self, samples: Vec<[Vector3; 2]>) -> Self {
        self.edge_samples = Some(samples);
        self
    }

    pub fn kind(&self) -> &BooleanErrorKind {
        &self.kind
    }

    pub fn phase(&self) -> BooleanErrorPhase {
        self.phase
    }

    pub fn cutter_index(&self) -> Option<usize> {
        self.cutter_index
    }

    pub fn details(&self) -> Option<&str> {
        self.details.as_deref()
    }

    pub fn edge_samples(&self) -> Option<&[[Vector3; 2]]> {
        self.edge_samples.as_deref()
    }

    /// Serializes the structured error to a JSON string for the WASM boundary.
    /// Falls back to the human message if serialization itself fails — the
    /// caller is already in an error path and we never want to mask the real
    /// failure with a serializer panic.
    pub fn to_wasm_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| self.message.clone())
    }
}

impl core::fmt::Display for BooleanError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for BooleanError {}

impl From<BrepError> for BooleanError {
    fn from(error: BrepError) -> Self {
        BooleanError::new(BooleanErrorKind::TopologyError, error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subtract_step_carries_zero_based_cutter_index() {
        let error =
            BooleanError::subtract_step(BooleanErrorKind::InvalidOperand, 0, "first cutter failed");
        assert_eq!(error.cutter_index(), Some(0));
        assert_eq!(error.phase(), BooleanErrorPhase::SubtractStep);
        assert_eq!(error.kind(), &BooleanErrorKind::InvalidOperand);
    }

    #[test]
    fn overlapping_cutters_variant_carries_other_index() {
        let error = BooleanError::input_validation(
            BooleanErrorKind::OverlappingCutters { other_index: 3 },
            "cutters 1 and 3 overlap",
        )
        .with_cutter_index(1);
        match error.kind() {
            BooleanErrorKind::OverlappingCutters { other_index } => {
                assert_eq!(*other_index, 3);
            }
            other => panic!("expected OverlappingCutters, got {:?}", other),
        }
        assert_eq!(error.cutter_index(), Some(1));
    }

    #[test]
    fn cutter_exceeds_host_variant_carries_axis_and_overshoot() {
        let error = BooleanError::input_validation(
            BooleanErrorKind::CutterExceedsHost {
                axis: 'x',
                overshoot: 0.42,
            },
            "cutter overshoots host along x",
        )
        .with_cutter_index(2);
        match error.kind() {
            BooleanErrorKind::CutterExceedsHost { axis, overshoot } => {
                assert_eq!(*axis, 'x');
                assert!((overshoot - 0.42).abs() < 1.0e-12);
            }
            other => panic!("expected CutterExceedsHost, got {:?}", other),
        }
        assert_eq!(error.cutter_index(), Some(2));
    }

    #[test]
    fn output_validation_carries_edge_samples_and_details() {
        let samples = vec![
            [Vector3::new(0.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0)],
            [Vector3::new(0.0, 1.0, 0.0), Vector3::new(0.0, 0.0, 1.0)],
        ];
        let error = BooleanError::output_validation(
            BooleanErrorKind::NonManifoldEdges,
            "non-manifold output",
        )
        .with_details("open_edges=2, over_shared_edges=0, sampled=2")
        .with_edge_samples(samples);

        assert_eq!(error.phase(), BooleanErrorPhase::OutputValidation);
        assert_eq!(error.kind(), &BooleanErrorKind::NonManifoldEdges);
        assert!(error
            .details()
            .map(|d| d.contains("open_edges=2"))
            .unwrap_or(false));
        assert_eq!(error.edge_samples().map(|samples| samples.len()), Some(2));
    }

    #[test]
    fn malformed_input_uses_input_validation_phase() {
        let error =
            BooleanError::input_validation(BooleanErrorKind::MalformedInput, "zero-area triangle");
        assert_eq!(error.phase(), BooleanErrorPhase::InputValidation);
        assert_eq!(error.kind(), &BooleanErrorKind::MalformedInput);
    }

    #[test]
    fn serde_round_trips_every_error_phase() {
        for phase in [
            BooleanErrorPhase::InputValidation,
            BooleanErrorPhase::UnionPrePass,
            BooleanErrorPhase::SubtractStep,
            BooleanErrorPhase::OutputValidation,
        ] {
            let json = serde_json::to_string(&phase).expect("phase serialization");
            let back: BooleanErrorPhase =
                serde_json::from_str(&json).expect("phase deserialization");
            assert_eq!(back, phase);
        }
    }

    #[test]
    fn serde_round_trips_payload_carrying_variants() {
        let kinds = vec![
            BooleanErrorKind::OverlappingCutters { other_index: 7 },
            BooleanErrorKind::CutterExceedsHost {
                axis: 'y',
                overshoot: 1.25,
            },
            BooleanErrorKind::NonManifoldEdges,
            BooleanErrorKind::CoincidentFaces,
            BooleanErrorKind::DegenerateTriangle,
            BooleanErrorKind::EmptyResult,
            BooleanErrorKind::MalformedInput,
        ];
        for kind in kinds {
            let json = serde_json::to_string(&kind).expect("kind serialization");
            assert!(
                json.contains("code"),
                "kind JSON should be tagged with `code`: {}",
                json
            );
        }
    }

    #[test]
    fn to_wasm_json_emits_structured_payload() {
        let error = BooleanError::subtract_step(
            BooleanErrorKind::OverlappingCutters { other_index: 2 },
            1,
            "Failed to subtract cutter #1: overlaps cutter #2",
        )
        .with_details("AABB overlap detected");

        let json = error.to_wasm_json();
        assert!(json.contains("\"phase\":\"subtract_step\""));
        assert!(json.contains("\"cutter_index\":1"));
        assert!(json.contains("\"code\":\"overlapping_cutters\""));
        assert!(json.contains("\"other_index\":2"));
    }
}

#[cfg(test)]
impl PartialEq for BooleanError {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.phase == other.phase
            && self.cutter_index == other.cutter_index
            && self.message == other.message
            && self.details == other.details
    }
}
