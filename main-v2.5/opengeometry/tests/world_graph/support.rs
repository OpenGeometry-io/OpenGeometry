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
    for axis in 0..3 {
        assert!((a[axis] - b[axis]).abs() < 1e-12, "{a:?} != {b:?}");
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
