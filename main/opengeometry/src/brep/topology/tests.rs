use super::entities::EdgeGeometry;
use super::envelope::BrepEnvelope;
use super::fixtures::circular_face;
use crate::brep::error::GeometryError;
use crate::brep::geometry::CurveGeometry;
use crate::brep::pcurve::PcurveGeometry;
use crate::math::Interval;

#[test]
fn full_circle_loop_is_valid_and_strictly_versioned() {
    let b = circular_face();
    b.validate().unwrap();
    let json = b.to_json().unwrap();
    BrepEnvelope::from_json(&json).unwrap();
    let mut bad: serde_json::Value = serde_json::from_str(&json).unwrap();
    bad["schema_version"] = serde_json::json!(1);
    assert!(matches!(
        BrepEnvelope::from_json(&bad.to_string()),
        Err(GeometryError::UnsupportedSchema { found: 1 })
    ));
    bad.as_object_mut().unwrap().remove("schema_version");
    assert!(BrepEnvelope::from_json(&bad.to_string()).is_err());
}
#[test]
fn missing_pcurve_and_malformed_cycle_are_rejected() {
    let mut b = circular_face();
    b.topology.halfedges[0].geometry_use.pcurve = None;
    assert!(b.validate().is_err());
    let mut b = circular_face();
    b.topology.halfedges[0].next = Some(10);
    assert!(b.validate().is_err());
    let mut b = circular_face();
    b.geometry.pcurves[0] = PcurveGeometry::Line2 {
        origin: [0.0; 2],
        direction: [1.0, 0.0],
    };
    assert!(b.validate().is_err());
}
#[test]
fn pcurve_outside_uv_bounds_beyond_geometric_tolerance_is_rejected() {
    let mut b = circular_face();
    b.topology.faces[0].trim.uv_bounds[0] = Interval::new(-0.999, 0.999).unwrap();
    assert!(matches!(
        b.validate(),
        Err(GeometryError::InvalidTopology(message)) if message.contains("leaves face 0 UV bounds")
    ));
}
#[test]
fn old_json_and_unknown_fields_are_not_accepted() {
    assert!(BrepEnvelope::from_json("{\"vertices\":[],\"faces\":[]}").is_err());
    let mut json: serde_json::Value =
        serde_json::from_str(&circular_face().to_json().unwrap()).unwrap();
    json["legacy"] = serde_json::json!(true);
    assert!(BrepEnvelope::from_json(&json.to_string()).is_err());
}
#[test]
fn malformed_charts_and_undeclared_degenerate_edges_are_rejected() {
    let mut b = circular_face();
    b.topology.faces[0].trim.chart = u32::MAX;
    assert!(
        matches!(b.validate(), Err(GeometryError::MissingReference { kind, .. }) if kind == "chart")
    );
    let mut b = circular_face();
    b.geometry.curves[0] = CurveGeometry::Line {
        origin: [1.0, 0.0, 0.0],
        direction: [1.0, 0.0, 0.0],
    };
    b.topology.edges[0].geometry = EdgeGeometry::Curve {
        curve: 0,
        range: Interval::new(0.0, 1e-12).unwrap(),
    };
    b.geometry.pcurves[0] = PcurveGeometry::Line2 {
        origin: [1.0, 0.0],
        direction: [1.0, 0.0],
    };
    assert!(
        matches!(b.validate(), Err(GeometryError::InvalidTopology(message)) if message.contains("regular line"))
    );
    let mut b = circular_face();
    b.accuracy.geometric = 10.0;
    b.topology.edges[0].geometry = EdgeGeometry::Collapsed { vertex: 0 };
    b.geometry.pcurves[0] = PcurveGeometry::Line2 {
        origin: [1.0, 0.0],
        direction: [0.0; 2],
    };
    assert!(
        matches!(b.validate(), Err(GeometryError::InvalidTopology(message)) if message.contains("surface singularity"))
    );
    let mut b = circular_face();
    b.topology.vertices[0].tolerance = 1e-18;
    b.topology.edges[0].tolerance = 1e-18;
    assert!(
        matches!(b.validate(), Err(GeometryError::InvalidTopology(message)) if message.contains("edge endpoints"))
    );
}
