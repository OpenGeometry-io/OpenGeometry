use super::curved_face::curved_face;
use super::deviation::edge_count;
use super::edges::{sample_edge, synchronize_edge_samples};
use super::grid::{grid_size, rectangle, Rect};
use super::loop_minimum::raise_short_planar_loops;
use super::mesh::Tessellation;
use super::planar_face::planar_face;
use super::trimmed_face::{curved_trimmed_face, FaceGrid};
use crate::brep::{
    BrepEnvelope, Curve, CurveGeometry, EdgeGeometry, GeometryError, Surface, SurfaceGeometry,
};
use crate::math::{scale, Point3};

pub fn tessellate(
    brep: &BrepEnvelope,
    deflection: f64,
    max_triangles: usize,
) -> Result<Tessellation, GeometryError> {
    brep.validate()?;
    if !deflection.is_finite()
        || deflection <= 2.0 * brep.accuracy.geometric
        || max_triangles == 0
        || max_triangles > 10_000_000
    {
        return Err(GeometryError::InvalidGeometry(
            "deflection must exceed twice geometric tolerance; triangle limit must be 1..=10000000"
                .into(),
        ));
    }
    let error = deflection * 0.5;
    let magnitude = coordinate_magnitude(brep)?;
    let coordinate_floor = 64.0 * f64::EPSILON * magnitude;
    if deflection <= 2.0 * (brep.accuracy.geometric + coordinate_floor) {
        return Err(GeometryError::LimitExceeded(
            "coordinate precision cannot meet requested deflection".into(),
        ));
    }
    let mut counts = Vec::with_capacity(brep.topology.edges.len());
    for edge in &brep.topology.edges {
        counts.push(edge_count(brep, edge.id, error, max_triangles)?);
    }
    raise_short_planar_loops(brep, &mut counts)?;
    let mut grids = Vec::with_capacity(brep.topology.faces.len());
    for face in &brep.topology.faces {
        let surface = brep.geometry.surface(face.surface)?;
        if matches!(surface, SurfaceGeometry::Plane { .. }) {
            grids.push(None);
            continue;
        }
        let rect = match rectangle(brep, face) {
            Ok(rect) => Some(rect),
            Err(GeometryError::UnsupportedGeometry(_)) => None,
            Err(error) => return Err(error),
        };
        let desired = grid_size(surface, face.trim.uv_bounds, error, max_triangles)?;
        if let Some(rect) = &rect {
            for (i, &h) in rect.halfedges.iter().enumerate() {
                let id = brep.topology.halfedges[h as usize].edge;
                counts[id as usize] = counts[id as usize].max(desired[i % 2]);
            }
        }
        grids.push(Some((rect, desired)));
    }

    synchronize_edge_samples(brep, &grids, &mut counts);
    let sample_count = counts
        .iter()
        .try_fold(0usize, |total, &n| total.checked_add(n + 1))
        .ok_or_else(|| GeometryError::LimitExceeded("edge samples".into()))?;
    if sample_count > max_triangles * 3 {
        return Err(GeometryError::LimitExceeded("edge sample budget".into()));
    }
    let mut samples = Vec::with_capacity(counts.len());
    for (id, n) in counts.into_iter().enumerate() {
        samples.push(sample_edge(brep, id as u32, n)?);
    }
    let mut mesh = Tessellation::new(brep.revision, deflection);
    add_face_triangles(brep, &grids, &samples, error, &mut mesh, max_triangles)?;
    add_edge_outlines(brep, samples, &mut mesh);
    Ok(mesh)
}

pub(super) fn coordinate_magnitude(brep: &BrepEnvelope) -> Result<f64, GeometryError> {
    let mut magnitude: f64 = 0.0;
    for face in &brep.topology.faces {
        let surface = brep.geometry.surface(face.surface)?;
        for value in surface.frame().origin {
            magnitude = magnitude.max(value.abs());
        }
        let bounds = surface.enclose(face.trim.uv_bounds)?;
        for axis in bounds.axes {
            magnitude = magnitude.max(axis.lo.abs()).max(axis.hi.abs());
        }
    }
    for vertex in &brep.topology.vertices {
        for value in vertex.position {
            magnitude = magnitude.max(value.abs());
        }
    }
    for edge in &brep.topology.edges {
        if let EdgeGeometry::Curve { curve, range } = edge.geometry {
            let curve = brep.geometry.curve(curve)?;
            for axis in curve.enclose(range)?.axes {
                magnitude = magnitude.max(axis.lo.abs()).max(axis.hi.abs());
            }
            match curve.geometry {
                CurveGeometry::Circle { frame, radius } => {
                    magnitude = magnitude.max(*radius);
                    for value in frame.origin {
                        magnitude = magnitude.max(value.abs());
                    }
                }
                CurveGeometry::Ellipse {
                    frame,
                    major_radius,
                    ..
                } => {
                    magnitude = magnitude.max(*major_radius);
                    for value in frame.origin {
                        magnitude = magnitude.max(value.abs());
                    }
                }
                _ => {}
            }
        }
    }
    Ok(magnitude)
}

fn add_face_triangles(
    brep: &BrepEnvelope,
    grids: &[Option<(Option<Rect>, [usize; 2])>],
    samples: &[Vec<Point3>],
    error: f64,
    mesh: &mut Tessellation,
    max_triangles: usize,
) -> Result<(), GeometryError> {
    for (face, grid) in brep.topology.faces.iter().zip(grids) {
        let before = mesh.triangle_face_ids.len();
        match grid {
            Some((Some(rect), desired)) if face.trim.holes.is_empty() => {
                curved_face(brep, face, rect, *desired, samples, mesh, max_triangles)?
            }
            Some((rect, desired)) => curved_trimmed_face(
                brep,
                face,
                &FaceGrid {
                    rect: rect.as_ref(),
                    desired: *desired,
                },
                samples,
                error,
                mesh,
                max_triangles,
            )?,
            None => planar_face(brep, face, samples, mesh, max_triangles)?,
        }
        if mesh.triangle_face_ids.len() == before {
            return Err(GeometryError::UnresolvedTessellation(format!(
                "face {} produced no nondegenerate tessellation triangles",
                face.id
            )));
        }
    }
    Ok(())
}

fn add_edge_outlines(brep: &BrepEnvelope, samples: Vec<Vec<Point3>>, mesh: &mut Tessellation) {
    for (edge, points) in brep.topology.edges.iter().zip(samples) {
        let planar_subdivision = edge
            .twin_halfedge
            .and_then(|twin| {
                let first = &brep.topology.halfedges[edge.halfedge as usize];
                let second = &brep.topology.halfedges[twin as usize];
                let a = &brep.topology.faces[first.face? as usize];
                let b = &brep.topology.faces[second.face? as usize];
                match (
                    &brep.geometry.surfaces[a.surface as usize],
                    &brep.geometry.surfaces[b.surface as usize],
                ) {
                    (
                        SurfaceGeometry::Plane { frame: a_frame },
                        SurfaceGeometry::Plane { frame: b_frame },
                    ) => Some(
                        scale(a_frame.z, a.sense.multiplier())
                            == scale(b_frame.z, b.sense.multiplier()),
                    ),
                    _ => Some(false),
                }
            })
            .unwrap_or(false);
        if edge.chart_seam || planar_subdivision {
            continue;
        }
        for pair in points.windows(2) {
            mesh.outline_positions.extend(pair[0]);
            mesh.outline_positions.extend(pair[1]);
            mesh.outline_edge_ids.push(edge.id);
        }
    }
}
