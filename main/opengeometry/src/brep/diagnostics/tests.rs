use super::*;
use crate::brep::topology::circular_face;
use crate::brep::CurveGeometry;

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
    if let CurveGeometry::Circle { radius, .. } = &mut broken.geometry.curves[0] {
        *radius *= 1.1;
    }
    assert!(!validate_json(&serde_json::to_string(&broken).unwrap()).valid);
    assert!(!validate_json("{\"schema_version\":1}").valid);
}
