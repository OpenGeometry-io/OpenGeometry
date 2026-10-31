use super::area_bound::area_error_bound;
use super::display::MAX_TRIANGLES;
use super::tessellate::tessellate;
use crate::brep::{BrepEnvelope, GeometryError, SurfaceGeometry};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measure {
    pub volume: f64,
    pub area: f64,
    pub error_bound: f64,
    pub area_error_bound: f64,
    pub deflection: f64,
}

pub fn measure(body: &BrepEnvelope, deflection: f64) -> Result<Measure, GeometryError> {
    let mesh = tessellate(body, deflection, MAX_TRIANGLES)?;
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
    let error_bound = if curved {
        area * deflection * 4.0
    } else {
        1e-10 * area.max(1.0)
    };
    Ok(Measure {
        volume: six_volume.abs() / 6.0,
        area,
        error_bound,
        area_error_bound: area_error_bound(body, &mesh, deflection, area)?,
        deflection,
    })
}
