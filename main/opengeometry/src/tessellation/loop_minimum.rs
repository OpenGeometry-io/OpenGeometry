use crate::brep::{BrepEnvelope, CurveGeometry, EdgeGeometry, GeometryError, SurfaceGeometry};

pub(super) fn raise_short_planar_loops(
    brep: &BrepEnvelope,
    counts: &mut [usize],
) -> Result<(), GeometryError> {
    for minimum in [2, 3] {
        let points = planar_loop_points(brep, counts)?;
        for h in &brep.topology.halfedges {
            let short = h
                .loop_ref
                .and_then(|id| points[id as usize])
                .is_some_and(|n| n < 3);
            if short && conic(brep, h.edge) {
                counts[h.edge as usize] = counts[h.edge as usize].max(minimum);
            }
        }
    }
    Ok(())
}

fn planar_loop_points(
    brep: &BrepEnvelope,
    counts: &[usize],
) -> Result<Vec<Option<usize>>, GeometryError> {
    let mut points = Vec::with_capacity(brep.topology.loops.len());
    for face_loop in &brep.topology.loops {
        let face = &brep.topology.faces[face_loop.face_ref as usize];
        let surface = brep.geometry.surface(face.surface)?;
        points.push(matches!(surface, SurfaceGeometry::Plane { .. }).then_some(0));
    }
    for h in &brep.topology.halfedges {
        let edge = &brep.topology.edges[h.edge as usize];
        if let (Some(id), EdgeGeometry::Curve { .. }) = (h.loop_ref, &edge.geometry) {
            if let Some(total) = &mut points[id as usize] {
                *total += counts[h.edge as usize];
            }
        }
    }
    Ok(points)
}

fn conic(brep: &BrepEnvelope, edge: u32) -> bool {
    match brep.topology.edges[edge as usize].geometry {
        EdgeGeometry::Curve { curve, .. } => matches!(
            brep.geometry.curves[curve as usize],
            CurveGeometry::Circle { .. } | CurveGeometry::Ellipse { .. }
        ),
        EdgeGeometry::Collapsed { .. } => false,
    }
}
