use crate::digest::bits;
use crate::kernel::{BrepEnvelope, SurfaceGeometry, Tessellation};
use crate::mesh::VOLUME_DEFLECTION;
use crate::record::Record;

fn vertex(mesh: &Tessellation, index: u32) -> [f64; 3] {
    let at = index as usize * 3;
    [
        mesh.positions[at],
        mesh.positions[at + 1],
        mesh.positions[at + 2],
    ]
}

fn sums(mesh: &Tessellation) -> (f64, f64) {
    let mut six_volume = 0.0;
    let mut area = 0.0;
    for indices in mesh.indices.chunks_exact(3) {
        let [a, b, c] = [
            vertex(mesh, indices[0]),
            vertex(mesh, indices[1]),
            vertex(mesh, indices[2]),
        ];
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
    (six_volume, area)
}

pub(crate) fn record_volume(
    record: &mut Record,
    label: &str,
    brep: &BrepEnvelope,
    mesh: &Tessellation,
) {
    let valid_indices = mesh.indices.iter().all(|index| {
        let at = *index as usize * 3;
        at + 2 < mesh.positions.len()
    });
    record.section(&format!("{label}.volume"));
    record.field("deflection", bits(VOLUME_DEFLECTION));
    if !valid_indices {
        record.line("error: tessellation index out of range");
        return;
    }
    let (six_volume, area) = sums(mesh);
    let curved = brep
        .geometry
        .surfaces
        .iter()
        .any(|surface| !matches!(surface, SurfaceGeometry::Plane { .. }));
    let error_bound = if curved {
        area * VOLUME_DEFLECTION * 4.0
    } else {
        1e-10 * area.max(1.0)
    };
    record.field("value", bits(six_volume / 6.0));
    record.field("area", bits(area));
    record.field("error_bound", bits(error_bound));
}
