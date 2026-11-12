use crate::brep::{BrepEnvelope, Edge, Face, GeometryError, Surface, SurfaceGeometry};
use crate::math::{dot, norm, scale, Point3};

const TANGENT_ANGLE: f64 = 1e-6;

pub(super) fn check_smooth_angle(degrees: f64) -> Result<(), GeometryError> {
    if !(0.0..90.0).contains(&degrees) {
        return Err(GeometryError::InvalidGeometry(
            "smooth angle must be at least 0 and below 90 degrees".into(),
        ));
    }
    Ok(())
}

pub(super) fn oriented_normal(
    brep: &BrepEnvelope,
    face: &Face,
    p: Point3,
) -> Result<Point3, GeometryError> {
    let surface = brep.geometry.surface(face.surface)?;
    let uv = match surface {
        SurfaceGeometry::Plane { .. } => [0.0; 2],
        _ => surface.project(p, None)?,
    };
    Ok(scale(surface.normal_at(uv)?, face.sense.multiplier()))
}

pub(super) fn neighbour<'a>(brep: &'a BrepEnvelope, edge: &Edge, face: u32) -> Option<&'a Face> {
    let twin = edge.twin_halfedge?;
    [edge.halfedge, twin]
        .into_iter()
        .filter_map(|h| brep.topology.halfedges[h as usize].face)
        .find(|&f| f != face)
        .map(|f| &brep.topology.faces[f as usize])
}

pub(super) fn smooth_edges(
    brep: &BrepEnvelope,
    samples: &[Vec<Point3>],
    degrees: f64,
) -> Result<Vec<u8>, GeometryError> {
    let mut flags = Vec::with_capacity(samples.len());
    for (edge, points) in brep.topology.edges.iter().zip(samples) {
        flags.push(u8::from(
            edge.chart_seam || smooth(brep, edge, points, degrees)?,
        ));
    }
    Ok(flags)
}

fn smooth(
    brep: &BrepEnvelope,
    edge: &Edge,
    points: &[Point3],
    degrees: f64,
) -> Result<bool, GeometryError> {
    let Some(twin) = edge.twin_halfedge else {
        return Ok(false);
    };
    let faces = [edge.halfedge, twin].map(|h| brep.topology.halfedges[h as usize].face);
    let [Some(a), Some(b)] = faces.map(|f| f.map(|f| &brep.topology.faces[f as usize])) else {
        return Ok(false);
    };
    let surfaces = [a, b].map(|face| &brep.geometry.surfaces[face.surface as usize]);
    let planar = if let [SurfaceGeometry::Plane { frame: f }, SurfaceGeometry::Plane { frame: g }] =
        surfaces
    {
        if scale(f.z, a.sense.multiplier()) == scale(g.z, b.sense.multiplier()) {
            return Ok(true);
        }
        true
    } else {
        false
    };
    if degrees == 0.0 {
        return Ok(false);
    }
    let mut fold: f64 = 0.0;
    for &p in points {
        let [m, n] = [oriented_normal(brep, a, p)?, oriented_normal(brep, b, p)?];
        fold = fold.max((dot(m, n) / (norm(m) * norm(n))).clamp(-1.0, 1.0).acos());
    }
    Ok(fold <= TANGENT_ANGLE || (planar && fold.to_degrees() < degrees))
}
