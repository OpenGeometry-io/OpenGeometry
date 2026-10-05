use super::compare::{coordinates_match, frames_match};
use crate::brep::{
    coverage_gap, BrepEnvelope, CurveGeometry, EdgeGeometry, FaceProvenance, FaceRole, FaceSource,
    Frame3, GeometryError, GeometryQuality, GeometryStore, PcurveGeometry, SurfaceGeometry,
    Topology,
};
use crate::geom2d::{Pt2, RingRegion};
use crate::math::{add, dot, norm, scale, sub, Point3};
use crate::operations::modifying::boolean::types::BooleanOp;
use crate::primitives;

#[derive(Clone)]
struct PrismaticSide {
    from: Point3,
    to: Point3,
    source: FaceSource,
}

struct ExtrusionFrame {
    top_frame: Frame3,
    frame: Frame3,
    height: f64,
}

pub(crate) struct PrismaticInput<'a> {
    pub(crate) brep: &'a BrepEnvelope,
    pub(crate) frame: Frame3,
    pub(crate) height: f64,
    pub(crate) contours: Vec<Vec<Point3>>,
    sides: Vec<PrismaticSide>,
}

#[derive(Clone)]
pub(crate) struct PrismaticAxialSegment {
    pub(crate) lo: f64,
    pub(crate) hi: f64,
    pub(crate) lower: FaceProvenance,
    pub(crate) upper: FaceProvenance,
    pub(crate) include_a_sides: bool,
    pub(crate) include_b_sides: bool,
    pub(crate) side_role: FaceRole,
}

pub(crate) fn full_planar_extrusion(
    brep: &BrepEnvelope,
) -> Result<PrismaticInput<'_>, GeometryError> {
    brep.validate()?;
    if !matches!(brep.quality, GeometryQuality::Analytic)
        || brep.topology.faces.len() < 5
        || brep.topology.shells.len() != 1
        || brep.solids.len() != 1
        || !brep.solids[0].cavity_shells.is_empty()
        || !brep.topology.wires.is_empty()
    {
        return Err(prismatic_gap());
    }
    let ExtrusionFrame {
        top_frame,
        frame,
        height,
    } = extrusion_frame(brep)?;
    let top = &brep.topology.faces[0];
    let mut profile = Vec::with_capacity(top.trim.holes.len() + 1);
    profile.push(loop_points(brep, top.trim.outer, top_frame)?);
    for &hole in &top.trim.holes {
        profile.push(loop_points(brep, hole, top_frame)?);
    }
    check_canonical_extrusion(brep, frame, &profile, height)?;
    let contours = profile
        .iter()
        .map(|ring| {
            ring.iter()
                .map(|point| frame.point([point[0], point[1], 0.0]))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut face = 2_u32;
    let mut sides = Vec::new();
    for contour in &contours {
        for index in 0..contour.len() {
            sides.push(PrismaticSide {
                from: contour[index],
                to: contour[(index + 1) % contour.len()],
                source: brep_face_source(brep, face),
            });
            face += 1;
        }
    }
    Ok(PrismaticInput {
        brep,
        frame,
        height,
        contours,
        sides,
    })
}

pub(crate) fn prismatic_gap() -> GeometryError {
    coverage_gap(
        "canonical planar profile extrusion",
        "canonical planar profile extrusion",
    )
}

fn extrusion_frame(brep: &BrepEnvelope) -> Result<ExtrusionFrame, GeometryError> {
    let (top_frame, bottom_frame) = match (
        brep.geometry
            .surfaces
            .get(brep.topology.faces[0].surface as usize),
        brep.geometry
            .surfaces
            .get(brep.topology.faces[1].surface as usize),
    ) {
        (
            Some(SurfaceGeometry::Plane { frame: top }),
            Some(SurfaceGeometry::Plane { frame: bottom }),
        ) => (*top, *bottom),
        _ => return Err(prismatic_gap()),
    };
    let height = dot(sub(top_frame.origin, bottom_frame.origin), top_frame.z);
    if height <= 4.0 * brep.accuracy.geometric {
        return Err(prismatic_gap());
    }
    let frame = Frame3 {
        origin: sub(top_frame.origin, scale(top_frame.z, height)),
        x: top_frame.x,
        y: top_frame.y,
        z: top_frame.z,
    };
    frame.validate().map_err(|_| prismatic_gap())?;
    if norm(sub(bottom_frame.origin, frame.origin)) > brep.accuracy.geometric
        || norm(sub(bottom_frame.x, frame.x)) > 1.0e-12
        || norm(sub(bottom_frame.y, scale(frame.y, -1.0))) > 1.0e-12
        || norm(sub(bottom_frame.z, scale(frame.z, -1.0))) > 1.0e-12
    {
        return Err(prismatic_gap());
    }
    Ok(ExtrusionFrame {
        top_frame,
        frame,
        height,
    })
}

fn loop_points(
    brep: &BrepEnvelope,
    loop_id: u32,
    frame: Frame3,
) -> Result<Vec<[f64; 2]>, GeometryError> {
    let start = brep
        .topology
        .loops
        .get(loop_id as usize)
        .ok_or_else(prismatic_gap)?
        .start_halfedge;
    let mut current = start;
    let mut result = Vec::new();
    loop {
        let halfedge = brep
            .topology
            .halfedges
            .get(current as usize)
            .ok_or_else(prismatic_gap)?;
        let position = brep
            .topology
            .vertices
            .get(halfedge.from as usize)
            .ok_or_else(prismatic_gap)?
            .position;
        let local = frame.local(position);
        if local[2].abs() > brep.accuracy.geometric {
            return Err(prismatic_gap());
        }
        result.push([local[0], local[1]]);
        current = halfedge.next.ok_or_else(prismatic_gap)?;
        if current == start {
            break;
        }
        if result.len() > brep.topology.halfedges.len() {
            return Err(prismatic_gap());
        }
    }
    Ok(result)
}

fn check_canonical_extrusion(
    brep: &BrepEnvelope,
    frame: Frame3,
    profile: &[Vec<[f64; 2]>],
    height: f64,
) -> Result<(), GeometryError> {
    let canonical = primitives::linear_extrusion(
        brep.id.clone(),
        frame,
        profile[0].clone(),
        profile[1..].to_vec(),
        height,
        brep.accuracy,
    )?;
    if !planar_extrusion_geometry_matches(
        &brep.geometry,
        &canonical.geometry,
        brep.accuracy.geometric,
    ) || !planar_extrusion_topology_matches(
        &brep.topology,
        &canonical.topology,
        brep.accuracy.geometric,
    )? || brep.solids != canonical.solids
    {
        return Err(prismatic_gap());
    }
    Ok(())
}

fn planar_extrusion_geometry_matches(
    actual: &GeometryStore,
    expected: &GeometryStore,
    geometric: f64,
) -> bool {
    if actual.surfaces.len() != expected.surfaces.len()
        || actual.curves.len() != expected.curves.len()
        || actual.pcurves.len() != expected.pcurves.len()
        || !actual.intersections.is_empty()
        || !expected.intersections.is_empty()
    {
        return false;
    }
    let surfaces = actual
        .surfaces
        .iter()
        .zip(&expected.surfaces)
        .all(|(actual, expected)| match (actual, expected) {
            (
                SurfaceGeometry::Plane { frame: actual },
                SurfaceGeometry::Plane { frame: expected },
            ) => frames_match(*actual, *expected, geometric),
            _ => false,
        });
    let curves = actual
        .curves
        .iter()
        .zip(&expected.curves)
        .all(|(actual, expected)| match (actual, expected) {
            (
                CurveGeometry::Line {
                    origin: actual_origin,
                    direction: actual_direction,
                },
                CurveGeometry::Line {
                    origin: expected_origin,
                    direction: expected_direction,
                },
            ) => {
                coordinates_match(*actual_origin, *expected_origin, geometric)
                    && coordinates_match(*actual_direction, *expected_direction, 1.0e-12)
            }
            _ => false,
        });
    let pcurves = actual
        .pcurves
        .iter()
        .zip(&expected.pcurves)
        .all(|(actual, expected)| match (actual, expected) {
            (
                PcurveGeometry::Line2 {
                    origin: actual_origin,
                    direction: actual_direction,
                },
                PcurveGeometry::Line2 {
                    origin: expected_origin,
                    direction: expected_direction,
                },
            ) => {
                coordinates_match(*actual_origin, *expected_origin, geometric)
                    && coordinates_match(*actual_direction, *expected_direction, 1.0e-12)
            }
            _ => false,
        });
    surfaces && curves && pcurves
}

fn planar_extrusion_topology_matches(
    actual: &Topology,
    expected: &Topology,
    geometric: f64,
) -> Result<bool, GeometryError> {
    if actual.vertices.len() != expected.vertices.len()
        || actual.edges.len() != expected.edges.len()
        || actual.halfedges.len() != expected.halfedges.len()
        || actual.loops.len() != expected.loops.len()
        || actual.faces.len() != expected.faces.len()
        || actual.shells.len() != expected.shells.len()
        || actual.wires.len() != expected.wires.len()
    {
        return Ok(false);
    }
    let mut normalized = actual.clone();
    for (vertex, expected) in normalized.vertices.iter_mut().zip(&expected.vertices) {
        if norm(sub(vertex.position, expected.position)) > geometric {
            return Ok(false);
        }
        vertex.position = expected.position;
        vertex.tolerance = expected.tolerance;
    }
    for (edge, expected) in normalized.edges.iter_mut().zip(&expected.edges) {
        edge.tolerance = expected.tolerance;
        match (&mut edge.geometry, &expected.geometry) {
            (
                EdgeGeometry::Curve {
                    curve: actual_curve,
                    range: actual_range,
                },
                EdgeGeometry::Curve {
                    curve: expected_curve,
                    range: expected_range,
                },
            ) if actual_curve == expected_curve
                && (actual_range.lo - expected_range.lo).abs() <= geometric
                && (actual_range.hi - expected_range.hi).abs() <= geometric =>
            {
                *actual_range = *expected_range;
            }
            _ => return Ok(false),
        }
    }
    for (face, expected) in normalized.faces.iter_mut().zip(&expected.faces) {
        for axis in 0..2 {
            if (face.trim.uv_bounds[axis].lo - expected.trim.uv_bounds[axis].lo).abs() > geometric
                || (face.trim.uv_bounds[axis].hi - expected.trim.uv_bounds[axis].hi).abs()
                    > geometric
            {
                return Ok(false);
            }
        }
        face.trim.uv_bounds = expected.trim.uv_bounds;
        face.key = expected.key.clone();
        face.provenance = expected.provenance.clone();
    }
    Ok(normalized == *expected)
}

pub(crate) fn brep_face_source(brep: &BrepEnvelope, face: u32) -> FaceSource {
    FaceSource {
        entity: brep.id.clone(),
        body: brep.id.clone(),
        key: brep.topology.faces[face as usize].key.clone(),
        face,
    }
}

pub(crate) fn prismatic_face_provenance(
    a: &PrismaticInput<'_>,
    b: &PrismaticInput<'_>,
    from: Point3,
    to: Point3,
    operation: BooleanOp,
    tolerance: f64,
) -> Result<FaceProvenance, GeometryError> {
    let a_sources = a
        .sides
        .iter()
        .filter(|side| segment_contains(side, from, to, tolerance))
        .map(|side| side.source.clone())
        .collect::<Vec<_>>();
    let b_sources = b
        .sides
        .iter()
        .filter(|side| segment_contains(side, from, to, tolerance))
        .map(|side| side.source.clone())
        .collect::<Vec<_>>();
    if a_sources.is_empty() && b_sources.is_empty() {
        return Err(GeometryError::InvalidTopology(
            "planar boolean produced a boundary without source ancestry".into(),
        ));
    }
    let reversed = operation == BooleanOp::Subtraction && a_sources.is_empty();
    let role = if !a_sources.is_empty() && !b_sources.is_empty() {
        FaceRole::Coincident
    } else if reversed {
        FaceRole::Cut
    } else {
        FaceRole::Split
    };
    Ok(FaceProvenance {
        sources: unique_sources(a_sources.into_iter().chain(b_sources)),
        role,
        reversed,
    })
}

fn segment_contains(side: &PrismaticSide, from: Point3, to: Point3, tolerance: f64) -> bool {
    let edge = sub(side.to, side.from);
    let length = norm(edge);
    if length <= tolerance {
        return false;
    }
    let direction = scale(edge, 1.0 / length);
    [from, to].into_iter().all(|point| {
        let delta = sub(point, side.from);
        let parameter = dot(delta, direction);
        let closest = add(side.from, scale(direction, parameter));
        norm(sub(point, closest)) <= tolerance
            && parameter >= -tolerance
            && parameter <= length + tolerance
    })
}

pub(crate) fn unique_sources(sources: impl IntoIterator<Item = FaceSource>) -> Vec<FaceSource> {
    let mut result = Vec::new();
    for source in sources {
        if !result.iter().any(|existing: &FaceSource| {
            existing.entity == source.entity
                && existing.body == source.body
                && existing.face == source.face
                && existing.key == source.key
        }) {
            result.push(source);
        }
    }
    result
}

pub(crate) fn region_profiles(region: &RingRegion) -> (Vec<[f64; 2]>, Vec<Vec<[f64; 2]>>) {
    let convert = |ring: &[Pt2]| ring.iter().map(|point| [point.x, point.z]).collect();
    (
        convert(&region.outer),
        region.holes.iter().map(|hole| convert(hole)).collect(),
    )
}

pub(crate) fn profiles_match(a: &[Vec<Pt2>], b: &[Vec<Pt2>], tolerance: f64) -> bool {
    if a.len() != b.len() || a.is_empty() || !profile_rings_match(&a[0], &b[0], tolerance) {
        return false;
    }
    let mut matched = vec![false; b.len() - 1];
    a[1..].iter().all(|ring| {
        let Some(index) = b[1..].iter().enumerate().position(|(index, candidate)| {
            !matched[index] && profile_rings_match(ring, candidate, tolerance)
        }) else {
            return false;
        };
        matched[index] = true;
        true
    })
}

fn profile_rings_match(a: &[Pt2], b: &[Pt2], tolerance: f64) -> bool {
    a.len() == b.len()
        && (0..b.len()).any(|offset| {
            a.iter().enumerate().all(|(index, point)| {
                let candidate = b[(index + offset) % b.len()];
                (point.x - candidate.x).abs() <= tolerance
                    && (point.z - candidate.z).abs() <= tolerance
            })
        })
}

pub(crate) fn prismatic_profile_sources(
    input: &PrismaticInput<'_>,
    common_frame: Frame3,
    from: Pt2,
    to: Pt2,
    tolerance: f64,
) -> Vec<FaceSource> {
    input
        .sides
        .iter()
        .filter(|side| projected_segment_contains(side, common_frame, from, to, tolerance))
        .map(|side| side.source.clone())
        .collect()
}

fn projected_segment_contains(
    side: &PrismaticSide,
    frame: Frame3,
    from: Pt2,
    to: Pt2,
    tolerance: f64,
) -> bool {
    let project = |point: Point3| {
        let local = frame.local(point);
        Pt2::new(local[0], local[1])
    };
    let start = project(side.from);
    let end = project(side.to);
    let edge = Pt2::new(end.x - start.x, end.z - start.z);
    let length = (edge.x * edge.x + edge.z * edge.z).sqrt();
    if length <= tolerance {
        return false;
    }
    let direction = Pt2::new(edge.x / length, edge.z / length);
    [from, to].into_iter().all(|point| {
        let delta = Pt2::new(point.x - start.x, point.z - start.z);
        let parameter = delta.x * direction.x + delta.z * direction.z;
        let closest = Pt2::new(
            start.x + parameter * direction.x,
            start.z + parameter * direction.z,
        );
        let distance = ((point.x - closest.x).powi(2) + (point.z - closest.z).powi(2)).sqrt();
        distance <= tolerance && parameter >= -tolerance && parameter <= length + tolerance
    })
}
