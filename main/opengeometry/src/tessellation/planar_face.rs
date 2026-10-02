use super::mesh::Tessellation;
use crate::brep::{BrepEnvelope, Face, GeometryError, Orientation, Surface};
use crate::math::{scale, Point3};

pub(super) fn planar_face(
    brep: &BrepEnvelope,
    face: &Face,
    samples: &[Vec<Point3>],
    mesh: &mut Tessellation,
    max_triangles: usize,
) -> Result<(), GeometryError> {
    let surface = brep.geometry.surface(face.surface)?;
    let normal = scale(surface.normal_at([0.0; 2])?, face.sense.multiplier());
    let mut points = Vec::new();
    let mut uv_flat = Vec::new();
    let mut holes = Vec::new();
    for (i, &loop_id) in std::iter::once(&face.trim.outer)
        .chain(&face.trim.holes)
        .enumerate()
    {
        if i > 0 {
            holes.push(points.len());
        }
        let start = brep.topology.loops[loop_id as usize].start_halfedge;
        let mut current = start;
        loop {
            let h = &brep.topology.halfedges[current as usize];
            let source = &samples[h.edge as usize];
            if source.len() > 1 {
                let n = source.len() - 1;
                for j in 0..n {
                    let p = source[if h.geometry_use.sense == Orientation::Forward {
                        j
                    } else {
                        n - j
                    }];
                    let uv = surface.project(p, None)?;
                    points.push(p);
                    uv_flat.extend(uv);
                }
            }
            current = h
                .next
                .ok_or_else(|| GeometryError::InvalidTopology("open planar face loop".into()))?;
            if current == start {
                break;
            }
        }
        let loop_start = if i == 0 { 0 } else { holes[i - 1] };
        if points.len() - loop_start < 3 {
            return Err(GeometryError::InvalidTopology(
                "insufficient tessellated planar boundary".into(),
            ));
        }
    }
    let triangles = earcutr::earcut(&uv_flat, &holes, 2);
    if triangles.is_empty() && points.len() >= 3 {
        return Err(GeometryError::InvalidTopology(
            "planar triangulation failed".into(),
        ));
    }
    let mut ids = Vec::with_capacity(points.len());
    for p in points {
        ids.push(mesh.vertex(p, normal)?);
    }
    for tri in triangles.chunks_exact(3) {
        let a = *ids
            .get(tri[0])
            .ok_or_else(|| GeometryError::InvalidTopology("triangulation index".into()))?;
        let b = *ids
            .get(tri[1])
            .ok_or_else(|| GeometryError::InvalidTopology("triangulation index".into()))?;
        let c = *ids
            .get(tri[2])
            .ok_or_else(|| GeometryError::InvalidTopology("triangulation index".into()))?;
        mesh.triangle([a, b, c], face.id, normal, max_triangles)?;
    }
    Ok(())
}
