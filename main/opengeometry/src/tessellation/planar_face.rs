use super::mesh::Tessellation;
use crate::brep::{BrepEnvelope, Face, GeometryError, Orientation, Surface};
use crate::math::{cross, dot, norm, scale, sub, Point3};

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
    let mut triangles = earcutr::earcut(&uv_flat, &holes, 2);
    if triangles.is_empty() && points.len() >= 3 {
        return Err(GeometryError::InvalidTopology(
            "planar triangulation failed".into(),
        ));
    }
    flip_collinear_ears(&mut triangles, &points, normal, brep.accuracy.geometric);
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
        let [pa, pb, pc] = [a, b, c].map(|id| mesh.point(id));
        if dot(cross(sub(pb, pa), sub(pc, pa)), normal) == 0.0 {
            continue;
        }
        mesh.triangle([a, b, c], face.id, normal, max_triangles)?;
    }
    Ok(())
}

fn flip_collinear_ears(triangles: &mut [usize], points: &[Point3], normal: Point3, tolerance: f64) {
    let twice = |[a, b, c]: [usize; 3]| {
        dot(
            cross(sub(points[b], points[a]), sub(points[c], points[a])),
            normal,
        )
    };
    for t in 0..triangles.len() / 3 {
        let tri = [triangles[3 * t], triangles[3 * t + 1], triangles[3 * t + 2]];
        let lengths =
            [0, 1, 2].map(|k| norm(sub(points[tri[(k + 2) % 3]], points[tri[(k + 1) % 3]])));
        let k = (0..3)
            .max_by(|&i, &j| lengths[i].total_cmp(&lengths[j]))
            .unwrap_or(0);
        if twice(tri).abs() > tolerance * lengths[k] {
            continue;
        }
        let [b, c, a] = [tri[k], tri[(k + 1) % 3], tri[(k + 2) % 3]];
        let Some((u, d)) = (0..triangles.len() / 3).filter(|&u| u != t).find_map(|u| {
            (0..3)
                .find(|&j| triangles[3 * u + j] == a && triangles[3 * u + (j + 1) % 3] == c)
                .map(|j| (u, triangles[3 * u + (j + 2) % 3]))
        }) else {
            continue;
        };
        let side = twice([a, c, d]);
        if twice([a, b, d]) * side > 0.0 && twice([b, c, d]) * side > 0.0 {
            triangles[3 * t..3 * t + 3].copy_from_slice(&[a, b, d]);
            triangles[3 * u..3 * u + 3].copy_from_slice(&[b, c, d]);
        }
    }
}
