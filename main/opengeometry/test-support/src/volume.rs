use opengeometry::brep::{BrepEnvelope, SurfaceGeometry};
use opengeometry::tessellation::tessellate;

pub struct VolumeEstimate {
    pub value: f64,
    pub error_bound: f64,
}

pub fn estimate(body: &BrepEnvelope, deflection: f64) -> VolumeEstimate {
    let mesh = tessellate(body, deflection, 2_000_000).unwrap();
    let point = |index: u32| {
        let at = index as usize * 3;
        [
            mesh.positions[at],
            mesh.positions[at + 1],
            mesh.positions[at + 2],
        ]
    };
    let mut six_volume = 0.0;
    let mut area = 0.0;
    for indices in mesh.indices.chunks_exact(3) {
        let [a, b, c] = [point(indices[0]), point(indices[1]), point(indices[2])];
        let cross = [
            b[1] * c[2] - b[2] * c[1],
            b[2] * c[0] - b[0] * c[2],
            b[0] * c[1] - b[1] * c[0],
        ];
        six_volume += a[0] * cross[0] + a[1] * cross[1] + a[2] * cross[2];
        let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let normal = [
            ab[1] * ac[2] - ab[2] * ac[1],
            ab[2] * ac[0] - ab[0] * ac[2],
            ab[0] * ac[1] - ab[1] * ac[0],
        ];
        area += normal[0].hypot(normal[1]).hypot(normal[2]) * 0.5;
    }
    let curved = body
        .geometry
        .surfaces
        .iter()
        .any(|surface| !matches!(surface, SurfaceGeometry::Plane { .. }));
    VolumeEstimate {
        value: six_volume.abs() / 6.0,
        error_bound: if curved {
            area * deflection * 4.0
        } else {
            1e-10 * area.max(1.0)
        },
    }
}
