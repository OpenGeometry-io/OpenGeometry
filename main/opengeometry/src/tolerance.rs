//! Modeling tolerance hierarchy (debt item D2).
//!
//! A single global scalar cannot express that distances, areas, and angles
//! tolerate different errors, nor that geometry produced by an operation may
//! carry its own tolerance. Historically this kernel scattered ad-hoc epsilon
//! constants across modules (`EXTRUDE_EPSILON`, `CAP_ALIGNMENT_THRESHOLD`,
//! `STEP_LENGTH_EPSILON`, the boolean auto-scale in `BooleanOptions`, …). This
//! module is the single tolerance model that all comparison-based operations
//! consult, so near-coincident geometry is handled consistently instead of
//! per-operation.
//!
//! Conventions:
//! - Lengths are in **model units** (typically metres in OpenGeometry-using
//!   apps; see the unit model, debt D8).
//! - Angles are in **radians**.
//!
//! This is the interface other foundational items (D1 geometry, D4 booleans,
//! D5 validity, D7 profiles) are expected to consume. Per-entity tolerance
//! (per-vertex / per-edge) layers on top via [`ToleranceContext::effective`].

use openmaths::Vector3;
use serde::{Deserialize, Serialize};

/// Absolute floor for any auto-derived length tolerance, in model units.
/// Below this, floating-point noise dominates and comparisons are meaningless.
pub const MODELING_TOLERANCE_FLOOR: f64 = 1.0e-9;

/// Relative factor applied to an operand's bounding-box diagonal when a length
/// tolerance is auto-derived. Matches the historical boolean default so the
/// auto-scale path is byte-for-byte unchanged.
pub const DEFAULT_RELATIVE_TOLERANCE: f64 = 1.0e-8;

/// Default angular tolerance in radians (~5.7e-2 degrees). Two directions
/// whose angle is within this are treated as parallel.
pub const DEFAULT_ANGULAR_TOLERANCE: f64 = 1.0e-3;

/// Global modeling tolerances consulted by all comparison-based operations.
///
/// - `confusion` is the distance below which two points are considered the
///   same point. It is the core length comparison threshold (OCC's
///   `Precision::Confusion`).
/// - `modeling` is the working/build tolerance (`>= confusion`); it is the
///   default per-entity tolerance assigned to geometry an operation produces.
/// - `angular` is the angle (radians) below which two directions are parallel
///   (OCC's `Precision::Angular`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToleranceContext {
    pub confusion: f64,
    pub modeling: f64,
    pub angular: f64,
}

impl Default for ToleranceContext {
    fn default() -> Self {
        Self {
            confusion: MODELING_TOLERANCE_FLOOR,
            modeling: MODELING_TOLERANCE_FLOOR,
            angular: DEFAULT_ANGULAR_TOLERANCE,
        }
    }
}

impl ToleranceContext {
    /// Builds a context from an explicit modeling tolerance, using it for both
    /// `modeling` and `confusion` (clamped to the floor).
    pub fn from_modeling(modeling: f64) -> Self {
        let modeling = modeling.max(MODELING_TOLERANCE_FLOOR);
        Self {
            confusion: modeling,
            modeling,
            angular: DEFAULT_ANGULAR_TOLERANCE,
        }
    }

    /// Auto-derives a modeling tolerance from a bounding-box diagonal, using the
    /// same `diagonal * 1e-8` (floored at `1e-9`) rule the boolean pipeline has
    /// always used. Returned as a bare scalar so existing call sites can adopt
    /// it without behavioural change.
    pub fn derived_modeling_for_diagonal(diagonal: f64) -> f64 {
        if !diagonal.is_finite() {
            return MODELING_TOLERANCE_FLOOR;
        }
        (diagonal * DEFAULT_RELATIVE_TOLERANCE).max(MODELING_TOLERANCE_FLOOR)
    }

    /// Builds a context whose `modeling`/`confusion` are auto-derived from a
    /// bounding-box diagonal.
    pub fn from_diagonal(diagonal: f64) -> Self {
        Self::from_modeling(Self::derived_modeling_for_diagonal(diagonal))
    }

    /// The effective length tolerance for a comparison, taking an optional
    /// per-entity override (per-vertex / per-edge tolerance, D2 follow-on) into
    /// account. The looser of the two wins, since either party tolerating the
    /// error makes the comparison pass.
    pub fn effective(&self, entity_tolerance: Option<f64>) -> f64 {
        match entity_tolerance {
            Some(t) => self.confusion.max(t),
            None => self.confusion,
        }
    }

    /// Whether a (non-negative) distance is within confusion tolerance.
    pub fn distance_within(&self, distance: f64) -> bool {
        distance <= self.confusion
    }

    /// Whether two scalar lengths are equal within confusion tolerance.
    pub fn lengths_equal(&self, a: f64, b: f64) -> bool {
        (a - b).abs() <= self.confusion
    }

    /// Whether two points are coincident within confusion tolerance.
    /// Uses squared distance to avoid a `sqrt`.
    pub fn points_coincident(&self, a: Vector3, b: Vector3) -> bool {
        let dx = a.x - b.x;
        let dy = a.y - b.y;
        let dz = a.z - b.z;
        (dx * dx + dy * dy + dz * dz) <= self.confusion * self.confusion
    }

    /// Cosine of the angular tolerance — the threshold a direction dot product
    /// must meet to count as aligned.
    pub fn angular_cos(&self) -> f64 {
        self.angular.cos()
    }

    /// Whether two unit directions are parallel within angular tolerance
    /// (either same or opposite sense).
    pub fn directions_parallel(&self, cos_angle: f64) -> bool {
        cos_angle.abs() >= self.angular_cos()
    }

    /// Whether two unit directions are aligned (same sense) within angular
    /// tolerance.
    pub fn directions_aligned(&self, cos_angle: f64) -> bool {
        cos_angle >= self.angular_cos()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_floored() {
        let t = ToleranceContext::default();
        assert_eq!(t.confusion, MODELING_TOLERANCE_FLOOR);
        assert_eq!(t.modeling, MODELING_TOLERANCE_FLOOR);
    }

    #[test]
    fn from_modeling_clamps_to_floor() {
        let t = ToleranceContext::from_modeling(0.0);
        assert_eq!(t.modeling, MODELING_TOLERANCE_FLOOR);
        assert_eq!(t.confusion, MODELING_TOLERANCE_FLOOR);
    }

    #[test]
    fn derived_matches_historical_boolean_rule() {
        // Historical rule: (diagonal * 1e-8).max(1e-9).
        assert_eq!(
            ToleranceContext::derived_modeling_for_diagonal(10.0),
            (10.0_f64 * 1.0e-8).max(1.0e-9)
        );
        // Small diagonal floors out.
        assert_eq!(
            ToleranceContext::derived_modeling_for_diagonal(0.01),
            1.0e-9
        );
        // Non-finite is safe.
        assert_eq!(
            ToleranceContext::derived_modeling_for_diagonal(f64::INFINITY),
            MODELING_TOLERANCE_FLOOR
        );
    }

    #[test]
    fn effective_takes_the_looser_tolerance() {
        let t = ToleranceContext::from_modeling(1.0e-6);
        assert_eq!(t.effective(None), 1.0e-6);
        assert_eq!(t.effective(Some(1.0e-3)), 1.0e-3);
        assert_eq!(t.effective(Some(1.0e-9)), 1.0e-6);
    }

    #[test]
    fn points_coincident_respects_confusion() {
        let t = ToleranceContext::from_modeling(1.0e-3);
        let a = Vector3::new(0.0, 0.0, 0.0);
        let near = Vector3::new(5.0e-4, 0.0, 0.0);
        let far = Vector3::new(2.0e-3, 0.0, 0.0);
        assert!(t.points_coincident(a, near));
        assert!(!t.points_coincident(a, far));
    }

    #[test]
    fn directions_parallel_handles_both_senses() {
        let t = ToleranceContext::default();
        assert!(t.directions_parallel(1.0));
        assert!(t.directions_parallel(-1.0));
        assert!(t.directions_aligned(1.0));
        assert!(!t.directions_aligned(-1.0));
        assert!(!t.directions_parallel(0.0));
    }
}
