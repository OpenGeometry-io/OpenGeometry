use opengeometry::world_graph::{CreatingOperation, WorldGraph};
use opengeometry_test_support::volume;

pub(super) fn extrude(profile: &str, distance: f64) -> CreatingOperation {
    CreatingOperation::Extrude {
        profile: profile.into(),
        holes: Vec::new(),
        distance,
    }
}

pub(super) fn assert_volume(world: &WorldGraph, og_id: &str, expected: f64) {
    let body = world.brep(og_id).unwrap();
    body.validate().unwrap();
    let measured = volume::estimate(&body, 0.001);
    assert!(
        (measured.value - expected).abs() <= measured.error_bound,
        "{og_id}: {} versus {expected} with bound {}",
        measured.value,
        measured.error_bound
    );
}
