use crate::brep::{Accuracy, BrepEnvelope, Frame3};
use crate::math::{cross, dot, Point3};
use crate::primitives;
use crate::tessellation::tessellate;

pub(super) fn sphere(id: &str, center: Point3, radius: f64) -> BrepEnvelope {
    primitives::sphere(
        id.into(),
        Frame3 {
            origin: center,
            ..Frame3::IDENTITY
        },
        radius,
        Accuracy {
            geometric: 1e-9,
            intersection: 1e-10,
            tessellation: 0.01,
            exchange: 1e-5,
        },
    )
    .unwrap()
}

pub(super) fn cylinder(id: &str, origin_z: f64, height: f64, reverse: bool) -> BrepEnvelope {
    let frame = if reverse {
        Frame3 {
            origin: [0.0, 0.0, origin_z],
            x: [1.0, 0.0, 0.0],
            y: [0.0, -1.0, 0.0],
            z: [0.0, 0.0, -1.0],
        }
    } else {
        Frame3 {
            origin: [0.0, 0.0, origin_z],
            ..Frame3::IDENTITY
        }
    };
    primitives::cylinder(
        id.into(),
        frame,
        1.0,
        height,
        Accuracy {
            geometric: 1e-9,
            intersection: 1e-10,
            tessellation: 0.01,
            exchange: 1e-5,
        },
    )
    .unwrap()
}

pub(super) fn volume(brep: &BrepEnvelope) -> f64 {
    volume_at_deflection(brep, 0.02)
}

pub(super) fn volume_at_deflection(brep: &BrepEnvelope, deflection: f64) -> f64 {
    let mesh = tessellate(brep, deflection, 2_000_000).unwrap();
    mesh.indices
        .chunks_exact(3)
        .map(|ids| {
            let points: [Point3; 3] = std::array::from_fn(|i| {
                let start = ids[i] as usize * 3;
                [
                    mesh.positions[start],
                    mesh.positions[start + 1],
                    mesh.positions[start + 2],
                ]
            });
            dot(points[0], cross(points[1], points[2])) / 6.0
        })
        .sum()
}

pub(super) fn extrusion(id: &str, outer: Vec<[f64; 2]>, holes: Vec<Vec<[f64; 2]>>) -> BrepEnvelope {
    extrusion_span(id, 0.0, 3.0, outer, holes)
}

pub(super) fn extrusion_span(
    id: &str,
    origin_z: f64,
    height: f64,
    outer: Vec<[f64; 2]>,
    holes: Vec<Vec<[f64; 2]>>,
) -> BrepEnvelope {
    primitives::linear_extrusion(
        id.into(),
        Frame3 {
            origin: [0.0, 0.0, origin_z],
            ..Frame3::IDENTITY
        },
        outer,
        holes,
        height,
        Accuracy {
            geometric: 1.0e-9,
            intersection: 1.0e-10,
            tessellation: 0.01,
            exchange: 1.0e-5,
        },
    )
    .unwrap()
}
