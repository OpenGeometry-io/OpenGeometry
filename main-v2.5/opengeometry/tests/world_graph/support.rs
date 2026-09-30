use opengeometry::world_graph::{CopyOptions, Primitive};

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
