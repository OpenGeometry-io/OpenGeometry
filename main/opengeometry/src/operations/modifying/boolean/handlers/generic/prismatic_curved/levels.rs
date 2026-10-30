use crate::brep::{coverage_gap, BrepEnvelope, Face, Frame3, GeometryError, SurfaceGeometry};
use crate::math::{dot, norm, scale, sub, Point3};

const PARALLEL: f64 = 1.0 - 1.0e-10;

const PERPENDICULAR: f64 = 1.0e-10;

pub(super) struct Levels {
    pub(super) base: Frame3,
    pub(super) levels: Vec<f64>,
    pub(super) spans: [(f64, f64); 2],
}

pub(super) fn common_levels(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    geometric: f64,
) -> Result<Levels, GeometryError> {
    let base = common_frame(a, b, geometric)?;
    let spans = [span(a, base), span(b, base)];
    let mut levels = a
        .topology
        .vertices
        .iter()
        .chain(&b.topology.vertices)
        .map(|vertex| base.local(vertex.position)[2])
        .collect::<Vec<_>>();
    levels.sort_by(f64::total_cmp);
    levels.dedup_by(|above, below| *above - *below <= geometric);
    if levels.len() < 2 {
        return Err(gap());
    }
    if levels
        .windows(2)
        .any(|pair| pair[1] - pair[0] <= 4.0 * geometric)
    {
        return Err(tangent());
    }
    Ok(Levels {
        base,
        levels,
        spans,
    })
}

pub(super) fn gap() -> GeometryError {
    coverage_gap("prismatic curved solid", "prismatic curved solid")
}

pub(super) fn tangent() -> GeometryError {
    GeometryError::UnresolvedIntersection(
        "prismatic operands are tangent within geometric resolution".into(),
    )
}

fn span(body: &BrepEnvelope, base: Frame3) -> (f64, f64) {
    body.topology
        .vertices
        .iter()
        .map(|vertex| base.local(vertex.position)[2])
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), level| {
            (lo.min(level), hi.max(level))
        })
}

fn common_frame(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    geometric: f64,
) -> Result<Frame3, GeometryError> {
    for frame in [a, b].into_iter().flat_map(cylinder_frames) {
        if prismatic_along(a, frame.z, geometric)? && prismatic_along(b, frame.z, geometric)? {
            let low = [a, b]
                .iter()
                .flat_map(|body| &body.topology.vertices)
                .map(|vertex| dot(vertex.position, frame.z))
                .fold(f64::INFINITY, f64::min);
            return Frame3::from_axis(scale(frame.z, low), frame.z, frame.x);
        }
    }
    Err(gap())
}

fn cylinder_frames(body: &BrepEnvelope) -> Vec<Frame3> {
    body.topology
        .faces
        .iter()
        .filter_map(
            |face| match body.geometry.surfaces.get(face.surface as usize) {
                Some(SurfaceGeometry::Cylinder { frame, .. }) => Some(*frame),
                _ => None,
            },
        )
        .collect()
}

fn prismatic_along(
    body: &BrepEnvelope,
    axis: Point3,
    geometric: f64,
) -> Result<bool, GeometryError> {
    for face in &body.topology.faces {
        let wall = match body.geometry.surface(face.surface)? {
            SurfaceGeometry::Plane { frame } => {
                let alignment = dot(frame.z, axis).abs();
                if alignment > PARALLEL {
                    false
                } else if alignment < PERPENDICULAR {
                    true
                } else {
                    return Ok(false);
                }
            }
            SurfaceGeometry::Cylinder { frame, .. } if dot(frame.z, axis).abs() > PARALLEL => true,
            _ => return Ok(false),
        };
        if wall && !straight_wall(body, face, axis, geometric)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn straight_wall(
    body: &BrepEnvelope,
    face: &Face,
    axis: Point3,
    geometric: f64,
) -> Result<bool, GeometryError> {
    if !face.trim.holes.is_empty() {
        return Ok(false);
    }
    let start = body
        .topology
        .loops
        .get(face.trim.outer as usize)
        .ok_or_else(gap)?
        .start_halfedge;
    let mut footprints = Vec::<Point3>::new();
    let mut current = start;
    for _ in 0..body.topology.halfedges.len() {
        let halfedge = body
            .topology
            .halfedges
            .get(current as usize)
            .ok_or_else(gap)?;
        let position = body
            .topology
            .vertices
            .get(halfedge.from as usize)
            .ok_or_else(gap)?
            .position;
        let footprint = sub(position, scale(axis, dot(position, axis)));
        if footprints
            .iter()
            .all(|known| norm(sub(*known, footprint)) > geometric)
        {
            footprints.push(footprint);
        }
        current = halfedge.next.ok_or_else(gap)?;
        if current == start {
            return Ok(footprints.len() <= 2);
        }
    }
    Err(gap())
}
