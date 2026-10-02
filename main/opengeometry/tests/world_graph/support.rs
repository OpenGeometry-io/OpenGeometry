use opengeometry::world_graph::{CopyOptions, Placement, Primitive, Transform};
use serde::Serialize;

pub(super) fn copy_named(name: &str) -> CopyOptions {
    CopyOptions {
        og_id: Some(name.into()),
        ..CopyOptions::default()
    }
}

pub(super) fn box_primitive() -> Primitive {
    Primitive::Cuboid {
        width: 2.0,
        height: 2.0,
        depth: 2.0,
    }
}

pub(super) fn near(a: [f64; 3], b: [f64; 3]) {
    near_within(&a, &b, 1e-12);
}

pub(super) fn near_within(a: &[f64], b: &[f64], tolerance: f64) {
    assert_eq!(a.len(), b.len(), "{a:?} != {b:?}");
    for (x, y) in a.iter().zip(b) {
        assert!((x - y).abs() < tolerance, "{a:?} != {b:?}");
    }
}

pub(super) fn bits(value: &impl Serialize) -> String {
    serde_json::to_string(value).unwrap()
}

pub(super) fn place(placement: Placement) -> Transform {
    Transform::Place {
        origin: Some(placement.origin),
        x_direction: Some(placement.x_direction),
        normal: Some(placement.normal),
        scale: Some(placement.scale),
    }
}
