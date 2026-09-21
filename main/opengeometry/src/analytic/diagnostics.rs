use super::{BrepEnvelope, GeometryError};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ValidationReport {
    pub valid: bool,
    pub validation_level: &'static str,
    pub error: Option<GeometryError>,
}

pub fn validate_json(serialized: &str) -> ValidationReport {
    let result = BrepEnvelope::from_json(serialized);
    ValidationReport {
        valid: result.is_ok(),
        validation_level: "v2 structure and sampled geometry residuals",
        error: result.err(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytic::topology::tests::circular_face;

    #[test]
    fn diagnostics_reject_missing_links_and_detached_curves() {
        let brep = circular_face();
        assert!(validate_json(&brep.to_json().unwrap()).valid);
        let mut broken = brep.clone();
        broken.topology.halfedges[0].next = Some(u32::MAX);
        let report = validate_json(&serde_json::to_string(&broken).unwrap());
        assert!(!report.valid);
        assert!(report.error.is_some());
        let mut broken = brep;
        if let crate::analytic::CurveGeometry::Circle { radius, .. } =
            &mut broken.geometry.curves[0]
        {
            *radius *= 1.1;
        }
        assert!(!validate_json(&serde_json::to_string(&broken).unwrap()).valid);
        assert!(!validate_json("{\"schema_version\":1}").valid);
    }
}
