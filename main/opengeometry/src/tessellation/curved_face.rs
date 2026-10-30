use super::edges::boundary_point;
use super::grid::Rect;
use super::mesh::Tessellation;
use crate::brep::{BrepEnvelope, Face, GeometryError, Surface, SurfaceGeometry};
use crate::math::{scale, Point3};

pub(super) fn curved_face(
    brep: &BrepEnvelope,
    face: &Face,
    rect: &Rect,
    desired: [usize; 2],
    samples: &[Vec<Point3>],
    mesh: &mut Tessellation,
    max_triangles: usize,
) -> Result<(), GeometryError> {
    let mut size = desired;
    for (i, &h) in rect.halfedges.iter().enumerate() {
        let n = samples[brep.topology.halfedges[h as usize].edge as usize]
            .len()
            .saturating_sub(1);
        size[i % 2] = size[i % 2].max(n);
    }
    let [nu, nv] = size;
    let [u, v] = face.trim.uv_bounds;
    let surface = brep.geometry.surface(face.surface)?;
    let start = mesh.positions.len() / 3;
    if nu
        .checked_add(1)
        .and_then(|n| nv.checked_add(1).and_then(|m| n.checked_mul(m)))
        .is_none_or(|n| n > max_triangles * 3)
    {
        return Err(GeometryError::LimitExceeded(
            "tessellation grid vertices".into(),
        ));
    }
    for j in 0..=nv {
        for i in 0..=nu {
            let uv = [
                (u.lo + u.width() * i as f64 / nu as f64).clamp(u.lo, u.hi),
                (v.lo + v.width() * j as f64 / nv as f64).clamp(v.lo, v.hi),
            ];
            let point = if j == 0 {
                boundary_point(brep, samples, rect.halfedges[0], i, nu, 0)?
            } else if j == nv {
                boundary_point(brep, samples, rect.halfedges[2], i, nu, 0)?
            } else if i == 0 {
                boundary_point(brep, samples, rect.halfedges[3], j, nv, 1)?
            } else if i == nu {
                boundary_point(brep, samples, rect.halfedges[1], j, nv, 1)?
            } else {
                surface.point_at(uv)?
            };
            let normal_uv = if matches!(surface, SurfaceGeometry::Cone { .. }) && uv[1] == 0.0 {
                [uv[0], v.hi]
            } else {
                uv
            };
            let normal = scale(surface.normal_at(normal_uv)?, face.sense.multiplier());
            mesh.vertex(point, normal)?;
        }
    }
    for j in 0..nv {
        for i in 0..nu {
            let a = (start + j * (nu + 1) + i) as u32;
            let b = a + 1;
            let c = a + (nu + 1) as u32;
            let d = c + 1;
            let uv = [
                u.lo + u.width() * (i as f64 + 0.5) / nu as f64,
                v.lo + v.width() * (j as f64 + 0.5) / nv as f64,
            ];
            let normal = scale(surface.normal_at(uv)?, face.sense.multiplier());
            mesh.triangle([a, b, d], face.id, normal, max_triangles)?;
            mesh.triangle([a, d, c], face.id, normal, max_triangles)?;
        }
    }
    Ok(())
}
