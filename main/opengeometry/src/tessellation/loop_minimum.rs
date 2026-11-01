use crate::brep::{BrepEnvelope, CurveGeometry, EdgeGeometry, GeometryError, SurfaceGeometry};

const PLANAR_LOOP_POINTS: usize = 3;
const CURVED_TRIM_SAMPLES: usize = 4;

pub(super) fn raise_short_loops(
    brep: &BrepEnvelope,
    counts: &mut [usize],
) -> Result<(), GeometryError> {
    for minimum in [2, 3, 4] {
        let points = loop_points(brep, counts)?;
        for h in &brep.topology.halfedges {
            let Some((total, planar)) = h.loop_ref.map(|id| points[id as usize]) else {
                continue;
            };
            let short = if planar {
                minimum <= PLANAR_LOOP_POINTS && total < PLANAR_LOOP_POINTS
            } else {
                total < CURVED_TRIM_SAMPLES
            };
            if short && raisable(brep, h.edge, planar) {
                counts[h.edge as usize] = counts[h.edge as usize].max(minimum);
            }
        }
    }
    Ok(())
}

fn loop_points(brep: &BrepEnvelope, counts: &[usize]) -> Result<Vec<(usize, bool)>, GeometryError> {
    let mut points = Vec::with_capacity(brep.topology.loops.len());
    for face_loop in &brep.topology.loops {
        let face = &brep.topology.faces[face_loop.face_ref as usize];
        let surface = brep.geometry.surface(face.surface)?;
        points.push((0, matches!(surface, SurfaceGeometry::Plane { .. })));
    }
    for h in &brep.topology.halfedges {
        let Some(id) = h.loop_ref else {
            continue;
        };
        let (total, planar) = &mut points[id as usize];
        match brep.topology.edges[h.edge as usize].geometry {
            EdgeGeometry::Curve { .. } => *total += counts[h.edge as usize],
            EdgeGeometry::Collapsed { .. } if !*planar => *total += 1,
            EdgeGeometry::Collapsed { .. } => {}
        }
    }
    Ok(points)
}

fn raisable(brep: &BrepEnvelope, edge: u32, planar: bool) -> bool {
    let EdgeGeometry::Curve { curve, .. } = brep.topology.edges[edge as usize].geometry else {
        return false;
    };
    match brep.geometry.curves[curve as usize] {
        CurveGeometry::Circle { .. } | CurveGeometry::Ellipse { .. } => true,
        CurveGeometry::Intersection { .. } => !planar,
        CurveGeometry::Line { .. } => false,
    }
}
