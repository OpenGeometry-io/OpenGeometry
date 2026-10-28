use opengeometry::brep::BrepEnvelope;
use opengeometry::world_graph::Primitive;
use opengeometry_test_support::volume::VolumeEstimate;
use serde_json::json;

pub(super) fn cuboid(width: f64) -> Primitive {
    Primitive::Cuboid {
        width,
        height: 2.0,
        depth: 2.0,
    }
}

pub(super) fn assert_valid_closed(body: &BrepEnvelope, solids: usize, label: &str) {
    body.validate().unwrap();
    let value = serde_json::to_value(body).unwrap();
    assert_eq!(value["solids"].as_array().unwrap().len(), solids, "{label}");
    let shells = value["topology"]["shells"].as_array().unwrap();
    assert!(
        shells.iter().all(|shell| shell["is_closed"] == json!(true)),
        "{label}"
    );
}

pub(super) fn assert_volume(measured: VolumeEstimate, expected: f64, label: &str) {
    assert!(
        (measured.value - expected).abs() <= measured.error_bound,
        "{label}: measured {} within {}, expected {expected}",
        measured.value,
        measured.error_bound
    );
}
