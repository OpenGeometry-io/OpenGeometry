use super::{
    face_intersection::{intersect_breps, intersect_faces, FaceView},
    geometry::{add, cross, dot, norm, scale, sub, unit},
    intersection::IntersectionDefinition,
    primitives::{self, boundary, plane_boundary, uv_line, Builder},
    query::{classify_point, face_contains_uv, PointClassification},
    ssi::intersect_surfaces,
    topology::*,
    Curve, CurveGeometry, Frame3, GeometryError, Point3, Surface, SurfaceGeometry,
};
use crate::{
    geometry::{
        boolean2d::{boolean_oriented_regions, PlanarBooleanOp, RingRegion},
        poly2d::{self, Pt2},
    },
    math::{
        interval::Interval,
        predicates::{sphere_sphere_relation, Sign},
    },
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BooleanOp {
    Union,
    Intersection,
    Subtraction,
}

#[derive(Clone, Debug, Serialize)]
pub struct FaceMapping {
    pub source: FaceSource,
    pub result_faces: Vec<u32>,
}
#[derive(Clone, Debug, Serialize)]
pub struct BooleanReport {
    pub operation: BooleanOp,
    pub quality: GeometryQuality,
    pub contacts: Vec<Point3>,
    pub coincident: bool,
    pub face_mappings: Vec<FaceMapping>,
}
pub struct BooleanResult {
    pub brep: BrepEnvelope,
    pub report: BooleanReport,
}

fn comparable<T: Serialize>(input: &T) -> Result<serde_json::Value, GeometryError> {
    serde_json::to_value(input).map_err(|error| GeometryError::InvalidGeometry(error.to_string()))
}

fn coordinates_match<const N: usize>(a: [f64; N], b: [f64; N], tolerance: f64) -> bool {
    a.into_iter()
        .zip(b)
        .all(|(left, right)| (left - right).abs() <= tolerance)
}

fn frames_match(a: Frame3, b: Frame3, geometric: f64) -> bool {
    coordinates_match(a.origin, b.origin, geometric)
        && coordinates_match(a.x, b.x, 1e-12)
        && coordinates_match(a.y, b.y, 1e-12)
        && coordinates_match(a.z, b.z, 1e-12)
}

fn cylinder_geometry_matches(
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
    let scalar = geometric.max(1e-12);
    let surfaces = actual
        .surfaces
        .iter()
        .zip(&expected.surfaces)
        .all(|(actual, expected)| match (actual, expected) {
            (
                SurfaceGeometry::Plane { frame: actual },
                SurfaceGeometry::Plane { frame: expected },
            ) => frames_match(*actual, *expected, geometric),
            (
                SurfaceGeometry::Cylinder {
                    frame: actual_frame,
                    radius: actual_radius,
                },
                SurfaceGeometry::Cylinder {
                    frame: expected_frame,
                    radius: expected_radius,
                },
            ) => {
                frames_match(*actual_frame, *expected_frame, geometric)
                    && (actual_radius - expected_radius).abs() <= geometric
            }
            (
                SurfaceGeometry::Cone {
                    frame: actual_frame,
                    semi_angle: actual_angle,
                },
                SurfaceGeometry::Cone {
                    frame: expected_frame,
                    semi_angle: expected_angle,
                },
            ) => {
                frames_match(*actual_frame, *expected_frame, geometric)
                    && (actual_angle - expected_angle).abs() <= 1.0e-12
            }
            _ => false,
        });
    let curves = actual
        .curves
        .iter()
        .zip(&expected.curves)
        .all(|(actual, expected)| match (actual, expected) {
            (
                CurveGeometry::Circle {
                    frame: actual_frame,
                    radius: actual_radius,
                },
                CurveGeometry::Circle {
                    frame: expected_frame,
                    radius: expected_radius,
                },
            ) => {
                frames_match(*actual_frame, *expected_frame, geometric)
                    && (actual_radius - expected_radius).abs() <= geometric
            }
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
                    && coordinates_match(*actual_direction, *expected_direction, 1e-12)
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
                coordinates_match(*actual_origin, *expected_origin, scalar)
                    && coordinates_match(*actual_direction, *expected_direction, 1e-12)
            }
            (
                PcurveGeometry::Conic2 {
                    origin: actual_origin,
                    axis_a: actual_a,
                    axis_b: actual_b,
                },
                PcurveGeometry::Conic2 {
                    origin: expected_origin,
                    axis_a: expected_a,
                    axis_b: expected_b,
                },
            ) => {
                coordinates_match(*actual_origin, *expected_origin, scalar)
                    && coordinates_match(*actual_a, *expected_a, scalar)
                    && coordinates_match(*actual_b, *expected_b, scalar)
            }
            _ => false,
        });
    surfaces && curves && pcurves
}

fn normalize_cylinder_topology(actual: &mut Topology, expected: &Topology, geometric: f64) -> bool {
    if actual.vertices.len() != expected.vertices.len()
        || actual.edges.len() != expected.edges.len()
        || actual.faces.len() != expected.faces.len()
    {
        return false;
    }
    let scalar = geometric.max(1e-12);
    for (actual, expected) in actual.edges.iter_mut().zip(&expected.edges) {
        match (&mut actual.geometry, &expected.geometry) {
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
                && (actual_range.lo - expected_range.lo).abs() <= scalar
                && (actual_range.hi - expected_range.hi).abs() <= scalar =>
            {
                *actual_range = *expected_range;
            }
            (
                EdgeGeometry::Collapsed { vertex: actual },
                EdgeGeometry::Collapsed { vertex: expected },
            ) if actual == expected => {}
            _ => return false,
        }
    }
    for (actual, expected) in actual.faces.iter_mut().zip(&expected.faces) {
        for axis in 0..2 {
            let a = &mut actual.trim.uv_bounds[axis];
            let e = expected.trim.uv_bounds[axis];
            if (a.lo - e.lo).abs() > scalar || (a.hi - e.hi).abs() > scalar {
                return false;
            }
            *a = e;
        }
    }
    true
}

struct SphereInput<'a> {
    brep: &'a BrepEnvelope,
    frame: Frame3,
    radius: f64,
}

struct CylinderInput<'a> {
    brep: &'a BrepEnvelope,
    frame: Frame3,
    radius: f64,
    height: f64,
}

struct TorusInput<'a> {
    brep: &'a BrepEnvelope,
    frame: Frame3,
    major_radius: f64,
    minor_radius: f64,
}

struct ConicSectionInput<'a> {
    brep: &'a BrepEnvelope,
    frame: Frame3,
    semi_angle: f64,
    axial_range: Interval,
    lower_radius: f64,
    upper_radius: f64,
}

fn torus_gap() -> GeometryError {
    GeometryError::CoverageGap {
        families: ["canonical full ring torus".into(), "analytic solid".into()],
    }
}

fn full_torus(brep: &BrepEnvelope) -> Result<TorusInput<'_>, GeometryError> {
    brep.validate()?;
    let (frame, major_radius, minor_radius) = match brep.geometry.surfaces.as_slice() {
        [SurfaceGeometry::Torus {
            frame,
            major_radius,
            minor_radius,
        }] => (*frame, *major_radius, *minor_radius),
        _ => return Err(torus_gap()),
    };
    if !matches!(brep.quality, GeometryQuality::Analytic)
        || !brep.geometry.intersections.is_empty()
        || brep.topology.vertices.len() != 1
        || brep.topology.edges.len() != 2
        || brep.topology.halfedges.len() != 4
        || brep.topology.loops.len() != 1
        || brep.topology.faces.len() != 1
        || !brep.topology.wires.is_empty()
        || brep.topology.shells.len() != 1
        || brep.solids.len() != 1
    {
        return Err(torus_gap());
    }
    let canonical = primitives::torus(
        brep.id.clone(),
        frame,
        major_radius,
        minor_radius,
        brep.accuracy,
    )?;
    let mut topology = brep.topology.clone();
    for (actual, expected) in topology
        .vertices
        .iter_mut()
        .zip(&canonical.topology.vertices)
    {
        if norm(sub(actual.position, expected.position)) > brep.accuracy.geometric {
            return Err(torus_gap());
        }
        actual.position = expected.position;
        actual.tolerance = expected.tolerance;
    }
    for (actual, expected) in topology.edges.iter_mut().zip(&canonical.topology.edges) {
        actual.tolerance = expected.tolerance;
        let (
            EdgeGeometry::Curve {
                curve: actual_curve,
                range: actual_range,
            },
            EdgeGeometry::Curve {
                curve: expected_curve,
                range: expected_range,
            },
        ) = (&mut actual.geometry, &expected.geometry)
        else {
            return Err(torus_gap());
        };
        if actual_curve != expected_curve
            || (actual_range.lo - expected_range.lo).abs() > brep.accuracy.geometric
            || (actual_range.hi - expected_range.hi).abs() > brep.accuracy.geometric
        {
            return Err(torus_gap());
        }
        *actual_range = *expected_range;
    }
    topology.faces[0].key = canonical.topology.faces[0].key.clone();
    topology.faces[0].provenance = canonical.topology.faces[0].provenance.clone();
    topology.faces[0].trim.uv_bounds = canonical.topology.faces[0].trim.uv_bounds;
    if comparable(&brep.geometry)? != comparable(&canonical.geometry)?
        || comparable(&topology)? != comparable(&canonical.topology)?
        || comparable(&brep.solids)? != comparable(&canonical.solids)?
    {
        return Err(torus_gap());
    }
    Ok(TorusInput {
        brep,
        frame,
        major_radius,
        minor_radius,
    })
}

fn cylinder_gap() -> GeometryError {
    GeometryError::CoverageGap {
        families: [
            "canonical full cylinder".into(),
            "canonical full cylinder".into(),
        ],
    }
}

fn full_cylinder(brep: &BrepEnvelope) -> Result<CylinderInput<'_>, GeometryError> {
    brep.validate()?;
    let (frame, radius) = match brep.geometry.surfaces.as_slice() {
        [SurfaceGeometry::Cylinder { frame, radius }, SurfaceGeometry::Plane { .. }, SurfaceGeometry::Plane { .. }] => {
            (*frame, *radius)
        }
        _ => return Err(cylinder_gap()),
    };
    if !matches!(brep.quality, GeometryQuality::Analytic)
        || !brep.geometry.intersections.is_empty()
        || brep.topology.faces.len() != 3
        || brep.topology.vertices.len() != 2
        || brep.topology.edges.len() != 3
        || brep.topology.halfedges.len() != 6
        || brep.topology.loops.len() != 3
        || !brep.topology.wires.is_empty()
        || brep.topology.shells.len() != 1
        || brep.solids.len() != 1
    {
        return Err(cylinder_gap());
    }
    let axial_range = brep.topology.faces[0].trim.uv_bounds[1];
    if axial_range.lo != 0.0 {
        return Err(cylinder_gap());
    }
    let height = axial_range.hi;
    let canonical = primitives::cylinder(brep.id.clone(), frame, radius, height, brep.accuracy)?;
    let mut topology = brep.topology.clone();
    for (face, expected) in topology.faces.iter_mut().zip(&canonical.topology.faces) {
        face.key = expected.key.clone();
        face.provenance = expected.provenance.clone();
    }
    for vertex in &mut topology.vertices {
        vertex.tolerance = brep.accuracy.geometric;
        let expected = canonical.topology.vertices[vertex.id as usize].position;
        if norm(sub(vertex.position, expected)) > brep.accuracy.geometric {
            return Err(cylinder_gap());
        }
        vertex.position = expected;
    }
    for edge in &mut topology.edges {
        edge.tolerance = brep.accuracy.geometric;
    }
    if !normalize_cylinder_topology(&mut topology, &canonical.topology, brep.accuracy.geometric) {
        return Err(cylinder_gap());
    }
    if !cylinder_geometry_matches(&brep.geometry, &canonical.geometry, brep.accuracy.geometric)
        || comparable(&topology)? != comparable(&canonical.topology)?
        || comparable(&brep.solids)? != comparable(&canonical.solids)?
    {
        return Err(cylinder_gap());
    }
    Ok(CylinderInput {
        brep,
        frame,
        radius,
        height,
    })
}

fn conic_section_gap() -> GeometryError {
    GeometryError::CoverageGap {
        families: ["canonical cone or frustum".into(), "analytic solid".into()],
    }
}

fn full_conic_section(brep: &BrepEnvelope) -> Result<ConicSectionInput<'_>, GeometryError> {
    brep.validate()?;
    let (frame, semi_angle) = match brep.geometry.surfaces.first() {
        Some(SurfaceGeometry::Cone { frame, semi_angle }) => (*frame, *semi_angle),
        _ => return Err(conic_section_gap()),
    };
    if !matches!(brep.quality, GeometryQuality::Analytic)
        || !brep.geometry.intersections.is_empty()
        || !brep.topology.wires.is_empty()
        || brep.topology.shells.len() != 1
        || brep.solids.len() != 1
        || !brep.solids[0].cavity_shells.is_empty()
        || !matches!(brep.geometry.surfaces.len(), 2 | 3)
        || brep.geometry.surfaces[1..]
            .iter()
            .any(|surface| !matches!(surface, SurfaceGeometry::Plane { .. }))
        || brep.topology.faces.len() != brep.geometry.surfaces.len()
    {
        return Err(conic_section_gap());
    }
    let axial_range = brep.topology.faces[0].trim.uv_bounds[1];
    if axial_range.lo < 0.0
        || axial_range.hi <= axial_range.lo
        || semi_angle <= 0.0
        || semi_angle >= std::f64::consts::FRAC_PI_2
    {
        return Err(conic_section_gap());
    }
    let slope = semi_angle.tan();
    let lower_radius = axial_range.lo * slope;
    let upper_radius = axial_range.hi * slope;
    let canonical = if axial_range.lo <= brep.accuracy.geometric {
        if axial_range.lo != 0.0 || brep.geometry.surfaces.len() != 2 {
            return Err(conic_section_gap());
        }
        let base_frame = Frame3 {
            origin: frame.point([0.0, 0.0, axial_range.hi]),
            x: frame.x,
            y: scale(frame.y, -1.0),
            z: scale(frame.z, -1.0),
        };
        primitives::cone(
            brep.id.clone(),
            base_frame,
            upper_radius,
            axial_range.hi,
            brep.accuracy,
        )?
    } else {
        if brep.geometry.surfaces.len() != 3 {
            return Err(conic_section_gap());
        }
        let base_frame = Frame3 {
            origin: frame.point([0.0, 0.0, axial_range.lo]),
            ..frame
        };
        primitives::frustum(
            brep.id.clone(),
            base_frame,
            lower_radius,
            upper_radius,
            axial_range.width(),
            brep.accuracy,
        )?
    };
    let mut topology = brep.topology.clone();
    for (face, expected) in topology.faces.iter_mut().zip(&canonical.topology.faces) {
        face.key = expected.key.clone();
        face.provenance = expected.provenance.clone();
    }
    for vertex in &mut topology.vertices {
        vertex.tolerance = brep.accuracy.geometric;
        let expected = canonical
            .topology
            .vertices
            .get(vertex.id as usize)
            .ok_or_else(conic_section_gap)?
            .position;
        if norm(sub(vertex.position, expected)) > brep.accuracy.geometric {
            return Err(conic_section_gap());
        }
        vertex.position = expected;
    }
    for edge in &mut topology.edges {
        edge.tolerance = brep.accuracy.geometric;
    }
    if !normalize_cylinder_topology(&mut topology, &canonical.topology, brep.accuracy.geometric)
        || !cylinder_geometry_matches(&brep.geometry, &canonical.geometry, brep.accuracy.geometric)
        || comparable(&topology)? != comparable(&canonical.topology)?
        || comparable(&brep.solids)? != comparable(&canonical.solids)?
    {
        return Err(conic_section_gap());
    }
    Ok(ConicSectionInput {
        brep,
        frame,
        semi_angle,
        axial_range,
        lower_radius,
        upper_radius,
    })
}
fn gap() -> GeometryError {
    GeometryError::CoverageGap {
        families: ["full sphere input".into(), "full sphere input".into()],
    }
}
fn full_sphere(brep: &BrepEnvelope) -> Result<SphereInput<'_>, GeometryError> {
    brep.validate()?;
    let (frame, radius) = match brep.geometry.surfaces.as_slice() {
        [SurfaceGeometry::Sphere { frame, radius }] => (*frame, *radius),
        _ => return Err(gap()),
    };
    if !matches!(brep.quality, GeometryQuality::Analytic)
        || brep.geometry.curves.len() != 1
        || brep.geometry.pcurves.len() != 4
        || !brep.geometry.intersections.is_empty()
        || brep.topology.faces.len() != 1
        || brep.topology.vertices.len() != 2
        || brep.topology.edges.len() != 3
        || brep.topology.halfedges.len() != 4
        || brep.topology.loops.len() != 1
        || !brep.topology.wires.is_empty()
        || brep.topology.shells.len() != 1
        || brep.solids.len() != 1
    {
        return Err(gap());
    }
    let canonical = primitives::sphere(brep.id.clone(), frame, radius, brep.accuracy)?;
    let mut topology = brep.topology.clone();
    topology.faces[0].key = canonical.topology.faces[0].key.clone();
    topology.faces[0].provenance = canonical.topology.faces[0].provenance.clone();
    for vertex in &mut topology.vertices {
        vertex.tolerance = brep.accuracy.geometric;
        let expected = canonical.topology.vertices[vertex.id as usize].position;
        if norm(sub(vertex.position, expected)) > brep.accuracy.geometric {
            return Err(gap());
        }
        vertex.position = expected;
    }
    for edge in &mut topology.edges {
        edge.tolerance = brep.accuracy.geometric;
    }
    let value = |input| {
        serde_json::to_value(input).map_err(|e| GeometryError::InvalidGeometry(e.to_string()))
    };
    if value(&brep.geometry)? != value(&canonical.geometry)?
        || serde_json::to_value(&topology)
            .map_err(|e| GeometryError::InvalidGeometry(e.to_string()))?
            != serde_json::to_value(&canonical.topology)
                .map_err(|e| GeometryError::InvalidGeometry(e.to_string()))?
        || serde_json::to_value(&brep.solids)
            .map_err(|e| GeometryError::InvalidGeometry(e.to_string()))?
            != serde_json::to_value(&canonical.solids)
                .map_err(|e| GeometryError::InvalidGeometry(e.to_string()))?
    {
        return Err(gap());
    }
    Ok(SphereInput {
        brep,
        frame,
        radius,
    })
}
fn source(input: &SphereInput<'_>) -> FaceSource {
    FaceSource {
        entity: input.brep.id.clone(),
        body: input.brep.id.clone(),
        key: input.brep.topology.faces[0].key.clone(),
        face: 0,
    }
}
fn provenance(input: &SphereInput<'_>, role: FaceRole, reversed: bool) -> FaceProvenance {
    FaceProvenance {
        sources: vec![source(input)],
        role,
        reversed,
    }
}
fn reverse(sense: Orientation) -> Orientation {
    if sense == Orientation::Forward {
        Orientation::Reverse
    } else {
        Orientation::Forward
    }
}
fn reverse_face(brep: &mut BrepEnvelope, face: u32) {
    brep.topology.faces[face as usize].sense = reverse(brep.topology.faces[face as usize].sense);
    for h in &mut brep.topology.halfedges {
        if h.face == Some(face) {
            std::mem::swap(&mut h.from, &mut h.to);
            std::mem::swap(&mut h.next, &mut h.prev);
            h.geometry_use.sense = reverse(h.geometry_use.sense);
        }
    }
    for vertex in &mut brep.topology.vertices {
        vertex.outgoing_halfedge = brep
            .topology
            .halfedges
            .iter()
            .find(|h| h.from == vertex.id)
            .map(|h| h.id);
    }
}

fn append_analytic_input(
    out: &mut BrepEnvelope,
    input: &BrepEnvelope,
) -> Result<Vec<FaceMapping>, GeometryError> {
    let offset = |value: usize, table: &str| {
        u32::try_from(value).map_err(|_| GeometryError::LimitExceeded(table.into()))
    };
    let surface_offset = offset(out.geometry.surfaces.len(), "surface IDs")?;
    let curve_offset = offset(out.geometry.curves.len(), "curve IDs")?;
    let pcurve_offset = offset(out.geometry.pcurves.len(), "pcurve IDs")?;
    let intersection_offset = offset(out.geometry.intersections.len(), "intersection IDs")?;
    let vertex_offset = offset(out.topology.vertices.len(), "vertex IDs")?;
    let edge_offset = offset(out.topology.edges.len(), "edge IDs")?;
    let halfedge_offset = offset(out.topology.halfedges.len(), "halfedge IDs")?;
    let loop_offset = offset(out.topology.loops.len(), "loop IDs")?;
    let face_offset = offset(out.topology.faces.len(), "face IDs")?;
    let wire_offset = offset(out.topology.wires.len(), "wire IDs")?;
    let shell_offset = offset(out.topology.shells.len(), "shell IDs")?;

    out.geometry
        .surfaces
        .extend(input.geometry.surfaces.clone());
    out.geometry
        .intersections
        .extend(
            input
                .geometry
                .intersections
                .iter()
                .cloned()
                .map(|mut definition| {
                    definition.surfaces = definition.surfaces.map(|id| id + surface_offset);
                    definition
                }),
        );
    out.geometry
        .curves
        .extend(input.geometry.curves.iter().cloned().map(|mut curve| {
            if let CurveGeometry::Intersection { definition } = &mut curve {
                *definition += intersection_offset;
            }
            curve
        }));
    out.geometry
        .pcurves
        .extend(input.geometry.pcurves.iter().cloned().map(|mut pcurve| {
            match &mut pcurve {
                PcurveGeometry::ProjectedCurve { curve, surface, .. } => {
                    *curve += curve_offset;
                    *surface += surface_offset;
                }
                PcurveGeometry::IntersectionSide { definition, .. } => {
                    *definition += intersection_offset;
                }
                PcurveGeometry::Line2 { .. } | PcurveGeometry::Conic2 { .. } => {}
            }
            pcurve
        }));

    out.topology
        .vertices
        .extend(input.topology.vertices.iter().cloned().map(|mut vertex| {
            vertex.id += vertex_offset;
            vertex.outgoing_halfedge = vertex.outgoing_halfedge.map(|id| id + halfedge_offset);
            vertex
        }));
    out.topology
        .edges
        .extend(input.topology.edges.iter().cloned().map(|mut edge| {
            edge.id += edge_offset;
            edge.halfedge += halfedge_offset;
            edge.twin_halfedge = edge.twin_halfedge.map(|id| id + halfedge_offset);
            match &mut edge.geometry {
                EdgeGeometry::Curve { curve, .. } => *curve += curve_offset,
                EdgeGeometry::Collapsed { vertex } => *vertex += vertex_offset,
            }
            edge
        }));
    out.topology.halfedges.extend(
        input
            .topology
            .halfedges
            .iter()
            .cloned()
            .map(|mut halfedge| {
                halfedge.id += halfedge_offset;
                halfedge.from += vertex_offset;
                halfedge.to += vertex_offset;
                halfedge.edge += edge_offset;
                halfedge.twin = halfedge.twin.map(|id| id + halfedge_offset);
                halfedge.next = halfedge.next.map(|id| id + halfedge_offset);
                halfedge.prev = halfedge.prev.map(|id| id + halfedge_offset);
                halfedge.face = halfedge.face.map(|id| id + face_offset);
                halfedge.loop_ref = halfedge.loop_ref.map(|id| id + loop_offset);
                halfedge.wire_ref = halfedge.wire_ref.map(|id| id + wire_offset);
                halfedge.geometry_use.pcurve =
                    halfedge.geometry_use.pcurve.map(|id| id + pcurve_offset);
                halfedge
            }),
    );
    out.topology
        .loops
        .extend(input.topology.loops.iter().cloned().map(|mut loop_| {
            loop_.id += loop_offset;
            loop_.start_halfedge += halfedge_offset;
            loop_.face_ref += face_offset;
            loop_
        }));

    let mut mappings = Vec::with_capacity(input.topology.faces.len());
    out.topology
        .faces
        .extend(input.topology.faces.iter().cloned().map(|mut face| {
            let source = FaceSource {
                entity: input.id.clone(),
                body: input.id.clone(),
                key: face.key.clone(),
                face: face.id,
            };
            face.id += face_offset;
            face.key = format!("{}:{}", input.id, face.key);
            face.surface += surface_offset;
            face.trim.outer += loop_offset;
            for hole in &mut face.trim.holes {
                *hole += loop_offset;
            }
            face.shell_ref = face.shell_ref.map(|id| id + shell_offset);
            face.provenance = FaceProvenance {
                sources: vec![source.clone()],
                role: FaceRole::Preserved,
                reversed: false,
            };
            mappings.push(FaceMapping {
                source,
                result_faces: vec![face.id],
            });
            face
        }));
    out.topology
        .wires
        .extend(input.topology.wires.iter().cloned().map(|mut wire| {
            wire.id += wire_offset;
            wire.start_halfedge += halfedge_offset;
            wire
        }));
    out.topology
        .shells
        .extend(input.topology.shells.iter().cloned().map(|mut shell| {
            shell.id += shell_offset;
            for face in &mut shell.faces {
                *face += face_offset;
            }
            shell
        }));
    out.solids
        .extend(input.solids.iter().cloned().map(|mut solid| {
            solid.outer_shell += shell_offset;
            for shell in &mut solid.cavity_shells {
                *shell += shell_offset;
            }
            solid
        }));
    Ok(mappings)
}

fn disjoint_boolean(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<Option<BooleanResult>, GeometryError> {
    let (Some(a_bounds), Some(b_bounds)) = (a.bounds()?, b.bounds()?) else {
        return Ok(None);
    };
    let clearance = 4.0 * a.accuracy.geometric.max(b.accuracy.geometric);
    let separated = (0..3).any(|axis| {
        a_bounds.axes[axis].hi + clearance < b_bounds.axes[axis].lo
            || b_bounds.axes[axis].hi + clearance < a_bounds.axes[axis].lo
    });
    if !separated {
        return Ok(None);
    }
    Ok(Some(separate_boolean(a, b, operation, id, Vec::new())?))
}

fn separate_boolean(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
    contacts: Vec<Point3>,
) -> Result<BooleanResult, GeometryError> {
    if !matches!(a.quality, GeometryQuality::Analytic)
        || !matches!(b.quality, GeometryQuality::Analytic)
    {
        return Err(GeometryError::UnsupportedGeometry(
            "analytic boolean operands must have analytic quality".into(),
        ));
    }
    let accuracy = Accuracy {
        geometric: a.accuracy.geometric.max(b.accuracy.geometric),
        intersection: a.accuracy.intersection.max(b.accuracy.intersection),
        tessellation: a.accuracy.tessellation.max(b.accuracy.tessellation),
        exchange: a.accuracy.exchange.max(b.accuracy.exchange),
    };
    let mut out = BrepEnvelope::new(id, accuracy)?;
    let mut mappings = match operation {
        BooleanOp::Union | BooleanOp::Subtraction => append_analytic_input(&mut out, a)?,
        BooleanOp::Intersection => a
            .topology
            .faces
            .iter()
            .map(|face| FaceMapping {
                source: FaceSource {
                    entity: a.id.clone(),
                    body: a.id.clone(),
                    key: face.key.clone(),
                    face: face.id,
                },
                result_faces: Vec::new(),
            })
            .collect(),
    };
    match operation {
        BooleanOp::Union => mappings.extend(append_analytic_input(&mut out, b)?),
        BooleanOp::Intersection | BooleanOp::Subtraction => {
            mappings.extend(b.topology.faces.iter().map(|face| FaceMapping {
                source: FaceSource {
                    entity: b.id.clone(),
                    body: b.id.clone(),
                    key: face.key.clone(),
                    face: face.id,
                },
                result_faces: Vec::new(),
            }))
        }
    }
    out.revision = a
        .revision
        .max(b.revision)
        .checked_add(1)
        .ok_or_else(|| GeometryError::LimitExceeded("boolean revision overflow".into()))?;
    out.validate()?;
    Ok(BooleanResult {
        brep: out,
        report: BooleanReport {
            operation,
            quality: GeometryQuality::Analytic,
            contacts,
            coincident: false,
            face_mappings: mappings,
        },
    })
}

pub(super) fn analytic_face_mappings<'a>(
    out: &BrepEnvelope,
    inputs: impl IntoIterator<Item = &'a BrepEnvelope>,
) -> Vec<FaceMapping> {
    inputs
        .into_iter()
        .flat_map(|input| {
            input.topology.faces.iter().map(|face| {
                let source = FaceSource {
                    entity: input.id.clone(),
                    body: input.id.clone(),
                    key: face.key.clone(),
                    face: face.id,
                };
                let result_faces = out
                    .topology
                    .faces
                    .iter()
                    .filter(|result| {
                        result.provenance.sources.iter().any(|candidate| {
                            candidate.entity == source.entity
                                && candidate.body == source.body
                                && candidate.key == source.key
                                && candidate.face == source.face
                        })
                    })
                    .map(|result| result.id)
                    .collect();
                FaceMapping {
                    source,
                    result_faces,
                }
            })
        })
        .collect()
}

#[derive(Clone)]
struct PrismaticSide {
    from: Point3,
    to: Point3,
    source: FaceSource,
}

pub(super) struct PrismaticInput<'a> {
    pub(super) brep: &'a BrepEnvelope,
    pub(super) frame: Frame3,
    pub(super) height: f64,
    pub(super) contours: Vec<Vec<Point3>>,
    sides: Vec<PrismaticSide>,
}

fn prismatic_gap() -> GeometryError {
    GeometryError::CoverageGap {
        families: [
            "canonical planar profile extrusion".into(),
            "canonical planar profile extrusion".into(),
        ],
    }
}

pub(super) fn brep_face_source(brep: &BrepEnvelope, face: u32) -> FaceSource {
    FaceSource {
        entity: brep.id.clone(),
        body: brep.id.clone(),
        key: brep.topology.faces[face as usize].key.clone(),
        face,
    }
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
    Ok(comparable(&normalized)? == comparable(expected)?)
}

pub(super) fn full_planar_extrusion(
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
    let top = &brep.topology.faces[0];
    let mut profile = Vec::with_capacity(top.trim.holes.len() + 1);
    profile.push(loop_points(brep, top.trim.outer, top_frame)?);
    for &hole in &top.trim.holes {
        profile.push(loop_points(brep, hole, top_frame)?);
    }
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
    )? || comparable(&brep.solids)? != comparable(&canonical.solids)?
    {
        return Err(prismatic_gap());
    }
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

fn unique_sources(sources: impl IntoIterator<Item = FaceSource>) -> Vec<FaceSource> {
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

fn prismatic_face_provenance(
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

fn region_profiles(region: &RingRegion) -> (Vec<[f64; 2]>, Vec<Vec<[f64; 2]>>) {
    let convert = |ring: &[Pt2]| ring.iter().map(|point| [point.x, point.z]).collect();
    (
        convert(&region.outer),
        region.holes.iter().map(|hole| convert(hole)).collect(),
    )
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

fn profiles_match(a: &[Vec<Pt2>], b: &[Vec<Pt2>], tolerance: f64) -> bool {
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

pub(super) fn prismatic_profile_sources(
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

#[derive(Clone)]
struct PrismaticAxialSegment {
    lo: f64,
    hi: f64,
    lower: FaceProvenance,
    upper: FaceProvenance,
    include_a_sides: bool,
    include_b_sides: bool,
    side_role: FaceRole,
}

fn axial_profile_boolean(
    a: &PrismaticInput<'_>,
    b: &PrismaticInput<'_>,
    a_contours: &[Vec<Pt2>],
    a_span: [f64; 2],
    b_span: [f64; 2],
    operation: BooleanOp,
    id: String,
    accuracy: Accuracy,
) -> Result<BooleanResult, GeometryError> {
    let a_caps = [brep_face_source(a.brep, 1), brep_face_source(a.brep, 0)];
    let b_caps = [brep_face_source(b.brep, 1), brep_face_source(b.brep, 0)];
    let cap_at = |boundary: f64, span: [f64; 2], caps: &[FaceSource; 2]| {
        if (boundary - span[0]).abs() <= accuracy.geometric {
            Some(caps[0].clone())
        } else if (boundary - span[1]).abs() <= accuracy.geometric {
            Some(caps[1].clone())
        } else {
            None
        }
    };
    let preserved_cap = |sources: Vec<FaceSource>| {
        let role = if sources.len() > 1 {
            FaceRole::Coincident
        } else {
            FaceRole::Preserved
        };
        FaceProvenance {
            sources,
            role,
            reversed: false,
        }
    };
    let overlap = a_span[1].min(b_span[1]) - a_span[0].max(b_span[0]);
    if overlap != 0.0 && overlap.abs() <= 4.0 * accuracy.geometric {
        return Err(GeometryError::UnresolvedIntersection(
            "profile extrusion axial overlap or clearance is below geometric resolution".into(),
        ));
    }
    let mut segments = Vec::new();
    match operation {
        BooleanOp::Union if overlap >= 0.0 => {
            let lo = a_span[0].min(b_span[0]);
            let hi = a_span[1].max(b_span[1]);
            let lower = [cap_at(lo, a_span, &a_caps), cap_at(lo, b_span, &b_caps)]
                .into_iter()
                .flatten()
                .collect();
            let upper = [cap_at(hi, a_span, &a_caps), cap_at(hi, b_span, &b_caps)]
                .into_iter()
                .flatten()
                .collect();
            segments.push(PrismaticAxialSegment {
                lo,
                hi,
                lower: preserved_cap(lower),
                upper: preserved_cap(upper),
                include_a_sides: true,
                include_b_sides: true,
                side_role: FaceRole::Coincident,
            });
        }
        BooleanOp::Union => {
            for (span, caps, include_a_sides, include_b_sides) in [
                (a_span, a_caps.clone(), true, false),
                (b_span, b_caps.clone(), false, true),
            ] {
                segments.push(PrismaticAxialSegment {
                    lo: span[0],
                    hi: span[1],
                    lower: preserved_cap(vec![caps[0].clone()]),
                    upper: preserved_cap(vec![caps[1].clone()]),
                    include_a_sides,
                    include_b_sides,
                    side_role: FaceRole::Preserved,
                });
            }
        }
        BooleanOp::Intersection if overlap > 0.0 => {
            let lo = a_span[0].max(b_span[0]);
            let hi = a_span[1].min(b_span[1]);
            let lower = [cap_at(lo, a_span, &a_caps), cap_at(lo, b_span, &b_caps)]
                .into_iter()
                .flatten()
                .collect();
            let upper = [cap_at(hi, a_span, &a_caps), cap_at(hi, b_span, &b_caps)]
                .into_iter()
                .flatten()
                .collect();
            segments.push(PrismaticAxialSegment {
                lo,
                hi,
                lower: preserved_cap(lower),
                upper: preserved_cap(upper),
                include_a_sides: true,
                include_b_sides: true,
                side_role: FaceRole::Coincident,
            });
        }
        BooleanOp::Intersection => {}
        BooleanOp::Subtraction if overlap <= 0.0 => {
            segments.push(PrismaticAxialSegment {
                lo: a_span[0],
                hi: a_span[1],
                lower: preserved_cap(vec![a_caps[0].clone()]),
                upper: preserved_cap(vec![a_caps[1].clone()]),
                include_a_sides: true,
                include_b_sides: false,
                side_role: FaceRole::Preserved,
            });
        }
        BooleanOp::Subtraction => {
            if b_span[0] > a_span[0] {
                segments.push(PrismaticAxialSegment {
                    lo: a_span[0],
                    hi: b_span[0].min(a_span[1]),
                    lower: preserved_cap(vec![a_caps[0].clone()]),
                    upper: FaceProvenance {
                        sources: vec![b_caps[0].clone()],
                        role: FaceRole::Cut,
                        reversed: true,
                    },
                    include_a_sides: true,
                    include_b_sides: false,
                    side_role: FaceRole::Split,
                });
            }
            if b_span[1] < a_span[1] {
                segments.push(PrismaticAxialSegment {
                    lo: b_span[1].max(a_span[0]),
                    hi: a_span[1],
                    lower: FaceProvenance {
                        sources: vec![b_caps[1].clone()],
                        role: FaceRole::Cut,
                        reversed: true,
                    },
                    upper: preserved_cap(vec![a_caps[1].clone()]),
                    include_a_sides: true,
                    include_b_sides: false,
                    side_role: FaceRole::Split,
                });
            }
        }
    }

    let outer = a_contours[0]
        .iter()
        .map(|point| [point.x, point.z])
        .collect::<Vec<_>>();
    let holes = a_contours[1..]
        .iter()
        .map(|ring| ring.iter().map(|point| [point.x, point.z]).collect())
        .collect::<Vec<Vec<_>>>();
    let mut out = BrepEnvelope::new(id, accuracy)?;
    for (index, segment) in segments.into_iter().enumerate() {
        let mut frame = a.frame;
        frame.origin = a.frame.point([0.0, 0.0, segment.lo]);
        let part = primitives::linear_extrusion(
            format!("{}:axial-region-{index}", out.id),
            frame,
            outer.clone(),
            holes.clone(),
            segment.hi - segment.lo,
            accuracy,
        )?;
        let face_offset = out.topology.faces.len() as u32;
        append_analytic_input(&mut out, &part)?;
        out.topology.faces[face_offset as usize].provenance = segment.upper;
        out.topology.faces[(face_offset + 1) as usize].provenance = segment.lower;
        let mut local_face = 2_u32;
        for ring in a_contours {
            for edge in 0..ring.len() {
                let from = ring[edge];
                let to = ring[(edge + 1) % ring.len()];
                let mut sources = Vec::new();
                if segment.include_a_sides {
                    sources.extend(prismatic_profile_sources(
                        a,
                        a.frame,
                        from,
                        to,
                        accuracy.geometric,
                    ));
                }
                if segment.include_b_sides {
                    sources.extend(prismatic_profile_sources(
                        b,
                        a.frame,
                        from,
                        to,
                        accuracy.geometric,
                    ));
                }
                let sources = unique_sources(sources);
                if sources.is_empty() {
                    return Err(GeometryError::InvalidTopology(
                        "axial profile boolean produced a side without source ancestry".into(),
                    ));
                }
                out.topology.faces[(face_offset + local_face) as usize].provenance =
                    FaceProvenance {
                        sources,
                        role: segment.side_role,
                        reversed: false,
                    };
                local_face += 1;
            }
        }
    }
    out.revision = a
        .brep
        .revision
        .max(b.brep.revision)
        .checked_add(1)
        .ok_or_else(|| GeometryError::LimitExceeded("boolean revision overflow".into()))?;
    out.validate()?;
    let face_mappings = analytic_face_mappings(&out, [a.brep, b.brep]);
    Ok(BooleanResult {
        brep: out,
        report: BooleanReport {
            operation,
            quality: GeometryQuality::Analytic,
            contacts: Vec::new(),
            coincident: overlap >= 0.0,
            face_mappings,
        },
    })
}

fn boolean_planar_extrusions(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let a = full_planar_extrusion(a)?;
    let b = full_planar_extrusion(b)?;
    let accuracy = Accuracy {
        geometric: a.brep.accuracy.geometric.max(b.brep.accuracy.geometric),
        intersection: a
            .brep
            .accuracy
            .intersection
            .max(b.brep.accuracy.intersection),
        tessellation: a
            .brep
            .accuracy
            .tessellation
            .max(b.brep.accuracy.tessellation),
        exchange: a.brep.accuracy.exchange.max(b.brep.accuracy.exchange),
    };
    if dot(a.frame.z, b.frame.z) < 1.0 - 1.0e-12 {
        return Err(prismatic_gap());
    }
    let to_common = |contours: &[Vec<Point3>]| {
        contours
            .iter()
            .map(|ring| {
                ring.iter()
                    .map(|point| {
                        let local = a.frame.local(*point);
                        Pt2::new(local[0], local[1])
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    };
    let a_contours = to_common(&a.contours);
    let b_contours = to_common(&b.contours);
    let b_lo = dot(sub(b.frame.origin, a.frame.origin), a.frame.z);
    let b_hi = b_lo + b.height;
    if b.contours
        .iter()
        .flatten()
        .any(|point| (a.frame.local(*point)[2] - b_lo).abs() > accuracy.geometric)
    {
        return Err(prismatic_gap());
    }
    let a_span = [0.0, a.height];
    let b_span = [b_lo, b_hi];
    let coextensive =
        b_lo.abs() <= accuracy.geometric && (a.height - b_hi).abs() <= accuracy.geometric;
    if !coextensive {
        if !profiles_match(&a_contours, &b_contours, accuracy.geometric) {
            return Err(prismatic_gap());
        }
        return axial_profile_boolean(&a, &b, &a_contours, a_span, b_span, operation, id, accuracy);
    }
    let planar_operation = match operation {
        BooleanOp::Union => PlanarBooleanOp::Union,
        BooleanOp::Intersection => PlanarBooleanOp::Intersection,
        BooleanOp::Subtraction => PlanarBooleanOp::Subtraction,
    };
    let regions = boolean_oriented_regions(
        &a_contours,
        &b_contours,
        planar_operation,
        accuracy.intersection,
    );
    let mut out = BrepEnvelope::new(id, accuracy)?;
    for (region_index, region) in regions.iter().enumerate() {
        let (outer, holes) = region_profiles(region);
        let part = primitives::linear_extrusion(
            format!("{}:region-{region_index}", out.id),
            a.frame,
            outer,
            holes,
            a.height,
            accuracy,
        )?;
        let face_offset = out.topology.faces.len() as u32;
        append_analytic_input(&mut out, &part)?;
        let cap_sources = if operation == BooleanOp::Subtraction {
            [
                vec![brep_face_source(a.brep, 0)],
                vec![brep_face_source(a.brep, 1)],
            ]
        } else {
            [
                vec![brep_face_source(a.brep, 0), brep_face_source(b.brep, 0)],
                vec![brep_face_source(a.brep, 1), brep_face_source(b.brep, 1)],
            ]
        };
        for local in 0..2 {
            out.topology.faces[(face_offset + local) as usize].provenance = FaceProvenance {
                sources: cap_sources[local as usize].clone(),
                role: FaceRole::Split,
                reversed: false,
            };
        }
        let mut local_face = 2_u32;
        for ring in std::iter::once(&region.outer).chain(&region.holes) {
            for index in 0..ring.len() {
                let from = a.frame.point([ring[index].x, ring[index].z, 0.0]);
                let next = ring[(index + 1) % ring.len()];
                let to = a.frame.point([next.x, next.z, 0.0]);
                out.topology.faces[(face_offset + local_face) as usize].provenance =
                    prismatic_face_provenance(&a, &b, from, to, operation, accuracy.geometric)?;
                local_face += 1;
            }
        }
    }
    out.revision = a
        .brep
        .revision
        .max(b.brep.revision)
        .checked_add(1)
        .ok_or_else(|| GeometryError::LimitExceeded("boolean revision overflow".into()))?;
    out.validate()?;
    let face_mappings = analytic_face_mappings(&out, [a.brep, b.brep]);
    Ok(BooleanResult {
        brep: out,
        report: BooleanReport {
            operation,
            quality: GeometryQuality::Analytic,
            contacts: Vec::new(),
            coincident: false,
            face_mappings,
        },
    })
}

fn analytic_containment_boolean(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    b_inside_a: bool,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let accuracy = Accuracy {
        geometric: a.accuracy.geometric.max(b.accuracy.geometric),
        intersection: a.accuracy.intersection.max(b.accuracy.intersection),
        tessellation: a.accuracy.tessellation.max(b.accuracy.tessellation),
        exchange: a.accuracy.exchange.max(b.accuracy.exchange),
    };
    let mut out = BrepEnvelope::new(id, accuracy)?;
    let (contained, enclosing) = if b_inside_a { (b, a) } else { (a, b) };
    match operation {
        BooleanOp::Union => {
            append_analytic_input(&mut out, enclosing)?;
        }
        BooleanOp::Intersection => {
            append_analytic_input(&mut out, contained)?;
        }
        BooleanOp::Subtraction if b_inside_a => {
            append_analytic_input(&mut out, a)?;
            let face_offset = out.topology.faces.len() as u32;
            append_analytic_input(&mut out, b)?;
            let cavity = out
                .solids
                .pop()
                .ok_or_else(|| GeometryError::InvalidTopology("missing cutter solid".into()))?
                .outer_shell;
            out.solids[0].cavity_shells.push(cavity);
            for face in face_offset..out.topology.faces.len() as u32 {
                reverse_face(&mut out, face);
                out.topology.faces[face as usize].provenance.role = FaceRole::Cut;
                out.topology.faces[face as usize].provenance.reversed = true;
            }
        }
        BooleanOp::Subtraction => {}
    }
    out.revision = a
        .revision
        .max(b.revision)
        .checked_add(1)
        .ok_or_else(|| GeometryError::LimitExceeded("boolean revision overflow".into()))?;
    out.validate()?;
    let face_mappings = analytic_face_mappings(&out, [a, b]);
    Ok(BooleanResult {
        brep: out,
        report: BooleanReport {
            operation,
            quality: GeometryQuality::Analytic,
            contacts: Vec::new(),
            coincident: false,
            face_mappings,
        },
    })
}

fn interior_sample(brep: &BrepEnvelope) -> Result<Point3, GeometryError> {
    let bounds = brep.bounds()?.ok_or_else(|| {
        GeometryError::InvalidTopology("cannot classify an empty solid boundary".into())
    })?;
    let model_scale = bounds
        .axes
        .iter()
        .map(|axis| axis.width())
        .fold(brep.accuracy.geometric, f64::max);
    let offsets = [
        (model_scale * 1.0e-6).max(32.0 * brep.accuracy.geometric),
        16.0 * brep.accuracy.geometric,
        8.0 * brep.accuracy.geometric,
        4.0 * brep.accuracy.geometric,
    ];
    let samples = [0.5, 0.25, 0.75, 0.125, 0.875];
    for face in &brep.topology.faces {
        let surface = brep.geometry.surface(face.surface)?;
        for u in samples {
            for v in samples {
                let uv = [
                    face.trim.uv_bounds[0].lo + u * face.trim.uv_bounds[0].width(),
                    face.trim.uv_bounds[1].lo + v * face.trim.uv_bounds[1].width(),
                ];
                if face_contains_uv(brep, face, uv)? != Some(true) {
                    continue;
                }
                let boundary = surface.point_at(uv)?;
                let outward = scale(unit(surface.normal_at(uv)?)?, face.sense.multiplier());
                for offset in offsets {
                    let candidate = sub(boundary, scale(outward, offset));
                    if classify_point(brep, candidate)? == PointClassification::Inside {
                        return Ok(candidate);
                    }
                }
            }
        }
    }
    Err(GeometryError::UnresolvedIntersection(
        "could not certify an interior classification sample".into(),
    ))
}

fn generic_non_intersecting_boolean(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<Option<BooleanResult>, GeometryError> {
    let graph = match intersect_breps(a, b) {
        Ok(graph) => graph,
        Err(GeometryError::CoverageGap { .. }) => return Ok(None),
        Err(error) => return Err(error),
    };
    if graph
        .pairs
        .iter()
        .any(|pair| pair.graph.coincident || !pair.graph.branches.is_empty())
    {
        return Ok(None);
    }
    let mut contacts = graph
        .pairs
        .into_iter()
        .flat_map(|pair| pair.graph.contacts)
        .collect::<Vec<_>>();
    let tolerance = a.accuracy.geometric.max(b.accuracy.geometric);
    let mut unique = Vec::new();
    for point in contacts.drain(..) {
        if unique
            .iter()
            .all(|existing| norm(sub(*existing, point)) > tolerance)
        {
            unique.push(point);
        }
    }

    let a_in_b = classify_point(b, interior_sample(a)?)?;
    let b_in_a = classify_point(a, interior_sample(b)?)?;
    if matches!(
        a_in_b,
        PointClassification::Unknown | PointClassification::Boundary
    ) || matches!(
        b_in_a,
        PointClassification::Unknown | PointClassification::Boundary
    ) {
        return Err(GeometryError::UnresolvedIntersection(
            "generic boolean containment classification is unresolved".into(),
        ));
    }
    let a_inside_b = a_in_b == PointClassification::Inside;
    let b_inside_a = b_in_a == PointClassification::Inside;
    if a_inside_b && b_inside_a {
        return Err(GeometryError::UnresolvedIntersection(
            "mutual containment without a coincident boundary is unresolved".into(),
        ));
    }
    if a_inside_b || b_inside_a {
        return analytic_containment_boolean(a, b, b_inside_a, operation, id).map(Some);
    }
    separate_boolean(a, b, operation, id, unique).map(Some)
}

fn remap_closed_loop_pcurve(
    pcurve: &PcurveGeometry,
    curve: u32,
    surface: u32,
) -> Result<PcurveGeometry, GeometryError> {
    Ok(match pcurve {
        PcurveGeometry::Line2 { .. } | PcurveGeometry::Conic2 { .. } => pcurve.clone(),
        PcurveGeometry::ProjectedCurve {
            chart,
            uv_hint,
            uv_rate,
            parameter_origin,
            ..
        } => PcurveGeometry::ProjectedCurve {
            curve,
            surface,
            chart: *chart,
            uv_hint: *uv_hint,
            uv_rate: *uv_rate,
            parameter_origin: *parameter_origin,
        },
        PcurveGeometry::IntersectionSide { .. } => {
            return Err(GeometryError::InvalidGeometry(
                "intersection-side pcurve requires a remapped intersection definition".into(),
            ))
        }
    })
}

fn exact_closed_circle_pcurve(
    source_geometry: &GeometryStore,
    target_geometry: &GeometryStore,
    source_pcurve: u32,
    circle: &CurveGeometry,
    surface: u32,
    tolerance: f64,
) -> Result<Option<PcurveGeometry>, GeometryError> {
    if !matches!(
        source_geometry.pcurves.get(source_pcurve as usize),
        Some(PcurveGeometry::ProjectedCurve { .. })
    ) {
        return Ok(None);
    }
    let CurveGeometry::Circle {
        frame: circle,
        radius,
    } = circle
    else {
        return Ok(None);
    };
    let support = target_geometry.surface(surface)?;
    let frame = support.frame();
    let centre = frame.local(circle.origin);
    let axis_x = [dot(circle.x, frame.x), dot(circle.x, frame.y)];
    let axis_y = [dot(circle.y, frame.x), dot(circle.y, frame.y)];
    match support {
        SurfaceGeometry::Plane { .. }
            if centre[2].abs() <= tolerance
                && dot(circle.x, frame.z).abs() * radius <= tolerance
                && dot(circle.y, frame.z).abs() * radius <= tolerance =>
        {
            Ok(Some(PcurveGeometry::Conic2 {
                origin: [centre[0], centre[1]],
                axis_a: axis_x.map(|value| value * radius),
                axis_b: axis_y.map(|value| value * radius),
            }))
        }
        SurfaceGeometry::Cylinder {
            radius: support_radius,
            ..
        } if centre[0].hypot(centre[1]) <= tolerance
            && (radius - support_radius).abs() <= tolerance
            && (dot(circle.z, frame.z).abs() - 1.0).abs() <= 1.0e-10 =>
        {
            let rate = dot(cross(circle.x, circle.y), frame.z).signum();
            let phase = axis_x[1].atan2(axis_x[0]);
            let original = source_geometry.pcurve_at(source_pcurve, 0.0)?;
            let lifted_phase = phase
                + ((original[0] - phase) / std::f64::consts::TAU).round() * std::f64::consts::TAU;
            Ok(Some(PcurveGeometry::Line2 {
                origin: [lifted_phase, centre[2]],
                direction: [rate, 0.0],
            }))
        }
        _ => Ok(None),
    }
}

fn pcurve_winding(
    geometry: &GeometryStore,
    surface: u32,
    pcurve: u32,
    range: Interval,
) -> Result<[i32; 2], GeometryError> {
    let chart = &geometry.surface(surface)?.charts()[0];
    let start = geometry.pcurve_at(pcurve, range.lo)?;
    let end = geometry.pcurve_at(pcurve, range.hi)?;
    Ok(std::array::from_fn(|axis| {
        chart.periods[axis].map_or(0, |period| {
            ((end[axis] - start[axis]) / period).round() as i32
        })
    }))
}

fn pcurve_support_surface(geometry: &GeometryStore, pcurve: u32) -> Result<u32, GeometryError> {
    match geometry.pcurves.get(pcurve as usize) {
        Some(PcurveGeometry::ProjectedCurve { surface, .. }) => Ok(*surface),
        Some(PcurveGeometry::IntersectionSide { definition, side }) => {
            let definition = geometry
                .intersections
                .get(*definition as usize)
                .ok_or_else(|| GeometryError::MissingReference {
                    kind: "intersection definition".into(),
                    index: *definition,
                })?;
            Ok(definition.surfaces[if *side == IntersectionSide::A { 0 } else { 1 }])
        }
        Some(_) => Err(GeometryError::InvalidGeometry(
            "periodic pcurve has no support surface reference".into(),
        )),
        None => Err(GeometryError::MissingReference {
            kind: "pcurve".into(),
            index: pcurve,
        }),
    }
}

fn reframe_sphere_intersection(
    geometry: &mut GeometryStore,
    definition: &mut IntersectionDefinition,
    side: usize,
) -> Result<(), GeometryError> {
    let surface_id = definition.surfaces[side];
    let (frame, radius) = match geometry.surface(surface_id)? {
        SurfaceGeometry::Sphere { frame, radius } => (*frame, *radius),
        _ => {
            return Err(GeometryError::CoverageGap {
                families: ["periodic winding".into(), "non-spherical support".into()],
            })
        }
    };
    let references = [frame.x, frame.y, frame.z];
    let mut best: Option<(f64, SurfaceGeometry, Vec<[f64; 2]>)> = None;
    for i in -1..=1 {
        for j in -1..=1 {
            for k in -1..=1 {
                if [i, j, k] == [0, 0, 0] {
                    continue;
                }
                let axis = unit(frame.vector([i as f64, j as f64, k as f64]))?;
                let reference = references
                    .into_iter()
                    .min_by(|left, right| {
                        dot(*left, axis).abs().total_cmp(&dot(*right, axis).abs())
                    })
                    .unwrap();
                let candidate = SurfaceGeometry::Sphere {
                    frame: Frame3::from_axis(frame.origin, axis, reference)?,
                    radius,
                };
                let mut coordinates = Vec::with_capacity(definition.anchors.len());
                let mut hint = None;
                let mut failed = false;
                for anchor in &definition.anchors {
                    match candidate.project(anchor.point, hint) {
                        Ok(uv) => {
                            coordinates.push(uv);
                            hint = Some(uv);
                        }
                        Err(_) => {
                            failed = true;
                            break;
                        }
                    }
                }
                if failed {
                    continue;
                }
                let longitude_span = (coordinates.last().unwrap()[0] - coordinates[0][0]).abs();
                let pole_proximity = coordinates
                    .iter()
                    .map(|uv| uv[1].abs())
                    .fold(0.0_f64, f64::max);
                if longitude_span >= std::f64::consts::PI
                    || pole_proximity >= std::f64::consts::FRAC_PI_2 - 1.0e-6
                {
                    continue;
                }
                let score = pole_proximity + longitude_span;
                if best
                    .as_ref()
                    .is_none_or(|(best_score, _, _)| score < *best_score)
                {
                    best = Some((score, candidate, coordinates));
                }
            }
        }
    }
    let (_, candidate, coordinates) = best.ok_or_else(|| GeometryError::CoverageGap {
        families: ["sphere".into(), "unresolved periodic winding".into()],
    })?;
    geometry.surfaces[surface_id as usize] = candidate;
    for (anchor, uv) in definition.anchors.iter_mut().zip(&coordinates) {
        if side == 0 {
            anchor.uv_a = *uv;
        } else {
            anchor.uv_b = *uv;
        }
    }
    let offset = side * 2;
    for (index, tube) in definition.uv_tubes.iter_mut().enumerate() {
        for axis in 0..2 {
            let a = coordinates[index][axis];
            let b = coordinates[index + 1][axis];
            let padding = (a - b).abs().max(definition.residual_tolerance / radius) * 2.0;
            tube[offset + axis] = Interval::new(a.min(b) - padding, a.max(b) + padding)?;
        }
    }
    definition.validate(geometry)
}

fn generic_single_closed_loop_boolean(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<Option<BooleanResult>, GeometryError> {
    if a.solids.len() != 1
        || b.solids.len() != 1
        || a.topology.shells.len() != 1
        || b.topology.shells.len() != 1
        || !a.solids[0].cavity_shells.is_empty()
        || !b.solids[0].cavity_shells.is_empty()
    {
        return Ok(None);
    }
    let body_graph = intersect_breps(a, b)?;
    if body_graph.pairs.len() != 1 {
        return Ok(None);
    }
    let pair = &body_graph.pairs[0];
    if pair.graph.coincident || !pair.graph.contacts.is_empty() || pair.graph.branches.len() != 1 {
        return Ok(None);
    }
    let graph = &pair.graph;
    let branch = &graph.branches[0];
    let accuracy = Accuracy {
        geometric: a.accuracy.geometric.max(b.accuracy.geometric),
        intersection: a.accuracy.intersection.max(b.accuracy.intersection),
        tessellation: a.accuracy.tessellation.max(b.accuracy.tessellation),
        exchange: a.accuracy.exchange.max(b.accuracy.exchange),
    };
    if norm(sub(branch.endpoints[0], branch.endpoints[1])) > accuracy.intersection {
        return Ok(None);
    }
    let windings = [
        pcurve_winding(
            &graph.geometry,
            pcurve_support_surface(&graph.geometry, branch.pcurves[0])?,
            branch.pcurves[0],
            branch.range,
        )?,
        pcurve_winding(
            &graph.geometry,
            pcurve_support_surface(&graph.geometry, branch.pcurves[1])?,
            branch.pcurves[1],
            branch.range,
        )?,
    ];
    for side in 0..2 {
        if windings[side] != [0, 0]
            && !matches!(
                graph.geometry.surface(pcurve_support_surface(
                    &graph.geometry,
                    branch.pcurves[side],
                )?)?,
                SurfaceGeometry::Sphere { .. }
            )
        {
            return Ok(None);
        }
    }
    let inputs = [a, b];
    let face_ids = pair.faces;
    let loop_inside_other = [
        classify_point(
            b,
            graph.geometry.surfaces[0].point_at(intersection_loop_sample(graph, branch, 0)?)?,
        )?,
        classify_point(
            a,
            graph.geometry.surfaces[1].point_at(intersection_loop_sample(graph, branch, 1)?)?,
        )?,
    ];
    if loop_inside_other.iter().any(|classification| {
        matches!(
            classification,
            PointClassification::Unknown | PointClassification::Boundary
        )
    }) {
        return Err(GeometryError::UnresolvedIntersection(
            "closed SSI loop interior is unclassified".into(),
        ));
    }
    let loop_inside_other = loop_inside_other.map(|value| value == PointClassification::Inside);
    let (want_inside, reverse_selected) = match operation {
        BooleanOp::Union => ([false, false], [false, false]),
        BooleanOp::Intersection => ([true, true], [false, false]),
        BooleanOp::Subtraction => ([false, true], [false, true]),
    };
    let regions: [LoopRegion; 2] = std::array::from_fn(|side| {
        if loop_inside_other[side] == want_inside[side] {
            LoopRegion::Patch
        } else {
            LoopRegion::Complement
        }
    });

    let mut out = BrepEnvelope::new(id, accuracy)?;
    let mut selected_faces = [None; 2];
    let mut selected_surfaces = [None; 2];
    let mut selected_shells = Vec::new();
    for side in 0..2 {
        if regions[side] != LoopRegion::Complement {
            continue;
        }
        let face_offset = out.topology.faces.len() as u32;
        let shell_offset = out.topology.shells.len() as u32;
        append_analytic_input(&mut out, inputs[side])?;
        let selected_face = face_offset + face_ids[side];
        selected_faces[side] = Some(selected_face);
        selected_surfaces[side] = Some(out.topology.faces[selected_face as usize].surface);
        selected_shells.push(shell_offset);
        for face in face_offset..out.topology.faces.len() as u32 {
            out.topology.faces[face as usize].provenance.role = if face == selected_face {
                FaceRole::Split
            } else {
                FaceRole::Preserved
            };
        }
    }
    if selected_shells.is_empty() {
        out.topology.shells.push(Shell {
            id: 0,
            faces: Vec::new(),
            is_closed: true,
        });
        out.solids.push(SolidRegion {
            outer_shell: 0,
            cavity_shells: Vec::new(),
        });
    } else if selected_shells.len() == 2 {
        let faces = out.topology.shells[1].faces.clone();
        for face in &faces {
            out.topology.faces[*face as usize].shell_ref = Some(0);
        }
        out.topology.shells[0].faces.extend(faces);
        out.topology.shells.pop();
        out.solids.clear();
        out.solids.push(SolidRegion {
            outer_shell: 0,
            cavity_shells: Vec::new(),
        });
    }
    for side in 0..2 {
        if regions[side] == LoopRegion::Patch {
            selected_surfaces[side] = Some(out.geometry.surfaces.len() as u32);
            out.geometry
                .surfaces
                .push(graph.geometry.surfaces[side].clone());
        }
    }

    let source_curve = graph
        .geometry
        .curves
        .get(branch.curve as usize)
        .ok_or_else(|| GeometryError::MissingReference {
            kind: "curve".into(),
            index: branch.curve,
        })?
        .clone();
    let curve = out.geometry.curves.len() as u32;
    let pcurves: [PcurveGeometry; 2] = match source_curve {
        CurveGeometry::Intersection { definition } => {
            let mut definition_geometry = graph
                .geometry
                .intersections
                .get(definition as usize)
                .ok_or_else(|| GeometryError::MissingReference {
                    kind: "intersection definition".into(),
                    index: definition,
                })?
                .clone();
            definition_geometry.surfaces = [
                selected_surfaces[definition_geometry.surfaces[0] as usize].ok_or_else(|| {
                    GeometryError::InvalidTopology(
                        "closed-loop first support surface was not retained".into(),
                    )
                })?,
                selected_surfaces[definition_geometry.surfaces[1] as usize].ok_or_else(|| {
                    GeometryError::InvalidTopology(
                        "closed-loop second support surface was not retained".into(),
                    )
                })?,
            ];
            for side in 0..2 {
                if windings[side] == [0, 0] {
                    continue;
                }
                let definition_side = match graph.geometry.pcurves[branch.pcurves[side] as usize] {
                    PcurveGeometry::IntersectionSide {
                        side: IntersectionSide::A,
                        ..
                    } => 0,
                    PcurveGeometry::IntersectionSide {
                        side: IntersectionSide::B,
                        ..
                    } => 1,
                    _ => {
                        return Err(GeometryError::InvalidGeometry(
                            "winding intersection loop is missing its support pcurve".into(),
                        ))
                    }
                };
                reframe_sphere_intersection(
                    &mut out.geometry,
                    &mut definition_geometry,
                    definition_side,
                )?;
            }
            let definition = out.geometry.intersections.len() as u32;
            out.geometry.intersections.push(definition_geometry);
            out.geometry
                .curves
                .push(CurveGeometry::Intersection { definition });
            [0, 1]
                .map(
                    |side| match graph.geometry.pcurves.get(branch.pcurves[side] as usize) {
                        Some(PcurveGeometry::IntersectionSide {
                            side: intersection_side,
                            ..
                        }) => Ok(PcurveGeometry::IntersectionSide {
                            definition,
                            side: *intersection_side,
                        }),
                        Some(_) => Err(GeometryError::InvalidGeometry(
                            "intersection curve requires intersection-side pcurves".into(),
                        )),
                        None => Err(GeometryError::MissingReference {
                            kind: "pcurve".into(),
                            index: branch.pcurves[side],
                        }),
                    },
                )
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?
                .try_into()
                .map_err(|_| {
                    GeometryError::InvalidGeometry("closed SSI loop requires two pcurves".into())
                })?
        }
        curve_geometry => {
            out.geometry.curves.push(curve_geometry.clone());
            [0, 1]
                .map(|side| {
                    if let Some(exact) = exact_closed_circle_pcurve(
                        &graph.geometry,
                        &out.geometry,
                        branch.pcurves[side],
                        &curve_geometry,
                        selected_surfaces[side].unwrap(),
                        out.accuracy.geometric,
                    )? {
                        return Ok(exact);
                    }
                    remap_closed_loop_pcurve(
                        &graph.geometry.pcurves[branch.pcurves[side] as usize],
                        curve,
                        selected_surfaces[side].unwrap(),
                    )
                })
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?
                .try_into()
                .map_err(|_| {
                    GeometryError::InvalidGeometry("closed SSI loop requires two pcurves".into())
                })?
        }
    };
    let vertex = out.topology.vertices.len() as u32;
    out.topology.vertices.push(Vertex {
        id: vertex,
        position: branch.endpoints[0],
        outgoing_halfedge: None,
        tolerance: accuracy.geometric,
    });
    let edge = out.topology.edges.len() as u32;
    out.topology.edges.push(Edge {
        id: edge,
        geometry: EdgeGeometry::Curve {
            curve,
            range: branch.range,
        },
        halfedge: u32::MAX,
        twin_halfedge: None,
        tolerance: accuracy.geometric,
        chart_seam: false,
    });
    for side in 0..2 {
        let nominal_sense = if side == 0 {
            Orientation::Forward
        } else {
            Orientation::Reverse
        };
        let loop_sense = if reverse_selected[side] {
            reverse(nominal_sense)
        } else {
            nominal_sense
        };
        if regions[side] == LoopRegion::Complement {
            let face = selected_faces[side].unwrap();
            let loop_id = add_closed_intersection_loop(
                &mut out,
                edge,
                vertex,
                face,
                pcurves[side].clone(),
                loop_sense,
                true,
            )?;
            let loop_bounds = topology_loop_bounds(&out, loop_id, accuracy.intersection)?;
            for axis in 0..2 {
                let bounds = out.topology.faces[face as usize].trim.uv_bounds[axis];
                out.topology.faces[face as usize].trim.uv_bounds[axis] = Interval::new(
                    bounds.lo.min(loop_bounds[axis].lo),
                    bounds.hi.max(loop_bounds[axis].hi),
                )?;
            }
            out.topology.faces[face as usize].trim.holes.push(loop_id);
        } else {
            let face = out.topology.faces.len() as u32;
            let loop_id = add_closed_intersection_loop(
                &mut out,
                edge,
                vertex,
                face,
                pcurves[side].clone(),
                loop_sense,
                false,
            )?;
            let uv_bounds = topology_loop_bounds(&out, loop_id, accuracy.intersection)?;
            let source_face = &inputs[side].topology.faces[face_ids[side] as usize];
            out.topology.faces.push(Face {
                id: face,
                key: format!("{}:intersection-patch", source_face.key),
                surface: selected_surfaces[side].unwrap(),
                sense: source_face.sense,
                trim: TrimRegion {
                    chart: source_face.trim.chart,
                    uv_bounds,
                    outer: loop_id,
                    holes: Vec::new(),
                },
                shell_ref: Some(0),
                provenance: FaceProvenance {
                    sources: vec![brep_face_source(inputs[side], face_ids[side])],
                    role: if reverse_selected[side] {
                        FaceRole::Cut
                    } else {
                        FaceRole::Split
                    },
                    reversed: reverse_selected[side],
                },
            });
            out.topology.shells[0].faces.push(face);
            selected_faces[side] = Some(face);
        }
    }
    for side in 0..2 {
        if reverse_selected[side] {
            reverse_face(&mut out, selected_faces[side].unwrap());
        }
    }
    out.revision = a
        .revision
        .max(b.revision)
        .checked_add(1)
        .ok_or_else(|| GeometryError::LimitExceeded("boolean revision overflow".into()))?;
    out.validate()?;
    Ok(Some(BooleanResult {
        report: BooleanReport {
            operation,
            quality: GeometryQuality::Analytic,
            contacts: Vec::new(),
            coincident: false,
            face_mappings: analytic_face_mappings(&out, [a, b]),
        },
        brep: out,
    }))
}

fn generic_multiple_closed_loops_boolean(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<Option<BooleanResult>, GeometryError> {
    if a.solids.len() != 1
        || b.solids.len() != 1
        || a.topology.shells.len() != 1
        || b.topology.shells.len() != 1
        || !a.solids[0].cavity_shells.is_empty()
        || !b.solids[0].cavity_shells.is_empty()
    {
        return Ok(None);
    }
    let body_graph = intersect_breps(a, b)?;
    if body_graph.pairs.len() != 1 {
        return Ok(None);
    }
    let pair = &body_graph.pairs[0];
    if pair.graph.coincident || !pair.graph.contacts.is_empty() || pair.graph.branches.len() < 2 {
        return Ok(None);
    }
    let graph = &pair.graph;
    let accuracy = Accuracy {
        geometric: a.accuracy.geometric.max(b.accuracy.geometric),
        intersection: a.accuracy.intersection.max(b.accuracy.intersection),
        tessellation: a.accuracy.tessellation.max(b.accuracy.tessellation),
        exchange: a.accuracy.exchange.max(b.accuracy.exchange),
    };
    for branch in &graph.branches {
        if norm(sub(branch.endpoints[0], branch.endpoints[1])) > accuracy.intersection {
            return Ok(None);
        }
        for side in 0..2 {
            if pcurve_winding(
                &graph.geometry,
                pcurve_support_surface(&graph.geometry, branch.pcurves[side])?,
                branch.pcurves[side],
                branch.range,
            )? != [0, 0]
            {
                return Ok(None);
            }
        }
    }

    let inputs = [a, b];
    let face_ids = pair.faces;
    let (want_inside, reverse_selected) = match operation {
        BooleanOp::Union => ([false, false], [false, false]),
        BooleanOp::Intersection => ([true, true], [false, false]),
        BooleanOp::Subtraction => ([false, true], [false, true]),
    };
    let mut loop_regions = [Vec::new(), Vec::new()];
    for branch in &graph.branches {
        for side in 0..2 {
            let uv = intersection_loop_sample(graph, branch, side)?;
            let point = graph.geometry.surfaces[side].point_at(uv)?;
            let inside = match classify_point(inputs[1 - side], point)? {
                PointClassification::Inside => true,
                PointClassification::Outside => false,
                PointClassification::Unknown | PointClassification::Boundary => {
                    return Err(GeometryError::UnresolvedIntersection(
                        "closed SSI loop interior is unclassified".into(),
                    ))
                }
            };
            loop_regions[side].push(if inside == want_inside[side] {
                LoopRegion::Patch
            } else {
                LoopRegion::Complement
            });
        }
    }
    if [0, 1].into_iter().any(|side| {
        loop_regions[side]
            .iter()
            .any(|region| *region != loop_regions[side][0])
    }) {
        return Ok(None);
    }
    let selected_regions = [loop_regions[0][0], loop_regions[1][0]];
    for side in 0..2 {
        if selected_regions[side] == LoopRegion::Patch && inputs[side].topology.faces.len() != 1 {
            return Ok(None);
        }
    }

    let mut out = BrepEnvelope::new(id, accuracy)?;
    let mut selected_faces = [Vec::new(), Vec::new()];
    let mut selected_surfaces = [None, None];
    let mut selected_shells = Vec::new();
    for side in 0..2 {
        if selected_regions[side] == LoopRegion::Complement {
            let face_offset = out.topology.faces.len() as u32;
            let shell_offset = out.topology.shells.len() as u32;
            append_analytic_input(&mut out, inputs[side])?;
            let face = face_offset + face_ids[side];
            selected_faces[side].push(face);
            selected_surfaces[side] = Some(out.topology.faces[face as usize].surface);
            selected_shells.push(shell_offset);
            for candidate in face_offset..out.topology.faces.len() as u32 {
                out.topology.faces[candidate as usize].provenance.role = if candidate == face {
                    FaceRole::Split
                } else {
                    FaceRole::Preserved
                };
            }
        } else {
            selected_surfaces[side] = Some(out.geometry.surfaces.len() as u32);
            out.geometry
                .surfaces
                .push(graph.geometry.surfaces[side].clone());
        }
    }
    if selected_shells.is_empty() {
        out.topology.shells.push(Shell {
            id: 0,
            faces: Vec::new(),
            is_closed: true,
        });
        out.solids.push(SolidRegion {
            outer_shell: 0,
            cavity_shells: Vec::new(),
        });
    } else if selected_shells.len() == 2 {
        let faces = out.topology.shells[1].faces.clone();
        for face in &faces {
            out.topology.faces[*face as usize].shell_ref = Some(0);
        }
        out.topology.shells[0].faces.extend(faces);
        out.topology.shells.pop();
        out.solids.clear();
        out.solids.push(SolidRegion {
            outer_shell: 0,
            cavity_shells: Vec::new(),
        });
    }

    let selected_surfaces = [selected_surfaces[0].unwrap(), selected_surfaces[1].unwrap()];
    for (branch_index, branch) in graph.branches.iter().enumerate() {
        let (edge, vertex, pcurves) =
            append_closed_branch_geometry(&mut out, graph, branch, selected_surfaces)?;
        for side in 0..2 {
            let nominal_sense = if side == 0 {
                Orientation::Forward
            } else {
                Orientation::Reverse
            };
            let loop_sense = if reverse_selected[side] {
                reverse(nominal_sense)
            } else {
                nominal_sense
            };
            if selected_regions[side] == LoopRegion::Complement {
                let face = selected_faces[side][0];
                let loop_id = add_closed_intersection_loop(
                    &mut out,
                    edge,
                    vertex,
                    face,
                    pcurves[side].clone(),
                    loop_sense,
                    true,
                )?;
                let loop_bounds = topology_loop_bounds(&out, loop_id, accuracy.intersection)?;
                for axis in 0..2 {
                    let bounds = out.topology.faces[face as usize].trim.uv_bounds[axis];
                    out.topology.faces[face as usize].trim.uv_bounds[axis] = Interval::new(
                        bounds.lo.min(loop_bounds[axis].lo),
                        bounds.hi.max(loop_bounds[axis].hi),
                    )?;
                }
                out.topology.faces[face as usize].trim.holes.push(loop_id);
            } else {
                let face = out.topology.faces.len() as u32;
                let loop_id = add_closed_intersection_loop(
                    &mut out,
                    edge,
                    vertex,
                    face,
                    pcurves[side].clone(),
                    loop_sense,
                    false,
                )?;
                let uv_bounds = topology_loop_bounds(&out, loop_id, accuracy.intersection)?;
                let source_face = &inputs[side].topology.faces[face_ids[side] as usize];
                out.topology.faces.push(Face {
                    id: face,
                    key: format!("{}:intersection-patch-{branch_index}", source_face.key),
                    surface: selected_surfaces[side],
                    sense: source_face.sense,
                    trim: TrimRegion {
                        chart: source_face.trim.chart,
                        uv_bounds,
                        outer: loop_id,
                        holes: Vec::new(),
                    },
                    shell_ref: Some(0),
                    provenance: FaceProvenance {
                        sources: vec![brep_face_source(inputs[side], face_ids[side])],
                        role: if reverse_selected[side] {
                            FaceRole::Cut
                        } else {
                            FaceRole::Split
                        },
                        reversed: reverse_selected[side],
                    },
                });
                out.topology.shells[0].faces.push(face);
                selected_faces[side].push(face);
            }
        }
    }
    for side in 0..2 {
        if reverse_selected[side] {
            for face in selected_faces[side].clone() {
                reverse_face(&mut out, face);
            }
        }
    }
    out.revision = a
        .revision
        .max(b.revision)
        .checked_add(1)
        .ok_or_else(|| GeometryError::LimitExceeded("boolean revision overflow".into()))?;
    out.validate()?;
    Ok(Some(BooleanResult {
        report: BooleanReport {
            operation,
            quality: GeometryQuality::Analytic,
            contacts: Vec::new(),
            coincident: false,
            face_mappings: analytic_face_mappings(&out, [a, b]),
        },
        brep: out,
    }))
}

fn generic_two_sided_cutter_band_subtraction(
    host: &BrepEnvelope,
    cutter: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<Option<BooleanResult>, GeometryError> {
    if operation != BooleanOp::Subtraction
        || host.solids.len() != 1
        || cutter.solids.len() != 1
        || host.topology.shells.len() != 1
        || cutter.topology.shells.len() != 1
        || !host.solids[0].cavity_shells.is_empty()
        || !cutter.solids[0].cavity_shells.is_empty()
    {
        return Ok(None);
    }
    let body_graph = intersect_breps(host, cutter)?;
    if body_graph.pairs.len() != 2
        || body_graph.pairs[0].faces[1] != body_graph.pairs[1].faces[1]
        || body_graph.pairs[0].faces[0] == body_graph.pairs[1].faces[0]
    {
        return Ok(None);
    }
    let mut closed_branches = Vec::with_capacity(2);
    for pair in &body_graph.pairs {
        if pair.graph.coincident
            || !pair.graph.contacts.is_empty()
            || pair.graph.branches.len() != 1
        {
            return Ok(None);
        }
        let branch = &pair.graph.branches[0];
        let closure = norm(sub(branch.endpoints[0], branch.endpoints[1]));
        let intersection = host.accuracy.intersection.max(cutter.accuracy.intersection);
        let geometric = host.accuracy.geometric.max(cutter.accuracy.geometric);
        let closed = if closure <= intersection {
            super::face_intersection::IntersectionBranch {
                curve: branch.curve,
                pcurves: branch.pcurves,
                range: branch.range,
                endpoints: branch.endpoints,
            }
        } else if let Some(CurveGeometry::Circle { frame, radius }) =
            pair.graph.geometry.curves.get(branch.curve as usize)
        {
            let tau = std::f64::consts::TAU;
            if closure > 4.0 * geometric
                || branch.range.lo.abs() * radius > 4.0 * geometric
                || (branch.range.hi - tau).abs() * radius > 4.0 * geometric
            {
                return Ok(None);
            }
            let point = add(frame.origin, scale(frame.x, *radius));
            super::face_intersection::IntersectionBranch {
                curve: branch.curve,
                pcurves: branch.pcurves,
                range: Interval::new(0.0, tau)?,
                endpoints: [point, point],
            }
        } else {
            return Ok(None);
        };
        if pcurve_winding(
            &pair.graph.geometry,
            pcurve_support_surface(&pair.graph.geometry, closed.pcurves[0])?,
            closed.pcurves[0],
            closed.range,
        )? != [0, 0]
        {
            return Ok(None);
        }
        closed_branches.push(closed);
    }

    let accuracy = Accuracy {
        geometric: host.accuracy.geometric.max(cutter.accuracy.geometric),
        intersection: host.accuracy.intersection.max(cutter.accuracy.intersection),
        tessellation: host.accuracy.tessellation.max(cutter.accuracy.tessellation),
        exchange: host.accuracy.exchange.max(cutter.accuracy.exchange),
    };
    let mut out = BrepEnvelope::new(id, accuracy)?;
    append_analytic_input(&mut out, host)?;
    let cutter_surface = out.geometry.surfaces.len() as u32;
    out.geometry
        .surfaces
        .push(body_graph.pairs[0].graph.geometry.surfaces[1].clone());
    let cut_face = out.topology.faces.len() as u32;
    let mut cut_loops = Vec::with_capacity(2);
    let mut cut_bounds: Option<[Interval; 2]> = None;
    for (pair, branch) in body_graph.pairs.iter().zip(&closed_branches) {
        let host_face = pair.faces[0];
        let host_surface = out.topology.faces[host_face as usize].surface;
        let (edge, vertex, pcurves) = append_closed_branch_geometry(
            &mut out,
            &pair.graph,
            branch,
            [host_surface, cutter_surface],
        )?;
        let host_loop = add_closed_intersection_loop(
            &mut out,
            edge,
            vertex,
            host_face,
            pcurves[0].clone(),
            Orientation::Forward,
            true,
        )?;
        let host_bounds = topology_loop_bounds(&out, host_loop, accuracy.intersection)?;
        for axis in 0..2 {
            let bounds = out.topology.faces[host_face as usize].trim.uv_bounds[axis];
            out.topology.faces[host_face as usize].trim.uv_bounds[axis] = Interval::new(
                bounds.lo.min(host_bounds[axis].lo),
                bounds.hi.max(host_bounds[axis].hi),
            )?;
        }
        out.topology.faces[host_face as usize]
            .trim
            .holes
            .push(host_loop);
        out.topology.faces[host_face as usize].provenance.role = FaceRole::Split;

        let cutter_loop = add_closed_intersection_loop(
            &mut out,
            edge,
            vertex,
            cut_face,
            pcurves[1].clone(),
            Orientation::Forward,
            !cut_loops.is_empty(),
        )?;
        let bounds = topology_loop_bounds(&out, cutter_loop, accuracy.intersection)?;
        cut_bounds = Some(match cut_bounds {
            Some(previous) => [0, 1]
                .map(|axis| {
                    Interval::new(
                        previous[axis].lo.min(bounds[axis].lo),
                        previous[axis].hi.max(bounds[axis].hi),
                    )
                })
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?
                .try_into()
                .map_err(|_| GeometryError::InvalidGeometry("cutter band UV bounds".into()))?,
            None => bounds,
        });
        cut_loops.push(cutter_loop);
    }
    let source_face = &cutter.topology.faces[body_graph.pairs[0].faces[1] as usize];
    out.topology.faces.push(Face {
        id: cut_face,
        key: format!("{}:through-cut", source_face.key),
        surface: cutter_surface,
        sense: source_face.sense,
        trim: TrimRegion {
            chart: source_face.trim.chart,
            uv_bounds: cut_bounds.ok_or_else(|| {
                GeometryError::InvalidTopology("cutter band has no trim bounds".into())
            })?,
            outer: cut_loops[0],
            holes: cut_loops[1..].to_vec(),
        },
        shell_ref: Some(0),
        provenance: FaceProvenance {
            sources: vec![brep_face_source(cutter, source_face.id)],
            role: FaceRole::Cut,
            reversed: true,
        },
    });
    out.topology.shells[0].faces.push(cut_face);
    reverse_face(&mut out, cut_face);
    out.revision = host
        .revision
        .max(cutter.revision)
        .checked_add(1)
        .ok_or_else(|| GeometryError::LimitExceeded("boolean revision overflow".into()))?;
    out.validate()?;
    Ok(Some(BooleanResult {
        report: BooleanReport {
            operation,
            quality: GeometryQuality::Analytic,
            contacts: Vec::new(),
            coincident: false,
            face_mappings: analytic_face_mappings(&out, [host, cutter]),
        },
        brep: out,
    }))
}

fn append_closed_branch_geometry(
    out: &mut BrepEnvelope,
    graph: &super::face_intersection::IntersectionGraph,
    branch: &super::face_intersection::IntersectionBranch,
    selected_surfaces: [u32; 2],
) -> Result<(u32, u32, [PcurveGeometry; 2]), GeometryError> {
    let source_curve = graph
        .geometry
        .curves
        .get(branch.curve as usize)
        .ok_or_else(|| GeometryError::MissingReference {
            kind: "curve".into(),
            index: branch.curve,
        })?
        .clone();
    let curve = out.geometry.curves.len() as u32;
    let pcurves = match source_curve {
        CurveGeometry::Intersection { definition } => {
            let mut definition_geometry = graph
                .geometry
                .intersections
                .get(definition as usize)
                .ok_or_else(|| GeometryError::MissingReference {
                    kind: "intersection definition".into(),
                    index: definition,
                })?
                .clone();
            definition_geometry.surfaces = definition_geometry
                .surfaces
                .map(|surface| selected_surfaces[surface as usize]);
            let definition = out.geometry.intersections.len() as u32;
            out.geometry.intersections.push(definition_geometry);
            out.geometry
                .curves
                .push(CurveGeometry::Intersection { definition });
            [0, 1]
                .map(
                    |side| match graph.geometry.pcurves.get(branch.pcurves[side] as usize) {
                        Some(PcurveGeometry::IntersectionSide {
                            side: intersection_side,
                            ..
                        }) => Ok(PcurveGeometry::IntersectionSide {
                            definition,
                            side: *intersection_side,
                        }),
                        Some(_) => Err(GeometryError::InvalidGeometry(
                            "intersection curve requires intersection-side pcurves".into(),
                        )),
                        None => Err(GeometryError::MissingReference {
                            kind: "pcurve".into(),
                            index: branch.pcurves[side],
                        }),
                    },
                )
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?
                .try_into()
                .map_err(|_| {
                    GeometryError::InvalidGeometry("closed SSI loop requires two pcurves".into())
                })?
        }
        curve_geometry => {
            out.geometry.curves.push(curve_geometry.clone());
            [0, 1]
                .map(|side| {
                    if let Some(exact) = exact_closed_circle_pcurve(
                        &graph.geometry,
                        &out.geometry,
                        branch.pcurves[side],
                        &curve_geometry,
                        selected_surfaces[side],
                        out.accuracy.geometric,
                    )? {
                        return Ok(exact);
                    }
                    remap_closed_loop_pcurve(
                        &graph.geometry.pcurves[branch.pcurves[side] as usize],
                        curve,
                        selected_surfaces[side],
                    )
                })
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?
                .try_into()
                .map_err(|_| {
                    GeometryError::InvalidGeometry("closed SSI loop requires two pcurves".into())
                })?
        }
    };
    let vertex = out.topology.vertices.len() as u32;
    out.topology.vertices.push(Vertex {
        id: vertex,
        position: branch.endpoints[0],
        outgoing_halfedge: None,
        tolerance: out.accuracy.geometric,
    });
    let edge = out.topology.edges.len() as u32;
    out.topology.edges.push(Edge {
        id: edge,
        geometry: EdgeGeometry::Curve {
            curve,
            range: branch.range,
        },
        halfedge: u32::MAX,
        twin_halfedge: None,
        tolerance: out.accuracy.geometric,
        chart_seam: false,
    });
    Ok((edge, vertex, pcurves))
}

fn coincident_boolean(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let accuracy = Accuracy {
        geometric: a.accuracy.geometric.max(b.accuracy.geometric),
        intersection: a.accuracy.intersection.max(b.accuracy.intersection),
        tessellation: a.accuracy.tessellation.max(b.accuracy.tessellation),
        exchange: a.accuracy.exchange.max(b.accuracy.exchange),
    };
    let mut out = BrepEnvelope::new(id, accuracy)?;
    if operation != BooleanOp::Subtraction {
        append_analytic_input(&mut out, a)?;
        for face in &mut out.topology.faces {
            let counterpart = b.topology.faces.get(face.id as usize).ok_or_else(|| {
                GeometryError::InvalidTopology(
                    "coincident operands have incompatible face ownership".into(),
                )
            })?;
            face.provenance.sources.push(FaceSource {
                entity: b.id.clone(),
                body: b.id.clone(),
                key: counterpart.key.clone(),
                face: counterpart.id,
            });
            face.provenance.role = FaceRole::Coincident;
        }
    }
    out.revision = a
        .revision
        .max(b.revision)
        .checked_add(1)
        .ok_or_else(|| GeometryError::LimitExceeded("boolean revision overflow".into()))?;
    out.validate()?;
    Ok(BooleanResult {
        report: BooleanReport {
            operation,
            quality: GeometryQuality::Analytic,
            contacts: Vec::new(),
            coincident: true,
            face_mappings: analytic_face_mappings(&out, [a, b]),
        },
        brep: out,
    })
}

fn torus_containment_boolean(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let a = full_torus(a)?;
    let b = full_torus(b)?;
    let accuracy = a.brep.accuracy.geometric.max(b.brep.accuracy.geometric);
    let axes_match = norm(cross(a.frame.z, b.frame.z)) <= 1e-12 && dot(a.frame.z, b.frame.z) > 0.0;
    let displacement = a.frame.local(b.frame.origin);
    let radial_offset = displacement[0].hypot(displacement[1]);
    if !axes_match || radial_offset > accuracy {
        return Err(GeometryError::CoverageGap {
            families: ["ring torus".into(), "noncoaxial ring torus".into()],
        });
    }
    let centerline_distance = (a.major_radius - b.major_radius).hypot(displacement[2]);
    let a_contains_b = centerline_distance + b.minor_radius < a.minor_radius - accuracy;
    let b_contains_a = centerline_distance + a.minor_radius < b.minor_radius - accuracy;
    let same =
        centerline_distance <= accuracy && (a.minor_radius - b.minor_radius).abs() <= accuracy;
    if same {
        return coincident_boolean(a.brep, b.brep, operation, id);
    }
    if a_contains_b || b_contains_a {
        return analytic_containment_boolean(a.brep, b.brep, a_contains_b, operation, id);
    }
    if (centerline_distance + b.minor_radius - a.minor_radius).abs() <= accuracy
        || (centerline_distance + a.minor_radius - b.minor_radius).abs() <= accuracy
    {
        return Err(GeometryError::UnresolvedIntersection(
            "torus containment boundary is below geometric resolution".into(),
        ));
    }
    Err(GeometryError::CoverageGap {
        families: ["ring torus".into(), "intersecting ring torus".into()],
    })
}

fn conic_radius_at(input: &ConicSectionInput<'_>, axial: f64) -> f64 {
    axial * input.semi_angle.tan()
}

fn conic_containment_boolean(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let a = full_conic_section(a)?;
    let b = full_conic_section(b)?;
    let accuracy = a.brep.accuracy.geometric.max(b.brep.accuracy.geometric);
    if norm(cross(a.frame.z, b.frame.z)) > 1.0e-12 || dot(a.frame.z, b.frame.z) <= 0.0 {
        return Err(GeometryError::CoverageGap {
            families: [
                "cone or frustum".into(),
                "noncoaxial cone or frustum".into(),
            ],
        });
    }
    let displacement = sub(b.frame.origin, a.frame.origin);
    let axial_offset = dot(displacement, a.frame.z);
    let radial_offset = norm(sub(displacement, scale(a.frame.z, axial_offset)));
    if radial_offset > accuracy {
        return Err(GeometryError::CoverageGap {
            families: [
                "cone or frustum".into(),
                "noncoaxial cone or frustum".into(),
            ],
        });
    }

    let b_in_a_range = Interval::new(
        axial_offset + b.axial_range.lo,
        axial_offset + b.axial_range.hi,
    )?;
    let a_in_b_range = Interval::new(
        a.axial_range.lo - axial_offset,
        a.axial_range.hi - axial_offset,
    )?;
    let containment_margins =
        |inner: &ConicSectionInput<'_>, outer: &ConicSectionInput<'_>, inner_in_outer: Interval| {
            [
                inner_in_outer.lo - outer.axial_range.lo,
                outer.axial_range.hi - inner_in_outer.hi,
                conic_radius_at(outer, inner_in_outer.lo) - inner.lower_radius,
                conic_radius_at(outer, inner_in_outer.hi) - inner.upper_radius,
            ]
        };
    let b_in_a = containment_margins(&b, &a, b_in_a_range);
    let a_in_b = containment_margins(&a, &b, a_in_b_range);
    let strictly_inside = |margins: [f64; 4]| margins.into_iter().all(|margin| margin > accuracy);
    let near_inside = |margins: [f64; 4]| {
        margins.into_iter().all(|margin| margin >= -accuracy)
            && margins.into_iter().any(|margin| margin.abs() <= accuracy)
    };
    let b_inside_a = strictly_inside(b_in_a);
    let a_inside_b = strictly_inside(a_in_b);
    if b_inside_a || a_inside_b {
        return analytic_containment_boolean(a.brep, b.brep, b_inside_a, operation, id);
    }

    let same = a.brep.topology.faces.len() == b.brep.topology.faces.len()
        && (b_in_a_range.lo - a.axial_range.lo).abs() <= accuracy
        && (b_in_a_range.hi - a.axial_range.hi).abs() <= accuracy
        && (b.lower_radius - a.lower_radius).abs() <= accuracy
        && (b.upper_radius - a.upper_radius).abs() <= accuracy;
    if same {
        return coincident_boolean(a.brep, b.brep, operation, id);
    }
    if near_inside(b_in_a) || near_inside(a_in_b) {
        return Err(GeometryError::UnresolvedIntersection(
            "cone/frustum containment boundary is below geometric resolution".into(),
        ));
    }
    Err(GeometryError::CoverageGap {
        families: [
            "cone or frustum".into(),
            "intersecting cone or frustum".into(),
        ],
    })
}

fn sphere_conic_containment(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let (sphere, conic, sphere_is_a) = if matches!(
        a.geometry.surfaces.first(),
        Some(SurfaceGeometry::Sphere { .. })
    ) {
        (full_sphere(a)?, full_conic_section(b)?, true)
    } else {
        (full_sphere(b)?, full_conic_section(a)?, false)
    };
    let clearance = 4.0
        * sphere
            .brep
            .accuracy
            .geometric
            .max(conic.brep.accuracy.geometric);
    let center = conic.frame.local(sphere.frame.origin);
    let radial = center[0].hypot(center[1]);
    let (sin_angle, cos_angle) = conic.semi_angle.sin_cos();
    let sphere_margin = (center[2] - conic.axial_range.lo - sphere.radius)
        .min(conic.axial_range.hi - center[2] - sphere.radius)
        .min(center[2] * sin_angle - radial * cos_angle - sphere.radius);
    let conic_extent = [
        (radial + conic.lower_radius).hypot(conic.axial_range.lo - center[2]),
        (radial + conic.upper_radius).hypot(conic.axial_range.hi - center[2]),
    ]
    .into_iter()
    .fold(0.0_f64, f64::max);
    let conic_margin = sphere.radius - conic_extent;
    let sphere_inside_conic = sphere_margin > clearance;
    let conic_inside_sphere = conic_margin > clearance;
    if sphere_inside_conic || conic_inside_sphere {
        let b_inside_a = if sphere_is_a {
            conic_inside_sphere
        } else {
            sphere_inside_conic
        };
        return analytic_containment_boolean(a, b, b_inside_a, operation, id);
    }
    if sphere_margin.abs() <= clearance || conic_margin.abs() <= clearance {
        return Err(GeometryError::UnresolvedIntersection(
            "sphere/cone containment boundary is below geometric resolution".into(),
        ));
    }
    Err(GeometryError::CoverageGap {
        families: ["sphere".into(), "intersecting cone or frustum".into()],
    })
}

fn cylinder_conic_containment(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let (cylinder, conic, cylinder_is_a) = if matches!(
        a.geometry.surfaces.first(),
        Some(SurfaceGeometry::Cylinder { .. })
    ) {
        (full_cylinder(a)?, full_conic_section(b)?, true)
    } else {
        (full_cylinder(b)?, full_conic_section(a)?, false)
    };
    let clearance = 4.0
        * cylinder
            .brep
            .accuracy
            .geometric
            .max(conic.brep.accuracy.geometric);
    let alignment = dot(cylinder.frame.z, conic.frame.z);
    if norm(cross(cylinder.frame.z, conic.frame.z)) > 1.0e-12 {
        return Err(GeometryError::CoverageGap {
            families: ["cylinder".into(), "noncoaxial cone or frustum".into()],
        });
    }
    let displacement = sub(conic.frame.origin, cylinder.frame.origin);
    let cylinder_axis_offset = dot(displacement, cylinder.frame.z);
    let radial_offset = norm(sub(
        displacement,
        scale(cylinder.frame.z, cylinder_axis_offset),
    ));
    if radial_offset > clearance {
        return Err(GeometryError::CoverageGap {
            families: ["cylinder".into(), "noncoaxial cone or frustum".into()],
        });
    }

    let conic_ends_in_cylinder = [
        cylinder_axis_offset + alignment * conic.axial_range.lo,
        cylinder_axis_offset + alignment * conic.axial_range.hi,
    ];
    let conic_axial_lo = conic_ends_in_cylinder[0].min(conic_ends_in_cylinder[1]);
    let conic_axial_hi = conic_ends_in_cylinder[0].max(conic_ends_in_cylinder[1]);
    let conic_margin = conic_axial_lo
        .min(cylinder.height - conic_axial_hi)
        .min(cylinder.radius - conic.lower_radius.max(conic.upper_radius));

    let cylinder_origin_in_conic = dot(
        sub(cylinder.frame.origin, conic.frame.origin),
        conic.frame.z,
    );
    let cylinder_ends_in_conic = [
        cylinder_origin_in_conic,
        cylinder_origin_in_conic + alignment * cylinder.height,
    ];
    let cylinder_axial_lo = cylinder_ends_in_conic[0].min(cylinder_ends_in_conic[1]);
    let cylinder_axial_hi = cylinder_ends_in_conic[0].max(cylinder_ends_in_conic[1]);
    let cylinder_margin = (cylinder_axial_lo - conic.axial_range.lo)
        .min(conic.axial_range.hi - cylinder_axial_hi)
        .min(conic_radius_at(&conic, cylinder_ends_in_conic[0]) - cylinder.radius)
        .min(conic_radius_at(&conic, cylinder_ends_in_conic[1]) - cylinder.radius);

    let conic_inside_cylinder = conic_margin > clearance;
    let cylinder_inside_conic = cylinder_margin > clearance;
    if conic_inside_cylinder || cylinder_inside_conic {
        let b_inside_a = if cylinder_is_a {
            conic_inside_cylinder
        } else {
            cylinder_inside_conic
        };
        return analytic_containment_boolean(a, b, b_inside_a, operation, id);
    }
    if conic_margin.abs() <= clearance || cylinder_margin.abs() <= clearance {
        return Err(GeometryError::UnresolvedIntersection(
            "cylinder/cone containment boundary is below geometric resolution".into(),
        ));
    }
    Err(GeometryError::CoverageGap {
        families: ["cylinder".into(), "intersecting cone or frustum".into()],
    })
}

fn conic_box_containment(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let (conic, box_, conic_is_a) = if matches!(
        a.geometry.surfaces.first(),
        Some(SurfaceGeometry::Cone { .. })
    ) {
        (
            full_conic_section(a)?,
            super::box_booleans::full_box(b)?,
            true,
        )
    } else {
        (
            full_conic_section(b)?,
            super::box_booleans::full_box(a)?,
            false,
        )
    };
    let clearance = 4.0
        * conic
            .brep
            .accuracy
            .geometric
            .max(box_.brep.accuracy.geometric);
    let box_axes = [box_.frame.x, box_.frame.y, box_.frame.z];
    let endpoint_centers = [
        conic.frame.point([0.0, 0.0, conic.axial_range.lo]),
        conic.frame.point([0.0, 0.0, conic.axial_range.hi]),
    ];
    let endpoint_radii = [conic.lower_radius, conic.upper_radius];
    let mut conic_margin = f64::INFINITY;
    for axis in 0..3 {
        let radial_extent_rate = (1.0 - dot(conic.frame.z, box_axes[axis]).powi(2))
            .max(0.0)
            .sqrt();
        let extents = [0, 1].map(|endpoint| {
            let center = dot(
                sub(endpoint_centers[endpoint], box_.frame.origin),
                box_axes[axis],
            );
            let radial = endpoint_radii[endpoint] * radial_extent_rate;
            [center - radial, center + radial]
        });
        let lo = extents[0][0].min(extents[1][0]);
        let hi = extents[0][1].max(extents[1][1]);
        conic_margin = conic_margin.min(lo).min(box_.size[axis] - hi);
    }

    let mut box_margin = f64::INFINITY;
    for corner in 0..8 {
        let point = box_.frame.point(std::array::from_fn(|axis| {
            if corner & (1 << axis) == 0 {
                0.0
            } else {
                box_.size[axis]
            }
        }));
        let local = conic.frame.local(point);
        box_margin = box_margin
            .min(local[2] - conic.axial_range.lo)
            .min(conic.axial_range.hi - local[2])
            .min(conic_radius_at(&conic, local[2]) - local[0].hypot(local[1]));
    }
    let conic_inside_box = conic_margin > clearance;
    let box_inside_conic = box_margin > clearance;
    if conic_inside_box || box_inside_conic {
        let b_inside_a = if conic_is_a {
            box_inside_conic
        } else {
            conic_inside_box
        };
        return analytic_containment_boolean(a, b, b_inside_a, operation, id);
    }
    if conic_margin.abs() <= clearance || box_margin.abs() <= clearance {
        return Err(GeometryError::UnresolvedIntersection(
            "cone/cuboid containment boundary is below geometric resolution".into(),
        ));
    }
    Err(GeometryError::CoverageGap {
        families: ["cone or frustum".into(), "cuboid".into()],
    })
}

fn torus_cylinder_containment(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let (torus, cylinder, torus_is_a) = if matches!(
        a.geometry.surfaces.first(),
        Some(SurfaceGeometry::Torus { .. })
    ) {
        (full_torus(a)?, full_cylinder(b)?, true)
    } else {
        (full_torus(b)?, full_cylinder(a)?, false)
    };
    let clearance = 4.0
        * torus
            .brep
            .accuracy
            .geometric
            .max(cylinder.brep.accuracy.geometric);
    if norm(cross(torus.frame.z, cylinder.frame.z)) > 1.0e-12 {
        return Err(GeometryError::CoverageGap {
            families: ["ring torus".into(), "noncoaxial cylinder".into()],
        });
    }
    let center = cylinder.frame.local(torus.frame.origin);
    let radial = center[0].hypot(center[1]);
    let torus_margin = (cylinder.radius - radial - torus.major_radius - torus.minor_radius)
        .min(center[2] - torus.minor_radius)
        .min(cylinder.height - center[2] - torus.minor_radius);
    if torus_margin > clearance {
        let b_inside_a = !torus_is_a;
        return analytic_containment_boolean(a, b, b_inside_a, operation, id);
    }
    if torus_margin.abs() <= clearance {
        return Err(GeometryError::UnresolvedIntersection(
            "torus/cylinder containment boundary is below geometric resolution".into(),
        ));
    }
    Err(GeometryError::CoverageGap {
        families: ["ring torus".into(), "intersecting cylinder".into()],
    })
}

fn torus_box_containment(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let (torus, box_, torus_is_a) = if matches!(
        a.geometry.surfaces.first(),
        Some(SurfaceGeometry::Torus { .. })
    ) {
        (full_torus(a)?, super::box_booleans::full_box(b)?, true)
    } else {
        (full_torus(b)?, super::box_booleans::full_box(a)?, false)
    };
    let clearance = 4.0
        * torus
            .brep
            .accuracy
            .geometric
            .max(box_.brep.accuracy.geometric);
    let box_axes = [box_.frame.x, box_.frame.y, box_.frame.z];
    let center = box_.frame.local(torus.frame.origin);
    let torus_margin = (0..3).fold(f64::INFINITY, |margin, axis| {
        let axial_rate = dot(torus.frame.z, box_axes[axis]);
        let radial_rate = (1.0 - axial_rate * axial_rate).max(0.0).sqrt();
        let extent = torus.major_radius * radial_rate + torus.minor_radius;
        margin
            .min(center[axis] - extent)
            .min(box_.size[axis] - center[axis] - extent)
    });
    if torus_margin > clearance {
        let b_inside_a = !torus_is_a;
        return analytic_containment_boolean(a, b, b_inside_a, operation, id);
    }
    if torus_margin.abs() <= clearance {
        return Err(GeometryError::UnresolvedIntersection(
            "torus/cuboid containment boundary is below geometric resolution".into(),
        ));
    }
    Err(GeometryError::CoverageGap {
        families: ["ring torus".into(), "cuboid".into()],
    })
}

fn torus_conic_containment(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let (torus, conic, torus_is_a) = if matches!(
        a.geometry.surfaces.first(),
        Some(SurfaceGeometry::Torus { .. })
    ) {
        (full_torus(a)?, full_conic_section(b)?, true)
    } else {
        (full_torus(b)?, full_conic_section(a)?, false)
    };
    let clearance = 4.0
        * torus
            .brep
            .accuracy
            .geometric
            .max(conic.brep.accuracy.geometric);
    if norm(cross(torus.frame.z, conic.frame.z)) > 1.0e-12 {
        return Err(GeometryError::CoverageGap {
            families: ["ring torus".into(), "noncoaxial cone or frustum".into()],
        });
    }
    let center = conic.frame.local(torus.frame.origin);
    let radial = center[0].hypot(center[1]);
    let (sin_angle, cos_angle) = conic.semi_angle.sin_cos();
    let torus_margin = (center[2] - conic.axial_range.lo - torus.minor_radius)
        .min(conic.axial_range.hi - center[2] - torus.minor_radius)
        .min(
            center[2] * sin_angle - (radial + torus.major_radius) * cos_angle - torus.minor_radius,
        );
    if torus_margin > clearance {
        let b_inside_a = !torus_is_a;
        return analytic_containment_boolean(a, b, b_inside_a, operation, id);
    }
    if torus_margin.abs() <= clearance {
        return Err(GeometryError::UnresolvedIntersection(
            "torus/cone containment boundary is below geometric resolution".into(),
        ));
    }
    Err(GeometryError::CoverageGap {
        families: ["ring torus".into(), "intersecting cone or frustum".into()],
    })
}

fn sphere_torus_containment(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let (sphere, torus, sphere_is_a) = if matches!(
        a.geometry.surfaces.first(),
        Some(SurfaceGeometry::Sphere { .. })
    ) {
        (full_sphere(a)?, full_torus(b)?, true)
    } else {
        (full_sphere(b)?, full_torus(a)?, false)
    };
    let accuracy = sphere
        .brep
        .accuracy
        .geometric
        .max(torus.brep.accuracy.geometric);
    let center = torus.frame.local(sphere.frame.origin);
    let radial = center[0].hypot(center[1]);
    let centerline_minimum = (radial - torus.major_radius).hypot(center[2]);
    let centerline_maximum = (radial + torus.major_radius).hypot(center[2]);
    let sphere_inside_torus = centerline_minimum + sphere.radius < torus.minor_radius - accuracy;
    let torus_inside_sphere = centerline_maximum + torus.minor_radius < sphere.radius - accuracy;
    if sphere_inside_torus || torus_inside_sphere {
        let b_inside_a = if sphere_is_a {
            torus_inside_sphere
        } else {
            sphere_inside_torus
        };
        return analytic_containment_boolean(a, b, b_inside_a, operation, id);
    }
    if (centerline_minimum + sphere.radius - torus.minor_radius).abs() <= accuracy
        || (centerline_maximum + torus.minor_radius - sphere.radius).abs() <= accuracy
    {
        return Err(GeometryError::UnresolvedIntersection(
            "sphere/torus containment boundary is below geometric resolution".into(),
        ));
    }
    Err(GeometryError::CoverageGap {
        families: ["sphere".into(), "intersecting ring torus".into()],
    })
}

fn sphere_cylinder_boolean(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let (sphere, cylinder, sphere_is_a) = if matches!(
        a.geometry.surfaces.first(),
        Some(SurfaceGeometry::Sphere { .. })
    ) {
        (full_sphere(a)?, full_cylinder(b)?, true)
    } else {
        (full_sphere(b)?, full_cylinder(a)?, false)
    };
    let clearance = 4.0
        * sphere
            .brep
            .accuracy
            .geometric
            .max(cylinder.brep.accuracy.geometric);
    let local = cylinder.frame.local(sphere.frame.origin);
    let radial = local[0].hypot(local[1]);
    let sphere_margin = (cylinder.radius - radial - sphere.radius)
        .min(local[2] - sphere.radius)
        .min(cylinder.height - local[2] - sphere.radius);
    let axial_extent = local[2].abs().max((cylinder.height - local[2]).abs());
    let cylinder_extent = (radial + cylinder.radius).hypot(axial_extent);
    let cylinder_margin = sphere.radius - cylinder_extent;
    let sphere_inside_cylinder = sphere_margin > clearance;
    let cylinder_inside_sphere = cylinder_margin > clearance;
    if sphere_inside_cylinder || cylinder_inside_sphere {
        let b_inside_a = if sphere_is_a {
            cylinder_inside_sphere
        } else {
            sphere_inside_cylinder
        };
        return analytic_containment_boolean(a, b, b_inside_a, operation, id);
    }
    if sphere_margin.abs() <= clearance || cylinder_margin.abs() <= clearance {
        return Err(GeometryError::UnresolvedIntersection(
            "sphere/cylinder containment boundary is below geometric resolution".into(),
        ));
    }
    offset_sphere_cylinder_boolean(&sphere, &cylinder, sphere_is_a, operation, id, clearance)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LoopRegion {
    Patch,
    Complement,
}

fn intersection_loop_sample(
    graph: &super::face_intersection::IntersectionGraph,
    branch: &super::face_intersection::IntersectionBranch,
    side: usize,
) -> Result<[f64; 2], GeometryError> {
    let subdivisions = 128;
    let mut points = Vec::with_capacity(subdivisions);
    for index in 0..subdivisions {
        let parameter = branch.range.lo + branch.range.width() * index as f64 / subdivisions as f64;
        points.push(graph.geometry.pcurve_at(branch.pcurves[side], parameter)?);
    }
    let mut twice_area = 0.0;
    let mut centroid = [0.0; 2];
    for index in 0..points.len() {
        let a = points[index];
        let b = points[(index + 1) % points.len()];
        let cross = a[0] * b[1] - b[0] * a[1];
        twice_area += cross;
        centroid[0] += (a[0] + b[0]) * cross;
        centroid[1] += (a[1] + b[1]) * cross;
    }
    if twice_area.abs() <= f64::EPSILON {
        return Ok([
            points.iter().map(|point| point[0]).sum::<f64>() / points.len() as f64,
            points.iter().map(|point| point[1]).sum::<f64>() / points.len() as f64,
        ]);
    }
    Ok([
        centroid[0] / (3.0 * twice_area),
        centroid[1] / (3.0 * twice_area),
    ])
}

fn intersection_loop_bounds(
    graph: &super::face_intersection::IntersectionGraph,
    branch: &super::face_intersection::IntersectionBranch,
    side: usize,
    tolerance: f64,
) -> Result<[Interval; 2], GeometryError> {
    pcurve_loop_bounds(
        &graph.geometry,
        branch.pcurves[side],
        branch.range,
        tolerance,
    )
}

fn topology_loop_bounds(
    brep: &BrepEnvelope,
    loop_id: u32,
    tolerance: f64,
) -> Result<[Interval; 2], GeometryError> {
    let loop_record = brep.topology.loops.get(loop_id as usize).ok_or_else(|| {
        GeometryError::MissingReference {
            kind: "loop".into(),
            index: loop_id,
        }
    })?;
    let halfedge = &brep.topology.halfedges[loop_record.start_halfedge as usize];
    let pcurve = halfedge.geometry_use.pcurve.ok_or_else(|| {
        GeometryError::InvalidTopology("intersection loop is missing its pcurve".into())
    })?;
    pcurve_loop_bounds(
        &brep.geometry,
        pcurve,
        brep.topology.edges[halfedge.edge as usize].geometry.range(),
        tolerance,
    )
}

fn pcurve_loop_bounds(
    geometry: &GeometryStore,
    pcurve: u32,
    range: Interval,
    tolerance: f64,
) -> Result<[Interval; 2], GeometryError> {
    let mut lo = [f64::INFINITY; 2];
    let mut hi = [f64::NEG_INFINITY; 2];
    for index in 0..=256 {
        let parameter = range.lo + range.width() * index as f64 / 256.0;
        let uv = geometry.pcurve_at(pcurve, parameter)?;
        for axis in 0..2 {
            lo[axis] = lo[axis].min(uv[axis]);
            hi[axis] = hi[axis].max(uv[axis]);
        }
    }
    let padding =
        [0, 1].map(|axis| (hi[axis] - lo[axis]).abs().max(tolerance) * 1.0e-6 + tolerance);
    Ok([
        Interval::new(lo[0] - padding[0], hi[0] + padding[0])?,
        Interval::new(lo[1] - padding[1], hi[1] + padding[1])?,
    ])
}

fn add_closed_intersection_loop(
    brep: &mut BrepEnvelope,
    edge: u32,
    vertex: u32,
    face: u32,
    pcurve_geometry: PcurveGeometry,
    sense: Orientation,
    is_hole: bool,
) -> Result<u32, GeometryError> {
    let loop_id = brep.topology.loops.len() as u32;
    let halfedge = brep.topology.halfedges.len() as u32;
    let pcurve = brep.geometry.pcurves.len() as u32;
    brep.geometry.pcurves.push(pcurve_geometry);
    let edge_record = brep.topology.edges.get_mut(edge as usize).ok_or_else(|| {
        GeometryError::InvalidTopology("intersection loop edge is missing".into())
    })?;
    let twin = if edge_record.halfedge == u32::MAX {
        edge_record.halfedge = halfedge;
        None
    } else {
        if edge_record.twin_halfedge.is_some() {
            return Err(GeometryError::InvalidTopology(
                "intersection loop edge is nonmanifold".into(),
            ));
        }
        edge_record.twin_halfedge = Some(halfedge);
        brep.topology.halfedges[edge_record.halfedge as usize].twin = Some(halfedge);
        Some(edge_record.halfedge)
    };
    brep.topology.halfedges.push(HalfEdge {
        id: halfedge,
        from: vertex,
        to: vertex,
        twin,
        next: Some(halfedge),
        prev: Some(halfedge),
        edge,
        face: Some(face),
        loop_ref: Some(loop_id),
        wire_ref: None,
        geometry_use: HalfEdgeGeometryUse {
            sense,
            pcurve: Some(pcurve),
            periodic_lift: [0; 2],
        },
    });
    if brep.topology.vertices[vertex as usize]
        .outgoing_halfedge
        .is_none()
    {
        brep.topology.vertices[vertex as usize].outgoing_halfedge = Some(halfedge);
    }
    brep.topology.loops.push(Loop {
        id: loop_id,
        start_halfedge: halfedge,
        face_ref: face,
        is_hole,
    });
    Ok(loop_id)
}

fn offset_sphere_cylinder_boolean(
    sphere: &SphereInput<'_>,
    cylinder: &CylinderInput<'_>,
    sphere_is_a: bool,
    operation: BooleanOp,
    id: String,
    clearance: f64,
) -> Result<BooleanResult, GeometryError> {
    let local = cylinder.frame.local(sphere.frame.origin);
    let radial = local[0].hypot(local[1]);
    let scale = sphere
        .radius
        .max(cylinder.radius)
        .max(cylinder.height)
        .max(1.0);
    let roundoff = 128.0 * f64::EPSILON * scale;
    if radial <= roundoff {
        return coaxial_sphere_cylinder_boolean(
            sphere,
            cylinder,
            sphere_is_a,
            operation,
            id,
            clearance,
        );
    }
    let sphere_low = local[2] - sphere.radius;
    let sphere_high = local[2] + sphere.radius;
    if sphere_low <= clearance || cylinder.height - sphere_high <= clearance {
        return Err(GeometryError::CoverageGap {
            families: ["sphere".into(), "finite cylinder cap intersection".into()],
        });
    }
    let graph = intersect_faces(
        FaceView {
            brep: sphere.brep,
            face: 0,
        },
        FaceView {
            brep: cylinder.brep,
            face: 0,
        },
    )?;
    if graph.coincident || !graph.contacts.is_empty() || graph.branches.len() != 1 {
        return Err(GeometryError::CoverageGap {
            families: ["sphere".into(), "multi-branch or tangent cylinder".into()],
        });
    }
    let branch = &graph.branches[0];
    if norm(sub(branch.endpoints[0], branch.endpoints[1])) > clearance {
        return Err(GeometryError::CoverageGap {
            families: ["sphere".into(), "open cylinder intersection branch".into()],
        });
    }
    let curve_definition = match graph.geometry.curves[branch.curve as usize] {
        CurveGeometry::Intersection { definition } => definition,
        _ => {
            return Err(GeometryError::CoverageGap {
                families: ["sphere".into(), "non-numerical cylinder branch".into()],
            })
        }
    };
    let loop_inside_other = [
        classify_point(
            cylinder.brep,
            graph.geometry.surfaces[0].point_at(intersection_loop_sample(&graph, branch, 0)?)?,
        )?,
        classify_point(
            sphere.brep,
            graph.geometry.surfaces[1].point_at(intersection_loop_sample(&graph, branch, 1)?)?,
        )?,
    ];
    if loop_inside_other.iter().any(|classification| {
        matches!(
            classification,
            PointClassification::Unknown | PointClassification::Boundary
        )
    }) {
        return Err(GeometryError::UnresolvedIntersection(
            "sphere/cylinder intersection loop interior is unclassified".into(),
        ));
    }
    let loop_inside_other = loop_inside_other.map(|value| value == PointClassification::Inside);
    let (want_inside, reverse_selected) = match operation {
        BooleanOp::Union => ([false, false], [false, false]),
        BooleanOp::Intersection => ([true, true], [false, false]),
        BooleanOp::Subtraction if sphere_is_a => ([false, true], [false, true]),
        BooleanOp::Subtraction => ([true, false], [true, false]),
    };
    let regions: [LoopRegion; 2] = std::array::from_fn(|side| {
        if loop_inside_other[side] == want_inside[side] {
            LoopRegion::Patch
        } else {
            LoopRegion::Complement
        }
    });
    let inputs = [sphere.brep, cylinder.brep];
    let mut out = BrepEnvelope::new(
        id,
        Accuracy {
            geometric: sphere
                .brep
                .accuracy
                .geometric
                .max(cylinder.brep.accuracy.geometric),
            intersection: sphere
                .brep
                .accuracy
                .intersection
                .max(cylinder.brep.accuracy.intersection),
            tessellation: sphere
                .brep
                .accuracy
                .tessellation
                .max(cylinder.brep.accuracy.tessellation),
            exchange: sphere
                .brep
                .accuracy
                .exchange
                .max(cylinder.brep.accuracy.exchange),
        },
    )?;
    let mut selected_faces = [None; 2];
    let mut selected_surfaces = [None; 2];
    let mut selected_shells = Vec::new();
    for side in 0..2 {
        if regions[side] != LoopRegion::Complement {
            continue;
        }
        let face_offset = out.topology.faces.len() as u32;
        let shell_offset = out.topology.shells.len() as u32;
        append_analytic_input(&mut out, inputs[side])?;
        selected_faces[side] = Some(face_offset);
        selected_surfaces[side] = Some(out.topology.faces[face_offset as usize].surface);
        selected_shells.push(shell_offset);
        let next_face = out.topology.faces.len() as u32;
        for face in face_offset..next_face {
            out.topology.faces[face as usize].provenance.role = if face == face_offset {
                FaceRole::Split
            } else {
                FaceRole::Preserved
            };
        }
    }
    if selected_shells.is_empty() {
        out.topology.shells.push(Shell {
            id: 0,
            faces: Vec::new(),
            is_closed: true,
        });
        out.solids.push(SolidRegion {
            outer_shell: 0,
            cavity_shells: Vec::new(),
        });
    } else if selected_shells.len() == 2 {
        let faces = out.topology.shells[1].faces.clone();
        for face in &faces {
            out.topology.faces[*face as usize].shell_ref = Some(0);
        }
        out.topology.shells[0].faces.extend(faces);
        out.topology.shells.pop();
        out.solids.clear();
        out.solids.push(SolidRegion {
            outer_shell: 0,
            cavity_shells: Vec::new(),
        });
    }
    for side in 0..2 {
        if regions[side] == LoopRegion::Patch {
            selected_surfaces[side] = Some(out.geometry.surfaces.len() as u32);
            out.geometry
                .surfaces
                .push(graph.geometry.surfaces[side].clone());
        }
    }
    let mut definition_geometry = graph.geometry.intersections[curve_definition as usize].clone();
    definition_geometry.surfaces = [
        selected_surfaces[definition_geometry.surfaces[0] as usize].ok_or_else(|| {
            GeometryError::InvalidTopology(
                "sphere-cylinder first support surface was not retained".into(),
            )
        })?,
        selected_surfaces[definition_geometry.surfaces[1] as usize].ok_or_else(|| {
            GeometryError::InvalidTopology(
                "sphere-cylinder second support surface was not retained".into(),
            )
        })?,
    ];
    let definition = out.geometry.intersections.len() as u32;
    out.geometry.intersections.push(definition_geometry);
    let curve = out.geometry.curves.len() as u32;
    out.geometry
        .curves
        .push(CurveGeometry::Intersection { definition });
    let vertex = out.topology.vertices.len() as u32;
    out.topology.vertices.push(Vertex {
        id: vertex,
        position: branch.endpoints[0],
        outgoing_halfedge: None,
        tolerance: out.accuracy.geometric,
    });
    let edge = out.topology.edges.len() as u32;
    out.topology.edges.push(Edge {
        id: edge,
        geometry: EdgeGeometry::Curve {
            curve,
            range: branch.range,
        },
        halfedge: u32::MAX,
        twin_halfedge: None,
        tolerance: out.accuracy.geometric,
        chart_seam: false,
    });
    for side in 0..2 {
        let pcurve = match graph.geometry.pcurves[branch.pcurves[side] as usize] {
            PcurveGeometry::IntersectionSide {
                side: intersection_side,
                ..
            } => PcurveGeometry::IntersectionSide {
                definition,
                side: intersection_side,
            },
            _ => {
                return Err(GeometryError::InvalidGeometry(
                    "numerical sphere-cylinder curve is missing an intersection-side pcurve".into(),
                ))
            }
        };
        let nominal_sense = if side == 0 {
            Orientation::Forward
        } else {
            Orientation::Reverse
        };
        let loop_sense = if reverse_selected[side] {
            reverse(nominal_sense)
        } else {
            nominal_sense
        };
        if regions[side] == LoopRegion::Complement {
            let face = selected_faces[side].unwrap();
            let loop_bounds =
                intersection_loop_bounds(&graph, branch, side, out.accuracy.intersection)?;
            for axis in 0..2 {
                let bounds = out.topology.faces[face as usize].trim.uv_bounds[axis];
                out.topology.faces[face as usize].trim.uv_bounds[axis] = Interval::new(
                    bounds.lo.min(loop_bounds[axis].lo),
                    bounds.hi.max(loop_bounds[axis].hi),
                )?;
            }
            let loop_id = add_closed_intersection_loop(
                &mut out, edge, vertex, face, pcurve, loop_sense, true,
            )?;
            out.topology.faces[face as usize].trim.holes.push(loop_id);
        } else {
            let face = out.topology.faces.len() as u32;
            let loop_id = add_closed_intersection_loop(
                &mut out, edge, vertex, face, pcurve, loop_sense, false,
            )?;
            out.topology.faces.push(Face {
                id: face,
                key: format!("{}:intersection-patch", inputs[side].topology.faces[0].key),
                surface: selected_surfaces[side].unwrap(),
                sense: Orientation::Forward,
                trim: TrimRegion {
                    chart: 0,
                    uv_bounds: intersection_loop_bounds(
                        &graph,
                        branch,
                        side,
                        out.accuracy.intersection,
                    )?,
                    outer: loop_id,
                    holes: Vec::new(),
                },
                shell_ref: Some(0),
                provenance: FaceProvenance {
                    sources: vec![brep_face_source(inputs[side], 0)],
                    role: if reverse_selected[side] {
                        FaceRole::Cut
                    } else {
                        FaceRole::Split
                    },
                    reversed: reverse_selected[side],
                },
            });
            out.topology.shells[0].faces.push(face);
            selected_faces[side] = Some(face);
        }
    }
    for side in 0..2 {
        if reverse_selected[side] {
            reverse_face(&mut out, selected_faces[side].unwrap());
        }
    }
    out.revision = sphere
        .brep
        .revision
        .max(cylinder.brep.revision)
        .checked_add(1)
        .ok_or_else(|| GeometryError::LimitExceeded("boolean revision overflow".into()))?;
    out.validate()?;
    let mappings = if sphere_is_a {
        analytic_face_mappings(&out, [sphere.brep, cylinder.brep])
    } else {
        analytic_face_mappings(&out, [cylinder.brep, sphere.brep])
    };
    Ok(BooleanResult {
        brep: out,
        report: BooleanReport {
            operation,
            quality: GeometryQuality::Analytic,
            contacts: Vec::new(),
            coincident: false,
            face_mappings: mappings,
        },
    })
}

fn sphere_box_boolean(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let (sphere, box_, sphere_is_a) = if matches!(
        a.geometry.surfaces.first(),
        Some(SurfaceGeometry::Sphere { .. })
    ) {
        (full_sphere(a)?, super::box_booleans::full_box(b)?, true)
    } else {
        (full_sphere(b)?, super::box_booleans::full_box(a)?, false)
    };
    let clearance = 4.0
        * sphere
            .brep
            .accuracy
            .geometric
            .max(box_.brep.accuracy.geometric);
    let center = box_.frame.local(sphere.frame.origin);
    let sphere_margin = (0..3).fold(f64::INFINITY, |margin, axis| {
        margin
            .min(center[axis] - sphere.radius)
            .min(box_.size[axis] - center[axis] - sphere.radius)
    });
    let mut farthest: f64 = 0.0;
    for corner in 0..8 {
        let point = box_.frame.point(std::array::from_fn(|axis| {
            if corner & (1 << axis) == 0 {
                0.0
            } else {
                box_.size[axis]
            }
        }));
        farthest = farthest.max(norm(sub(point, sphere.frame.origin)));
    }
    let box_margin = sphere.radius - farthest;
    let sphere_inside_box = sphere_margin > clearance;
    let box_inside_sphere = box_margin > clearance;
    if sphere_inside_box || box_inside_sphere {
        let b_inside_a = if sphere_is_a {
            box_inside_sphere
        } else {
            sphere_inside_box
        };
        return analytic_containment_boolean(a, b, b_inside_a, operation, id);
    }
    if sphere_margin.abs() <= clearance || box_margin.abs() <= clearance {
        return Err(GeometryError::UnresolvedIntersection(
            "sphere/cuboid containment boundary is below geometric resolution".into(),
        ));
    }
    single_face_sphere_box_boolean(&sphere, &box_, sphere_is_a, operation, id, clearance)
}

fn box_source(input: &super::box_booleans::BoxInput<'_>, face: usize) -> FaceSource {
    FaceSource {
        entity: input.brep.id.clone(),
        body: input.brep.id.clone(),
        key: input.brep.topology.faces[face].key.clone(),
        face: face as u32,
    }
}

fn single_face_sphere_box_boolean(
    sphere: &SphereInput<'_>,
    box_: &super::box_booleans::BoxInput<'_>,
    sphere_is_a: bool,
    operation: BooleanOp,
    id: String,
    clearance: f64,
) -> Result<BooleanResult, GeometryError> {
    let accuracy = Accuracy {
        geometric: sphere
            .brep
            .accuracy
            .geometric
            .max(box_.brep.accuracy.geometric),
        intersection: sphere
            .brep
            .accuracy
            .intersection
            .max(box_.brep.accuracy.intersection),
        tessellation: sphere
            .brep
            .accuracy
            .tessellation
            .max(box_.brep.accuracy.tessellation),
        exchange: sphere
            .brep
            .accuracy
            .exchange
            .max(box_.brep.accuracy.exchange),
    };
    let center = box_.frame.local(sphere.frame.origin);
    let box_axes = [box_.frame.x, box_.frame.y, box_.frame.z];
    let mut crossings = Vec::new();
    for axis in 0..3 {
        for upper in [false, true] {
            let signed_plane = if upper {
                center[axis] - box_.size[axis]
            } else {
                -center[axis]
            };
            let separation = signed_plane.abs() - sphere.radius;
            if separation.abs() <= clearance {
                return Err(GeometryError::UnresolvedIntersection(
                    "sphere is tangent to a cuboid face within geometric resolution".into(),
                ));
            }
            if separation < 0.0 {
                crossings.push((axis, upper, signed_plane));
            } else if signed_plane > sphere.radius {
                return Err(GeometryError::CoverageGap {
                    families: ["sphere".into(), "cuboid".into()],
                });
            }
        }
    }
    let [(axis, upper, signed_plane)] = crossings.as_slice() else {
        return Err(GeometryError::CoverageGap {
            families: ["sphere".into(), "multi-face cuboid intersection".into()],
        });
    };
    let axis = *axis;
    let upper = *upper;
    let signed_plane = *signed_plane;
    let inward = if upper {
        scale(box_axes[axis], -1.0)
    } else {
        box_axes[axis]
    };
    let tangent_axis = match axis {
        0 => box_axes[1],
        _ => box_axes[0],
    };
    let cap_frame = Frame3::from_axis(sphere.frame.origin, inward, tangent_axis)?;
    let latitude = (signed_plane / sphere.radius).asin();
    let circle_radius =
        ((sphere.radius - signed_plane.abs()) * (sphere.radius + signed_plane.abs())).sqrt();
    if circle_radius <= clearance {
        return Err(GeometryError::UnresolvedIntersection(
            "sphere/cuboid intersection circle is below geometric resolution".into(),
        ));
    }
    let circle_frame = Frame3 {
        origin: cap_frame.point([0.0, 0.0, signed_plane]),
        ..cap_frame
    };
    let entry_face = match axis {
        0 => 2 + usize::from(upper),
        1 => 4 + usize::from(upper),
        _ => usize::from(upper),
    };
    let mut builder = Builder::new(id, accuracy)?;
    let circle_vertex = builder.vertex(circle_frame.point([circle_radius, 0.0, 0.0]));
    let circle_edge = builder.edge(
        CurveGeometry::Circle {
            frame: circle_frame,
            radius: circle_radius,
        },
        Interval::new(0.0, std::f64::consts::TAU)?,
        false,
    );

    if operation == BooleanOp::Intersection || (operation == BooleanOp::Subtraction && sphere_is_a)
    {
        let keep_inside_box = operation == BooleanOp::Intersection;
        let disk_frame = if keep_inside_box {
            Frame3 {
                x: circle_frame.x,
                y: scale(circle_frame.y, -1.0),
                z: scale(circle_frame.z, -1.0),
                ..circle_frame
            }
        } else {
            circle_frame
        };
        builder.face(
            "cuboid:cap",
            SurfaceGeometry::Plane { frame: disk_frame },
            [[-circle_radius, circle_radius]; 2],
            vec![boundary(
                circle_edge,
                circle_vertex,
                circle_vertex,
                if keep_inside_box {
                    Orientation::Reverse
                } else {
                    Orientation::Forward
                },
                PcurveGeometry::Conic2 {
                    origin: [0.0; 2],
                    axis_a: [circle_radius, 0.0],
                    axis_b: [
                        0.0,
                        if keep_inside_box {
                            -circle_radius
                        } else {
                            circle_radius
                        },
                    ],
                },
            )],
        )?;
        builder.brep.topology.faces[0].provenance = face_provenance(
            vec![box_source(box_, entry_face)],
            if keep_inside_box {
                FaceRole::Split
            } else {
                FaceRole::Cut
            },
            !keep_inside_box,
        );
        patch(
            &mut builder,
            sphere,
            cap_frame,
            latitude,
            keep_inside_box,
            circle_edge,
            circle_vertex,
            false,
        )?;
    } else {
        let points: Vec<_> = (0..8)
            .map(|index| {
                box_.frame.point(std::array::from_fn(|coordinate| {
                    if index & (1 << coordinate) == 0 {
                        0.0
                    } else {
                        box_.size[coordinate]
                    }
                }))
            })
            .collect();
        let vertices = points
            .iter()
            .map(|point| builder.vertex(*point))
            .collect::<Vec<_>>();
        let mut edges = [[u32::MAX; 3]; 8];
        for index in 0..8 {
            for coordinate in 0..3 {
                if index & (1 << coordinate) == 0 {
                    edges[index][coordinate] = builder.edge(
                        CurveGeometry::Line {
                            origin: points[index],
                            direction: box_axes[coordinate],
                        },
                        Interval::new(0.0, box_.size[coordinate])?,
                        false,
                    );
                }
            }
        }
        for (face_index, (key, indices, normal)) in [
            ("bottom", [0, 2, 3, 1], scale(box_.frame.z, -1.0)),
            ("top", [4, 5, 7, 6], box_.frame.z),
            ("left", [0, 4, 6, 2], scale(box_.frame.x, -1.0)),
            ("right", [1, 3, 7, 5], box_.frame.x),
            ("front", [0, 1, 5, 4], scale(box_.frame.y, -1.0)),
            ("back", [2, 6, 7, 3], box_.frame.y),
        ]
        .into_iter()
        .enumerate()
        {
            let face_frame = Frame3::from_axis(
                points[indices[0]],
                normal,
                sub(points[indices[1]], points[indices[0]]),
            )?;
            let mut bounds = [[f64::INFINITY, f64::NEG_INFINITY]; 2];
            let mut uses = Vec::with_capacity(4);
            for index in 0..4 {
                let from = indices[index];
                let to = indices[(index + 1) % 4];
                let coordinate = (from ^ to).trailing_zeros() as usize;
                uses.push(plane_boundary(
                    &builder,
                    face_frame,
                    edges[from.min(to)][coordinate],
                    vertices[from],
                    vertices[to],
                    if from < to {
                        Orientation::Forward
                    } else {
                        Orientation::Reverse
                    },
                )?);
                let uv = face_frame.local(points[from]);
                for coordinate in 0..2 {
                    bounds[coordinate][0] = bounds[coordinate][0].min(uv[coordinate]);
                    bounds[coordinate][1] = bounds[coordinate][1].max(uv[coordinate]);
                }
            }
            for bound in &mut bounds {
                bound[0] -= accuracy.geometric;
                bound[1] += accuracy.geometric;
            }
            let holes = if face_index == entry_face {
                vec![vec![plane_boundary(
                    &builder,
                    face_frame,
                    circle_edge,
                    circle_vertex,
                    circle_vertex,
                    Orientation::Forward,
                )?]]
            } else {
                Vec::new()
            };
            builder.face_with_holes(
                key,
                SurfaceGeometry::Plane { frame: face_frame },
                bounds,
                uses,
                holes,
            )?;
            builder.brep.topology.faces[face_index].provenance = face_provenance(
                vec![box_source(box_, face_index)],
                if face_index == entry_face {
                    FaceRole::Split
                } else {
                    FaceRole::Preserved
                },
                false,
            );
        }
        patch(
            &mut builder,
            sphere,
            cap_frame,
            latitude,
            operation == BooleanOp::Subtraction,
            circle_edge,
            circle_vertex,
            operation == BooleanOp::Subtraction,
        )?;
    }

    let mut out = builder.finish()?;
    out.revision = sphere
        .brep
        .revision
        .max(box_.brep.revision)
        .checked_add(1)
        .ok_or_else(|| GeometryError::LimitExceeded("boolean revision overflow".into()))?;
    out.validate()?;
    let inputs = if sphere_is_a {
        [sphere.brep, box_.brep]
    } else {
        [box_.brep, sphere.brep]
    };
    Ok(BooleanResult {
        report: BooleanReport {
            operation,
            quality: GeometryQuality::Analytic,
            contacts: Vec::new(),
            coincident: false,
            face_mappings: analytic_face_mappings(&out, inputs),
        },
        brep: out,
    })
}

fn perpendicular_cylinder_box_boolean(
    box_: &super::box_booleans::BoxInput<'_>,
    cylinder: &CylinderInput<'_>,
    cylinder_is_a: bool,
    operation: BooleanOp,
    id: String,
    accuracy: Accuracy,
) -> Result<BooleanResult, GeometryError> {
    let box_axes = [box_.frame.x, box_.frame.y, box_.frame.z];
    let (axis, alignment) = box_axes
        .iter()
        .enumerate()
        .map(|(axis, direction)| (axis, dot(cylinder.frame.z, *direction)))
        .max_by(|left, right| left.1.abs().total_cmp(&right.1.abs()))
        .ok_or_else(|| GeometryError::InvalidGeometry("cuboid has no frame axes".into()))?;
    if 1.0 - alignment.abs() > 1.0e-12 {
        return Err(GeometryError::CoverageGap {
            families: ["cylinder".into(), "oblique cuboid intersection".into()],
        });
    }
    let local_origin = box_.frame.local(cylinder.frame.origin);
    let other_axes = match axis {
        0 => [1, 2],
        1 => [0, 2],
        _ => [0, 1],
    };
    let radial_margins = other_axes.map(|candidate| {
        (local_origin[candidate] - cylinder.radius)
            .min(box_.size[candidate] - local_origin[candidate] - cylinder.radius)
    });
    if radial_margins
        .iter()
        .any(|margin| margin.abs() <= 4.0 * accuracy.geometric)
    {
        return Err(GeometryError::UnresolvedIntersection(
            "cylindrical opening is tangent to a cuboid side within geometric resolution".into(),
        ));
    }
    if radial_margins.iter().any(|margin| *margin < 0.0) {
        return Err(GeometryError::CoverageGap {
            families: ["cylinder".into(), "side-intersecting cuboid".into()],
        });
    }
    let axial_end = local_origin[axis] + alignment * cylinder.height;
    let axial_span = [
        local_origin[axis].min(axial_end),
        local_origin[axis].max(axial_end),
    ];
    let low_clearance = -axial_span[0];
    let high_clearance = axial_span[1] - box_.size[axis];
    if low_clearance.abs() <= 4.0 * accuracy.geometric
        || high_clearance.abs() <= 4.0 * accuracy.geometric
    {
        return Err(GeometryError::UnresolvedIntersection(
            "cylinder cap meets a cuboid boundary within geometric resolution".into(),
        ));
    }
    let crosses_low = low_clearance > 4.0 * accuracy.geometric;
    let crosses_high = high_clearance > 4.0 * accuracy.geometric;
    let clipped_low = axial_span[0].max(0.0);
    let clipped_high = axial_span[1].min(box_.size[axis]);
    if clipped_high - clipped_low <= 4.0 * accuracy.geometric {
        return Err(GeometryError::UnresolvedIntersection(
            "cylinder/cuboid overlap is below geometric resolution".into(),
        ));
    }
    if !crosses_low && !crosses_high {
        return Err(GeometryError::CoverageGap {
            families: ["cylinder".into(), "contained cuboid intersection".into()],
        });
    }

    let mut center = local_origin;
    center[axis] = clipped_low;
    let low_center = box_.frame.point(center);
    let tunnel_frame = Frame3::from_axis(low_center, box_axes[axis], box_axes[other_axes[0]])?;
    let tunnel_height = clipped_high - clipped_low;
    let low_face = match axis {
        0 => 2,
        1 => 4,
        _ => 0,
    };
    let high_face = low_face + 1;
    let cylinder_low_face = if alignment > 0.0 { 1 } else { 2 };
    let cylinder_high_face = if alignment > 0.0 { 2 } else { 1 };
    if operation == BooleanOp::Intersection {
        let mut out =
            primitives::cylinder(id, tunnel_frame, cylinder.radius, tunnel_height, accuracy)?;
        out.topology.faces[0].provenance =
            face_provenance(vec![cylinder_source(cylinder, 0)], FaceRole::Split, false);
        out.topology.faces[1].provenance = face_provenance(
            vec![if crosses_low {
                box_source(box_, low_face)
            } else {
                cylinder_source(cylinder, cylinder_low_face)
            }],
            FaceRole::Split,
            false,
        );
        out.topology.faces[2].provenance = face_provenance(
            vec![if crosses_high {
                box_source(box_, high_face)
            } else {
                cylinder_source(cylinder, cylinder_high_face)
            }],
            FaceRole::Split,
            false,
        );
        out.revision = box_
            .brep
            .revision
            .max(cylinder.brep.revision)
            .checked_add(1)
            .ok_or_else(|| GeometryError::LimitExceeded("boolean revision overflow".into()))?;
        out.validate()?;
        return Ok(BooleanResult {
            report: BooleanReport {
                operation,
                quality: GeometryQuality::Analytic,
                contacts: Vec::new(),
                coincident: false,
                face_mappings: analytic_face_mappings(&out, [box_.brep, cylinder.brep]),
            },
            brep: out,
        });
    }
    if operation == BooleanOp::Subtraction && cylinder_is_a {
        let mut out = BrepEnvelope::new(id, accuracy)?;
        if crosses_low {
            let mut frame = tunnel_frame;
            frame.origin = box_.frame.point({
                let mut point = local_origin;
                point[axis] = axial_span[0];
                point
            });
            append_cylinder_segment(
                &mut out,
                frame,
                cylinder.radius,
                -axial_span[0],
                [
                    face_provenance(vec![cylinder_source(cylinder, 0)], FaceRole::Split, false),
                    face_provenance(
                        vec![cylinder_source(cylinder, cylinder_low_face)],
                        FaceRole::Preserved,
                        false,
                    ),
                    face_provenance(vec![box_source(box_, low_face)], FaceRole::Cut, true),
                ],
                0,
            )?;
        }
        if crosses_high {
            let mut frame = tunnel_frame;
            frame.origin = box_.frame.point({
                let mut point = local_origin;
                point[axis] = box_.size[axis];
                point
            });
            append_cylinder_segment(
                &mut out,
                frame,
                cylinder.radius,
                axial_span[1] - box_.size[axis],
                [
                    face_provenance(vec![cylinder_source(cylinder, 0)], FaceRole::Split, false),
                    face_provenance(vec![box_source(box_, high_face)], FaceRole::Cut, true),
                    face_provenance(
                        vec![cylinder_source(cylinder, cylinder_high_face)],
                        FaceRole::Preserved,
                        false,
                    ),
                ],
                usize::from(crosses_low),
            )?;
        }
        out.revision = box_
            .brep
            .revision
            .max(cylinder.brep.revision)
            .checked_add(1)
            .ok_or_else(|| GeometryError::LimitExceeded("boolean revision overflow".into()))?;
        out.validate()?;
        let inputs = if cylinder_is_a {
            [cylinder.brep, box_.brep]
        } else {
            [box_.brep, cylinder.brep]
        };
        return Ok(BooleanResult {
            report: BooleanReport {
                operation,
                quality: GeometryQuality::Analytic,
                contacts: Vec::new(),
                coincident: false,
                face_mappings: analytic_face_mappings(&out, inputs),
            },
            brep: out,
        });
    }
    let mut builder = Builder::new(id, accuracy)?;
    let points: Vec<_> = (0..8)
        .map(|index| {
            box_.frame.point(std::array::from_fn(|coordinate| {
                if index & (1 << coordinate) == 0 {
                    0.0
                } else {
                    box_.size[coordinate]
                }
            }))
        })
        .collect();
    let vertices = points
        .iter()
        .map(|point| builder.vertex(*point))
        .collect::<Vec<_>>();
    let mut edges = [[u32::MAX; 3]; 8];
    for index in 0..8 {
        for coordinate in 0..3 {
            if index & (1 << coordinate) == 0 {
                edges[index][coordinate] = builder.edge(
                    CurveGeometry::Line {
                        origin: points[index],
                        direction: box_axes[coordinate],
                    },
                    Interval::new(0.0, box_.size[coordinate])?,
                    false,
                );
            }
        }
    }
    let tau = std::f64::consts::TAU;
    let high_frame = Frame3 {
        origin: tunnel_frame.point([0.0, 0.0, tunnel_height]),
        ..tunnel_frame
    };
    let low_boundary = if operation == BooleanOp::Subtraction || crosses_low {
        Some(circular_boundary(
            &mut builder,
            tunnel_frame,
            cylinder.radius,
            0.0,
        )?)
    } else {
        None
    };
    let high_boundary = if operation == BooleanOp::Subtraction || crosses_high {
        Some(circular_boundary(
            &mut builder,
            tunnel_frame,
            cylinder.radius,
            tunnel_height,
        )?)
    } else {
        None
    };
    let seam = if operation == BooleanOp::Subtraction {
        Some(builder.edge(
            CurveGeometry::Line {
                origin: tunnel_frame.point([cylinder.radius, 0.0, 0.0]),
                direction: tunnel_frame.z,
            },
            Interval::new(0.0, tunnel_height)?,
            true,
        ))
    } else {
        None
    };
    for (face_index, (key, indices, normal)) in [
        ("bottom", [0, 2, 3, 1], scale(box_.frame.z, -1.0)),
        ("top", [4, 5, 7, 6], box_.frame.z),
        ("left", [0, 4, 6, 2], scale(box_.frame.x, -1.0)),
        ("right", [1, 3, 7, 5], box_.frame.x),
        ("front", [0, 1, 5, 4], scale(box_.frame.y, -1.0)),
        ("back", [2, 6, 7, 3], box_.frame.y),
    ]
    .into_iter()
    .enumerate()
    {
        let face_frame = Frame3::from_axis(
            points[indices[0]],
            normal,
            sub(points[indices[1]], points[indices[0]]),
        )?;
        let mut bounds = [[f64::INFINITY, f64::NEG_INFINITY]; 2];
        let mut uses = Vec::with_capacity(4);
        for index in 0..4 {
            let from = indices[index];
            let to = indices[(index + 1) % 4];
            let coordinate = (from ^ to).trailing_zeros() as usize;
            uses.push(plane_boundary(
                &builder,
                face_frame,
                edges[from.min(to)][coordinate],
                vertices[from],
                vertices[to],
                if from < to {
                    Orientation::Forward
                } else {
                    Orientation::Reverse
                },
            )?);
            let uv = face_frame.local(points[from]);
            for coordinate in 0..2 {
                bounds[coordinate][0] = bounds[coordinate][0].min(uv[coordinate]);
                bounds[coordinate][1] = bounds[coordinate][1].max(uv[coordinate]);
            }
        }
        for bound in &mut bounds {
            bound[0] -= accuracy.geometric;
            bound[1] += accuracy.geometric;
        }
        let holes = if (face_index == low_face && crosses_low)
            || (face_index == high_face && crosses_high)
        {
            let is_low = face_index == low_face;
            let circle_frame = if is_low { tunnel_frame } else { high_frame };
            let circle_center = face_frame.local(circle_frame.origin);
            let (vertex, circle) = if is_low {
                low_boundary.ok_or_else(|| {
                    GeometryError::InvalidTopology("missing low cylinder boundary".into())
                })?
            } else {
                high_boundary.ok_or_else(|| {
                    GeometryError::InvalidTopology("missing high cylinder boundary".into())
                })?
            };
            vec![vec![boundary(
                circle,
                vertex,
                vertex,
                if is_low {
                    Orientation::Forward
                } else {
                    Orientation::Reverse
                },
                PcurveGeometry::Conic2 {
                    origin: [circle_center[0], circle_center[1]],
                    axis_a: [
                        cylinder.radius * dot(circle_frame.x, face_frame.x),
                        cylinder.radius * dot(circle_frame.x, face_frame.y),
                    ],
                    axis_b: [
                        cylinder.radius * dot(circle_frame.y, face_frame.x),
                        cylinder.radius * dot(circle_frame.y, face_frame.y),
                    ],
                },
            )]]
        } else {
            Vec::new()
        };
        builder.face_with_holes(
            key,
            SurfaceGeometry::Plane { frame: face_frame },
            bounds,
            uses,
            holes,
        )?;
        builder.brep.topology.faces[face_index].provenance = face_provenance(
            vec![box_source(box_, face_index)],
            if (face_index == low_face && crosses_low) || (face_index == high_face && crosses_high)
            {
                FaceRole::Split
            } else {
                FaceRole::Preserved
            },
            false,
        );
    }
    if operation == BooleanOp::Subtraction {
        let (low_vertex, low_circle) = low_boundary.ok_or_else(|| {
            GeometryError::InvalidTopology("missing low cylinder boundary".into())
        })?;
        let (high_vertex, high_circle) = high_boundary.ok_or_else(|| {
            GeometryError::InvalidTopology("missing high cylinder boundary".into())
        })?;
        let seam = seam.ok_or_else(|| {
            GeometryError::InvalidTopology("missing cylinder seam boundary".into())
        })?;
        let tunnel_face = builder.brep.topology.faces.len() as u32;
        builder.face(
            "cutter:tunnel",
            SurfaceGeometry::Cylinder {
                frame: tunnel_frame,
                radius: cylinder.radius,
            },
            [[0.0, tau], [0.0, tunnel_height]],
            vec![
                boundary(
                    low_circle,
                    low_vertex,
                    low_vertex,
                    Orientation::Forward,
                    uv_line([0.0, 0.0], [1.0, 0.0]),
                ),
                boundary(
                    seam,
                    low_vertex,
                    high_vertex,
                    Orientation::Forward,
                    uv_line([tau, 0.0], [0.0, 1.0]),
                ),
                boundary(
                    high_circle,
                    high_vertex,
                    high_vertex,
                    Orientation::Reverse,
                    uv_line([0.0, tunnel_height], [1.0, 0.0]),
                ),
                boundary(
                    seam,
                    high_vertex,
                    low_vertex,
                    Orientation::Reverse,
                    uv_line([0.0, 0.0], [0.0, 1.0]),
                ),
            ],
        )?;
        builder.brep.topology.faces[tunnel_face as usize].provenance =
            face_provenance(vec![cylinder_source(cylinder, 0)], FaceRole::Cut, true);
        reverse_face(&mut builder.brep, tunnel_face);
        if crosses_low != crosses_high {
            let (key, circle, vertex, mut cap_frame, sense, axis_b, cylinder_cap) = if crosses_low {
                (
                    "cutter:pocket_cap",
                    high_circle,
                    high_vertex,
                    high_frame,
                    Orientation::Forward,
                    [0.0, cylinder.radius],
                    if alignment > 0.0 { 2 } else { 1 },
                )
            } else {
                let mut cap_frame = tunnel_frame;
                cap_frame.y = scale(cap_frame.y, -1.0);
                cap_frame.z = scale(cap_frame.z, -1.0);
                (
                    "cutter:pocket_cap",
                    low_circle,
                    low_vertex,
                    cap_frame,
                    Orientation::Reverse,
                    [0.0, -cylinder.radius],
                    if alignment > 0.0 { 1 } else { 2 },
                )
            };
            let cap_face = builder.brep.topology.faces.len() as u32;
            cap_frame.origin = if crosses_low {
                high_frame.origin
            } else {
                tunnel_frame.origin
            };
            builder.face(
                key,
                SurfaceGeometry::Plane { frame: cap_frame },
                [[-cylinder.radius, cylinder.radius]; 2],
                vec![boundary(
                    circle,
                    vertex,
                    vertex,
                    sense,
                    PcurveGeometry::Conic2 {
                        origin: [0.0; 2],
                        axis_a: [cylinder.radius, 0.0],
                        axis_b,
                    },
                )],
            )?;
            builder.brep.topology.faces[cap_face as usize].provenance = face_provenance(
                vec![cylinder_source(cylinder, cylinder_cap)],
                FaceRole::Cut,
                true,
            );
            reverse_face(&mut builder.brep, cap_face);
        }
    } else {
        if crosses_low {
            let entry = low_boundary.ok_or_else(|| {
                GeometryError::InvalidTopology("missing low cylinder boundary".into())
            })?;
            let outer_z = axial_span[0] - clipped_low;
            let outer = circular_boundary(&mut builder, tunnel_frame, cylinder.radius, outer_z)?;
            cylinder_band(
                &mut builder,
                tunnel_frame,
                cylinder.radius,
                outer_z,
                0.0,
                outer,
                entry,
                face_provenance(vec![cylinder_source(cylinder, 0)], FaceRole::Split, false),
                false,
            )?;
            cylinder_cap(
                &mut builder,
                tunnel_frame,
                cylinder.radius,
                outer_z,
                outer,
                false,
                face_provenance(
                    vec![cylinder_source(cylinder, cylinder_low_face)],
                    FaceRole::Preserved,
                    false,
                ),
            )?;
        }
        if crosses_high {
            let entry = high_boundary.ok_or_else(|| {
                GeometryError::InvalidTopology("missing high cylinder boundary".into())
            })?;
            let outer_z = axial_span[1] - clipped_low;
            let outer = circular_boundary(&mut builder, tunnel_frame, cylinder.radius, outer_z)?;
            cylinder_band(
                &mut builder,
                tunnel_frame,
                cylinder.radius,
                tunnel_height,
                outer_z,
                entry,
                outer,
                face_provenance(vec![cylinder_source(cylinder, 0)], FaceRole::Split, false),
                false,
            )?;
            cylinder_cap(
                &mut builder,
                tunnel_frame,
                cylinder.radius,
                outer_z,
                outer,
                true,
                face_provenance(
                    vec![cylinder_source(cylinder, cylinder_high_face)],
                    FaceRole::Preserved,
                    false,
                ),
            )?;
        }
    }
    let mut out = builder.finish()?;
    out.revision = box_
        .brep
        .revision
        .max(cylinder.brep.revision)
        .checked_add(1)
        .ok_or_else(|| GeometryError::LimitExceeded("boolean revision overflow".into()))?;
    out.validate()?;
    let inputs = if cylinder_is_a {
        [cylinder.brep, box_.brep]
    } else {
        [box_.brep, cylinder.brep]
    };
    Ok(BooleanResult {
        report: BooleanReport {
            operation,
            quality: GeometryQuality::Analytic,
            contacts: Vec::new(),
            coincident: false,
            face_mappings: analytic_face_mappings(&out, inputs),
        },
        brep: out,
    })
}

fn cylinder_box_boolean(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let (cylinder, box_, cylinder_is_a) = if matches!(
        a.geometry.surfaces.first(),
        Some(SurfaceGeometry::Cylinder { .. })
    ) {
        (full_cylinder(a)?, super::box_booleans::full_box(b)?, true)
    } else {
        (full_cylinder(b)?, super::box_booleans::full_box(a)?, false)
    };
    let clearance = 4.0
        * cylinder
            .brep
            .accuracy
            .geometric
            .max(box_.brep.accuracy.geometric);
    let origin = box_.frame.local(cylinder.frame.origin);
    let box_axes = [box_.frame.x, box_.frame.y, box_.frame.z];
    let mut cylinder_margin = f64::INFINITY;
    for axis in 0..3 {
        let axial_rate = dot(cylinder.frame.z, box_axes[axis]);
        let radial_extent = cylinder.radius * (1.0 - axial_rate * axial_rate).max(0.0).sqrt();
        let other = origin[axis] + axial_rate * cylinder.height;
        let lo = origin[axis].min(other) - radial_extent;
        let hi = origin[axis].max(other) + radial_extent;
        cylinder_margin = cylinder_margin.min(lo).min(box_.size[axis] - hi);
    }
    let mut box_margin = f64::INFINITY;
    for corner in 0..8 {
        let point = box_.frame.point(std::array::from_fn(|axis| {
            if corner & (1 << axis) == 0 {
                0.0
            } else {
                box_.size[axis]
            }
        }));
        let local = cylinder.frame.local(point);
        box_margin = box_margin
            .min(cylinder.radius - local[0].hypot(local[1]))
            .min(local[2])
            .min(cylinder.height - local[2]);
    }
    let cylinder_inside_box = cylinder_margin > clearance;
    let box_inside_cylinder = box_margin > clearance;
    if cylinder_inside_box || box_inside_cylinder {
        let b_inside_a = if cylinder_is_a {
            box_inside_cylinder
        } else {
            cylinder_inside_box
        };
        return analytic_containment_boolean(a, b, b_inside_a, operation, id);
    }
    if cylinder_margin.abs() <= clearance || box_margin.abs() <= clearance {
        return Err(GeometryError::UnresolvedIntersection(
            "cylinder/cuboid containment boundary is below geometric resolution".into(),
        ));
    }
    let accuracy = Accuracy {
        geometric: cylinder
            .brep
            .accuracy
            .geometric
            .max(box_.brep.accuracy.geometric),
        intersection: cylinder
            .brep
            .accuracy
            .intersection
            .max(box_.brep.accuracy.intersection),
        tessellation: cylinder
            .brep
            .accuracy
            .tessellation
            .max(box_.brep.accuracy.tessellation),
        exchange: cylinder
            .brep
            .accuracy
            .exchange
            .max(box_.brep.accuracy.exchange),
    };
    perpendicular_cylinder_box_boolean(&box_, &cylinder, cylinder_is_a, operation, id, accuracy)
}

fn cylinder_source(input: &CylinderInput<'_>, face: u32) -> FaceSource {
    FaceSource {
        entity: input.brep.id.clone(),
        body: input.brep.id.clone(),
        key: input.brep.topology.faces[face as usize].key.clone(),
        face,
    }
}

fn face_provenance(sources: Vec<FaceSource>, role: FaceRole, reversed: bool) -> FaceProvenance {
    FaceProvenance {
        sources,
        role,
        reversed,
    }
}

fn circular_boundary(
    builder: &mut Builder,
    frame: Frame3,
    radius: f64,
    z: f64,
) -> Result<(u32, u32), GeometryError> {
    let mut circle_frame = frame;
    circle_frame.origin = frame.point([0.0, 0.0, z]);
    let vertex = builder.vertex(frame.point([radius, 0.0, z]));
    let edge = builder.edge(
        CurveGeometry::Circle {
            frame: circle_frame,
            radius,
        },
        Interval::new(0.0, std::f64::consts::TAU)?,
        false,
    );
    Ok((vertex, edge))
}

fn cylinder_band(
    builder: &mut Builder,
    frame: Frame3,
    radius: f64,
    z0: f64,
    z1: f64,
    low: (u32, u32),
    high: (u32, u32),
    provenance: FaceProvenance,
    reversed: bool,
) -> Result<(), GeometryError> {
    use Orientation::{Forward as F, Reverse as R};
    let seam = builder.edge(
        CurveGeometry::Line {
            origin: frame.point([radius, 0.0, z0]),
            direction: frame.z,
        },
        Interval::new(0.0, z1 - z0)?,
        true,
    );
    let face = builder.brep.topology.faces.len() as u32;
    let tau = std::f64::consts::TAU;
    builder.face(
        &format!("{face}:cylinder-band"),
        SurfaceGeometry::Cylinder { frame, radius },
        [[0.0, tau], [z0, z1]],
        vec![
            boundary(low.1, low.0, low.0, F, uv_line([0.0, z0], [1.0, 0.0])),
            boundary(seam, low.0, high.0, F, uv_line([tau, z0], [0.0, 1.0])),
            boundary(high.1, high.0, high.0, R, uv_line([0.0, z1], [1.0, 0.0])),
            boundary(seam, high.0, low.0, R, uv_line([0.0, z0], [0.0, 1.0])),
        ],
    )?;
    builder.brep.topology.faces[face as usize].provenance = provenance;
    if reversed {
        reverse_face(&mut builder.brep, face);
    }
    Ok(())
}

fn cylinder_cap(
    builder: &mut Builder,
    frame: Frame3,
    radius: f64,
    z: f64,
    boundary_edge: (u32, u32),
    top: bool,
    provenance: FaceProvenance,
) -> Result<(), GeometryError> {
    use Orientation::{Forward as F, Reverse as R};
    let mut surface_frame = frame;
    surface_frame.origin = frame.point([0.0, 0.0, z]);
    if !top {
        surface_frame.y = scale(surface_frame.y, -1.0);
        surface_frame.z = scale(surface_frame.z, -1.0);
    }
    let face = builder.brep.topology.faces.len() as u32;
    builder.face(
        &format!("{face}:cylinder-cap"),
        SurfaceGeometry::Plane {
            frame: surface_frame,
        },
        [[-radius, radius]; 2],
        vec![boundary(
            boundary_edge.1,
            boundary_edge.0,
            boundary_edge.0,
            if top { F } else { R },
            PcurveGeometry::Conic2 {
                origin: [0.0; 2],
                axis_a: [radius, 0.0],
                axis_b: [0.0, if top { radius } else { -radius }],
            },
        )],
    )?;
    builder.brep.topology.faces[face as usize].provenance = provenance;
    Ok(())
}

fn sphere_band(
    builder: &mut Builder,
    input: &SphereInput<'_>,
    frame: Frame3,
    latitude0: f64,
    latitude1: f64,
    low: (u32, u32),
    high: (u32, u32),
    reversed: bool,
) -> Result<(), GeometryError> {
    use Orientation::{Forward as F, Reverse as R};
    let seam = builder.edge(
        CurveGeometry::Circle {
            frame: Frame3 {
                x: frame.x,
                y: frame.z,
                z: scale(frame.y, -1.0),
                ..frame
            },
            radius: input.radius,
        },
        Interval::new(latitude0, latitude1)?,
        true,
    );
    let face = builder.brep.topology.faces.len() as u32;
    let tau = std::f64::consts::TAU;
    builder.face(
        &format!("{face}:sphere-band"),
        SurfaceGeometry::Sphere {
            frame,
            radius: input.radius,
        },
        [[0.0, tau], [latitude0, latitude1]],
        vec![
            boundary(
                low.1,
                low.0,
                low.0,
                F,
                uv_line([0.0, latitude0], [1.0, 0.0]),
            ),
            boundary(seam, low.0, high.0, F, uv_line([tau, 0.0], [0.0, 1.0])),
            boundary(
                high.1,
                high.0,
                high.0,
                R,
                uv_line([0.0, latitude1], [1.0, 0.0]),
            ),
            boundary(seam, high.0, low.0, R, uv_line([0.0, 0.0], [0.0, 1.0])),
        ],
    )?;
    builder.brep.topology.faces[face as usize].provenance = provenance(
        input,
        if reversed {
            FaceRole::Cut
        } else {
            FaceRole::Split
        },
        reversed,
    );
    if reversed {
        reverse_face(&mut builder.brep, face);
    }
    Ok(())
}

fn finish_sphere_cylinder_result(
    mut brep: BrepEnvelope,
    sphere: &SphereInput<'_>,
    cylinder: &CylinderInput<'_>,
    sphere_is_a: bool,
    operation: BooleanOp,
) -> Result<BooleanResult, GeometryError> {
    brep.revision = sphere
        .brep
        .revision
        .max(cylinder.brep.revision)
        .checked_add(1)
        .ok_or_else(|| GeometryError::LimitExceeded("boolean revision overflow".into()))?;
    brep.validate()?;
    let inputs = if sphere_is_a {
        [sphere.brep, cylinder.brep]
    } else {
        [cylinder.brep, sphere.brep]
    };
    let face_mappings = analytic_face_mappings(&brep, inputs);
    Ok(BooleanResult {
        brep,
        report: BooleanReport {
            operation,
            quality: GeometryQuality::Analytic,
            contacts: Vec::new(),
            coincident: false,
            face_mappings,
        },
    })
}

fn coaxial_sphere_cylinder_boolean(
    sphere: &SphereInput<'_>,
    cylinder: &CylinderInput<'_>,
    sphere_is_a: bool,
    operation: BooleanOp,
    id: String,
    clearance: f64,
) -> Result<BooleanResult, GeometryError> {
    let local = cylinder.frame.local(sphere.frame.origin);
    let radial = local[0].hypot(local[1]);
    let scale = sphere
        .radius
        .max(cylinder.radius)
        .max(cylinder.height)
        .max(1.0);
    let roundoff = 128.0 * f64::EPSILON * scale;
    if radial > roundoff {
        return Err(if radial <= clearance {
            GeometryError::UnresolvedIntersection(
                "sphere/cylinder axes differ below geometric resolution".into(),
            )
        } else {
            GeometryError::CoverageGap {
                families: ["sphere".into(), "noncoaxial cylinder".into()],
            }
        });
    }
    let radius_gap = sphere.radius - cylinder.radius;
    if radius_gap <= clearance {
        return Err(if radius_gap.abs() <= clearance {
            GeometryError::UnresolvedIntersection(
                "sphere/cylinder tangency is below geometric resolution".into(),
            )
        } else {
            GeometryError::CoverageGap {
                families: ["sphere".into(), "cylinder".into()],
            }
        });
    }
    let sphere_low = local[2] - sphere.radius;
    let sphere_high = local[2] + sphere.radius;
    let lower_clearance = sphere_low;
    let upper_clearance = cylinder.height - sphere_high;
    if lower_clearance <= clearance || upper_clearance <= clearance {
        let near = lower_clearance.abs() <= clearance || upper_clearance.abs() <= clearance;
        return Err(if near {
            GeometryError::UnresolvedIntersection(
                "sphere/cylinder cap event is below geometric resolution".into(),
            )
        } else {
            GeometryError::CoverageGap {
                families: ["sphere".into(), "finite cylinder cap".into()],
            }
        });
    }

    let accuracy = Accuracy {
        geometric: sphere
            .brep
            .accuracy
            .geometric
            .max(cylinder.brep.accuracy.geometric),
        intersection: sphere
            .brep
            .accuracy
            .intersection
            .max(cylinder.brep.accuracy.intersection),
        tessellation: sphere
            .brep
            .accuracy
            .tessellation
            .max(cylinder.brep.accuracy.tessellation),
        exchange: sphere
            .brep
            .accuracy
            .exchange
            .max(cylinder.brep.accuracy.exchange),
    };
    let half = ((sphere.radius - cylinder.radius) * (sphere.radius + cylinder.radius)).sqrt();
    let z0 = local[2] - half;
    let z1 = local[2] + half;
    let latitude0 = (-half / sphere.radius).asin();
    let latitude1 = (half / sphere.radius).asin();
    let sphere_frame = Frame3 {
        origin: sphere.frame.origin,
        x: cylinder.frame.x,
        y: cylinder.frame.y,
        z: cylinder.frame.z,
    };
    let cylinder_lateral = cylinder_source(cylinder, 0);
    let cylinder_caps = [cylinder_source(cylinder, 1), cylinder_source(cylinder, 2)];

    if operation == BooleanOp::Subtraction && !sphere_is_a {
        let mut out = BrepEnvelope::new(id, accuracy)?;
        for north in [false, true] {
            let mut builder = Builder::new(
                format!("{}:{}", out.id, if north { "upper" } else { "lower" }),
                accuracy,
            )?;
            let intersection = circular_boundary(
                &mut builder,
                cylinder.frame,
                cylinder.radius,
                if north { z1 } else { z0 },
            )?;
            let end = circular_boundary(
                &mut builder,
                cylinder.frame,
                cylinder.radius,
                if north { cylinder.height } else { 0.0 },
            )?;
            if north {
                cylinder_band(
                    &mut builder,
                    cylinder.frame,
                    cylinder.radius,
                    z1,
                    cylinder.height,
                    intersection,
                    end,
                    face_provenance(vec![cylinder_lateral.clone()], FaceRole::Split, false),
                    false,
                )?;
                cylinder_cap(
                    &mut builder,
                    cylinder.frame,
                    cylinder.radius,
                    cylinder.height,
                    end,
                    true,
                    face_provenance(vec![cylinder_caps[1].clone()], FaceRole::Preserved, false),
                )?;
                patch(
                    &mut builder,
                    sphere,
                    sphere_frame,
                    latitude1,
                    true,
                    intersection.1,
                    intersection.0,
                    true,
                )?;
            } else {
                cylinder_band(
                    &mut builder,
                    cylinder.frame,
                    cylinder.radius,
                    0.0,
                    z0,
                    end,
                    intersection,
                    face_provenance(vec![cylinder_lateral.clone()], FaceRole::Split, false),
                    false,
                )?;
                cylinder_cap(
                    &mut builder,
                    cylinder.frame,
                    cylinder.radius,
                    0.0,
                    end,
                    false,
                    face_provenance(vec![cylinder_caps[0].clone()], FaceRole::Preserved, false),
                )?;
                patch(
                    &mut builder,
                    sphere,
                    sphere_frame,
                    latitude0,
                    false,
                    intersection.1,
                    intersection.0,
                    true,
                )?;
            }
            let part = builder.finish()?;
            let provenances: Vec<_> = part
                .topology
                .faces
                .iter()
                .map(|face| face.provenance.clone())
                .collect();
            let face_offset = out.topology.faces.len();
            append_analytic_input(&mut out, &part)?;
            for (face, provenance) in out.topology.faces[face_offset..]
                .iter_mut()
                .zip(provenances)
            {
                face.provenance = provenance;
            }
        }
        return finish_sphere_cylinder_result(out, sphere, cylinder, sphere_is_a, operation);
    }

    let mut builder = Builder::new(id, accuracy)?;
    let low = circular_boundary(&mut builder, cylinder.frame, cylinder.radius, z0)?;
    let high = circular_boundary(&mut builder, cylinder.frame, cylinder.radius, z1)?;
    match operation {
        BooleanOp::Intersection => {
            patch(
                &mut builder,
                sphere,
                sphere_frame,
                latitude0,
                false,
                low.1,
                low.0,
                false,
            )?;
            cylinder_band(
                &mut builder,
                cylinder.frame,
                cylinder.radius,
                z0,
                z1,
                low,
                high,
                face_provenance(vec![cylinder_lateral], FaceRole::Split, false),
                false,
            )?;
            patch(
                &mut builder,
                sphere,
                sphere_frame,
                latitude1,
                true,
                high.1,
                high.0,
                false,
            )?;
        }
        BooleanOp::Subtraction => {
            sphere_band(
                &mut builder,
                sphere,
                sphere_frame,
                latitude0,
                latitude1,
                low,
                high,
                false,
            )?;
            cylinder_band(
                &mut builder,
                cylinder.frame,
                cylinder.radius,
                z0,
                z1,
                low,
                high,
                face_provenance(vec![cylinder_lateral], FaceRole::Cut, true),
                true,
            )?;
        }
        BooleanOp::Union => {
            let bottom = circular_boundary(&mut builder, cylinder.frame, cylinder.radius, 0.0)?;
            let top = circular_boundary(
                &mut builder,
                cylinder.frame,
                cylinder.radius,
                cylinder.height,
            )?;
            cylinder_band(
                &mut builder,
                cylinder.frame,
                cylinder.radius,
                0.0,
                z0,
                bottom,
                low,
                face_provenance(vec![cylinder_lateral.clone()], FaceRole::Split, false),
                false,
            )?;
            cylinder_cap(
                &mut builder,
                cylinder.frame,
                cylinder.radius,
                0.0,
                bottom,
                false,
                face_provenance(vec![cylinder_caps[0].clone()], FaceRole::Preserved, false),
            )?;
            sphere_band(
                &mut builder,
                sphere,
                sphere_frame,
                latitude0,
                latitude1,
                low,
                high,
                false,
            )?;
            cylinder_band(
                &mut builder,
                cylinder.frame,
                cylinder.radius,
                z1,
                cylinder.height,
                high,
                top,
                face_provenance(vec![cylinder_lateral], FaceRole::Split, false),
                false,
            )?;
            cylinder_cap(
                &mut builder,
                cylinder.frame,
                cylinder.radius,
                cylinder.height,
                top,
                true,
                face_provenance(vec![cylinder_caps[1].clone()], FaceRole::Preserved, false),
            )?;
        }
    }
    finish_sphere_cylinder_result(builder.finish()?, sphere, cylinder, sphere_is_a, operation)
}

fn append_cylinder_segment(
    out: &mut BrepEnvelope,
    frame: Frame3,
    radius: f64,
    height: f64,
    provenances: [FaceProvenance; 3],
    index: usize,
) -> Result<(), GeometryError> {
    if height <= 4.0 * out.accuracy.geometric {
        return Err(GeometryError::UnresolvedIntersection(
            "cylinder boolean leaves a sub-tolerance axial segment".into(),
        ));
    }
    let part = primitives::cylinder(
        format!("{}:segment-{index}", out.id),
        frame,
        radius,
        height,
        out.accuracy,
    )?;
    let face_offset = out.topology.faces.len();
    append_analytic_input(out, &part)?;
    for (face, provenance) in out.topology.faces[face_offset..]
        .iter_mut()
        .zip(provenances)
    {
        face.provenance = provenance;
    }
    Ok(())
}

fn append_annular_cylinder(
    out: &mut BrepEnvelope,
    frame: Frame3,
    inner_radius: f64,
    outer_radius: f64,
    height: f64,
    provenances: [FaceProvenance; 4],
) -> Result<(), GeometryError> {
    let part = primitives::annular_cylinder(
        format!("{}:annular", out.id),
        frame,
        inner_radius,
        outer_radius,
        height,
        out.accuracy,
    )?;
    let face_offset = out.topology.faces.len();
    append_analytic_input(out, &part)?;
    for (face, provenance) in out.topology.faces[face_offset..]
        .iter_mut()
        .zip(provenances)
    {
        face.provenance = provenance;
    }
    Ok(())
}

struct NumericalCylinderBoundary {
    definition: u32,
    edge: u32,
    vertex: u32,
    cutter_start: [f64; 2],
    cutter_end: [f64; 2],
    cutter_middle: [f64; 2],
}

fn chart_seam_rotation(graph: &super::face_intersection::IntersectionGraph) -> f64 {
    let tau = std::f64::consts::TAU;
    let mut parameters = graph
        .branches
        .iter()
        .filter_map(
            |branch| match graph.geometry.curves[branch.curve as usize] {
                CurveGeometry::Intersection { definition } => Some(definition),
                _ => None,
            },
        )
        .flat_map(|definition| {
            graph.geometry.intersections[definition as usize]
                .anchors
                .iter()
                .map(|anchor| anchor.uv_a[0].rem_euclid(tau))
        })
        .collect::<Vec<_>>();
    parameters.sort_by(f64::total_cmp);
    parameters.dedup_by(|left, right| (*left - *right).abs() <= f64::EPSILON * 64.0);
    if parameters.is_empty() {
        return 0.0;
    }
    let mut largest = (0.0, parameters[0]);
    for index in 0..parameters.len() {
        let start = parameters[index];
        let end = if index + 1 < parameters.len() {
            parameters[index + 1]
        } else {
            parameters[0] + tau
        };
        if end - start > largest.0 {
            largest = (end - start, start);
        }
    }
    (largest.1 + 0.5 * largest.0).rem_euclid(tau)
}

fn rotate_cylinder_chart(frame: Frame3, angle: f64) -> Frame3 {
    let (sine, cosine) = angle.sin_cos();
    Frame3 {
        x: add(scale(frame.x, cosine), scale(frame.y, sine)),
        y: add(scale(frame.y, cosine), scale(frame.x, -sine)),
        ..frame
    }
}

fn rotate_first_intersection_chart(
    definition: &mut super::intersection::IntersectionDefinition,
    angle: f64,
    accuracy: Accuracy,
) -> Result<(), GeometryError> {
    let tau = std::f64::consts::TAU;
    let original_first = definition
        .anchors
        .first()
        .ok_or_else(|| GeometryError::InvalidGeometry("empty intersection branch".into()))?
        .uv_a[0];
    let mut previous: Option<f64> = None;
    for anchor in &mut definition.anchors {
        let mut value = (anchor.uv_a[0] - angle).rem_euclid(tau);
        if let Some(previous) = previous {
            value += ((previous - value) / tau).round() * tau;
        }
        anchor.uv_a[0] = value;
        previous = Some(value);
    }
    let lo = definition
        .anchors
        .iter()
        .map(|anchor| anchor.uv_a[0])
        .fold(f64::INFINITY, f64::min);
    let hi = definition
        .anchors
        .iter()
        .map(|anchor| anchor.uv_a[0])
        .fold(f64::NEG_INFINITY, f64::max);
    if hi - lo >= tau - accuracy.intersection {
        return Err(GeometryError::CoverageGap {
            families: [
                "cylinder through subtraction".into(),
                "intersection loop crossing every chart seam".into(),
            ],
        });
    }
    let shift = -(lo / tau).floor() * tau;
    for anchor in &mut definition.anchors {
        anchor.uv_a[0] += shift;
    }
    let chart_shift = definition.anchors[0].uv_a[0] - original_first;
    for tube in &mut definition.uv_tubes {
        tube[0] = Interval::new(tube[0].lo + chart_shift, tube[0].hi + chart_shift)?;
    }
    Ok(())
}

fn transverse_cylinder_through_subtraction(
    host: &CylinderInput<'_>,
    cutter: &CylinderInput<'_>,
    id: String,
    accuracy: Accuracy,
) -> Result<BooleanResult, GeometryError> {
    let mut body_graph = intersect_breps(host.brep, cutter.brep)?;
    if body_graph.pairs.len() != 1 {
        return Err(GeometryError::CoverageGap {
            families: [
                "cylinder through subtraction".into(),
                "cap-intersecting cylinder".into(),
            ],
        });
    }
    let pair = body_graph
        .pairs
        .pop()
        .ok_or_else(|| GeometryError::CoverageGap {
            families: [
                "cylinder through subtraction".into(),
                "missing lateral intersection graph".into(),
            ],
        })?;
    if pair.faces != [0, 0]
        || pair.graph.coincident
        || !pair.graph.contacts.is_empty()
        || pair.graph.branches.len() != 2
    {
        return Err(GeometryError::CoverageGap {
            families: [
                "cylinder through subtraction".into(),
                "non-transverse cylinder graph".into(),
            ],
        });
    }

    let seam_rotation = chart_seam_rotation(&pair.graph);
    let host_frame = rotate_cylinder_chart(host.frame, seam_rotation);
    let mut builder = Builder::new(id, accuracy)?;
    let low = builder.vertex(host_frame.point([host.radius, 0.0, 0.0]));
    let high = builder.vertex(host_frame.point([host.radius, 0.0, host.height]));
    let tau = std::f64::consts::TAU;
    let bottom = builder.edge(
        CurveGeometry::Circle {
            frame: host_frame,
            radius: host.radius,
        },
        Interval::new(0.0, tau)?,
        false,
    );
    let mut top_frame = host_frame;
    top_frame.origin = host_frame.point([0.0, 0.0, host.height]);
    let top = builder.edge(
        CurveGeometry::Circle {
            frame: top_frame,
            radius: host.radius,
        },
        Interval::new(0.0, tau)?,
        false,
    );
    let host_seam = builder.edge(
        CurveGeometry::Line {
            origin: host_frame.point([host.radius, 0.0, 0.0]),
            direction: host_frame.z,
        },
        Interval::new(0.0, host.height)?,
        true,
    );

    let mut numerical = Vec::with_capacity(2);
    for branch in &pair.graph.branches {
        let CurveGeometry::Intersection { definition } =
            pair.graph.geometry.curves[branch.curve as usize]
        else {
            return Err(GeometryError::InvalidGeometry(
                "universal face graph did not retain its intersection definition".into(),
            ));
        };
        let mut definition = pair.graph.geometry.intersections[definition as usize].clone();
        definition.surfaces = [0, 3];
        rotate_first_intersection_chart(&mut definition, seam_rotation, accuracy)?;
        let definition_id = builder.brep.geometry.intersections.len() as u32;
        builder.brep.geometry.intersections.push(definition);
        let curve = CurveGeometry::Intersection {
            definition: definition_id,
        };
        let start = pair
            .graph
            .geometry
            .curve(branch.curve)?
            .point_at(branch.range.lo)?;
        let end = pair
            .graph
            .geometry
            .curve(branch.curve)?
            .point_at(branch.range.hi)?;
        if norm(sub(start, end)) > accuracy.intersection {
            return Err(GeometryError::CoverageGap {
                families: [
                    "cylinder through subtraction".into(),
                    "open lateral intersection branch".into(),
                ],
            });
        }
        let vertex = builder.vertex(start);
        let edge = builder.edge(curve, branch.range, false);
        numerical.push(NumericalCylinderBoundary {
            definition: definition_id,
            edge,
            vertex,
            cutter_start: pair
                .graph
                .geometry
                .pcurve_at(branch.pcurves[1], branch.range.lo)?,
            cutter_end: pair
                .graph
                .geometry
                .pcurve_at(branch.pcurves[1], branch.range.hi)?,
            cutter_middle: pair
                .graph
                .geometry
                .pcurve_at(branch.pcurves[1], branch.range.midpoint())?,
        });
    }
    numerical.sort_by(|left, right| left.cutter_middle[1].total_cmp(&right.cutter_middle[1]));
    let upper_shift =
        ((numerical[0].cutter_end[0] - numerical[1].cutter_start[0]) / tau).round() * tau;
    {
        let upper = &mut numerical[1];
        upper.cutter_start[0] += upper_shift;
        upper.cutter_end[0] += upper_shift;
        upper.cutter_middle[0] += upper_shift;
        let definition = &mut builder.brep.geometry.intersections[upper.definition as usize];
        for anchor in &mut definition.anchors {
            anchor.uv_b[0] += upper_shift;
        }
        for tube in &mut definition.uv_tubes {
            tube[2] = Interval::new(tube[2].lo + upper_shift, tube[2].hi + upper_shift)?;
        }
    }
    let lower = &numerical[0];
    let upper = &numerical[1];
    if upper.cutter_middle[1] - lower.cutter_middle[1] <= 4.0 * accuracy.geometric {
        return Err(GeometryError::UnresolvedIntersection(
            "cylinder through-cut branches are below geometric resolution".into(),
        ));
    }

    let host_holes = numerical
        .iter()
        .map(|numeric_boundary| {
            vec![boundary(
                numeric_boundary.edge,
                numeric_boundary.vertex,
                numeric_boundary.vertex,
                Orientation::Forward,
                PcurveGeometry::IntersectionSide {
                    definition: numeric_boundary.definition,
                    side: IntersectionSide::A,
                },
            )]
        })
        .collect();
    builder.face_with_holes(
        "host:lateral",
        SurfaceGeometry::Cylinder {
            frame: host_frame,
            radius: host.radius,
        },
        [[0.0, tau], [0.0, host.height]],
        vec![
            boundary(
                bottom,
                low,
                low,
                Orientation::Forward,
                uv_line([0.0, 0.0], [1.0, 0.0]),
            ),
            boundary(
                host_seam,
                low,
                high,
                Orientation::Forward,
                uv_line([tau, 0.0], [0.0, 1.0]),
            ),
            boundary(
                top,
                high,
                high,
                Orientation::Reverse,
                uv_line([0.0, host.height], [1.0, 0.0]),
            ),
            boundary(
                host_seam,
                high,
                low,
                Orientation::Reverse,
                uv_line([0.0, 0.0], [0.0, 1.0]),
            ),
        ],
        host_holes,
    )?;

    let mut lower_cap_frame = host_frame;
    lower_cap_frame.y = scale(lower_cap_frame.y, -1.0);
    lower_cap_frame.z = scale(lower_cap_frame.z, -1.0);
    builder.face(
        "host:lower_cap",
        SurfaceGeometry::Plane {
            frame: lower_cap_frame,
        },
        [[-host.radius, host.radius]; 2],
        vec![boundary(
            bottom,
            low,
            low,
            Orientation::Reverse,
            PcurveGeometry::Conic2 {
                origin: [0.0; 2],
                axis_a: [host.radius, 0.0],
                axis_b: [0.0, -host.radius],
            },
        )],
    )?;
    builder.face(
        "host:upper_cap",
        SurfaceGeometry::Plane { frame: top_frame },
        [[-host.radius, host.radius]; 2],
        vec![boundary(
            top,
            high,
            high,
            Orientation::Forward,
            PcurveGeometry::Conic2 {
                origin: [0.0; 2],
                axis_a: [host.radius, 0.0],
                axis_b: [0.0, host.radius],
            },
        )],
    )?;

    let lower_end = lower.cutter_end;
    let upper_start = upper.cutter_start;
    let upper_right_u = upper_start[0];
    let lower_v = lower_end[1];
    let upper_v = upper_start[1];
    let arc_range = Interval::new(
        lower_end[0].min(upper_right_u),
        lower_end[0].max(upper_right_u),
    )?;
    if arc_range.width() * cutter.radius <= 4.0 * accuracy.geometric {
        return Err(GeometryError::UnresolvedIntersection(
            "cylinder cut chart seam arc is below geometric resolution".into(),
        ));
    }
    let mut seam_circle_frame = cutter.frame;
    seam_circle_frame.origin = cutter.frame.point([0.0, 0.0, lower_v]);
    let seam_arc = builder.edge(
        CurveGeometry::Circle {
            frame: seam_circle_frame,
            radius: cutter.radius,
        },
        arc_range,
        true,
    );
    let seam_middle_point = cutter
        .brep
        .geometry
        .surface(0)?
        .point_at([upper_right_u, lower_v])?;
    let seam_middle = builder.vertex(seam_middle_point);
    let seam_vector = sub(
        builder.brep.topology.vertices[upper.vertex as usize].position,
        seam_middle_point,
    );
    let seam_length = norm(seam_vector);
    if seam_length <= 4.0 * accuracy.geometric {
        return Err(GeometryError::UnresolvedIntersection(
            "cylinder cut chart seam generator is below geometric resolution".into(),
        ));
    }
    let seam_generator = builder.edge(
        CurveGeometry::Line {
            origin: seam_middle_point,
            direction: unit(seam_vector)?,
        },
        Interval::new(0.0, seam_length)?,
        true,
    );
    let arc_forward = lower_end[0] < upper_right_u;
    let first_arc_sense = if arc_forward {
        Orientation::Forward
    } else {
        Orientation::Reverse
    };
    let second_arc_sense = reverse(first_arc_sense);
    let second_arc_start = if second_arc_sense == Orientation::Forward {
        arc_range.lo
    } else {
        arc_range.hi
    };
    let second_arc_end = if second_arc_sense == Orientation::Forward {
        arc_range.hi
    } else {
        arc_range.lo
    };
    let arc_shift = lower.cutter_start[0] - second_arc_end;
    let winding_error = upper.cutter_end[0] - (second_arc_start + arc_shift);
    let winding_residual = winding_error - (winding_error / tau).round() * tau;
    if winding_residual.abs() > accuracy.intersection / cutter.radius {
        return Err(GeometryError::UnresolvedIntersection(
            "cylinder cut chart seam winding is inconsistent".into(),
        ));
    }
    let cutter_face = builder.brep.topology.faces.len() as u32;
    let mut cutter_u_bounds = [f64::INFINITY, f64::NEG_INFINITY];
    let mut cutter_v_bounds = [f64::INFINITY, f64::NEG_INFINITY];
    for boundary in &numerical {
        let definition = &builder.brep.geometry.intersections[boundary.definition as usize];
        for anchor in &definition.anchors {
            cutter_u_bounds[0] = cutter_u_bounds[0].min(anchor.uv_b[0]);
            cutter_u_bounds[1] = cutter_u_bounds[1].max(anchor.uv_b[0]);
            cutter_v_bounds[0] = cutter_v_bounds[0].min(anchor.uv_b[1]);
            cutter_v_bounds[1] = cutter_v_bounds[1].max(anchor.uv_b[1]);
        }
    }
    for value in [upper_right_u, upper.cutter_end[0], lower.cutter_start[0]] {
        cutter_u_bounds[0] = cutter_u_bounds[0].min(value);
        cutter_u_bounds[1] = cutter_u_bounds[1].max(value);
    }
    for value in [lower_v, upper_v] {
        cutter_v_bounds[0] = cutter_v_bounds[0].min(value);
        cutter_v_bounds[1] = cutter_v_bounds[1].max(value);
    }
    builder.face(
        "cutter:tunnel",
        SurfaceGeometry::Cylinder {
            frame: cutter.frame,
            radius: cutter.radius,
        },
        [cutter_u_bounds, cutter_v_bounds],
        vec![
            boundary(
                lower.edge,
                lower.vertex,
                lower.vertex,
                Orientation::Forward,
                PcurveGeometry::IntersectionSide {
                    definition: lower.definition,
                    side: IntersectionSide::B,
                },
            ),
            boundary(
                seam_arc,
                lower.vertex,
                seam_middle,
                first_arc_sense,
                uv_line([0.0, lower_v], [1.0, 0.0]),
            ),
            boundary(
                seam_generator,
                seam_middle,
                upper.vertex,
                Orientation::Forward,
                uv_line(
                    [upper_right_u, lower_v],
                    [0.0, (upper_v - lower_v) / seam_length],
                ),
            ),
            boundary(
                upper.edge,
                upper.vertex,
                upper.vertex,
                Orientation::Forward,
                PcurveGeometry::IntersectionSide {
                    definition: upper.definition,
                    side: IntersectionSide::B,
                },
            ),
            boundary(
                seam_generator,
                upper.vertex,
                seam_middle,
                Orientation::Reverse,
                uv_line(
                    [upper.cutter_end[0], lower_v],
                    [0.0, (upper_v - lower_v) / seam_length],
                ),
            ),
            boundary(
                seam_arc,
                seam_middle,
                lower.vertex,
                second_arc_sense,
                uv_line([arc_shift, lower_v], [1.0, 0.0]),
            ),
        ],
    )?;
    builder.brep.topology.faces[0].provenance =
        face_provenance(vec![cylinder_source(host, 0)], FaceRole::Split, false);
    builder.brep.topology.faces[1].provenance =
        face_provenance(vec![cylinder_source(host, 1)], FaceRole::Preserved, false);
    builder.brep.topology.faces[2].provenance =
        face_provenance(vec![cylinder_source(host, 2)], FaceRole::Preserved, false);
    builder.brep.topology.faces[cutter_face as usize].provenance =
        face_provenance(vec![cylinder_source(cutter, 0)], FaceRole::Cut, true);
    reverse_face(&mut builder.brep, cutter_face);
    let mut out = builder.finish()?;
    out.revision = host
        .brep
        .revision
        .max(cutter.brep.revision)
        .checked_add(1)
        .ok_or_else(|| GeometryError::LimitExceeded("boolean revision overflow".into()))?;
    out.validate()?;
    let face_mappings = cylinder_face_mappings(&out, [host, cutter]);
    Ok(BooleanResult {
        brep: out,
        report: BooleanReport {
            operation: BooleanOp::Subtraction,
            quality: GeometryQuality::Analytic,
            contacts: Vec::new(),
            coincident: false,
            face_mappings,
        },
    })
}

fn cylinder_face_mappings(out: &BrepEnvelope, inputs: [&CylinderInput<'_>; 2]) -> Vec<FaceMapping> {
    let mut mappings = Vec::with_capacity(6);
    for input in inputs {
        for face in 0..3 {
            let source = cylinder_source(input, face);
            let result_faces = out
                .topology
                .faces
                .iter()
                .filter(|result| {
                    result.provenance.sources.iter().any(|candidate| {
                        candidate.entity == source.entity
                            && candidate.body == source.body
                            && candidate.key == source.key
                            && candidate.face == source.face
                    })
                })
                .map(|result| result.id)
                .collect();
            mappings.push(FaceMapping {
                source,
                result_faces,
            });
        }
    }
    mappings
}

#[derive(Clone, Copy)]
struct CylinderArc {
    frame: Frame3,
    radius: f64,
    range: Interval,
    start: usize,
    end: usize,
    reversed: bool,
}

fn circle_parameter(frame: Frame3, point: Point3) -> f64 {
    let local = frame.local(point);
    local[1].atan2(local[0]).rem_euclid(std::f64::consts::TAU)
}

fn selected_cylinder_arc(
    frame: Frame3,
    radius: f64,
    points: [Point3; 2],
    other_center: Point3,
    other_radius: f64,
    keep_inside: bool,
    reversed: bool,
) -> Result<CylinderArc, GeometryError> {
    let angles = points.map(|point| circle_parameter(frame, point));
    let tau = std::f64::consts::TAU;
    let forward_end = if angles[1] > angles[0] {
        angles[1]
    } else {
        angles[1] + tau
    };
    let candidates = [
        (angles[0], forward_end, 0, 1),
        (angles[1], angles[0] + tau, 1, 0),
    ];
    for (lo, hi, start, end) in candidates {
        let midpoint = frame.point([
            radius * ((lo + hi) * 0.5).cos(),
            radius * ((lo + hi) * 0.5).sin(),
            0.0,
        ]);
        let inside = norm(sub(midpoint, other_center)) < other_radius;
        if inside == keep_inside {
            return Ok(CylinderArc {
                frame,
                radius,
                range: Interval::new(lo, hi)?,
                start,
                end,
                reversed,
            });
        }
    }
    Err(GeometryError::UnresolvedIntersection(
        "could not isolate the selected cylinder boundary arc".into(),
    ))
}

fn arc_angle_at(arc: CylinderArc, point: usize) -> Result<f64, GeometryError> {
    if point == arc.start {
        Ok(arc.range.lo)
    } else if point == arc.end {
        Ok(arc.range.hi)
    } else {
        Err(GeometryError::InvalidTopology(
            "cylinder arc endpoint is not part of the intersection".into(),
        ))
    }
}

fn add_cylinder_arc_face(
    builder: &mut Builder,
    key: &str,
    arc: CylinderArc,
    bottom_edge: u32,
    top_edge: u32,
    vertical_edges: [u32; 2],
    vertices: [[u32; 2]; 2],
    height: f64,
) -> Result<(), GeometryError> {
    use Orientation::{Forward as F, Reverse as R};
    let bottom = vertices[0];
    let top = vertices[1];
    let start = arc.start;
    let end = arc.end;
    let vertical = |point: usize| -> Result<primitives::Use, GeometryError> {
        Ok(boundary(
            vertical_edges[point],
            bottom[point],
            top[point],
            F,
            uv_line([arc_angle_at(arc, point)?, 0.0], [0.0, 1.0]),
        ))
    };
    let vertical_reverse = |point: usize| -> Result<primitives::Use, GeometryError> {
        Ok(boundary(
            vertical_edges[point],
            top[point],
            bottom[point],
            R,
            uv_line([arc_angle_at(arc, point)?, 0.0], [0.0, 1.0]),
        ))
    };
    let uses = if arc.reversed {
        vec![
            boundary(
                bottom_edge,
                bottom[end],
                bottom[start],
                R,
                uv_line([0.0; 2], [1.0, 0.0]),
            ),
            vertical(start)?,
            boundary(
                top_edge,
                top[start],
                top[end],
                F,
                uv_line([0.0, height], [1.0, 0.0]),
            ),
            vertical_reverse(end)?,
        ]
    } else {
        vec![
            boundary(
                bottom_edge,
                bottom[start],
                bottom[end],
                F,
                uv_line([0.0; 2], [1.0, 0.0]),
            ),
            vertical(end)?,
            boundary(
                top_edge,
                top[end],
                top[start],
                R,
                uv_line([0.0, height], [1.0, 0.0]),
            ),
            vertical_reverse(start)?,
        ]
    };
    let face = builder.brep.topology.faces.len();
    builder.face(
        key,
        SurfaceGeometry::Cylinder {
            frame: arc.frame,
            radius: arc.radius,
        },
        [[arc.range.lo, arc.range.hi], [0.0, height]],
        uses,
    )?;
    if arc.reversed {
        builder.brep.topology.faces[face].sense = R;
    }
    Ok(())
}

fn shifted_cylinder_arc(mut arc: CylinderArc, axis: Point3, offset: f64) -> CylinderArc {
    arc.frame.origin = std::array::from_fn(|i| arc.frame.origin[i] + axis[i] * offset);
    arc
}

fn arc_endpoints(arc: CylinderArc, sense: Orientation) -> (usize, usize) {
    if sense == Orientation::Forward {
        (arc.start, arc.end)
    } else {
        (arc.end, arc.start)
    }
}

fn two_arc_plane_uses(
    builder: &Builder,
    frame: Frame3,
    first: (CylinderArc, u32),
    second: (CylinderArc, u32),
    vertices: [u32; 2],
) -> Result<Vec<primitives::Use>, GeometryError> {
    for first_sense in [Orientation::Forward, Orientation::Reverse] {
        let (first_from, first_to) = arc_endpoints(first.0, first_sense);
        let second_sense = if second.0.start == first_to && second.0.end == first_from {
            Orientation::Forward
        } else if second.0.end == first_to && second.0.start == first_from {
            Orientation::Reverse
        } else {
            continue;
        };
        let mut samples = Vec::with_capacity(34);
        for (arc, sense) in [(first.0, first_sense), (second.0, second_sense)] {
            for sample in 0..=16 {
                if !samples.is_empty() && sample == 0 {
                    continue;
                }
                let fraction = sample as f64 / 16.0;
                let parameter = if sense == Orientation::Forward {
                    arc.range.lo + arc.range.width() * fraction
                } else {
                    arc.range.hi - arc.range.width() * fraction
                };
                samples.push(frame.local(arc.frame.point([
                    arc.radius * parameter.cos(),
                    arc.radius * parameter.sin(),
                    0.0,
                ])));
            }
        }
        let signed_area = samples
            .iter()
            .zip(samples.iter().cycle().skip(1))
            .take(samples.len())
            .map(|(a, b)| a[0] * b[1] - b[0] * a[1])
            .sum::<f64>()
            * 0.5;
        if signed_area > 0.0 {
            return Ok(vec![
                plane_boundary(
                    builder,
                    frame,
                    first.1,
                    vertices[first_from],
                    vertices[first_to],
                    first_sense,
                )?,
                plane_boundary(
                    builder,
                    frame,
                    second.1,
                    vertices[arc_endpoints(second.0, second_sense).0],
                    vertices[arc_endpoints(second.0, second_sense).1],
                    second_sense,
                )?,
            ]);
        }
    }
    Err(GeometryError::InvalidTopology(
        "two-arc planar boundary does not form a positive loop".into(),
    ))
}

fn two_arc_plane_bounds(frame: Frame3, arcs: [CylinderArc; 2], tolerance: f64) -> [[f64; 2]; 2] {
    let mut bounds = [[f64::INFINITY, f64::NEG_INFINITY]; 2];
    for arc in arcs {
        for point in [
            arc.frame.origin,
            std::array::from_fn(|i| arc.frame.origin[i] + arc.frame.x[i] * arc.radius),
            std::array::from_fn(|i| arc.frame.origin[i] - arc.frame.x[i] * arc.radius),
            std::array::from_fn(|i| arc.frame.origin[i] + arc.frame.y[i] * arc.radius),
            std::array::from_fn(|i| arc.frame.origin[i] - arc.frame.y[i] * arc.radius),
        ] {
            let uv = frame.local(point);
            for axis in 0..2 {
                bounds[axis][0] = bounds[axis][0].min(uv[axis]);
                bounds[axis][1] = bounds[axis][1].max(uv[axis]);
            }
        }
    }
    for bound in &mut bounds {
        bound[0] -= tolerance;
        bound[1] += tolerance;
    }
    bounds
}

fn parallel_cylinder_boolean(
    a: &CylinderInput<'_>,
    b: &CylinderInput<'_>,
    alignment: f64,
    operation: BooleanOp,
    id: String,
    accuracy: Accuracy,
) -> Result<BooleanResult, GeometryError> {
    let mut b_frame = a.frame;
    b_frame.origin = if alignment > 0.0 {
        b.frame.origin
    } else {
        b.frame.point([0.0, 0.0, b.height])
    };
    let delta = sub(b_frame.origin, a.frame.origin);
    let axial = dot(delta, a.frame.z);
    let radial = sub(delta, scale(a.frame.z, axial));
    let distance = norm(radial);
    let sum = a.radius + b.radius;
    let difference = (a.radius - b.radius).abs();
    let clearance = 4.0 * accuracy.geometric;
    let relation_roundoff = 128.0
        * f64::EPSILON
        * distance.max(sum).max(a.height).max(
            a.frame
                .origin
                .into_iter()
                .chain(b_frame.origin)
                .fold(1.0_f64, |largest, value| largest.max(value.abs())),
        );
    if (distance - sum).abs() <= clearance {
        if (distance - sum).abs() <= relation_roundoff {
            let toward_b = unit(radial)?;
            let bottom: Point3 =
                std::array::from_fn(|axis| a.frame.origin[axis] + toward_b[axis] * a.radius);
            let top: Point3 = std::array::from_fn(|axis| bottom[axis] + a.frame.z[axis] * a.height);
            return separate_boolean(a.brep, b.brep, operation, id, vec![bottom, top]);
        }
        return Err(GeometryError::UnresolvedIntersection(
            "parallel cylinders are nearly externally tangent".into(),
        ));
    }
    if (distance - difference).abs() <= clearance {
        return Err(GeometryError::UnresolvedIntersection(
            "parallel cylinders are internally tangent or nearly tangent".into(),
        ));
    }
    if distance <= difference {
        let b_inside_a = b.radius < a.radius;
        if operation != BooleanOp::Subtraction || !b_inside_a {
            return contained_cylinders(a, b, b_inside_a, operation, id, accuracy);
        }
        let mut out = primitives::cylinder_with_circular_hole(
            id, a.frame, b_frame, b.radius, a.radius, a.height, accuracy,
        )?;
        out.topology.faces[0].provenance =
            face_provenance(vec![cylinder_source(a, 0)], FaceRole::Preserved, false);
        out.topology.faces[1].provenance =
            face_provenance(vec![cylinder_source(b, 0)], FaceRole::Cut, true);
        for (face, source) in [(2, cylinder_source(a, 1)), (3, cylinder_source(a, 2))] {
            out.topology.faces[face].provenance =
                face_provenance(vec![source], FaceRole::Split, false);
        }
        out.revision = a
            .brep
            .revision
            .max(b.brep.revision)
            .checked_add(1)
            .ok_or_else(|| GeometryError::LimitExceeded("boolean revision overflow".into()))?;
        out.validate()?;
        let face_mappings = cylinder_face_mappings(&out, [a, b]);
        return Ok(BooleanResult {
            brep: out,
            report: BooleanReport {
                operation,
                quality: GeometryQuality::Analytic,
                contacts: Vec::new(),
                coincident: true,
                face_mappings,
            },
        });
    }
    if distance >= sum {
        return separate_boolean(a.brep, b.brep, operation, id, Vec::new());
    }

    let toward_b = unit(radial)?;
    let lateral = unit(cross(a.frame.z, toward_b))?;
    let along =
        (distance * distance + a.radius * a.radius - b.radius * b.radius) / (2.0 * distance);
    let half_squared = (a.radius - along) * (a.radius + along);
    if half_squared <= clearance * clearance {
        return Err(GeometryError::UnresolvedIntersection(
            "parallel cylinder intersection chord is below geometric resolution".into(),
        ));
    }
    let half = half_squared.sqrt();
    let chord_center: Point3 =
        std::array::from_fn(|axis| a.frame.origin[axis] + toward_b[axis] * along);
    let points = [
        std::array::from_fn(|axis| chord_center[axis] + lateral[axis] * half),
        std::array::from_fn(|axis| chord_center[axis] - lateral[axis] * half),
    ];
    let (a_inside, b_inside, reverse_b) = match operation {
        BooleanOp::Union => (false, false, false),
        BooleanOp::Intersection => (true, true, false),
        BooleanOp::Subtraction => (false, true, true),
    };
    let a_arc = selected_cylinder_arc(
        a.frame,
        a.radius,
        points,
        b_frame.origin,
        b.radius,
        a_inside,
        false,
    )?;
    let b_arc = selected_cylinder_arc(
        b_frame,
        b.radius,
        points,
        a.frame.origin,
        a.radius,
        b_inside,
        reverse_b,
    )?;

    let mut builder = Builder::new(id, accuracy)?;
    let bottom_vertices = [builder.vertex(points[0]), builder.vertex(points[1])];
    let top_points =
        points.map(|point| std::array::from_fn(|axis| point[axis] + a.frame.z[axis] * a.height));
    let top_vertices = [builder.vertex(top_points[0]), builder.vertex(top_points[1])];
    let vertices = [bottom_vertices, top_vertices];
    let vertical_range = Interval::new(0.0, a.height)?;
    let vertical_edges = [0, 1].map(|point| {
        builder.edge(
            CurveGeometry::Line {
                origin: points[point],
                direction: a.frame.z,
            },
            vertical_range,
            false,
        )
    });
    let add_arc_edges =
        |builder: &mut Builder, arc: CylinderArc| -> Result<[u32; 2], GeometryError> {
            let mut top_frame = arc.frame;
            top_frame.origin =
                std::array::from_fn(|axis| arc.frame.origin[axis] + a.frame.z[axis] * a.height);
            Ok([
                builder.edge(
                    CurveGeometry::Circle {
                        frame: arc.frame,
                        radius: arc.radius,
                    },
                    arc.range,
                    false,
                ),
                builder.edge(
                    CurveGeometry::Circle {
                        frame: top_frame,
                        radius: arc.radius,
                    },
                    arc.range,
                    false,
                ),
            ])
        };
    let a_edges = add_arc_edges(&mut builder, a_arc)?;
    let b_edges = add_arc_edges(&mut builder, b_arc)?;
    add_cylinder_arc_face(
        &mut builder,
        "a:lateral",
        a_arc,
        a_edges[0],
        a_edges[1],
        vertical_edges,
        vertices,
        a.height,
    )?;
    add_cylinder_arc_face(
        &mut builder,
        "b:lateral",
        b_arc,
        b_edges[0],
        b_edges[1],
        vertical_edges,
        vertices,
        a.height,
    )?;

    let mut lower_frame = a.frame;
    lower_frame.y = scale(lower_frame.y, -1.0);
    lower_frame.z = scale(lower_frame.z, -1.0);
    let mut upper_frame = a.frame;
    upper_frame.origin = a.frame.point([0.0, 0.0, a.height]);
    let mut planar_bounds = [[f64::INFINITY, f64::NEG_INFINITY]; 2];
    for point in [
        a.frame.origin,
        b_frame.origin,
        points[0],
        points[1],
        std::array::from_fn(|axis| a.frame.origin[axis] + a.frame.x[axis] * a.radius),
        std::array::from_fn(|axis| a.frame.origin[axis] - a.frame.x[axis] * a.radius),
        std::array::from_fn(|axis| a.frame.origin[axis] + a.frame.y[axis] * a.radius),
        std::array::from_fn(|axis| a.frame.origin[axis] - a.frame.y[axis] * a.radius),
        std::array::from_fn(|axis| b_frame.origin[axis] + a.frame.x[axis] * b.radius),
        std::array::from_fn(|axis| b_frame.origin[axis] - a.frame.x[axis] * b.radius),
        std::array::from_fn(|axis| b_frame.origin[axis] + a.frame.y[axis] * b.radius),
        std::array::from_fn(|axis| b_frame.origin[axis] - a.frame.y[axis] * b.radius),
    ] {
        let uv = upper_frame.local(point);
        for axis in 0..2 {
            planar_bounds[axis][0] = planar_bounds[axis][0].min(uv[axis]);
            planar_bounds[axis][1] = planar_bounds[axis][1].max(uv[axis]);
        }
    }
    for bound in &mut planar_bounds {
        bound[0] -= accuracy.geometric;
        bound[1] += accuracy.geometric;
    }
    let cap_uses = |builder: &Builder,
                    frame: Frame3,
                    top: bool|
     -> Result<Vec<primitives::Use>, GeometryError> {
        let vertex_set = if top { top_vertices } else { bottom_vertices };
        let edge_index = usize::from(top);
        let mut uses = Vec::with_capacity(2);
        for (arc, edges) in [(a_arc, a_edges), (b_arc, b_edges)] {
            let sense = match (top, arc.reversed) {
                (false, false) => Orientation::Reverse,
                (false, true) => Orientation::Forward,
                (true, false) => Orientation::Forward,
                (true, true) => Orientation::Reverse,
            };
            let (from, to) = if sense == Orientation::Forward {
                (vertex_set[arc.start], vertex_set[arc.end])
            } else {
                (vertex_set[arc.end], vertex_set[arc.start])
            };
            uses.push(plane_boundary(
                builder,
                frame,
                edges[edge_index],
                from,
                to,
                sense,
            )?);
        }
        Ok(uses)
    };
    let lower_uses = cap_uses(&builder, lower_frame, false)?;
    builder.face(
        "lower_cap",
        SurfaceGeometry::Plane { frame: lower_frame },
        planar_bounds,
        lower_uses,
    )?;
    let upper_uses = cap_uses(&builder, upper_frame, true)?;
    builder.face(
        "upper_cap",
        SurfaceGeometry::Plane { frame: upper_frame },
        planar_bounds,
        upper_uses,
    )?;
    let mut out = builder.finish()?;
    out.topology.faces[0].provenance =
        face_provenance(vec![cylinder_source(a, 0)], FaceRole::Split, false);
    out.topology.faces[1].provenance = face_provenance(
        vec![cylinder_source(b, 0)],
        if reverse_b {
            FaceRole::Cut
        } else {
            FaceRole::Split
        },
        reverse_b,
    );
    let a_caps = [cylinder_source(a, 1), cylinder_source(a, 2)];
    let b_caps = if alignment > 0.0 {
        [cylinder_source(b, 1), cylinder_source(b, 2)]
    } else {
        [cylinder_source(b, 2), cylinder_source(b, 1)]
    };
    for (face, cap) in [(2, 0), (3, 1)] {
        let sources = if operation == BooleanOp::Subtraction {
            vec![a_caps[cap].clone()]
        } else {
            vec![a_caps[cap].clone(), b_caps[cap].clone()]
        };
        out.topology.faces[face].provenance = face_provenance(sources, FaceRole::Split, false);
    }
    out.revision = a
        .brep
        .revision
        .max(b.brep.revision)
        .checked_add(1)
        .ok_or_else(|| GeometryError::LimitExceeded("boolean revision overflow".into()))?;
    out.validate()?;
    let face_mappings = cylinder_face_mappings(&out, [a, b]);
    Ok(BooleanResult {
        brep: out,
        report: BooleanReport {
            operation,
            quality: GeometryQuality::Analytic,
            contacts: Vec::new(),
            coincident: true,
            face_mappings,
        },
    })
}

fn coextensive_cylinder_radii(
    a: &CylinderInput<'_>,
    b: &CylinderInput<'_>,
    alignment: f64,
    operation: BooleanOp,
    id: String,
    accuracy: Accuracy,
) -> Result<BooleanResult, GeometryError> {
    let a_lateral = cylinder_source(a, 0);
    let b_lateral = cylinder_source(b, 0);
    let a_caps = [cylinder_source(a, 1), cylinder_source(a, 2)];
    let b_caps = if alignment > 0.0 {
        [cylinder_source(b, 1), cylinder_source(b, 2)]
    } else {
        [cylinder_source(b, 2), cylinder_source(b, 1)]
    };
    let a_is_larger = a.radius > b.radius;
    let mut out = BrepEnvelope::new(id, accuracy)?;
    match operation {
        BooleanOp::Union | BooleanOp::Intersection => {
            let choose_a = if operation == BooleanOp::Union {
                a_is_larger
            } else {
                !a_is_larger
            };
            let (radius, lateral) = if choose_a {
                (a.radius, a_lateral.clone())
            } else {
                (b.radius, b_lateral.clone())
            };
            let part = primitives::cylinder(
                format!("{}:radial", out.id),
                a.frame,
                radius,
                a.height,
                accuracy,
            )?;
            append_analytic_input(&mut out, &part)?;
            out.topology.faces[0].provenance =
                face_provenance(vec![lateral], FaceRole::Preserved, false);
            for (face, sources) in [
                (1, vec![a_caps[0].clone(), b_caps[0].clone()]),
                (2, vec![a_caps[1].clone(), b_caps[1].clone()]),
            ] {
                out.topology.faces[face].provenance =
                    face_provenance(sources, FaceRole::Split, false);
            }
        }
        BooleanOp::Subtraction if a_is_larger => {
            append_annular_cylinder(
                &mut out,
                a.frame,
                b.radius,
                a.radius,
                a.height,
                [
                    face_provenance(vec![a_lateral], FaceRole::Preserved, false),
                    face_provenance(vec![b_lateral], FaceRole::Cut, true),
                    face_provenance(vec![a_caps[0].clone()], FaceRole::Split, false),
                    face_provenance(vec![a_caps[1].clone()], FaceRole::Split, false),
                ],
            )?;
        }
        BooleanOp::Subtraction => {}
    }
    out.revision = a
        .brep
        .revision
        .max(b.brep.revision)
        .checked_add(1)
        .ok_or_else(|| GeometryError::LimitExceeded("boolean revision overflow".into()))?;
    out.validate()?;
    let face_mappings = cylinder_face_mappings(&out, [a, b]);
    Ok(BooleanResult {
        brep: out,
        report: BooleanReport {
            operation,
            quality: GeometryQuality::Analytic,
            contacts: Vec::new(),
            coincident: true,
            face_mappings,
        },
    })
}

fn contained_cylinders(
    a: &CylinderInput<'_>,
    b: &CylinderInput<'_>,
    b_inside_a: bool,
    operation: BooleanOp,
    id: String,
    accuracy: Accuracy,
) -> Result<BooleanResult, GeometryError> {
    let mut out = BrepEnvelope::new(id, accuracy)?;
    let (contained, enclosing) = if b_inside_a { (b, a) } else { (a, b) };
    match operation {
        BooleanOp::Union => {
            append_analytic_input(&mut out, enclosing.brep)?;
        }
        BooleanOp::Intersection => {
            append_analytic_input(&mut out, contained.brep)?;
        }
        BooleanOp::Subtraction if b_inside_a => {
            append_analytic_input(&mut out, a.brep)?;
            let face_offset = out.topology.faces.len() as u32;
            append_analytic_input(&mut out, b.brep)?;
            let cavity = out
                .solids
                .pop()
                .ok_or_else(|| GeometryError::InvalidTopology("missing cutter solid".into()))?
                .outer_shell;
            out.solids[0].cavity_shells.push(cavity);
            for face in face_offset..out.topology.faces.len() as u32 {
                reverse_face(&mut out, face);
                out.topology.faces[face as usize].provenance.role = FaceRole::Cut;
                out.topology.faces[face as usize].provenance.reversed = true;
            }
        }
        BooleanOp::Subtraction => {}
    }
    out.revision = a
        .brep
        .revision
        .max(b.brep.revision)
        .checked_add(1)
        .ok_or_else(|| GeometryError::LimitExceeded("boolean revision overflow".into()))?;
    out.validate()?;
    let face_mappings = cylinder_face_mappings(&out, [a, b]);
    Ok(BooleanResult {
        brep: out,
        report: BooleanReport {
            operation,
            quality: GeometryQuality::Analytic,
            contacts: Vec::new(),
            coincident: false,
            face_mappings,
        },
    })
}

fn embedded_transverse_cylinder_boolean(
    a: &CylinderInput<'_>,
    b: &CylinderInput<'_>,
    alignment: f64,
    radial: Point3,
    b_span: [f64; 2],
    operation: BooleanOp,
    id: String,
    accuracy: Accuracy,
) -> Result<BooleanResult, GeometryError> {
    if !matches!(operation, BooleanOp::Union | BooleanOp::Subtraction) {
        return Err(GeometryError::InvalidGeometry(
            "embedded transverse cylinder builder requires union or subtraction".into(),
        ));
    }
    let axis = a.frame.z;
    let distance = norm(radial);
    let toward_b = unit(radial)?;
    let lateral = unit(cross(axis, toward_b))?;
    let along =
        (distance * distance + a.radius * a.radius - b.radius * b.radius) / (2.0 * distance);
    let half_squared = (a.radius - along) * (a.radius + along);
    let clearance = 4.0 * accuracy.geometric;
    if half_squared <= clearance * clearance {
        return Err(GeometryError::UnresolvedIntersection(
            "embedded transverse cylinder chord is below geometric resolution".into(),
        ));
    }
    let half = half_squared.sqrt();
    let chord_center: Point3 = std::array::from_fn(|i| a.frame.origin[i] + toward_b[i] * along);
    let points = [
        std::array::from_fn(|i| chord_center[i] + lateral[i] * half),
        std::array::from_fn(|i| chord_center[i] - lateral[i] * half),
    ];
    let mut b_base_frame = a.frame;
    b_base_frame.origin = std::array::from_fn(|i| a.frame.origin[i] + radial[i]);
    let a_outer = selected_cylinder_arc(
        a.frame,
        a.radius,
        points,
        b_base_frame.origin,
        b.radius,
        false,
        false,
    )?;
    let a_inner = selected_cylinder_arc(
        a.frame,
        a.radius,
        points,
        b_base_frame.origin,
        b.radius,
        true,
        false,
    )?;
    let b_boundary = selected_cylinder_arc(
        b_base_frame,
        b.radius,
        points,
        a.frame.origin,
        a.radius,
        operation == BooleanOp::Subtraction,
        operation == BooleanOp::Subtraction,
    )?;
    let levels = [0.0, b_span[0], b_span[1], a.height];
    if levels.windows(2).any(|span| span[1] - span[0] <= clearance) {
        return Err(GeometryError::UnresolvedIntersection(
            "embedded transverse cylinder leaves a sub-tolerance axial segment".into(),
        ));
    }

    let mut builder = Builder::new(id, accuracy)?;
    let vertices: [[u32; 2]; 4] = std::array::from_fn(|level| {
        points.map(|point| {
            builder.vertex(std::array::from_fn(|i| point[i] + axis[i] * levels[level]))
        })
    });
    let vertical_ranges = [
        Interval::new(0.0, levels[1] - levels[0])?,
        Interval::new(0.0, levels[2] - levels[1])?,
        Interval::new(0.0, levels[3] - levels[2])?,
    ];
    let vertical_edges: [[u32; 2]; 3] = std::array::from_fn(|segment| {
        [0, 1].map(|point| {
            builder.edge(
                CurveGeometry::Line {
                    origin: std::array::from_fn(|i| points[point][i] + axis[i] * levels[segment]),
                    direction: axis,
                },
                vertical_ranges[segment],
                false,
            )
        })
    });
    let mut arc_edge = |arc: CylinderArc, level: usize, seam: bool| {
        let shifted = shifted_cylinder_arc(arc, axis, levels[level]);
        builder.edge(
            CurveGeometry::Circle {
                frame: shifted.frame,
                radius: shifted.radius,
            },
            shifted.range,
            seam,
        )
    };
    let a_outer_edges = [
        arc_edge(a_outer, 0, false),
        arc_edge(a_outer, 1, true),
        arc_edge(a_outer, 2, true),
        arc_edge(a_outer, 3, false),
    ];
    let a_inner_edges = [
        arc_edge(a_inner, 0, false),
        arc_edge(a_inner, 1, false),
        arc_edge(a_inner, 2, false),
        arc_edge(a_inner, 3, false),
    ];
    let b_edges = [
        arc_edge(b_boundary, 1, false),
        arc_edge(b_boundary, 2, false),
    ];
    drop(arc_edge);

    for segment in 0..3 {
        let height = levels[segment + 1] - levels[segment];
        add_cylinder_arc_face(
            &mut builder,
            &format!("a:outer:{segment}"),
            shifted_cylinder_arc(a_outer, axis, levels[segment]),
            a_outer_edges[segment],
            a_outer_edges[segment + 1],
            vertical_edges[segment],
            [vertices[segment], vertices[segment + 1]],
            height,
        )?;
        let (arc, edges, key) = if segment == 1 {
            (b_boundary, b_edges, "b:boundary")
        } else {
            (
                a_inner,
                [a_inner_edges[segment], a_inner_edges[segment + 1]],
                if segment == 0 {
                    "a:inner:lower"
                } else {
                    "a:inner:upper"
                },
            )
        };
        add_cylinder_arc_face(
            &mut builder,
            key,
            shifted_cylinder_arc(arc, axis, levels[segment]),
            edges[0],
            edges[1],
            vertical_edges[segment],
            [vertices[segment], vertices[segment + 1]],
            height,
        )?;
    }

    let mut bottom_frame = a.frame;
    bottom_frame.y = scale(bottom_frame.y, -1.0);
    bottom_frame.z = scale(bottom_frame.z, -1.0);
    let bottom_arcs = [
        shifted_cylinder_arc(a_outer, axis, levels[0]),
        shifted_cylinder_arc(a_inner, axis, levels[0]),
    ];
    let bottom_uses = two_arc_plane_uses(
        &builder,
        bottom_frame,
        (bottom_arcs[0], a_outer_edges[0]),
        (bottom_arcs[1], a_inner_edges[0]),
        vertices[0],
    )?;
    builder.face(
        "a:lower-cap",
        SurfaceGeometry::Plane {
            frame: bottom_frame,
        },
        two_arc_plane_bounds(bottom_frame, bottom_arcs, accuracy.geometric),
        bottom_uses,
    )?;

    for (level, key) in [(1, "b:lower-interface"), (2, "b:upper-interface")] {
        let outward_positive = (operation == BooleanOp::Union && level == 2)
            || (operation == BooleanOp::Subtraction && level == 1);
        let mut frame = a.frame;
        frame.origin = a.frame.point([0.0, 0.0, levels[level]]);
        if !outward_positive {
            frame.y = scale(frame.y, -1.0);
            frame.z = scale(frame.z, -1.0);
        }
        let arcs = [
            shifted_cylinder_arc(a_inner, axis, levels[level]),
            shifted_cylinder_arc(b_boundary, axis, levels[level]),
        ];
        let edges = if level == 1 {
            [a_inner_edges[1], b_edges[0]]
        } else {
            [a_inner_edges[2], b_edges[1]]
        };
        let uses = two_arc_plane_uses(
            &builder,
            frame,
            (arcs[0], edges[0]),
            (arcs[1], edges[1]),
            vertices[level],
        )?;
        builder.face(
            key,
            SurfaceGeometry::Plane { frame },
            two_arc_plane_bounds(frame, arcs, accuracy.geometric),
            uses,
        )?;
    }

    let mut top_frame = a.frame;
    top_frame.origin = a.frame.point([0.0, 0.0, a.height]);
    let top_arcs = [
        shifted_cylinder_arc(a_outer, axis, levels[3]),
        shifted_cylinder_arc(a_inner, axis, levels[3]),
    ];
    let top_uses = two_arc_plane_uses(
        &builder,
        top_frame,
        (top_arcs[0], a_outer_edges[3]),
        (top_arcs[1], a_inner_edges[3]),
        vertices[3],
    )?;
    builder.face(
        "a:upper-cap",
        SurfaceGeometry::Plane { frame: top_frame },
        two_arc_plane_bounds(top_frame, top_arcs, accuracy.geometric),
        top_uses,
    )?;

    let mut out = builder.finish()?;
    let a_lateral = cylinder_source(a, 0);
    let b_lateral = cylinder_source(b, 0);
    for face in [0, 1, 2, 4, 5] {
        out.topology.faces[face].provenance =
            face_provenance(vec![a_lateral.clone()], FaceRole::Split, false);
    }
    out.topology.faces[3].provenance = face_provenance(
        vec![b_lateral],
        if operation == BooleanOp::Subtraction {
            FaceRole::Cut
        } else {
            FaceRole::Split
        },
        operation == BooleanOp::Subtraction,
    );
    let a_caps = [cylinder_source(a, 1), cylinder_source(a, 2)];
    out.topology.faces[6].provenance =
        face_provenance(vec![a_caps[0].clone()], FaceRole::Preserved, false);
    out.topology.faces[9].provenance =
        face_provenance(vec![a_caps[1].clone()], FaceRole::Preserved, false);
    let b_caps = if alignment > 0.0 {
        [cylinder_source(b, 1), cylinder_source(b, 2)]
    } else {
        [cylinder_source(b, 2), cylinder_source(b, 1)]
    };
    for (face, source) in [(7, b_caps[0].clone()), (8, b_caps[1].clone())] {
        out.topology.faces[face].provenance = face_provenance(
            vec![source],
            if operation == BooleanOp::Subtraction {
                FaceRole::Cut
            } else {
                FaceRole::Split
            },
            operation == BooleanOp::Subtraction,
        );
    }
    out.revision = a
        .brep
        .revision
        .max(b.brep.revision)
        .checked_add(1)
        .ok_or_else(|| GeometryError::LimitExceeded("boolean revision overflow".into()))?;
    out.validate()?;
    let face_mappings = cylinder_face_mappings(&out, [a, b]);
    Ok(BooleanResult {
        brep: out,
        report: BooleanReport {
            operation,
            quality: GeometryQuality::Analytic,
            contacts: Vec::new(),
            coincident: false,
            face_mappings,
        },
    })
}

fn sliced_parallel_cylinder_boolean(
    a: &CylinderInput<'_>,
    b: &CylinderInput<'_>,
    alignment: f64,
    radial: Point3,
    a_span: [f64; 2],
    b_span: [f64; 2],
    span: [f64; 2],
    operation: BooleanOp,
    id: String,
    accuracy: Accuracy,
    roundoff: f64,
) -> Result<BooleanResult, GeometryError> {
    let mut a_frame = a.frame;
    a_frame.origin = a.frame.point([0.0, 0.0, span[0]]);
    let mut b_frame = a_frame;
    b_frame.origin = std::array::from_fn(|axis| a_frame.origin[axis] + radial[axis]);
    let height = span[1] - span[0];
    let a_slice = CylinderInput {
        brep: a.brep,
        frame: a_frame,
        radius: a.radius,
        height,
    };
    let b_slice = CylinderInput {
        brep: b.brep,
        frame: b_frame,
        radius: b.radius,
        height,
    };
    let mut result = parallel_cylinder_boolean(&a_slice, &b_slice, 1.0, operation, id, accuracy)?;
    result.report.coincident = false;
    if operation == BooleanOp::Intersection && result.brep.topology.faces.len() == 4 {
        let a_caps = [cylinder_source(a, 1), cylinder_source(a, 2)];
        let b_caps = if alignment > 0.0 {
            [cylinder_source(b, 1), cylinder_source(b, 2)]
        } else {
            [cylinder_source(b, 2), cylinder_source(b, 1)]
        };
        for (face, boundary, end) in [(2, span[0], 0), (3, span[1], 1)] {
            let mut sources = Vec::with_capacity(2);
            if (boundary - a_span[end]).abs() <= roundoff {
                sources.push(a_caps[end].clone());
            }
            if (boundary - b_span[end]).abs() <= roundoff {
                sources.push(b_caps[end].clone());
            }
            if sources.len() > 1 {
                result.report.coincident = true;
            }
            if sources.is_empty() {
                return Err(GeometryError::InvalidTopology(
                    "parallel cylinder slice cap has no source face".into(),
                ));
            }
            result.brep.topology.faces[face].provenance =
                face_provenance(sources, FaceRole::Split, false);
        }
        result.report.face_mappings = cylinder_face_mappings(&result.brep, [a, b]);
        result.brep.validate()?;
    } else if operation == BooleanOp::Subtraction {
        result.report.coincident =
            (a_span[0] - b_span[0]).abs() <= roundoff || (a_span[1] - b_span[1]).abs() <= roundoff;
    }
    Ok(result)
}

fn boolean_cylinders(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let a = full_cylinder(a)?;
    let b = full_cylinder(b)?;
    let accuracy = Accuracy {
        geometric: a.brep.accuracy.geometric.max(b.brep.accuracy.geometric),
        intersection: a
            .brep
            .accuracy
            .intersection
            .max(b.brep.accuracy.intersection),
        tessellation: a
            .brep
            .accuracy
            .tessellation
            .max(b.brep.accuracy.tessellation),
        exchange: a.brep.accuracy.exchange.max(b.brep.accuracy.exchange),
    };
    let alignment = dot(a.frame.z, b.frame.z);
    if 1.0 - alignment.abs() > 1e-12 {
        if operation == BooleanOp::Subtraction {
            return transverse_cylinder_through_subtraction(&a, &b, id, accuracy);
        }
        return Err(GeometryError::CoverageGap {
            families: ["cylinder".into(), "nonparallel cylinder".into()],
        });
    }
    let delta = sub(b.frame.origin, a.frame.origin);
    let axial = dot(delta, a.frame.z);
    let radial = sub(delta, scale(a.frame.z, axial));
    let a_span = [0.0, a.height];
    let b_end = axial + alignment.signum() * b.height;
    let b_span = [axial.min(b_end), axial.max(b_end)];
    let span_error = (a_span[0] - b_span[0])
        .abs()
        .max((a_span[1] - b_span[1]).abs());
    let scale = a
        .frame
        .origin
        .into_iter()
        .chain(b.frame.origin)
        .chain([a.radius, b.radius, a.height, b.height])
        .fold(1.0_f64, |largest, value| largest.max(value.abs()));
    let roundoff = 128.0 * f64::EPSILON * scale;
    let radial_distance = norm(radial);
    if radial_distance > roundoff {
        if radial_distance <= accuracy.intersection {
            return Err(GeometryError::UnresolvedIntersection(
                "cylinder axes differ within the intersection budget".into(),
            ));
        }
        if span_error <= roundoff {
            return parallel_cylinder_boolean(&a, &b, alignment, operation, id, accuracy);
        }
        let clearance = 4.0 * accuracy.geometric;
        let overlap_span = [a_span[0].max(b_span[0]), a_span[1].min(b_span[1])];
        let overlap = overlap_span[1] - overlap_span[0];
        if overlap.abs() <= clearance {
            return Err(GeometryError::UnresolvedIntersection(
                "parallel cylinder axial contact is below geometric resolution".into(),
            ));
        }
        if overlap < 0.0 {
            return separate_boolean(a.brep, b.brep, operation, id, Vec::new());
        }
        if operation == BooleanOp::Intersection {
            return sliced_parallel_cylinder_boolean(
                &a,
                &b,
                alignment,
                radial,
                a_span,
                b_span,
                overlap_span,
                operation,
                id,
                accuracy,
                roundoff,
            );
        }
        if operation == BooleanOp::Subtraction {
            let b_covers_a = b_span[0] <= a_span[0] + roundoff && b_span[1] >= a_span[1] - roundoff;
            if b_covers_a {
                return sliced_parallel_cylinder_boolean(
                    &a, &b, alignment, radial, a_span, b_span, a_span, operation, id, accuracy,
                    roundoff,
                );
            }
            let b_nearly_covers_a =
                b_span[0] <= a_span[0] + clearance && b_span[1] >= a_span[1] - clearance;
            if b_nearly_covers_a {
                return Err(GeometryError::UnresolvedIntersection(
                    "parallel cylinder subtraction leaves a sub-tolerance axial segment".into(),
                ));
            }
        }
        let b_embedded_in_a =
            b_span[0] > a_span[0] + clearance && b_span[1] < a_span[1] - clearance;
        let transverse_overlap = radial_distance > (a.radius - b.radius).abs() + clearance
            && radial_distance < a.radius + b.radius - clearance;
        if b_embedded_in_a
            && transverse_overlap
            && matches!(operation, BooleanOp::Union | BooleanOp::Subtraction)
        {
            return embedded_transverse_cylinder_boolean(
                &a, &b, alignment, radial, b_span, operation, id, accuracy,
            );
        }
        let a_embedded_in_b =
            a_span[0] > b_span[0] + clearance && a_span[1] < b_span[1] - clearance;
        if a_embedded_in_b && transverse_overlap && operation == BooleanOp::Union {
            return boolean_cylinders(b.brep, a.brep, operation, id);
        }
        return Err(GeometryError::CoverageGap {
            families: [
                "parallel cylinder".into(),
                "non-coextensive parallel cylinder".into(),
            ],
        });
    }
    if a.radius != b.radius {
        if (a.radius - b.radius).abs() <= accuracy.intersection {
            return Err(GeometryError::UnresolvedIntersection(
                "cylinder radii differ within the intersection budget".into(),
            ));
        }
        if span_error <= roundoff {
            return coextensive_cylinder_radii(&a, &b, alignment, operation, id, accuracy);
        }
        let clearance = 4.0 * accuracy.geometric;
        let b_inside_a = b.radius + clearance < a.radius
            && b_span[0] > a_span[0] + clearance
            && b_span[1] < a_span[1] - clearance;
        let a_inside_b = a.radius + clearance < b.radius
            && a_span[0] > b_span[0] + clearance
            && a_span[1] < b_span[1] - clearance;
        if b_inside_a || a_inside_b {
            return contained_cylinders(&a, &b, b_inside_a, operation, id, accuracy);
        }
        let near_radial_contact = (a.radius - b.radius).abs() <= clearance;
        let near_axial_contact = [
            (a_span[0] - b_span[0]).abs(),
            (a_span[0] - b_span[1]).abs(),
            (a_span[1] - b_span[0]).abs(),
            (a_span[1] - b_span[1]).abs(),
        ]
        .into_iter()
        .any(|gap| gap <= clearance);
        if near_radial_contact || near_axial_contact {
            return Err(GeometryError::UnresolvedIntersection(
                "cylinder containment boundary is below geometric resolution".into(),
            ));
        }
        if span_error <= 4.0 * accuracy.geometric {
            return Err(GeometryError::UnresolvedIntersection(
                "cylinder axial spans differ below geometric resolution".into(),
            ));
        }
        return Err(GeometryError::CoverageGap {
            families: [
                "cylinder".into(),
                "different-radius coaxial cylinder with unequal axial spans".into(),
            ],
        });
    }
    let overlap = a_span[1].min(b_span[1]) - a_span[0].max(b_span[0]);
    if overlap != 0.0 && overlap.abs() <= 4.0 * accuracy.geometric {
        return Err(GeometryError::UnresolvedIntersection(
            "cylinder axial overlap or clearance is below geometric resolution".into(),
        ));
    }

    let a_lateral = cylinder_source(&a, 0);
    let b_lateral = cylinder_source(&b, 0);
    let a_caps = [cylinder_source(&a, 1), cylinder_source(&a, 2)];
    let b_caps = if alignment > 0.0 {
        [cylinder_source(&b, 1), cylinder_source(&b, 2)]
    } else {
        [cylinder_source(&b, 2), cylinder_source(&b, 1)]
    };
    let cap_at = |boundary: f64, low: bool, include_a: bool, include_b: bool| {
        let mut sources = Vec::new();
        if include_a && boundary == if low { a_span[0] } else { a_span[1] } {
            sources.push(a_caps[usize::from(!low)].clone());
        }
        if include_b && boundary == if low { b_span[0] } else { b_span[1] } {
            sources.push(b_caps[usize::from(!low)].clone());
        }
        sources
    };

    struct Segment {
        lo: f64,
        hi: f64,
        provenances: [FaceProvenance; 3],
    }
    let mut segments = Vec::new();
    let coincident = overlap >= 0.0;
    match operation {
        BooleanOp::Union if overlap >= 0.0 => {
            let lo = a_span[0].min(b_span[0]);
            let hi = a_span[1].max(b_span[1]);
            let lower = cap_at(lo, true, true, true);
            let upper = cap_at(hi, false, true, true);
            segments.push(Segment {
                lo,
                hi,
                provenances: [
                    face_provenance(
                        vec![a_lateral.clone(), b_lateral.clone()],
                        FaceRole::Coincident,
                        false,
                    ),
                    face_provenance(
                        lower.clone(),
                        if lower.len() > 1 {
                            FaceRole::Coincident
                        } else {
                            FaceRole::Preserved
                        },
                        false,
                    ),
                    face_provenance(
                        upper.clone(),
                        if upper.len() > 1 {
                            FaceRole::Coincident
                        } else {
                            FaceRole::Preserved
                        },
                        false,
                    ),
                ],
            });
        }
        BooleanOp::Union => {
            for (span, lateral, caps) in [
                (a_span, a_lateral.clone(), a_caps.clone()),
                (b_span, b_lateral.clone(), b_caps.clone()),
            ] {
                segments.push(Segment {
                    lo: span[0],
                    hi: span[1],
                    provenances: [
                        face_provenance(vec![lateral], FaceRole::Preserved, false),
                        face_provenance(vec![caps[0].clone()], FaceRole::Preserved, false),
                        face_provenance(vec![caps[1].clone()], FaceRole::Preserved, false),
                    ],
                });
            }
        }
        BooleanOp::Intersection if overlap > 0.0 => {
            let lo = a_span[0].max(b_span[0]);
            let hi = a_span[1].min(b_span[1]);
            let lower = cap_at(lo, true, lo == a_span[0], lo == b_span[0]);
            let upper = cap_at(hi, false, hi == a_span[1], hi == b_span[1]);
            segments.push(Segment {
                lo,
                hi,
                provenances: [
                    face_provenance(
                        vec![a_lateral.clone(), b_lateral.clone()],
                        FaceRole::Coincident,
                        false,
                    ),
                    face_provenance(
                        lower.clone(),
                        if lower.len() > 1 {
                            FaceRole::Coincident
                        } else {
                            FaceRole::Preserved
                        },
                        false,
                    ),
                    face_provenance(
                        upper.clone(),
                        if upper.len() > 1 {
                            FaceRole::Coincident
                        } else {
                            FaceRole::Preserved
                        },
                        false,
                    ),
                ],
            });
        }
        BooleanOp::Intersection => {}
        BooleanOp::Subtraction if overlap <= 0.0 => {
            segments.push(Segment {
                lo: a_span[0],
                hi: a_span[1],
                provenances: [
                    face_provenance(vec![a_lateral.clone()], FaceRole::Preserved, false),
                    face_provenance(vec![a_caps[0].clone()], FaceRole::Preserved, false),
                    face_provenance(vec![a_caps[1].clone()], FaceRole::Preserved, false),
                ],
            });
        }
        BooleanOp::Subtraction => {
            if b_span[0] > a_span[0] {
                segments.push(Segment {
                    lo: a_span[0],
                    hi: b_span[0].min(a_span[1]),
                    provenances: [
                        face_provenance(vec![a_lateral.clone()], FaceRole::Split, false),
                        face_provenance(vec![a_caps[0].clone()], FaceRole::Preserved, false),
                        face_provenance(vec![b_caps[0].clone()], FaceRole::Cut, true),
                    ],
                });
            }
            if b_span[1] < a_span[1] {
                segments.push(Segment {
                    lo: b_span[1].max(a_span[0]),
                    hi: a_span[1],
                    provenances: [
                        face_provenance(vec![a_lateral.clone()], FaceRole::Split, false),
                        face_provenance(vec![b_caps[1].clone()], FaceRole::Cut, true),
                        face_provenance(vec![a_caps[1].clone()], FaceRole::Preserved, false),
                    ],
                });
            }
        }
    }

    let mut out = BrepEnvelope::new(id, accuracy)?;
    for (index, segment) in segments.into_iter().enumerate() {
        let mut frame = a.frame;
        frame.origin = a.frame.point([0.0, 0.0, segment.lo]);
        append_cylinder_segment(
            &mut out,
            frame,
            a.radius,
            segment.hi - segment.lo,
            segment.provenances,
            index,
        )?;
    }
    out.revision = a
        .brep
        .revision
        .max(b.brep.revision)
        .checked_add(1)
        .ok_or_else(|| GeometryError::LimitExceeded("boolean revision overflow".into()))?;
    out.validate()?;
    let face_mappings = cylinder_face_mappings(&out, [&a, &b]);
    Ok(BooleanResult {
        brep: out,
        report: BooleanReport {
            operation,
            quality: GeometryQuality::Analytic,
            contacts: Vec::new(),
            coincident,
            face_mappings,
        },
    })
}

fn append_sphere(
    out: &mut BrepEnvelope,
    input: &SphereInput<'_>,
    reversed: bool,
) -> Result<u32, GeometryError> {
    let mut part = primitives::sphere(
        input.brep.id.clone(),
        input.frame,
        input.radius,
        out.accuracy,
    )?;
    part.topology.faces[0].provenance = provenance(
        input,
        if reversed {
            FaceRole::Cut
        } else {
            FaceRole::Preserved
        },
        reversed,
    );
    if reversed {
        reverse_face(&mut part, 0);
    }
    let [v, e, h, l, f, s, c, p, surface] = [
        out.topology.vertices.len(),
        out.topology.edges.len(),
        out.topology.halfedges.len(),
        out.topology.loops.len(),
        out.topology.faces.len(),
        out.topology.shells.len(),
        out.geometry.curves.len(),
        out.geometry.pcurves.len(),
        out.geometry.surfaces.len(),
    ]
    .map(|n| n as u32);
    for vertex in &mut part.topology.vertices {
        vertex.id += v;
        vertex.outgoing_halfedge = vertex.outgoing_halfedge.map(|id| id + h);
    }
    for edge in &mut part.topology.edges {
        edge.id += e;
        edge.halfedge += h;
        edge.twin_halfedge = edge.twin_halfedge.map(|id| id + h);
        match &mut edge.geometry {
            EdgeGeometry::Curve { curve, .. } => *curve += c,
            EdgeGeometry::Collapsed { vertex } => *vertex += v,
        }
    }
    for halfedge in &mut part.topology.halfedges {
        halfedge.id += h;
        halfedge.from += v;
        halfedge.to += v;
        halfedge.edge += e;
        halfedge.twin = halfedge.twin.map(|id| id + h);
        halfedge.next = halfedge.next.map(|id| id + h);
        halfedge.prev = halfedge.prev.map(|id| id + h);
        halfedge.face = halfedge.face.map(|id| id + f);
        halfedge.loop_ref = halfedge.loop_ref.map(|id| id + l);
        halfedge.geometry_use.pcurve = halfedge.geometry_use.pcurve.map(|id| id + p);
    }
    for loop_ in &mut part.topology.loops {
        loop_.id += l;
        loop_.start_halfedge += h;
        loop_.face_ref += f;
    }
    for face in &mut part.topology.faces {
        face.id += f;
        face.key = format!("{f}:{}", face.key);
        face.surface += surface;
        face.trim.outer += l;
        face.shell_ref = Some(s);
    }
    for shell in &mut part.topology.shells {
        shell.id += s;
        for face in &mut shell.faces {
            *face += f;
        }
    }
    out.geometry.surfaces.extend(part.geometry.surfaces);
    out.geometry.curves.extend(part.geometry.curves);
    out.geometry.pcurves.extend(part.geometry.pcurves);
    out.topology.vertices.extend(part.topology.vertices);
    out.topology.edges.extend(part.topology.edges);
    out.topology.halfedges.extend(part.topology.halfedges);
    out.topology.loops.extend(part.topology.loops);
    out.topology.faces.extend(part.topology.faces);
    out.topology.shells.extend(part.topology.shells);
    Ok(s)
}

fn patch(
    builder: &mut Builder,
    input: &SphereInput<'_>,
    frame: Frame3,
    latitude: f64,
    north: bool,
    shared_edge: u32,
    shared_vertex: u32,
    reversed: bool,
) -> Result<(), GeometryError> {
    use Orientation::{Forward as F, Reverse as R};
    let pole_lat = if north {
        std::f64::consts::FRAC_PI_2
    } else {
        -std::f64::consts::FRAC_PI_2
    };
    let (lo, hi) = if north {
        (latitude, pole_lat)
    } else {
        (pole_lat, latitude)
    };
    let surface = SurfaceGeometry::Sphere {
        frame,
        radius: input.radius,
    };
    let pole = builder.vertex(surface.point_at([0.0, pole_lat])?);
    let collapse = builder.edge_geometry(EdgeGeometry::Collapsed { vertex: pole }, true);
    let seam = builder.edge(
        CurveGeometry::Circle {
            frame: Frame3 {
                x: frame.x,
                y: frame.z,
                z: scale(frame.y, -1.0),
                ..frame
            },
            radius: input.radius,
        },
        Interval::new(lo, hi)?,
        true,
    );
    let tau = std::f64::consts::TAU;
    let (low, high) = if north {
        (shared_vertex, pole)
    } else {
        (pole, shared_vertex)
    };
    let bottom = if north {
        boundary(shared_edge, low, low, F, uv_line([0.0, lo], [1.0, 0.0]))
    } else {
        boundary(collapse, low, low, F, uv_line([0.0, lo], [tau, 0.0]))
    };
    let top = if north {
        boundary(collapse, high, high, F, uv_line([tau, hi], [-tau, 0.0]))
    } else {
        boundary(shared_edge, high, high, R, uv_line([0.0, hi], [1.0, 0.0]))
    };
    let face = builder.brep.topology.faces.len() as u32;
    builder.face(
        &format!("{face}:{}", input.brep.topology.faces[0].key),
        surface,
        [[0.0, tau], [lo, hi]],
        vec![
            bottom,
            boundary(seam, low, high, F, uv_line([tau, 0.0], [0.0, 1.0])),
            top,
            boundary(seam, high, low, R, uv_line([0.0, 0.0], [0.0, 1.0])),
        ],
    )?;
    builder.brep.topology.faces[face as usize].provenance = provenance(
        input,
        if reversed {
            FaceRole::Cut
        } else {
            FaceRole::Split
        },
        reversed,
    );
    if reversed {
        reverse_face(&mut builder.brep, face);
    }
    Ok(())
}

fn inset_prismatic_ring(
    ring: &[Point3],
    frame: Frame3,
    thickness: f64,
    tolerance: f64,
) -> Result<Vec<[f64; 2]>, GeometryError> {
    let points = ring
        .iter()
        .map(|point| {
            let local = frame.local(*point);
            Pt2::new(local[0], local[1])
        })
        .collect::<Vec<_>>();
    if points.len() < 3 {
        return Err(prismatic_gap());
    }
    let cross2 = |a: Pt2, b: Pt2| a.x * b.z - a.z * b.x;
    let direction = |from: Pt2, to: Pt2| -> Result<Pt2, GeometryError> {
        let delta = Pt2::new(to.x - from.x, to.z - from.z);
        let length = delta.x.hypot(delta.z);
        if length <= 4.0 * tolerance {
            return Err(GeometryError::UnresolvedIntersection(
                "profile shell contains a sub-tolerance edge".into(),
            ));
        }
        Ok(Pt2::new(delta.x / length, delta.z / length))
    };
    let mut result = Vec::with_capacity(points.len());
    for index in 0..points.len() {
        let point = points[index];
        let incoming = direction(points[(index + points.len() - 1) % points.len()], point)?;
        let outgoing = direction(point, points[(index + 1) % points.len()])?;
        let incoming_normal = Pt2::new(-incoming.z, incoming.x);
        let outgoing_normal = Pt2::new(-outgoing.z, outgoing.x);
        let incoming_origin = Pt2::new(
            point.x + thickness * incoming_normal.x,
            point.z + thickness * incoming_normal.z,
        );
        let outgoing_origin = Pt2::new(
            point.x + thickness * outgoing_normal.x,
            point.z + thickness * outgoing_normal.z,
        );
        let determinant = cross2(incoming, outgoing);
        let offset = if determinant.abs() <= 1.0e-12 {
            if incoming.x * outgoing.x + incoming.z * outgoing.z < 0.0 {
                return Err(GeometryError::UnresolvedIntersection(
                    "profile shell has a reversing corner".into(),
                ));
            }
            incoming_origin
        } else {
            let delta = Pt2::new(
                outgoing_origin.x - incoming_origin.x,
                outgoing_origin.z - incoming_origin.z,
            );
            let parameter = cross2(delta, outgoing) / determinant;
            Pt2::new(
                incoming_origin.x + parameter * incoming.x,
                incoming_origin.z + parameter * incoming.z,
            )
        };
        if !offset.x.is_finite()
            || !offset.z.is_finite()
            || (offset.x - point.x).hypot(offset.z - point.z) > thickness * 1.0e6
        {
            return Err(GeometryError::UnresolvedIntersection(
                "profile shell has an unbounded miter".into(),
            ));
        }
        result.push(offset);
    }
    let original_area = poly2d::signed_area2(&points);
    let result_area = poly2d::signed_area2(&result);
    if result_area.abs() <= tolerance * tolerance
        || original_area.signum() != result_area.signum()
        || poly2d::self_intersects2(&result, tolerance)
    {
        return Err(GeometryError::UnresolvedIntersection(
            "profile shell offset collapses or self-intersects".into(),
        ));
    }
    Ok(result.into_iter().map(|point| [point.x, point.z]).collect())
}

pub fn shell_brep(
    input: &BrepEnvelope,
    thickness: f64,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    input.validate()?;
    if !thickness.is_finite() || thickness <= 4.0 * input.accuracy.geometric {
        return Err(GeometryError::UnresolvedIntersection(
            "shell thickness is below geometric resolution".into(),
        ));
    }
    let inner_id = format!("{id}:offset-cavity");
    let inner = match input.geometry.surfaces.first() {
        Some(SurfaceGeometry::Sphere { .. }) => {
            let input = full_sphere(input)?;
            let radius = input.radius - thickness;
            if radius <= 4.0 * input.brep.accuracy.geometric {
                return Err(GeometryError::UnresolvedIntersection(
                    "shell thickness consumes the sphere interior".into(),
                ));
            }
            primitives::sphere(inner_id, input.frame, radius, input.brep.accuracy)?
        }
        Some(SurfaceGeometry::Cylinder { .. }) => {
            let input = full_cylinder(input)?;
            let radius = input.radius - thickness;
            let height = input.height - 2.0 * thickness;
            if radius <= 4.0 * input.brep.accuracy.geometric
                || height <= 4.0 * input.brep.accuracy.geometric
            {
                return Err(GeometryError::UnresolvedIntersection(
                    "shell thickness consumes the cylinder interior".into(),
                ));
            }
            let mut frame = input.frame;
            frame.origin = input.frame.point([0.0, 0.0, thickness]);
            primitives::cylinder(inner_id, frame, radius, height, input.brep.accuracy)?
        }
        Some(SurfaceGeometry::Torus { .. }) => {
            let input = full_torus(input)?;
            let minor_radius = input.minor_radius - thickness;
            if minor_radius <= 4.0 * input.brep.accuracy.geometric {
                return Err(GeometryError::UnresolvedIntersection(
                    "shell thickness consumes the torus interior".into(),
                ));
            }
            primitives::torus(
                inner_id,
                input.frame,
                input.major_radius,
                minor_radius,
                input.brep.accuracy,
            )?
        }
        Some(SurfaceGeometry::Plane { .. }) => match super::box_booleans::full_box(input) {
            Ok(input) => {
                let size = input.size.map(|dimension| dimension - 2.0 * thickness);
                if size
                    .iter()
                    .any(|dimension| *dimension <= 4.0 * input.brep.accuracy.geometric)
                {
                    return Err(GeometryError::UnresolvedIntersection(
                        "shell thickness consumes the cuboid interior".into(),
                    ));
                }
                let mut frame = input.frame;
                frame.origin = input.frame.point([thickness, thickness, thickness]);
                primitives::cuboid(inner_id, frame, size, input.brep.accuracy)?
            }
            Err(GeometryError::CoverageGap { .. }) => {
                let input = full_planar_extrusion(input)?;
                let height = input.height - 2.0 * thickness;
                if height <= 4.0 * input.brep.accuracy.geometric {
                    return Err(GeometryError::UnresolvedIntersection(
                        "shell thickness consumes the profile extrusion height".into(),
                    ));
                }
                let inset = input
                    .contours
                    .iter()
                    .map(|ring| {
                        inset_prismatic_ring(
                            ring,
                            input.frame,
                            thickness,
                            input.brep.accuracy.geometric,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let mut frame = input.frame;
                frame.origin = input.frame.point([0.0, 0.0, thickness]);
                primitives::linear_extrusion(
                    inner_id,
                    frame,
                    inset[0].clone(),
                    inset[1..].to_vec(),
                    height,
                    input.brep.accuracy,
                )
                .map_err(|error| match error {
                    GeometryError::InvalidGeometry(_)
                    | GeometryError::UnresolvedIntersection(_) => {
                        GeometryError::UnresolvedIntersection(
                            "shell offset self-intersects or consumes the profile interior".into(),
                        )
                    }
                    other => other,
                })?
            }
            Err(error) => return Err(error),
        },
        Some(SurfaceGeometry::Cone { .. }) => {
            let input = full_conic_section(input)?;
            let slope = input.semi_angle.tan();
            let normal_scale = (1.0 + slope * slope).sqrt();
            if input.lower_radius <= input.brep.accuracy.geometric {
                let apex = thickness * normal_scale / slope;
                let base = input.axial_range.hi - thickness;
                let height = base - apex;
                let radius = height * slope;
                if radius <= 4.0 * input.brep.accuracy.geometric
                    || height <= 4.0 * input.brep.accuracy.geometric
                {
                    return Err(GeometryError::UnresolvedIntersection(
                        "shell thickness consumes the cone interior".into(),
                    ));
                }
                let base_frame = Frame3 {
                    origin: input.frame.point([0.0, 0.0, base]),
                    x: input.frame.x,
                    y: scale(input.frame.y, -1.0),
                    z: scale(input.frame.z, -1.0),
                };
                primitives::cone(inner_id, base_frame, radius, height, input.brep.accuracy)?
            } else {
                let height = input.axial_range.width() - 2.0 * thickness;
                let lower_radius =
                    input.lower_radius + thickness * slope - thickness * normal_scale;
                let upper_radius =
                    input.upper_radius - thickness * slope - thickness * normal_scale;
                if height <= 4.0 * input.brep.accuracy.geometric
                    || lower_radius <= 4.0 * input.brep.accuracy.geometric
                    || upper_radius <= 4.0 * input.brep.accuracy.geometric
                {
                    return Err(GeometryError::UnresolvedIntersection(
                        "shell thickness consumes the frustum interior".into(),
                    ));
                }
                let frame = Frame3 {
                    origin: input
                        .frame
                        .point([0.0, 0.0, input.axial_range.lo + thickness]),
                    ..input.frame
                };
                primitives::frustum(
                    inner_id,
                    frame,
                    lower_radius,
                    upper_radius,
                    height,
                    input.brep.accuracy,
                )?
            }
        }
        None => {
            return Err(GeometryError::CoverageGap {
                families: ["restricted analytic shell".into(), "input solid".into()],
            })
        }
    };
    let inner_identity = inner.id.clone();
    let mut result = analytic_containment_boolean(input, &inner, true, BooleanOp::Subtraction, id)?;
    for face in &mut result.brep.topology.faces {
        for source in &mut face.provenance.sources {
            if source.entity == inner_identity && source.body == inner_identity {
                let original = input
                    .topology
                    .faces
                    .get(source.face as usize)
                    .ok_or_else(|| {
                        GeometryError::InvalidTopology(
                            "shell offset face has no source face".into(),
                        )
                    })?;
                *source = brep_face_source(input, original.id);
            }
        }
        face.provenance.sources = unique_sources(std::mem::take(&mut face.provenance.sources));
    }
    result.report.face_mappings = input
        .topology
        .faces
        .iter()
        .map(|face| {
            let source = brep_face_source(input, face.id);
            let result_faces = result
                .brep
                .topology
                .faces
                .iter()
                .filter(|candidate| {
                    candidate.provenance.sources.iter().any(|candidate| {
                        candidate.entity == source.entity
                            && candidate.body == source.body
                            && candidate.key == source.key
                            && candidate.face == source.face
                    })
                })
                .map(|candidate| candidate.id)
                .collect();
            FaceMapping {
                source,
                result_faces,
            }
        })
        .collect();
    result.brep.validate()?;
    Ok(result)
}

pub fn boolean_brep(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    a.validate()?;
    b.validate()?;
    if comparable(&a.geometry)? == comparable(&b.geometry)?
        && comparable(&a.topology)? == comparable(&b.topology)?
        && comparable(&a.solids)? == comparable(&b.solids)?
    {
        return coincident_boolean(a, b, operation, id);
    }
    if let Some(result) = disjoint_boolean(a, b, operation, id.clone())? {
        return Ok(result);
    }
    let a_is_sphere = matches!(
        a.geometry.surfaces.first(),
        Some(SurfaceGeometry::Sphere { .. })
    );
    let b_is_sphere = matches!(
        b.geometry.surfaces.first(),
        Some(SurfaceGeometry::Sphere { .. })
    );
    let a_is_cylinder = matches!(
        a.geometry.surfaces.first(),
        Some(SurfaceGeometry::Cylinder { .. })
    );
    let b_is_cylinder = matches!(
        b.geometry.surfaces.first(),
        Some(SurfaceGeometry::Cylinder { .. })
    );
    let a_is_torus = matches!(
        a.geometry.surfaces.first(),
        Some(SurfaceGeometry::Torus { .. })
    );
    let b_is_torus = matches!(
        b.geometry.surfaces.first(),
        Some(SurfaceGeometry::Torus { .. })
    );
    let a_is_conic = matches!(
        a.geometry.surfaces.first(),
        Some(SurfaceGeometry::Cone { .. })
    );
    let b_is_conic = matches!(
        b.geometry.surfaces.first(),
        Some(SurfaceGeometry::Cone { .. })
    );
    let a_is_box = !a.geometry.surfaces.is_empty()
        && a.geometry
            .surfaces
            .iter()
            .all(|surface| matches!(surface, SurfaceGeometry::Plane { .. }));
    let b_is_box = !b.geometry.surfaces.is_empty()
        && b.geometry
            .surfaces
            .iter()
            .all(|surface| matches!(surface, SurfaceGeometry::Plane { .. }));
    let specialized = (|| {
        if (a_is_sphere && b_is_cylinder) || (a_is_cylinder && b_is_sphere) {
            return sphere_cylinder_boolean(a, b, operation, id.clone());
        }
        if (a_is_sphere && b_is_conic) || (a_is_conic && b_is_sphere) {
            return sphere_conic_containment(a, b, operation, id.clone());
        }
        if (a_is_cylinder && b_is_conic) || (a_is_conic && b_is_cylinder) {
            return cylinder_conic_containment(a, b, operation, id.clone());
        }
        if (a_is_conic && b_is_box) || (a_is_box && b_is_conic) {
            return conic_box_containment(a, b, operation, id.clone());
        }
        if (a_is_sphere && b_is_torus) || (a_is_torus && b_is_sphere) {
            return sphere_torus_containment(a, b, operation, id.clone());
        }
        if (a_is_cylinder && b_is_torus) || (a_is_torus && b_is_cylinder) {
            return torus_cylinder_containment(a, b, operation, id.clone());
        }
        if (a_is_box && b_is_torus) || (a_is_torus && b_is_box) {
            return torus_box_containment(a, b, operation, id.clone());
        }
        if (a_is_conic && b_is_torus) || (a_is_torus && b_is_conic) {
            return torus_conic_containment(a, b, operation, id.clone());
        }
        if a_is_torus && b_is_torus {
            return torus_containment_boolean(a, b, operation, id.clone());
        }
        if a_is_conic && b_is_conic {
            return conic_containment_boolean(a, b, operation, id.clone());
        }
        if (a_is_sphere && b_is_box) || (a_is_box && b_is_sphere) {
            return sphere_box_boolean(a, b, operation, id.clone());
        }
        if (a_is_cylinder && b_is_box) || (a_is_box && b_is_cylinder) {
            return cylinder_box_boolean(a, b, operation, id.clone());
        }
        if a_is_cylinder && b_is_cylinder {
            return boolean_cylinders(a, b, operation, id.clone());
        }
        if a_is_box && b_is_box {
            return match super::box_booleans::boolean_boxes(a, b, operation, id.clone()) {
                Ok(result) => Ok(result),
                Err(GeometryError::CoverageGap { .. }) => {
                    match boolean_planar_extrusions(a, b, operation, id.clone()) {
                        Ok(result) => Ok(result),
                        Err(GeometryError::CoverageGap { .. }) => {
                            match super::box_booleans::boolean_rectilinear(
                                a,
                                b,
                                operation,
                                id.clone(),
                            ) {
                                Ok(result) => Ok(result),
                                Err(GeometryError::CoverageGap { .. })
                                    if operation == BooleanOp::Subtraction =>
                                {
                                    match super::planar_booleans::subtract_layered_extrusions(
                                        a,
                                        b,
                                        id.clone(),
                                    ) {
                                        Err(GeometryError::CoverageGap { .. }) => {
                                            super::planar_booleans::subtract_planar_polyhedra(
                                                a,
                                                b,
                                                id.clone(),
                                            )
                                        }
                                        result => result,
                                    }
                                }
                                Err(error) => Err(error),
                            }
                        }
                        Err(error) => Err(error),
                    }
                }
                Err(error) => Err(error),
            };
        }
        boolean_spheres(a, b, operation, id.clone())
    })();
    match specialized {
        Err(GeometryError::CoverageGap { .. }) => {
            if operation == BooleanOp::Subtraction
                && b.geometry
                    .surfaces
                    .iter()
                    .all(|surface| matches!(surface, SurfaceGeometry::Plane { .. }))
                && a.geometry
                    .surfaces
                    .iter()
                    .any(|surface| matches!(surface, SurfaceGeometry::Cylinder { .. }))
            {
                match super::curved_layered_boolean::subtract_vertical_arc_extrusion(
                    a,
                    b,
                    id.clone(),
                ) {
                    Ok(result) => return Ok(result),
                    Err(GeometryError::CoverageGap { .. }) => {}
                    Err(error) => return Err(error),
                }
            }
            if let Some(result) =
                generic_two_sided_cutter_band_subtraction(a, b, operation, id.clone())?
            {
                return Ok(result);
            }
            if let Some(result) =
                generic_multiple_closed_loops_boolean(a, b, operation, id.clone())?
            {
                return Ok(result);
            }
            if let Some(result) = generic_single_closed_loop_boolean(a, b, operation, id.clone())? {
                return Ok(result);
            }
            if let Some(result) = generic_non_intersecting_boolean(a, b, operation, id.clone())? {
                Ok(result)
            } else {
                Err(GeometryError::CoverageGap {
                    families: [
                        "generic analytic solid".into(),
                        "intersecting analytic solid".into(),
                    ],
                })
            }
        }
        result => result,
    }
}

/// Subtract through-depth planar and circular profiles from a rectangular
/// prism in one analytic arrangement. This avoids making a later circle cut
/// through the coplanar face tiles created by an earlier rectangular cut.
fn subtract_prismatic_profile_batch(
    host: &BrepEnvelope,
    cutters: &[BrepEnvelope],
    id: String,
) -> Result<Option<BooleanResult>, GeometryError> {
    use crate::geometry::curved_boolean2d::{boolean_curved_regions, CurveEdge2, CurveRegion2};

    let Ok(prism) = full_planar_extrusion(host) else {
        return Ok(None);
    };
    if prism.contours.len() != 1
        || prism.contours[0].len() != 4
        || host.topology.vertices.len() != 8
    {
        return Ok(None);
    }
    let first_axis = cutters.iter().find_map(|cutter| {
        full_planar_extrusion(cutter)
            .ok()
            .map(|input| input.frame.z)
            .or_else(|| full_cylinder(cutter).ok().map(|input| input.frame.z))
    });
    let Some(across) = first_axis else {
        return Ok(None);
    };
    let up = prism.frame.z;
    if dot(up, across).abs() > 1.0e-10 {
        return Ok(None);
    }
    let along = unit(cross(up, across))?;
    let basis = [along, up, across];
    let mut ranges = [[f64::INFINITY, f64::NEG_INFINITY]; 3];
    for vertex in &host.topology.vertices {
        for axis in 0..3 {
            let value = dot(vertex.position, basis[axis]);
            ranges[axis][0] = ranges[axis][0].min(value);
            ranges[axis][1] = ranges[axis][1].max(value);
        }
    }
    let geometric = std::iter::once(host)
        .chain(cutters.iter())
        .map(|brep| brep.accuracy.geometric)
        .fold(0.0_f64, f64::max);
    let dimensions = ranges.map(|range| range[1] - range[0]);
    if dimensions.iter().any(|value| *value <= 4.0 * geometric)
        || host.topology.vertices.iter().any(|vertex| {
            (0..3).any(|axis| {
                let value = dot(vertex.position, basis[axis]);
                (value - ranges[axis][0]).abs() > geometric
                    && (value - ranges[axis][1]).abs() > geometric
            })
        })
    {
        return Ok(None);
    }
    let frame = Frame3 {
        origin: add(
            add(scale(along, ranges[0][0]), scale(up, ranges[1][0])),
            scale(across, ranges[2][0]),
        ),
        x: along,
        y: up,
        z: across,
    };
    frame.validate()?;
    let line_ring = |points: &[[f64; 2]]| -> Vec<CurveEdge2> {
        points
            .iter()
            .enumerate()
            .map(|(index, point)| CurveEdge2::Line {
                from: *point,
                to: points[(index + 1) % points.len()],
            })
            .collect()
    };
    let reverse_ring = |ring: &mut Vec<CurveEdge2>| {
        *ring = ring.iter().rev().map(CurveEdge2::reverse).collect();
    };
    let winding_ring = |mut ring: Vec<CurveEdge2>, positive: bool| {
        let area = ring.iter().map(CurveEdge2::twice_area).sum::<f64>();
        if (area > 0.0) != positive {
            reverse_ring(&mut ring);
        }
        ring
    };
    let host_region = CurveRegion2 {
        outer: line_ring(&[
            [0.0, 0.0],
            [0.0, dimensions[1]],
            [dimensions[0], dimensions[1]],
            [dimensions[0], 0.0],
        ]),
        holes: Vec::new(),
    };
    let mut cut_regions = Vec::with_capacity(cutters.len());
    for cutter in cutters {
        let vertices_cover_depth = || {
            let mut lo = f64::INFINITY;
            let mut hi = f64::NEG_INFINITY;
            for vertex in &cutter.topology.vertices {
                let value = frame.local(vertex.position)[2];
                lo = lo.min(value);
                hi = hi.max(value);
            }
            lo <= geometric && hi >= dimensions[2] - geometric
        };
        if !vertices_cover_depth() {
            return Ok(None);
        }
        if let Ok(input) = full_planar_extrusion(cutter) {
            if dot(input.frame.z, across).abs() < 1.0 - 1.0e-10 {
                return Ok(None);
            }
            let mut rings = input.contours.iter().map(|contour| {
                line_ring(
                    &contour
                        .iter()
                        .map(|point| {
                            let local = frame.local(*point);
                            [local[0], local[1]]
                        })
                        .collect::<Vec<_>>(),
                )
            });
            let outer = winding_ring(
                rings.next().ok_or_else(|| {
                    GeometryError::InvalidTopology("profile cutter has no outer ring".into())
                })?,
                false,
            );
            let holes = rings.map(|ring| winding_ring(ring, true)).collect();
            cut_regions.push(CurveRegion2 { outer, holes });
        } else if let Ok(input) = full_cylinder(cutter) {
            if dot(input.frame.z, across).abs() < 1.0 - 1.0e-10 {
                return Ok(None);
            }
            let center = frame.local(input.frame.origin);
            cut_regions.push(CurveRegion2 {
                outer: vec![
                    CurveEdge2::Arc {
                        center: [center[0], center[1]],
                        radius: input.radius,
                        start_angle: 0.0,
                        sweep_angle: -std::f64::consts::PI,
                    },
                    CurveEdge2::Arc {
                        center: [center[0], center[1]],
                        radius: input.radius,
                        start_angle: -std::f64::consts::PI,
                        sweep_angle: -std::f64::consts::PI,
                    },
                ],
                holes: Vec::new(),
            });
        } else {
            return Ok(None);
        }
    }
    let regions = boolean_curved_regions(
        &[host_region],
        &cut_regions,
        PlanarBooleanOp::Subtraction,
        geometric,
    )
    .map_err(|reason| {
        GeometryError::UnresolvedIntersection(format!("mixed profile batch arrangement: {reason}"))
    })?;
    if regions.is_empty() {
        return Ok(None);
    }
    let convert = |ring: Vec<CurveEdge2>| -> Vec<primitives::ProfileEdge> {
        ring.into_iter()
            .map(|edge| match edge {
                CurveEdge2::Line { from, to } => primitives::ProfileEdge::Line { from, to },
                CurveEdge2::Arc {
                    center,
                    radius,
                    start_angle,
                    sweep_angle,
                } => primitives::ProfileEdge::Arc {
                    center,
                    radius,
                    start_angle,
                    sweep_angle,
                },
            })
            .collect()
    };
    let accuracy = Accuracy {
        geometric,
        intersection: std::iter::once(host)
            .chain(cutters.iter())
            .map(|brep| brep.accuracy.intersection)
            .fold(0.0_f64, f64::max),
        tessellation: std::iter::once(host)
            .chain(cutters.iter())
            .map(|brep| brep.accuracy.tessellation)
            .fold(0.0_f64, f64::max),
        exchange: std::iter::once(host)
            .chain(cutters.iter())
            .map(|brep| brep.accuracy.exchange)
            .fold(0.0_f64, f64::max),
    };
    let single_region = regions.len() == 1;
    let mut parts = Vec::with_capacity(regions.len());
    for (index, region) in regions.into_iter().enumerate() {
        parts.push(primitives::arc_edged_extrusion_with_holes(
            if single_region {
                id.clone()
            } else {
                format!("{id}:part:{index}")
            },
            frame,
            convert(region.outer),
            region.holes.into_iter().map(convert).collect(),
            dimensions[2],
            accuracy,
        )?);
    }
    let mut out = if single_region {
        parts.pop().ok_or_else(|| {
            GeometryError::InvalidTopology("profile batch produced no material region".into())
        })?
    } else {
        let mut compound = BrepEnvelope::new(id, accuracy)?;
        for part in &parts {
            append_analytic_input(&mut compound, part)?;
        }
        compound
    };
    let inputs = std::iter::once(host)
        .chain(cutters.iter())
        .collect::<Vec<_>>();
    for result_face in &mut out.topology.faces {
        let surface = out.geometry.surface(result_face.surface)?;
        let mut sources = Vec::new();
        let cap = matches!(surface, SurfaceGeometry::Plane { frame: plane }
            if dot(plane.z, across).abs() > 1.0 - 1.0e-10);
        if cap {
            let position = dot(surface.frame().origin, across);
            for face in &host.topology.faces {
                let SurfaceGeometry::Plane {
                    frame: source_frame,
                } = host.geometry.surface(face.surface)?
                else {
                    continue;
                };
                if dot(source_frame.z, across).abs() > 1.0 - 1.0e-10
                    && (dot(source_frame.origin, across) - position).abs() <= geometric
                {
                    sources.push(brep_face_source(host, face.id));
                }
            }
        } else {
            let uv = result_face.trim.uv_bounds.map(Interval::midpoint);
            let sample = surface.point_at(uv)?;
            for input in &inputs {
                for face in &input.topology.faces {
                    let source_surface = input.geometry.surface(face.surface)?;
                    let on = match source_surface {
                        SurfaceGeometry::Plane {
                            frame: source_frame,
                        } => {
                            source_frame.local(sample)[2].abs() <= geometric
                                && dot(source_frame.z, across).abs() < 1.0 - 1.0e-10
                        }
                        SurfaceGeometry::Cylinder {
                            frame: source_frame,
                            radius,
                        } => {
                            let local = source_frame.local(sample);
                            (local[0].hypot(local[1]) - radius).abs() <= geometric
                        }
                        _ => false,
                    };
                    if !on {
                        continue;
                    }
                    let hint = face.trim.uv_bounds.map(Interval::midpoint);
                    let source_uv = source_surface.project(sample, Some(hint))?;
                    if face_contains_uv(input, face, source_uv)? == Some(true) {
                        sources.push(brep_face_source(input, face.id));
                    }
                }
            }
        }
        if sources.is_empty() {
            return Err(GeometryError::InvalidTopology(format!(
                "profile batch output face {} has no source",
                result_face.key
            )));
        }
        let cut = sources
            .iter()
            .any(|source| cutters.iter().any(|cutter| cutter.id == source.entity));
        result_face.provenance = FaceProvenance {
            sources: unique_sources(sources),
            role: if cut { FaceRole::Cut } else { FaceRole::Split },
            reversed: cut,
        };
    }
    out.revision = std::iter::once(host)
        .chain(cutters.iter())
        .map(|brep| brep.revision)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| GeometryError::LimitExceeded("boolean revision overflow".into()))?;
    out.validate()?;
    Ok(Some(BooleanResult {
        report: BooleanReport {
            operation: BooleanOp::Subtraction,
            quality: GeometryQuality::Analytic,
            contacts: Vec::new(),
            coincident: false,
            face_mappings: analytic_face_mappings(&out, inputs),
        },
        brep: out,
    }))
}

pub fn subtract_planar_cutters(
    host: &BrepEnvelope,
    cutters: &[BrepEnvelope],
    id: String,
) -> Result<BooleanResult, GeometryError> {
    if cutters.is_empty() || cutters.len() > 100 {
        return Err(GeometryError::InvalidGeometry(
            "planar batch subtraction requires between one and 100 cutters".into(),
        ));
    }
    if cutters.len() == 1 {
        return boolean_brep(host, &cutters[0], BooleanOp::Subtraction, id);
    }
    let mut accuracy = host.accuracy;
    let mut bounds = Vec::with_capacity(cutters.len());
    for cutter in cutters {
        cutter.validate()?;
        accuracy.geometric = accuracy.geometric.max(cutter.accuracy.geometric);
        accuracy.intersection = accuracy.intersection.max(cutter.accuracy.intersection);
        accuracy.tessellation = accuracy.tessellation.max(cutter.accuracy.tessellation);
        accuracy.exchange = accuracy.exchange.max(cutter.accuracy.exchange);
        bounds.push(
            cutter.bounds()?.ok_or_else(|| {
                GeometryError::InvalidGeometry("planar cutter has no bounds".into())
            })?,
        );
    }
    let planar = cutters
        .iter()
        .filter(|cutter| {
            cutter
                .geometry
                .surfaces
                .iter()
                .all(|surface| matches!(surface, SurfaceGeometry::Plane { .. }))
        })
        .collect::<Vec<_>>();
    let cylinders = if planar.len() == cutters.len() {
        Vec::new()
    } else {
        cutters
            .iter()
            .filter(|cutter| full_cylinder(cutter).is_ok())
            .collect::<Vec<_>>()
    };
    if !cylinders.is_empty() && planar.len() + cylinders.len() == cutters.len() {
        if let Some(result) = subtract_prismatic_profile_batch(host, cutters, id.clone())? {
            return Ok(result);
        }
    }
    if !planar.is_empty()
        && !cylinders.is_empty()
        && planar.len() + cylinders.len() == cutters.len()
    {
        // Reconstruct planar voids first, then subtract transverse cylinders.
        // Cutters may overlap outside the host, so their world bounds cannot
        // decide whether the host cut is valid. Each chained Boolean must
        // validate its exact B-rep or return a coverage error.
        let planar_cutters = planar.into_iter().cloned().collect::<Vec<_>>();
        let mut result = subtract_planar_cutters(host, &planar_cutters, format!("{id}:planar"))
            .map_err(|error| match error {
                GeometryError::CoverageGap { .. } => GeometryError::CoverageGap {
                    families: ["mixed cutter batch".into(), "planar stage".into()],
                },
                other => other,
            })?;
        let count = cylinders.len();
        for (index, cylinder) in cylinders.into_iter().enumerate() {
            let prior = result;
            let mut next = boolean_brep(
                &prior.brep,
                cylinder,
                BooleanOp::Subtraction,
                if index + 1 == count {
                    id.clone()
                } else {
                    format!("{id}:round-{index}")
                },
            )
            .map_err(|error| match error {
                GeometryError::CoverageGap { .. } => GeometryError::CoverageGap {
                    families: ["mixed cutter batch".into(), "cylindrical stage".into()],
                },
                other => other,
            })?;
            // The generic two-sided builder records its immediate input as
            // the source of every copied face. Replace that intermediate
            // reference with the face's original ancestry so a mixed batch
            // still identifies the authored host and planar cutter.
            for face in &mut next.brep.topology.faces {
                let mut lineage = Vec::new();
                for source in std::mem::take(&mut face.provenance.sources) {
                    if source.entity == prior.brep.id && source.body == prior.brep.id {
                        let previous = prior
                            .brep
                            .topology
                            .faces
                            .get(source.face as usize)
                            .ok_or_else(|| {
                                GeometryError::InvalidTopology(
                                    "mixed cut source face is missing".into(),
                                )
                            })?;
                        if previous.provenance.sources.is_empty() {
                            lineage.push(source);
                        } else {
                            lineage.extend(previous.provenance.sources.iter().cloned());
                        }
                    } else {
                        lineage.push(source);
                    }
                }
                face.provenance.sources = unique_sources(lineage);
            }
            next.brep.validate()?;
            result = next;
        }
        result.report.face_mappings =
            analytic_face_mappings(&result.brep, std::iter::once(host).chain(cutters.iter()));
        return Ok(result);
    }
    if host
        .geometry
        .surfaces
        .iter()
        .any(|surface| matches!(surface, SurfaceGeometry::Cylinder { .. }))
        && cutters.iter().all(|cutter| {
            cutter
                .geometry
                .surfaces
                .iter()
                .all(|surface| matches!(surface, SurfaceGeometry::Plane { .. }))
        })
    {
        match super::curved_layered_boolean::subtract_vertical_arc_extrusion_batch(
            host,
            &cutters.iter().collect::<Vec<_>>(),
            id.clone(),
        ) {
            Ok(result) => return Ok(result),
            Err(GeometryError::CoverageGap { .. }) => {}
            Err(error) => return Err(error),
        }
    }
    for first in 0..cutters.len() {
        for second in first + 1..cutters.len() {
            if (0..3).all(|axis| {
                bounds[first].axes[axis]
                    .hi
                    .min(bounds[second].axes[axis].hi)
                    - bounds[first].axes[axis]
                        .lo
                        .max(bounds[second].axes[axis].lo)
                    > 4.0 * accuracy.geometric
            }) {
                return Err(GeometryError::CoverageGap {
                    families: [
                        "overlapping planar batch cutters".into(),
                        "overlapping planar batch cutters".into(),
                    ],
                });
            }
        }
    }
    let mut combined = BrepEnvelope::new(format!("{id}:cutters"), accuracy)?;
    for cutter in cutters {
        append_analytic_input(&mut combined, cutter)?;
    }
    combined.validate()?;
    super::box_booleans::boolean_rectilinear(host, &combined, BooleanOp::Subtraction, id)
}

pub fn boolean_spheres(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    a.validate()?;
    b.validate()?;
    let family = |brep: &BrepEnvelope| {
        match brep.geometry.surfaces.first() {
            Some(SurfaceGeometry::Sphere { .. }) => "sphere (canonical full sphere required)",
            Some(SurfaceGeometry::Cylinder { .. }) => "cylinder",
            Some(SurfaceGeometry::Cone { .. }) => "cone",
            Some(SurfaceGeometry::Torus { .. }) => "torus",
            Some(SurfaceGeometry::Plane { .. }) => "planar/compound solid",
            None => "empty solid",
        }
        .to_string()
    };
    let coverage = |error| match error {
        GeometryError::CoverageGap { .. } => GeometryError::CoverageGap {
            families: [family(a), family(b)],
        },
        other => other,
    };
    let a = full_sphere(a).map_err(&coverage)?;
    let b = full_sphere(b).map_err(&coverage)?;
    let accuracy = Accuracy {
        geometric: a.brep.accuracy.geometric.max(b.brep.accuracy.geometric),
        intersection: a
            .brep
            .accuracy
            .intersection
            .max(b.brep.accuracy.intersection),
        tessellation: a
            .brep
            .accuracy
            .tessellation
            .max(b.brep.accuracy.tessellation),
        exchange: a.brep.accuracy.exchange.max(b.brep.accuracy.exchange),
    };
    let mut out = BrepEnvelope::new(id, accuracy)?;
    let delta = sub(b.frame.origin, a.frame.origin);
    let distance = norm(delta);
    let [outer, inner] =
        sphere_sphere_relation(a.frame.origin, a.radius, b.frame.origin, b.radius)?;
    let coincident = distance == 0.0 && a.radius == b.radius;
    let mut supports = GeometryStore::new();
    supports.surfaces = vec![
        SurfaceGeometry::Sphere {
            frame: a.frame,
            radius: a.radius,
        },
        SurfaceGeometry::Sphere {
            frame: b.frame,
            radius: b.radius,
        },
    ];
    let intersections = intersect_surfaces(&mut supports, 0, 1, accuracy)?;
    let mut keep = |input: &SphereInput<'_>| -> Result<(), GeometryError> {
        let shell = append_sphere(&mut out, input, false)?;
        out.solids.push(SolidRegion {
            outer_shell: shell,
            cavity_shells: Vec::new(),
        });
        Ok(())
    };
    if coincident {
        if operation != BooleanOp::Subtraction {
            keep(&a)?;
            out.topology.faces[0].provenance.role = FaceRole::Coincident;
            out.topology.faces[0].provenance.sources.push(source(&b));
        }
    } else if outer != Sign::Negative {
        match operation {
            BooleanOp::Union => {
                keep(&a)?;
                keep(&b)?;
            }
            BooleanOp::Intersection => {}
            BooleanOp::Subtraction => keep(&a)?,
        }
    } else if inner != Sign::Positive {
        let a_outer = a.radius > b.radius;
        match operation {
            BooleanOp::Union => keep(if a_outer { &a } else { &b })?,
            BooleanOp::Intersection => keep(if a_outer { &b } else { &a })?,
            BooleanOp::Subtraction if a_outer => {
                if inner == Sign::Zero {
                    return Err(GeometryError::UnresolvedIntersection(
                        "internally tangent cavity boundaries require contact topology".into(),
                    ));
                }
                if a.radius - b.radius - distance <= 4.0 * accuracy.geometric {
                    return Err(GeometryError::UnresolvedIntersection(
                        "cavity clearance is below geometric resolution".into(),
                    ));
                }
                let shell = append_sphere(&mut out, &a, false)?;
                let cavity = append_sphere(&mut out, &b, true)?;
                out.solids.push(SolidRegion {
                    outer_shell: shell,
                    cavity_shells: vec![cavity],
                });
            }
            BooleanOp::Subtraction => {}
        }
    } else {
        let intersection = intersections.curves.first().ok_or_else(|| {
            GeometryError::UnresolvedIntersection(
                "transverse spheres have no intersection branch".into(),
            )
        })?;
        let circle = supports.curves[intersection.curve as usize].clone();
        let (circle_frame, circle_radius) = match circle {
            CurveGeometry::Circle { frame, radius } => (frame, radius),
            _ => return Err(gap()),
        };
        let axis = unit(delta)?;
        let frame_a = Frame3 {
            origin: a.frame.origin,
            z: axis,
            x: circle_frame.x,
            y: circle_frame.y,
        };
        let frame_b = Frame3 {
            origin: b.frame.origin,
            ..frame_a
        };
        let latitudes = [(&a, frame_a), (&b, frame_b)].map(|(input, frame)| {
            let local = frame.local(circle_frame.origin);
            (local[2] / input.radius).asin()
        });
        if circle_radius <= 4.0 * accuracy.geometric || latitudes.iter().any(|v| !v.is_finite()) {
            return Err(GeometryError::UnresolvedIntersection(
                "intersection cap is below geometric resolution".into(),
            ));
        }
        let north_a = operation == BooleanOp::Intersection;
        let north_b = operation == BooleanOp::Union;
        for (input, latitude, north) in [(&a, latitudes[0], north_a), (&b, latitudes[1], north_b)] {
            let cap_height = input.radius
                * (1.0
                    - if north {
                        latitude.sin()
                    } else {
                        -latitude.sin()
                    });
            if cap_height <= 4.0 * accuracy.geometric {
                return Err(GeometryError::UnresolvedIntersection(
                    "retained spherical cap is below geometric resolution".into(),
                ));
            }
        }
        let mut builder = Builder::new(out.id.clone(), accuracy)?;
        let vertex = builder.vertex(circle.elementary_point(0.0)?);
        let edge = builder.edge(circle, Interval::new(0.0, std::f64::consts::TAU)?, false);
        patch(
            &mut builder,
            &a,
            frame_a,
            latitudes[0],
            north_a,
            edge,
            vertex,
            false,
        )?;
        patch(
            &mut builder,
            &b,
            frame_b,
            latitudes[1],
            north_b,
            edge,
            vertex,
            operation == BooleanOp::Subtraction,
        )?;
        out = builder.finish()?;
    }
    out.revision = a
        .brep
        .revision
        .max(b.brep.revision)
        .checked_add(1)
        .ok_or_else(|| GeometryError::LimitExceeded("boolean revision overflow".into()))?;
    out.validate()?;
    let report = BooleanReport {
        operation,
        quality: GeometryQuality::Analytic,
        contacts: intersections.contacts,
        coincident,
        face_mappings: [&a, &b]
            .map(|input| {
                let source = source(input);
                let result_faces = out
                    .topology
                    .faces
                    .iter()
                    .filter(|face| {
                        face.provenance.sources.iter().any(|s| {
                            s.entity == source.entity
                                && s.body == source.body
                                && s.key == source.key
                                && s.face == source.face
                        })
                    })
                    .map(|face| face.id)
                    .collect();
                FaceMapping {
                    source,
                    result_faces,
                }
            })
            .into(),
    };
    Ok(BooleanResult { brep: out, report })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytic::{
        geometry::{cross, dot},
        tessellation::tessellate,
    };
    fn sphere(id: &str, center: Point3, radius: f64) -> BrepEnvelope {
        primitives::sphere(
            id.into(),
            Frame3 {
                origin: center,
                ..Frame3::IDENTITY
            },
            radius,
            Accuracy {
                geometric: 1e-9,
                intersection: 1e-10,
                tessellation: 0.01,
                exchange: 1e-5,
            },
        )
        .unwrap()
    }
    fn cylinder(id: &str, origin_z: f64, height: f64, reverse: bool) -> BrepEnvelope {
        let frame = if reverse {
            Frame3 {
                origin: [0.0, 0.0, origin_z],
                x: [1.0, 0.0, 0.0],
                y: [0.0, -1.0, 0.0],
                z: [0.0, 0.0, -1.0],
            }
        } else {
            Frame3 {
                origin: [0.0, 0.0, origin_z],
                ..Frame3::IDENTITY
            }
        };
        primitives::cylinder(
            id.into(),
            frame,
            1.0,
            height,
            Accuracy {
                geometric: 1e-9,
                intersection: 1e-10,
                tessellation: 0.01,
                exchange: 1e-5,
            },
        )
        .unwrap()
    }
    fn volume(brep: &BrepEnvelope) -> f64 {
        volume_at_deflection(brep, 0.02)
    }
    fn volume_at_deflection(brep: &BrepEnvelope, deflection: f64) -> f64 {
        let mesh = tessellate(brep, deflection, 2_000_000).unwrap();
        mesh.indices
            .chunks_exact(3)
            .map(|ids| {
                let points: [Point3; 3] = std::array::from_fn(|i| {
                    let start = ids[i] as usize * 3;
                    [
                        mesh.positions[start],
                        mesh.positions[start + 1],
                        mesh.positions[start + 2],
                    ]
                });
                dot(points[0], cross(points[1], points[2])) / 6.0
            })
            .sum()
    }
    #[test]
    fn nonparallel_cylinder_through_cut_keeps_numerical_ssi_authoritative() {
        let accuracy = Accuracy {
            geometric: 4e-8,
            intersection: 1e-8,
            tessellation: 0.01,
            exchange: 1e-5,
        };
        let host = primitives::cylinder(
            "host".into(),
            Frame3 {
                origin: [0.0, 0.0, -2.0],
                ..Frame3::IDENTITY
            },
            1.0,
            4.0,
            accuracy,
        )
        .unwrap();
        let cutter = primitives::cylinder(
            "cutter".into(),
            Frame3::from_axis([-2.0, 0.0, 0.2], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap(),
            0.6,
            4.0,
            accuracy,
        )
        .unwrap();
        let result =
            boolean_brep(&host, &cutter, BooleanOp::Subtraction, "through-cut".into()).unwrap();
        result.brep.validate().unwrap();
        assert_eq!(result.brep.topology.faces.len(), 4);
        assert_eq!(result.brep.topology.faces[0].trim.holes.len(), 2);
        assert_eq!(result.brep.topology.faces[3].provenance.role, FaceRole::Cut);
        assert!(result.brep.topology.faces[3].provenance.reversed);
        assert_eq!(result.brep.geometry.intersections.len(), 2);
        assert_eq!(result.report.face_mappings.len(), 6);
        assert_eq!(
            super::super::query::classify_point(&result.brep, [0.0, 0.0, 0.2]).unwrap(),
            super::super::query::PointClassification::Outside
        );
        assert_eq!(
            super::super::query::classify_point(&result.brep, [0.0, 0.9, 0.2]).unwrap(),
            super::super::query::PointClassification::Inside
        );
        let mesh = tessellate(&result.brep, 0.0075, 2_000_000).unwrap();
        assert!(
            mesh.triangle_face_ids.len() < 50_000,
            "cross-drill tessellation produced {} triangles",
            mesh.triangle_face_ids.len()
        );
    }

    #[test]
    fn perpendicular_cylinders_cut_exact_round_holes_through_every_cuboid_axis() {
        let accuracy = Accuracy {
            geometric: 1.0e-9,
            intersection: 1.0e-10,
            tessellation: 0.01,
            exchange: 1.0e-5,
        };
        let size = [4.0, 3.0, 2.0];
        for axis in 0..3 {
            let host = primitives::cuboid("box".into(), Frame3::IDENTITY, size, accuracy).unwrap();
            let mut local_origin = [2.0, 1.5, 1.0];
            local_origin[axis] = -1.0;
            let direction = std::array::from_fn(|coordinate| f64::from(coordinate == axis));
            let reference =
                std::array::from_fn(|coordinate| f64::from(coordinate == (axis + 1) % 3));
            let cutter = primitives::cylinder(
                "round-opening".into(),
                Frame3::from_axis(local_origin, direction, reference).unwrap(),
                0.4,
                size[axis] + 2.0,
                accuracy,
            )
            .unwrap();

            let subtraction = boolean_brep(
                &host,
                &cutter,
                BooleanOp::Subtraction,
                format!("cut-{axis}"),
            )
            .unwrap();
            subtraction.brep.validate().unwrap();
            assert_eq!(subtraction.brep.topology.faces.len(), 7);
            assert_eq!(
                subtraction
                    .brep
                    .topology
                    .faces
                    .iter()
                    .filter(|face| !face.trim.holes.is_empty())
                    .count(),
                2
            );
            assert!(matches!(
                subtraction.brep.geometry.surfaces.last(),
                Some(SurfaceGeometry::Cylinder { .. })
            ));
            assert_eq!(
                super::super::query::classify_point(&subtraction.brep, [2.0, 1.5, 1.0]).unwrap(),
                super::super::query::PointClassification::Outside
            );
            assert_eq!(
                super::super::query::classify_point(&subtraction.brep, [0.2, 0.2, 0.2]).unwrap(),
                super::super::query::PointClassification::Inside
            );
            assert_eq!(subtraction.report.face_mappings.len(), 9);

            let union =
                boolean_brep(&host, &cutter, BooleanOp::Union, format!("union-{axis}")).unwrap();
            union.brep.validate().unwrap();
            assert_eq!(union.brep.solids.len(), 1);
            assert_eq!(union.brep.topology.faces.len(), 10);
            assert_eq!(
                union
                    .brep
                    .topology
                    .faces
                    .iter()
                    .filter(|face| !face.trim.holes.is_empty())
                    .count(),
                2
            );
            for coordinate in [-0.5, size[axis] + 0.5] {
                let mut point = [2.0, 1.5, 1.0];
                point[axis] = coordinate;
                assert_eq!(
                    super::super::query::classify_point(&union.brep, point).unwrap(),
                    super::super::query::PointClassification::Inside
                );
            }
            assert_eq!(
                super::super::query::classify_point(&union.brep, [0.2, 0.2, 0.2]).unwrap(),
                super::super::query::PointClassification::Inside
            );
            tessellate(&union.brep, 0.01, 2_000_000).unwrap();

            let intersection = boolean_brep(
                &host,
                &cutter,
                BooleanOp::Intersection,
                format!("intersection-{axis}"),
            )
            .unwrap();
            intersection.brep.validate().unwrap();
            assert_eq!(intersection.brep.topology.faces.len(), 3);
            assert_eq!(
                super::super::query::classify_point(&intersection.brep, [2.0, 1.5, 1.0]).unwrap(),
                super::super::query::PointClassification::Inside
            );
            assert_eq!(
                super::super::query::classify_point(&intersection.brep, [0.2, 0.2, 0.2]).unwrap(),
                super::super::query::PointClassification::Outside
            );

            let cutter_remainder = boolean_brep(
                &cutter,
                &host,
                BooleanOp::Subtraction,
                format!("cylinder-minus-box-{axis}"),
            )
            .unwrap();
            cutter_remainder.brep.validate().unwrap();
            assert_eq!(cutter_remainder.brep.solids.len(), 2);
            assert_eq!(cutter_remainder.brep.topology.faces.len(), 6);
            assert_eq!(
                cutter_remainder
                    .brep
                    .topology
                    .faces
                    .iter()
                    .filter(|face| face.provenance.role == FaceRole::Cut
                        && face.provenance.reversed)
                    .count(),
                2
            );
            for coordinate in [-0.5, size[axis] + 0.5] {
                let mut point = [2.0, 1.5, 1.0];
                point[axis] = coordinate;
                assert_eq!(
                    super::super::query::classify_point(&cutter_remainder.brep, point).unwrap(),
                    super::super::query::PointClassification::Inside
                );
            }
            assert_eq!(
                super::super::query::classify_point(&cutter_remainder.brep, [2.0, 1.5, 1.0])
                    .unwrap(),
                super::super::query::PointClassification::Outside
            );
        }
    }

    #[test]
    fn perpendicular_cylinders_cut_exact_blind_pockets_from_both_cuboid_sides() {
        let accuracy = Accuracy {
            geometric: 1.0e-9,
            intersection: 1.0e-10,
            tessellation: 0.01,
            exchange: 1.0e-5,
        };
        let size = [4.0, 3.0, 2.0];
        let pocket_depth = 0.75;
        for axis in 0..3 {
            for entry_from_low in [true, false] {
                let host =
                    primitives::cuboid("box".into(), Frame3::IDENTITY, size, accuracy).unwrap();
                let mut origin = [2.0, 1.5, 1.0];
                origin[axis] = if entry_from_low {
                    -1.0
                } else {
                    size[axis] + 1.0
                };
                let direction = std::array::from_fn(|coordinate| {
                    if coordinate == axis {
                        if entry_from_low {
                            1.0
                        } else {
                            -1.0
                        }
                    } else {
                        0.0
                    }
                });
                let reference =
                    std::array::from_fn(|coordinate| f64::from(coordinate == (axis + 1) % 3));
                let cutter = primitives::cylinder(
                    "pocket".into(),
                    Frame3::from_axis(origin, direction, reference).unwrap(),
                    0.4,
                    1.0 + pocket_depth,
                    accuracy,
                )
                .unwrap();

                let subtraction = boolean_brep(
                    &host,
                    &cutter,
                    BooleanOp::Subtraction,
                    format!("pocket-{axis}-{entry_from_low}"),
                )
                .unwrap();
                subtraction.brep.validate().unwrap();
                assert_eq!(subtraction.brep.topology.faces.len(), 8);
                assert_eq!(
                    subtraction
                        .brep
                        .topology
                        .faces
                        .iter()
                        .filter(|face| !face.trim.holes.is_empty())
                        .count(),
                    1
                );
                assert_eq!(
                    subtraction
                        .brep
                        .topology
                        .faces
                        .iter()
                        .filter(|face| face.provenance.role == FaceRole::Cut)
                        .count(),
                    2
                );
                assert!(subtraction
                    .brep
                    .topology
                    .faces
                    .iter()
                    .filter(|face| face.provenance.role == FaceRole::Cut)
                    .all(|face| face.provenance.reversed));

                let union = boolean_brep(
                    &host,
                    &cutter,
                    BooleanOp::Union,
                    format!("pocket-union-{axis}-{entry_from_low}"),
                )
                .unwrap();
                union.brep.validate().unwrap();
                assert_eq!(union.brep.solids.len(), 1);
                assert_eq!(union.brep.topology.faces.len(), 8);
                assert_eq!(
                    union
                        .brep
                        .topology
                        .faces
                        .iter()
                        .filter(|face| !face.trim.holes.is_empty())
                        .count(),
                    1
                );

                let mut void_point = [2.0, 1.5, 1.0];
                void_point[axis] = if entry_from_low {
                    pocket_depth / 2.0
                } else {
                    size[axis] - pocket_depth / 2.0
                };
                assert_eq!(
                    super::super::query::classify_point(&subtraction.brep, void_point).unwrap(),
                    super::super::query::PointClassification::Outside
                );
                let mut material_point = void_point;
                material_point[axis] = if entry_from_low {
                    pocket_depth + 0.25
                } else {
                    size[axis] - pocket_depth - 0.25
                };
                assert_eq!(
                    super::super::query::classify_point(&subtraction.brep, material_point).unwrap(),
                    super::super::query::PointClassification::Inside
                );

                let intersection = boolean_brep(
                    &host,
                    &cutter,
                    BooleanOp::Intersection,
                    format!("pocket-intersection-{axis}-{entry_from_low}"),
                )
                .unwrap();
                intersection.brep.validate().unwrap();
                assert_eq!(intersection.brep.topology.faces.len(), 3);
                assert_eq!(
                    super::super::query::classify_point(&intersection.brep, void_point).unwrap(),
                    super::super::query::PointClassification::Inside
                );
                assert_eq!(
                    super::super::query::classify_point(&intersection.brep, material_point)
                        .unwrap(),
                    super::super::query::PointClassification::Outside
                );

                let cutter_remainder = boolean_brep(
                    &cutter,
                    &host,
                    BooleanOp::Subtraction,
                    format!("pocket-cutter-minus-box-{axis}-{entry_from_low}"),
                )
                .unwrap();
                cutter_remainder.brep.validate().unwrap();
                assert_eq!(cutter_remainder.brep.solids.len(), 1);
                assert_eq!(cutter_remainder.brep.topology.faces.len(), 3);
                assert_eq!(
                    cutter_remainder
                        .brep
                        .topology
                        .faces
                        .iter()
                        .filter(|face| face.provenance.role == FaceRole::Cut
                            && face.provenance.reversed)
                        .count(),
                    1
                );
                let mut exterior_point = [2.0, 1.5, 1.0];
                exterior_point[axis] = if entry_from_low {
                    -0.5
                } else {
                    size[axis] + 0.5
                };
                assert_eq!(
                    super::super::query::classify_point(&cutter_remainder.brep, exterior_point)
                        .unwrap(),
                    super::super::query::PointClassification::Inside
                );
                assert_eq!(
                    super::super::query::classify_point(&union.brep, exterior_point).unwrap(),
                    super::super::query::PointClassification::Inside
                );
                assert_eq!(
                    super::super::query::classify_point(&union.brep, material_point).unwrap(),
                    super::super::query::PointClassification::Inside
                );
                tessellate(&union.brep, 0.01, 2_000_000).unwrap();
                assert_eq!(
                    super::super::query::classify_point(&cutter_remainder.brep, void_point)
                        .unwrap(),
                    super::super::query::PointClassification::Outside
                );
            }
        }
    }

    #[test]
    fn spheres_cut_exact_single_face_pockets_on_every_cuboid_side() {
        let accuracy = Accuracy {
            geometric: 1.0e-9,
            intersection: 1.0e-10,
            tessellation: 0.01,
            exchange: 1.0e-5,
        };
        let size = [4.0, 3.0, 2.0];
        let radius = 0.75;
        for axis in 0..3 {
            for upper in [false, true] {
                for center_inside in [false, true] {
                    let host =
                        primitives::cuboid("box".into(), Frame3::IDENTITY, size, accuracy).unwrap();
                    let mut center = [2.0, 1.5, 1.0];
                    center[axis] = match (upper, center_inside) {
                        (false, false) => -0.25,
                        (false, true) => 0.25,
                        (true, false) => size[axis] + 0.25,
                        (true, true) => size[axis] - 0.25,
                    };
                    let cutter = primitives::sphere(
                        "spherical-pocket".into(),
                        Frame3 {
                            origin: center,
                            ..Frame3::IDENTITY
                        },
                        radius,
                        accuracy,
                    )
                    .unwrap();

                    let subtraction = boolean_brep(
                        &host,
                        &cutter,
                        BooleanOp::Subtraction,
                        format!("sphere-pocket-{axis}-{upper}-{center_inside}"),
                    )
                    .unwrap();
                    subtraction.brep.validate().unwrap();
                    assert_eq!(subtraction.brep.topology.faces.len(), 7);
                    assert_eq!(
                        subtraction
                            .brep
                            .topology
                            .faces
                            .iter()
                            .filter(|face| !face.trim.holes.is_empty())
                            .count(),
                        1
                    );
                    assert_eq!(
                        subtraction
                            .brep
                            .topology
                            .faces
                            .iter()
                            .filter(|face| face.provenance.role == FaceRole::Cut
                                && face.provenance.reversed)
                            .count(),
                        1
                    );
                    assert_eq!(subtraction.report.face_mappings.len(), 7);

                    let inward_sign = if upper { -1.0 } else { 1.0 };
                    let mut void_point = center;
                    void_point[axis] = if upper { size[axis] - 0.25 } else { 0.25 };
                    let mut material_point = center;
                    material_point[axis] += inward_sign * (radius + 0.25);
                    let mut sphere_only_point = center;
                    sphere_only_point[axis] = if upper { size[axis] + 0.25 } else { -0.25 };
                    assert_eq!(
                        super::super::query::classify_point(&subtraction.brep, void_point).unwrap(),
                        super::super::query::PointClassification::Outside
                    );
                    assert_eq!(
                        super::super::query::classify_point(&subtraction.brep, material_point)
                            .unwrap(),
                        super::super::query::PointClassification::Inside
                    );

                    let intersection = boolean_brep(
                        &host,
                        &cutter,
                        BooleanOp::Intersection,
                        format!("sphere-intersection-{axis}-{upper}-{center_inside}"),
                    )
                    .unwrap();
                    intersection.brep.validate().unwrap();
                    assert_eq!(intersection.brep.topology.faces.len(), 2);
                    assert_eq!(
                        super::super::query::classify_point(&intersection.brep, void_point)
                            .unwrap(),
                        super::super::query::PointClassification::Inside
                    );
                    assert_eq!(
                        super::super::query::classify_point(&intersection.brep, material_point)
                            .unwrap(),
                        super::super::query::PointClassification::Outside
                    );

                    let union = boolean_brep(
                        &host,
                        &cutter,
                        BooleanOp::Union,
                        format!("sphere-union-{axis}-{upper}-{center_inside}"),
                    )
                    .unwrap();
                    union.brep.validate().unwrap();
                    assert_eq!(union.brep.topology.faces.len(), 7);
                    for point in [void_point, material_point, sphere_only_point] {
                        assert_eq!(
                            super::super::query::classify_point(&union.brep, point).unwrap(),
                            super::super::query::PointClassification::Inside
                        );
                    }

                    let sphere_cut = boolean_brep(
                        &cutter,
                        &host,
                        BooleanOp::Subtraction,
                        format!("sphere-cut-{axis}-{upper}-{center_inside}"),
                    )
                    .unwrap();
                    sphere_cut.brep.validate().unwrap();
                    assert_eq!(sphere_cut.brep.topology.faces.len(), 2);
                    assert!(sphere_cut.brep.topology.faces.iter().any(|face| {
                        face.provenance.role == FaceRole::Cut && face.provenance.reversed
                    }));
                    assert_eq!(
                        super::super::query::classify_point(&sphere_cut.brep, void_point).unwrap(),
                        super::super::query::PointClassification::Outside
                    );
                    assert_eq!(
                        super::super::query::classify_point(&sphere_cut.brep, sphere_only_point)
                            .unwrap(),
                        super::super::query::PointClassification::Inside
                    );
                    tessellate(&subtraction.brep, 0.01, 2_000_000).unwrap();
                    tessellate(&intersection.brep, 0.01, 2_000_000).unwrap();
                    tessellate(&union.brep, 0.01, 2_000_000).unwrap();
                    tessellate(&sphere_cut.brep, 0.01, 2_000_000).unwrap();
                }
            }
        }
    }

    #[test]
    fn coaxial_cylinder_booleans_split_material_and_keep_cut_cap_ancestry() {
        let a = cylinder("a", 0.0, 4.0, false);
        let b = cylinder("b", 3.0, 2.0, true);
        for (operation, solids, faces, expected_volume) in [
            (BooleanOp::Union, 1, 3, 4.0 * std::f64::consts::PI),
            (BooleanOp::Intersection, 1, 3, 2.0 * std::f64::consts::PI),
            (BooleanOp::Subtraction, 2, 6, 2.0 * std::f64::consts::PI),
        ] {
            let result = boolean_brep(&a, &b, operation, format!("{operation:?}")).unwrap();
            assert_eq!(result.brep.solids.len(), solids);
            assert_eq!(result.brep.topology.faces.len(), faces);
            let measured = volume(&result.brep).abs();
            assert!(
                (measured - expected_volume).abs() < 0.2,
                "{operation:?}: measured {measured}, expected {expected_volume}"
            );
            assert_eq!(result.report.face_mappings.len(), 6);
            if operation == BooleanOp::Subtraction {
                let cut_caps: Vec<_> = result
                    .brep
                    .topology
                    .faces
                    .iter()
                    .filter(|face| face.provenance.role == FaceRole::Cut)
                    .collect();
                assert_eq!(cut_caps.len(), 2);
                assert!(cut_caps.iter().all(|face| {
                    face.provenance.reversed
                        && face.provenance.sources.len() == 1
                        && face.provenance.sources[0].entity == "b"
                }));
            }
        }
    }

    #[test]
    fn coaxial_cylinder_contact_is_regularized_and_near_gap_is_unresolved() {
        let a = cylinder("a", 0.0, 4.0, false);
        let touching = cylinder("b", 4.0, 2.0, false);
        let union = boolean_brep(&a, &touching, BooleanOp::Union, "union".into()).unwrap();
        assert_eq!(union.brep.solids.len(), 1);
        let union_volume = volume(&union.brep).abs();
        assert!(
            (union_volume - 6.0 * std::f64::consts::PI).abs() < 0.3,
            "measured union volume {union_volume}"
        );
        let intersection = boolean_brep(
            &a,
            &touching,
            BooleanOp::Intersection,
            "intersection".into(),
        )
        .unwrap();
        assert!(intersection.brep.solids.is_empty());
        let subtraction =
            boolean_brep(&a, &touching, BooleanOp::Subtraction, "subtraction".into()).unwrap();
        assert_eq!(subtraction.brep.solids.len(), 1);
        let subtraction_volume = volume(&subtraction.brep).abs();
        assert!(
            (subtraction_volume - 4.0 * std::f64::consts::PI).abs() < 0.2,
            "measured subtraction volume {subtraction_volume}"
        );

        let near = cylinder("near", 4.0 + 2e-9, 2.0, false);
        assert!(matches!(
            boolean_brep(&a, &near, BooleanOp::Union, "near".into()),
            Err(GeometryError::UnresolvedIntersection(_))
        ));
    }

    #[test]
    fn positive_similarity_roundoff_does_not_reject_full_cylinder_operands() {
        let rotation = std::f64::consts::PI / 7.0;
        let source_frame = Frame3 {
            origin: [0.0, 1.2, 0.0],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 0.0, -1.0],
            z: [0.0, 1.0, 0.0],
        };
        let source = primitives::cylinder(
            "c".into(),
            source_frame,
            1.0,
            2.0,
            cylinder("accuracy", 0.0, 1.0, false).accuracy,
        )
        .unwrap();
        let placed = crate::analytic::placement::placed(
            &source,
            Frame3 {
                origin: [0.2, 0.1, 0.2],
                x: [rotation.cos(), 0.0, -rotation.sin()],
                y: [0.0, 1.0, 0.0],
                z: [rotation.sin(), 0.0, rotation.cos()],
            },
            1.25,
        )
        .unwrap();
        assert!(full_cylinder(&placed).is_ok());
    }

    #[test]
    fn translated_y_axis_cylinder_can_split_into_two_valid_solids() {
        let frame = Frame3 {
            origin: [0.0; 3],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 0.0, -1.0],
            z: [0.0, 1.0, 0.0],
        };
        let accuracy = cylinder("accuracy", 0.0, 1.0, false).accuracy;
        let host = primitives::cylinder("host".into(), frame, 1.0, 2.0, accuracy).unwrap();
        let cutter = primitives::cylinder("cutter".into(), frame, 1.0, 0.8, accuracy).unwrap();
        let cutter = crate::analytic::placement::placed(
            &cutter,
            Frame3 {
                origin: [0.0, 0.6, 0.0],
                ..Frame3::IDENTITY
            },
            1.0,
        )
        .unwrap();
        let result = boolean_brep(&host, &cutter, BooleanOp::Subtraction, "result".into()).unwrap();
        assert_eq!(result.brep.solids.len(), 2);
        result.brep.validate().unwrap();
        tessellate(&result.brep, 0.01, 2_000_000).unwrap();
    }
    #[test]
    fn coextensive_unequal_cylinders_produce_exact_radial_results() {
        let accuracy = cylinder("accuracy", 0.0, 1.0, false).accuracy;
        let outer =
            primitives::cylinder("outer".into(), Frame3::IDENTITY, 2.0, 3.0, accuracy).unwrap();
        let inner =
            primitives::cylinder("inner".into(), Frame3::IDENTITY, 1.0, 3.0, accuracy).unwrap();
        let union = boolean_brep(&outer, &inner, BooleanOp::Union, "union".into()).unwrap();
        let intersection = boolean_brep(
            &outer,
            &inner,
            BooleanOp::Intersection,
            "intersection".into(),
        )
        .unwrap();
        let subtraction =
            boolean_brep(&outer, &inner, BooleanOp::Subtraction, "tube".into()).unwrap();
        assert_eq!(union.brep.topology.faces.len(), 3);
        assert_eq!(intersection.brep.topology.faces.len(), 3);
        assert_eq!(subtraction.brep.topology.faces.len(), 4);
        assert_eq!(subtraction.brep.topology.faces[2].trim.holes.len(), 1);
        assert_eq!(subtraction.brep.topology.faces[3].trim.holes.len(), 1);
        assert_eq!(
            subtraction.brep.topology.faces[1].provenance.role,
            FaceRole::Cut
        );
        assert!(subtraction.brep.topology.faces[1].provenance.reversed);
        assert_eq!(subtraction.report.face_mappings.len(), 6);
        let measured = volume(&subtraction.brep).abs();
        let expected = std::f64::consts::PI * (4.0 - 1.0) * 3.0;
        assert!(
            (measured - expected).abs() < 0.2,
            "{measured} != {expected}"
        );
        subtraction.brep.validate().unwrap();
        tessellate(&subtraction.brep, 0.01, 2_000_000).unwrap();

        let empty = boolean_brep(&inner, &outer, BooleanOp::Subtraction, "empty".into()).unwrap();
        assert!(empty.brep.solids.is_empty());
        let shifted = primitives::cylinder(
            "shifted".into(),
            Frame3 {
                origin: [0.0, 0.0, 0.5],
                ..Frame3::IDENTITY
            },
            1.0,
            3.0,
            accuracy,
        )
        .unwrap();
        assert!(matches!(
            boolean_brep(&outer, &shifted, BooleanOp::Subtraction, "gap".into()),
            Err(GeometryError::CoverageGap { .. })
        ));
    }
    #[test]
    fn enclosed_unequal_cylinder_becomes_an_oriented_cavity_shell() {
        let accuracy = cylinder("accuracy", 0.0, 1.0, false).accuracy;
        let host =
            primitives::cylinder("host".into(), Frame3::IDENTITY, 2.0, 4.0, accuracy).unwrap();
        let cutter = primitives::cylinder(
            "cutter".into(),
            Frame3 {
                origin: [0.0, 0.0, 1.0],
                ..Frame3::IDENTITY
            },
            0.75,
            2.0,
            accuracy,
        )
        .unwrap();
        let subtraction =
            boolean_brep(&host, &cutter, BooleanOp::Subtraction, "cavity".into()).unwrap();
        assert_eq!(subtraction.brep.solids.len(), 1);
        assert_eq!(subtraction.brep.topology.shells.len(), 2);
        assert_eq!(subtraction.brep.solids[0].cavity_shells, vec![1]);
        assert_eq!(subtraction.brep.topology.faces.len(), 6);
        assert!(subtraction.brep.topology.faces[3..]
            .iter()
            .all(|face| face.provenance.role == FaceRole::Cut && face.provenance.reversed));
        let measured = volume(&subtraction.brep).abs();
        let expected = std::f64::consts::PI * (2.0_f64.powi(2) * 4.0 - 0.75_f64.powi(2) * 2.0);
        assert!(
            (measured - expected).abs() < 0.3,
            "{measured} != {expected}"
        );
        subtraction.brep.validate().unwrap();
        tessellate(&subtraction.brep, 0.01, 2_000_000).unwrap();

        let union = boolean_brep(&host, &cutter, BooleanOp::Union, "union".into()).unwrap();
        let intersection = boolean_brep(
            &host,
            &cutter,
            BooleanOp::Intersection,
            "intersection".into(),
        )
        .unwrap();
        assert_eq!(union.brep.topology.faces.len(), 3);
        assert_eq!(intersection.brep.topology.faces.len(), 3);
        let empty = boolean_brep(&cutter, &host, BooleanOp::Subtraction, "empty".into()).unwrap();
        assert!(empty.brep.solids.is_empty());
    }
    #[test]
    fn transverse_parallel_cylinders_build_two_arc_caps_and_shared_edges() {
        let accuracy = cylinder("accuracy", 0.0, 1.0, false).accuracy;
        let a = primitives::cylinder("a".into(), Frame3::IDENTITY, 1.0, 2.0, accuracy).unwrap();
        let b = primitives::cylinder(
            "b".into(),
            Frame3 {
                origin: [1.0, 0.0, 0.0],
                ..Frame3::IDENTITY
            },
            1.0,
            2.0,
            accuracy,
        )
        .unwrap();
        let union = boolean_brep(&a, &b, BooleanOp::Union, "union".into()).unwrap();
        let intersection =
            boolean_brep(&a, &b, BooleanOp::Intersection, "intersection".into()).unwrap();
        let subtraction =
            boolean_brep(&a, &b, BooleanOp::Subtraction, "subtraction".into()).unwrap();
        for (name, result) in [
            ("union", &union),
            ("intersection", &intersection),
            ("subtraction", &subtraction),
        ] {
            assert_eq!(result.brep.solids.len(), 1, "{name}");
            assert_eq!(result.brep.topology.faces.len(), 4, "{name}");
            for cap in 2..4 {
                assert_eq!(
                    result
                        .brep
                        .topology
                        .halfedges
                        .iter()
                        .filter(|halfedge| halfedge.face == Some(cap))
                        .count(),
                    2,
                    "{name} cap {cap}"
                );
            }
            result.brep.validate().unwrap();
            assert!(!tessellate(&result.brep, 0.01, 2_000_000)
                .unwrap_or_else(|error| panic!("{name}: {error}"))
                .indices
                .is_empty());
            crate::analytic::exchange::export_step(&result.brep, "metre")
                .unwrap_or_else(|error| panic!("{name} STEP: {error}"));
            crate::analytic::ifc_exchange::prepare_ifc_body(&result.brep)
                .unwrap_or_else(|error| panic!("{name} IFC: {error}"));
        }
        assert_eq!(union.report.face_mappings.len(), 6);
        assert_eq!(intersection.report.face_mappings.len(), 6);
        assert_eq!(subtraction.report.face_mappings.len(), 6);
        assert_eq!(
            subtraction.brep.topology.faces[1].provenance.role,
            FaceRole::Cut
        );
        assert!(subtraction.brep.topology.faces[1].provenance.reversed);

        let cylinder_volume = 2.0 * std::f64::consts::PI;
        let intersection_volume = volume(&intersection.brep).abs();
        assert!(
            (volume(&union.brep).abs() + intersection_volume - 2.0 * cylinder_volume).abs() < 0.2
        );
        assert!(
            (volume(&subtraction.brep).abs() + intersection_volume - cylinder_volume).abs() < 0.2
        );

        let reversed = primitives::cylinder(
            "reversed".into(),
            Frame3 {
                origin: [1.0, 0.0, 2.0],
                x: [1.0, 0.0, 0.0],
                y: [0.0, -1.0, 0.0],
                z: [0.0, 0.0, -1.0],
            },
            1.0,
            2.0,
            accuracy,
        )
        .unwrap();
        let reversed_result = boolean_brep(
            &a,
            &reversed,
            BooleanOp::Intersection,
            "reversed-result".into(),
        )
        .unwrap();
        reversed_result.brep.validate().unwrap();
        assert!((volume(&reversed_result.brep).abs() - intersection_volume).abs() < 0.1);

        let shifted = primitives::cylinder(
            "shifted".into(),
            Frame3 {
                origin: [1.0, 0.0, 0.25],
                ..Frame3::IDENTITY
            },
            1.0,
            2.0,
            accuracy,
        )
        .unwrap();
        let shifted_union = boolean_brep(&a, &shifted, BooleanOp::Union, "gap".into());
        assert!(
            matches!(shifted_union, Err(GeometryError::UnresolvedIntersection(_))),
            "unexpected shifted-cylinder result: {:?}",
            shifted_union.err()
        );
        let tangent = primitives::cylinder(
            "tangent".into(),
            Frame3 {
                origin: [2.0, 0.0, 0.0],
                ..Frame3::IDENTITY
            },
            1.0,
            2.0,
            accuracy,
        )
        .unwrap();
        for (operation, solids) in [
            (BooleanOp::Union, 2),
            (BooleanOp::Intersection, 0),
            (BooleanOp::Subtraction, 1),
        ] {
            let contact =
                boolean_brep(&a, &tangent, operation, format!("contact-{operation:?}")).unwrap();
            assert_eq!(contact.brep.solids.len(), solids);
            assert_eq!(contact.report.contacts.len(), 2);
        }
        let near_tangent = primitives::cylinder(
            "near-tangent".into(),
            Frame3 {
                origin: [2.0 + 2e-9, 0.0, 0.0],
                ..Frame3::IDENTITY
            },
            1.0,
            2.0,
            accuracy,
        )
        .unwrap();
        assert!(matches!(
            boolean_brep(
                &a,
                &near_tangent,
                BooleanOp::Union,
                "near-tangent-result".into()
            ),
            Err(GeometryError::UnresolvedIntersection(_))
        ));
        let diagonal = primitives::cylinder(
            "diagonal".into(),
            Frame3 {
                origin: [1.5, 1.5, 0.0],
                ..Frame3::IDENTITY
            },
            1.0,
            2.0,
            accuracy,
        )
        .unwrap();
        let diagonal_union =
            boolean_brep(&a, &diagonal, BooleanOp::Union, "diagonal-union".into()).unwrap();
        assert_eq!(diagonal_union.brep.solids.len(), 2);
        assert!(diagonal_union.report.contacts.is_empty());
    }
    #[test]
    fn contained_parallel_cylinder_builds_an_eccentric_analytic_through_hole() {
        let accuracy = cylinder("accuracy", 0.0, 1.0, false).accuracy;
        let outer =
            primitives::cylinder("outer".into(), Frame3::IDENTITY, 2.0, 2.0, accuracy).unwrap();
        let inner = primitives::cylinder(
            "inner".into(),
            Frame3 {
                origin: [0.75, 0.0, 0.0],
                ..Frame3::IDENTITY
            },
            0.5,
            2.0,
            accuracy,
        )
        .unwrap();
        let union = boolean_brep(&outer, &inner, BooleanOp::Union, "union".into()).unwrap();
        let intersection = boolean_brep(
            &outer,
            &inner,
            BooleanOp::Intersection,
            "intersection".into(),
        )
        .unwrap();
        let subtraction =
            boolean_brep(&outer, &inner, BooleanOp::Subtraction, "subtraction".into()).unwrap();
        assert_eq!(union.brep.topology.faces.len(), 3);
        assert_eq!(intersection.brep.topology.faces.len(), 3);
        assert_eq!(subtraction.brep.topology.faces.len(), 4);
        assert_eq!(subtraction.brep.topology.faces[2].trim.holes.len(), 1);
        assert_eq!(subtraction.brep.topology.faces[3].trim.holes.len(), 1);
        assert_eq!(
            subtraction.brep.topology.faces[1].provenance.role,
            FaceRole::Cut
        );
        assert!(subtraction.brep.topology.faces[1].provenance.reversed);
        match subtraction.brep.geometry.surfaces[1] {
            SurfaceGeometry::Cylinder { frame, radius } => {
                assert_eq!(frame.origin, [0.75, 0.0, 0.0]);
                assert_eq!(radius, 0.5);
            }
            _ => panic!("inner cut face lost its cylinder support"),
        }
        subtraction.brep.validate().unwrap();
        tessellate(&subtraction.brep, 0.01, 2_000_000).unwrap();
        crate::analytic::exchange::export_step(&subtraction.brep, "metre").unwrap();
        crate::analytic::ifc_exchange::prepare_ifc_body(&subtraction.brep).unwrap();
        let expected = std::f64::consts::PI * (4.0 - 0.25) * 2.0;
        assert!((volume(&subtraction.brep).abs() - expected).abs() < 0.25);

        let touching = primitives::cylinder(
            "touching".into(),
            Frame3 {
                origin: [1.5, 0.0, 0.0],
                ..Frame3::IDENTITY
            },
            0.5,
            2.0,
            accuracy,
        )
        .unwrap();
        assert!(matches!(
            boolean_brep(
                &outer,
                &touching,
                BooleanOp::Subtraction,
                "touching-result".into()
            ),
            Err(GeometryError::UnresolvedIntersection(_))
        ));
    }
    #[test]
    fn noncoextensive_parallel_cylinders_intersect_and_cut_when_the_span_is_bounded() {
        let accuracy = cylinder("accuracy", 0.0, 1.0, false).accuracy;
        let host =
            primitives::cylinder("host".into(), Frame3::IDENTITY, 1.0, 3.0, accuracy).unwrap();
        let short = primitives::cylinder(
            "short".into(),
            Frame3 {
                origin: [1.0, 0.0, 1.0],
                ..Frame3::IDENTITY
            },
            1.0,
            1.0,
            accuracy,
        )
        .unwrap();
        let intersection = boolean_brep(
            &host,
            &short,
            BooleanOp::Intersection,
            "intersection".into(),
        )
        .unwrap();
        assert_eq!(intersection.brep.topology.faces.len(), 4);
        assert_eq!(intersection.brep.solids.len(), 1);
        assert!(intersection.brep.topology.faces[2..].iter().all(|face| face
            .provenance
            .sources
            .len()
            == 1
            && face.provenance.sources[0].entity == "short"));
        intersection.brep.validate().unwrap();
        tessellate(&intersection.brep, 0.01, 2_000_000).unwrap();
        let lens_area = 2.0 * std::f64::consts::FRAC_PI_3 - 3.0_f64.sqrt() * 0.5;
        assert!((volume(&intersection.brep).abs() - lens_area).abs() < 0.12);
        let union = boolean_brep(&host, &short, BooleanOp::Union, "partial-union".into()).unwrap();
        let reversed_union = boolean_brep(
            &short,
            &host,
            BooleanOp::Union,
            "reversed-partial-union".into(),
        )
        .unwrap();
        let subtraction = boolean_brep(
            &host,
            &short,
            BooleanOp::Subtraction,
            "partial-subtraction".into(),
        )
        .unwrap();
        for (name, result) in [("union", &union), ("subtraction", &subtraction)] {
            assert_eq!(result.brep.topology.faces.len(), 10, "{name}");
            assert_eq!(result.brep.solids.len(), 1, "{name}");
            result.brep.validate().unwrap();
            tessellate(&result.brep, 0.01, 2_000_000)
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            crate::analytic::exchange::export_step(&result.brep, "metre")
                .unwrap_or_else(|error| panic!("{name} STEP: {error}"));
            crate::analytic::ifc_exchange::prepare_ifc_body(&result.brep)
                .unwrap_or_else(|error| panic!("{name} IFC: {error}"));
        }
        assert_eq!(
            subtraction.brep.topology.faces[3].provenance.role,
            FaceRole::Cut
        );
        assert!(subtraction.brep.topology.faces[3].provenance.reversed);
        assert!(subtraction.brep.topology.faces[7..9]
            .iter()
            .all(|face| face.provenance.role == FaceRole::Cut && face.provenance.reversed));
        let host_volume = 3.0 * std::f64::consts::PI;
        assert!(
            (volume(&union.brep).abs() - (host_volume + std::f64::consts::PI - lens_area)).abs()
                < 0.2
        );
        assert!((volume(&subtraction.brep).abs() - (host_volume - lens_area)).abs() < 0.2);
        assert!((volume(&reversed_union.brep).abs() - volume(&union.brep).abs()).abs() < 0.1);
        reversed_union.brep.validate().unwrap();

        let through = primitives::cylinder(
            "through".into(),
            Frame3 {
                origin: [1.0, 0.0, -1.0],
                ..Frame3::IDENTITY
            },
            1.0,
            5.0,
            accuracy,
        )
        .unwrap();
        let cut = boolean_brep(
            &host,
            &through,
            BooleanOp::Subtraction,
            "through-cut".into(),
        )
        .unwrap();
        assert_eq!(cut.brep.topology.faces.len(), 4);
        assert_eq!(cut.brep.topology.faces[1].provenance.role, FaceRole::Cut);
        assert!(cut.brep.topology.faces[1].provenance.reversed);
        assert!(!cut.report.coincident);
        cut.brep.validate().unwrap();
        tessellate(&cut.brep, 0.01, 2_000_000).unwrap();
        let expected_cut_volume = (std::f64::consts::PI - lens_area) * 3.0;
        assert!((volume(&cut.brep).abs() - expected_cut_volume).abs() < 0.2);
    }
    #[test]
    fn mixed_sphere_cylinder_containment_preserves_exact_cavity_supports() {
        let accuracy = cylinder("accuracy", 0.0, 1.0, false).accuracy;
        let cylinder_host =
            primitives::cylinder("cylinder".into(), Frame3::IDENTITY, 2.0, 4.0, accuracy).unwrap();
        let sphere_cutter = primitives::sphere(
            "sphere".into(),
            Frame3 {
                origin: [0.0, 0.0, 2.0],
                ..Frame3::IDENTITY
            },
            0.5,
            accuracy,
        )
        .unwrap();
        let cylindrical_cavity = boolean_brep(
            &cylinder_host,
            &sphere_cutter,
            BooleanOp::Subtraction,
            "cylindrical-cavity".into(),
        )
        .unwrap();
        assert_eq!(cylindrical_cavity.brep.solids.len(), 1);
        assert_eq!(cylindrical_cavity.brep.topology.shells.len(), 2);
        assert_eq!(cylindrical_cavity.brep.solids[0].cavity_shells, vec![1]);
        assert_eq!(cylindrical_cavity.brep.topology.faces.len(), 4);
        assert_eq!(
            cylindrical_cavity.brep.topology.faces[3].provenance.role,
            FaceRole::Cut
        );
        assert!(
            cylindrical_cavity.brep.topology.faces[3]
                .provenance
                .reversed
        );
        cylindrical_cavity.brep.validate().unwrap();
        tessellate(&cylindrical_cavity.brep, 0.01, 2_000_000).unwrap();

        let sphere_host =
            primitives::sphere("sphere-host".into(), Frame3::IDENTITY, 3.0, accuracy).unwrap();
        let cylinder_cutter = primitives::cylinder(
            "cylinder-cutter".into(),
            Frame3 {
                origin: [0.0, 0.0, -0.5],
                ..Frame3::IDENTITY
            },
            0.5,
            1.0,
            accuracy,
        )
        .unwrap();
        let spherical_cavity = boolean_brep(
            &sphere_host,
            &cylinder_cutter,
            BooleanOp::Subtraction,
            "spherical-cavity".into(),
        )
        .unwrap();
        assert_eq!(spherical_cavity.brep.topology.faces.len(), 4);
        assert_eq!(spherical_cavity.brep.solids[0].cavity_shells, vec![1]);
        assert!(spherical_cavity.brep.topology.faces[1..]
            .iter()
            .all(|face| face.provenance.role == FaceRole::Cut && face.provenance.reversed));
        spherical_cavity.brep.validate().unwrap();
        tessellate(&spherical_cavity.brep, 0.01, 2_000_000).unwrap();

        let union = boolean_brep(
            &cylinder_host,
            &sphere_cutter,
            BooleanOp::Union,
            "union".into(),
        )
        .unwrap();
        let intersection = boolean_brep(
            &cylinder_host,
            &sphere_cutter,
            BooleanOp::Intersection,
            "intersection".into(),
        )
        .unwrap();
        assert_eq!(union.brep.topology.faces.len(), 3);
        assert_eq!(intersection.brep.topology.faces.len(), 1);
        let crossing = primitives::sphere(
            "crossing".into(),
            Frame3 {
                origin: [1.8, 0.0, 2.0],
                ..Frame3::IDENTITY
            },
            0.5,
            accuracy,
        )
        .unwrap();
        for (name, left, right, operation) in [
            (
                "crossing-union",
                &cylinder_host,
                &crossing,
                BooleanOp::Union,
            ),
            (
                "crossing-intersection",
                &cylinder_host,
                &crossing,
                BooleanOp::Intersection,
            ),
            (
                "crossing-cylinder-cut",
                &cylinder_host,
                &crossing,
                BooleanOp::Subtraction,
            ),
            (
                "crossing-sphere-cut",
                &crossing,
                &cylinder_host,
                BooleanOp::Subtraction,
            ),
        ] {
            let result = boolean_brep(left, right, operation, name.into())
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            assert!(
                matches!(result.report.quality, GeometryQuality::Analytic),
                "{name}"
            );
            assert_eq!(result.brep.solids.len(), 1, "{name}");
            assert!(result.brep.topology.faces.len() >= 2, "{name}");
            assert!(!result.brep.topology.edges.is_empty(), "{name}");
            assert!(
                result
                    .brep
                    .topology
                    .faces
                    .iter()
                    .all(|face| !face.provenance.sources.is_empty()),
                "{name}"
            );
            assert!(result
                .brep
                .geometry
                .curves
                .iter()
                .any(|curve| matches!(curve, CurveGeometry::Intersection { .. })));
            result.brep.validate().unwrap();
            tessellate(&result.brep, 0.01, 2_000_000)
                .unwrap_or_else(|error| panic!("{name} tessellation: {error}"));
        }
    }
    #[test]
    fn coaxial_sphere_cylinder_intersection_builds_shared_analytic_circles() {
        let accuracy = cylinder("accuracy", 0.0, 1.0, false).accuracy;
        let sphere = primitives::sphere("sphere".into(), Frame3::IDENTITY, 1.5, accuracy).unwrap();
        let cylinder = primitives::cylinder(
            "cylinder".into(),
            Frame3 {
                origin: [0.0, 0.0, -2.0],
                ..Frame3::IDENTITY
            },
            0.75,
            4.0,
            accuracy,
        )
        .unwrap();
        let union = boolean_brep(&sphere, &cylinder, BooleanOp::Union, "union".into()).unwrap();
        let intersection = boolean_brep(
            &sphere,
            &cylinder,
            BooleanOp::Intersection,
            "intersection".into(),
        )
        .unwrap();
        let sphere_cut = boolean_brep(
            &sphere,
            &cylinder,
            BooleanOp::Subtraction,
            "sphere-cut".into(),
        )
        .unwrap();
        let cylinder_cut = boolean_brep(
            &cylinder,
            &sphere,
            BooleanOp::Subtraction,
            "cylinder-cut".into(),
        )
        .unwrap();
        assert_eq!(union.brep.topology.faces.len(), 5);
        assert_eq!(intersection.brep.topology.faces.len(), 3);
        assert_eq!(sphere_cut.brep.topology.faces.len(), 2);
        assert_eq!(cylinder_cut.brep.topology.faces.len(), 6);
        assert_eq!(cylinder_cut.brep.solids.len(), 2);
        assert!(sphere_cut
            .brep
            .topology
            .faces
            .iter()
            .any(|face| face.provenance.role == FaceRole::Cut && face.provenance.reversed));
        assert_eq!(
            cylinder_cut
                .brep
                .topology
                .faces
                .iter()
                .filter(|face| face.provenance.role == FaceRole::Cut && face.provenance.reversed)
                .count(),
            2
        );
        for (name, result) in [
            ("union", &union),
            ("intersection", &intersection),
            ("sphere-cut", &sphere_cut),
            ("cylinder-cut", &cylinder_cut),
        ] {
            result.brep.validate().unwrap();
            let mesh = tessellate(&result.brep, 0.01, 2_000_000)
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            assert!(mesh.indices.len() > 0);
            assert_eq!(result.report.face_mappings.len(), 4);
            crate::analytic::exchange::export_step(&result.brep, "metre")
                .unwrap_or_else(|error| panic!("{name} STEP: {error}"));
            crate::analytic::ifc_exchange::prepare_ifc_body(&result.brep)
                .unwrap_or_else(|error| panic!("{name} IFC: {error}"));
        }
        let sphere_volume = 4.0 * std::f64::consts::PI * 1.5_f64.powi(3) / 3.0;
        let cylinder_volume = std::f64::consts::PI * 0.75_f64.powi(2) * 4.0;
        let intersection_volume = volume(&intersection.brep).abs();
        assert!(
            (volume(&union.brep).abs() + intersection_volume - sphere_volume - cylinder_volume)
                .abs()
                < 0.3
        );
        assert!((volume(&sphere_cut.brep).abs() + intersection_volume - sphere_volume).abs() < 0.3);
        assert!(
            (volume(&cylinder_cut.brep).abs() + intersection_volume - cylinder_volume).abs() < 0.3
        );

        let noncoaxial = primitives::cylinder(
            "noncoaxial".into(),
            Frame3 {
                origin: [0.2, 0.0, -2.0],
                ..Frame3::IDENTITY
            },
            0.75,
            4.0,
            accuracy,
        )
        .unwrap();
        assert!(matches!(
            boolean_brep(&sphere, &noncoaxial, BooleanOp::Union, "gap".into()),
            Err(GeometryError::CoverageGap { .. })
        ));
    }
    #[test]
    fn mixed_cuboid_containment_builds_exact_curved_and_planar_cavities() {
        let accuracy = cylinder("accuracy", 0.0, 1.0, false).accuracy;
        let box_host = primitives::cuboid(
            "box".into(),
            Frame3 {
                origin: [-2.0, -2.0, -2.0],
                ..Frame3::IDENTITY
            },
            [4.0; 3],
            accuracy,
        )
        .unwrap();
        let sphere_cutter =
            primitives::sphere("sphere".into(), Frame3::IDENTITY, 0.5, accuracy).unwrap();
        let cylinder_cutter = primitives::cylinder(
            "cylinder".into(),
            Frame3 {
                origin: [0.0, 0.0, -0.5],
                ..Frame3::IDENTITY
            },
            0.5,
            1.0,
            accuracy,
        )
        .unwrap();
        for (name, cutter, expected_faces) in [
            ("sphere", &sphere_cutter, 7),
            ("cylinder", &cylinder_cutter, 9),
        ] {
            let result = boolean_brep(
                &box_host,
                cutter,
                BooleanOp::Subtraction,
                format!("{name}-cavity"),
            )
            .unwrap();
            assert_eq!(result.brep.solids.len(), 1);
            assert_eq!(result.brep.topology.shells.len(), 2);
            assert_eq!(result.brep.solids[0].cavity_shells, vec![1]);
            assert_eq!(result.brep.topology.faces.len(), expected_faces);
            assert!(result.brep.topology.faces[6..]
                .iter()
                .all(|face| face.provenance.role == FaceRole::Cut && face.provenance.reversed));
            result.brep.validate().unwrap();
            tessellate(&result.brep, 0.01, 2_000_000).unwrap();
        }

        let sphere_host =
            primitives::sphere("sphere-host".into(), Frame3::IDENTITY, 4.0, accuracy).unwrap();
        let small_box = primitives::cuboid(
            "small-box".into(),
            Frame3 {
                origin: [-0.5; 3],
                ..Frame3::IDENTITY
            },
            [1.0; 3],
            accuracy,
        )
        .unwrap();
        let sphere_box = boolean_brep(
            &sphere_host,
            &small_box,
            BooleanOp::Subtraction,
            "sphere-box".into(),
        )
        .unwrap();
        assert_eq!(sphere_box.brep.topology.faces.len(), 7);
        assert_eq!(sphere_box.brep.solids[0].cavity_shells, vec![1]);

        let cylinder_host = primitives::cylinder(
            "cylinder-host".into(),
            Frame3 {
                origin: [0.0, 0.0, -2.0],
                ..Frame3::IDENTITY
            },
            3.0,
            4.0,
            accuracy,
        )
        .unwrap();
        let cylinder_box = boolean_brep(
            &cylinder_host,
            &small_box,
            BooleanOp::Subtraction,
            "cylinder-box".into(),
        )
        .unwrap();
        assert_eq!(cylinder_box.brep.topology.faces.len(), 9);
        assert_eq!(cylinder_box.brep.solids[0].cavity_shells, vec![1]);

        let crossing = primitives::sphere(
            "crossing".into(),
            Frame3 {
                origin: [1.8, 0.0, 0.0],
                ..Frame3::IDENTITY
            },
            0.5,
            accuracy,
        )
        .unwrap();
        let crossing_union = boolean_brep(
            &box_host,
            &crossing,
            BooleanOp::Union,
            "crossing-union".into(),
        )
        .unwrap();
        assert_eq!(crossing_union.brep.topology.faces.len(), 7);
        assert_eq!(
            crossing_union
                .brep
                .topology
                .faces
                .iter()
                .filter(|face| !face.trim.holes.is_empty())
                .count(),
            1
        );
        crossing_union.brep.validate().unwrap();
        tessellate(&crossing_union.brep, 0.01, 2_000_000).unwrap();
    }
    #[test]
    fn transverse_spheres_preserve_surfaces_shared_edge_and_occupancy_volumes() {
        let a = sphere("a", [0.0; 3], 1.0);
        let b = sphere("b", [1.0, 0.0, 0.0], 1.0);
        let unit_volume = 4.0 * std::f64::consts::PI / 3.0;
        let lens = 5.0 * std::f64::consts::PI / 12.0;
        for (op, expected) in [
            (BooleanOp::Union, 2.0 * unit_volume - lens),
            (BooleanOp::Intersection, lens),
            (BooleanOp::Subtraction, unit_volume - lens),
        ] {
            let r = boolean_spheres(&a, &b, op, "r".into()).unwrap();
            assert_eq!(r.brep.topology.faces.len(), 2);
            let occupied = |point: Point3| {
                let inside_a = norm(sub(point, [0.0; 3])) < 1.0;
                let inside_b = norm(sub(point, [1.0, 0.0, 0.0])) < 1.0;
                match op {
                    BooleanOp::Union => inside_a || inside_b,
                    BooleanOp::Intersection => inside_a && inside_b,
                    BooleanOp::Subtraction => inside_a && !inside_b,
                }
            };
            for face in &r.brep.topology.faces {
                let surface = r.brep.geometry.surface(face.surface).unwrap();
                let v = face.trim.uv_bounds[1];
                for fraction in [0.25, 0.5, 0.75] {
                    let uv = [0.9, v.lo + fraction * v.width()];
                    let point = surface.point_at(uv).unwrap();
                    let outward = scale(surface.normal_at(uv).unwrap(), face.sense.multiplier());
                    assert!(occupied(sub(point, scale(outward, 1e-5))));
                    assert!(!occupied(crate::analytic::geometry::add(
                        point,
                        scale(outward, 1e-5)
                    )));
                }
            }
            assert!(r
                .brep
                .geometry
                .surfaces
                .iter()
                .all(|s| matches!(s, SurfaceGeometry::Sphere { .. })));
            let shared: Vec<_> = r
                .brep
                .topology
                .edges
                .iter()
                .filter(|e| !e.chart_seam)
                .collect();
            assert_eq!(shared.len(), 1);
            assert!(shared[0].twin_halfedge.is_some());
            assert!((volume(&r.brep) - expected).abs() < 0.04);
            assert_eq!(r.report.face_mappings[0].result_faces, vec![0]);
            assert_eq!(r.report.face_mappings[1].result_faces, vec![1]);
            assert_eq!(
                r.brep.topology.faces[1].provenance.reversed,
                op == BooleanOp::Subtraction
            );
        }
    }
    #[test]
    fn enclosed_subtraction_orients_cavity_and_preserves_ancestry() {
        let a = sphere("host", [0.0; 3], 2.0);
        let b = sphere("cutter", [0.2, 0.0, 0.0], 0.5);
        let r = boolean_spheres(&a, &b, BooleanOp::Subtraction, "r".into()).unwrap();
        assert_eq!(r.brep.solids[0].cavity_shells, vec![1]);
        assert_eq!(r.brep.topology.faces[1].sense, Orientation::Reverse);
        assert_eq!(
            r.brep.topology.faces[1].provenance.sources[0].entity,
            "cutter"
        );
        assert!((volume(&r.brep) - 4.0 * std::f64::consts::PI / 3.0 * (8.0 - 0.125)).abs() < 0.1);
    }
    #[test]
    fn contact_disjoint_containment_and_coincident_regularize_deterministically() {
        let a = sphere("a", [0.0; 3], 1.0);
        for x in [2.0, 2.5] {
            let b = sphere("b", [x, 0.0, 0.0], 1.0);
            let r = boolean_spheres(&a, &b, BooleanOp::Union, "r".into()).unwrap();
            assert_eq!(r.brep.solids.len(), 2);
            assert_eq!(r.report.contacts.len(), usize::from(x == 2.0));
            assert!(boolean_spheres(&a, &b, BooleanOp::Intersection, "r".into())
                .unwrap()
                .brep
                .solids
                .is_empty());
        }
        let b = sphere("b", [0.0; 3], 1.0);
        let r = boolean_spheres(&a, &b, BooleanOp::Union, "r".into()).unwrap();
        assert!(r.report.coincident);
        assert_eq!(r.brep.topology.faces[0].provenance.sources.len(), 2);
        assert!(boolean_spheres(&a, &b, BooleanOp::Subtraction, "r".into())
            .unwrap()
            .brep
            .topology
            .faces
            .is_empty());
        let big = sphere("big", [0.0; 3], 2.0);
        assert!(
            boolean_spheres(&a, &big, BooleanOp::Subtraction, "r".into())
                .unwrap()
                .brep
                .solids
                .is_empty()
        );
        assert_eq!(
            boolean_spheres(&a, &big, BooleanOp::Intersection, "r".into())
                .unwrap()
                .brep
                .topology
                .faces[0]
                .provenance
                .sources[0]
                .entity,
            "a"
        );
    }
    #[test]
    fn unsupported_inputs_and_invalid_geometry_never_enter_a_mesh_bridge() {
        let a = sphere("a", [0.0; 3], 1.0);
        let cylinder =
            primitives::cylinder("c".into(), Frame3::IDENTITY, 1.0, 2.0, a.accuracy).unwrap();
        assert!(matches!(
            boolean_spheres(&a, &cylinder, BooleanOp::Union, "r".into()),
            Err(GeometryError::CoverageGap { .. })
        ));
        let mut invalid = a.clone();
        invalid.topology.faces[0].surface = 100;
        assert!(matches!(
            boolean_spheres(&a, &invalid, BooleanOp::Union, "r".into()),
            Err(GeometryError::MissingReference { .. })
        ));
        let mut trimmed = a.clone();
        trimmed.topology.faces[0].trim.uv_bounds[1].hi = 0.5;
        assert!(boolean_spheres(&a, &trimmed, BooleanOp::Union, "r".into()).is_err());
    }

    #[test]
    fn provably_disjoint_surface_families_use_exact_regularized_results() {
        let cylinder_accuracy = sphere("reference", [0.0; 3], 1.0).accuracy;
        let torus_accuracy = Accuracy {
            geometric: cylinder_accuracy.geometric * 2.0,
            intersection: cylinder_accuracy.intersection * 2.0,
            tessellation: cylinder_accuracy.tessellation * 2.0,
            exchange: cylinder_accuracy.exchange * 2.0,
        };
        let cylinder = primitives::cylinder(
            "cylinder".into(),
            Frame3::IDENTITY,
            1.0,
            2.0,
            cylinder_accuracy,
        )
        .unwrap();
        let torus = primitives::torus(
            "torus".into(),
            Frame3 {
                origin: [12.0, 0.0, 0.0],
                ..Frame3::IDENTITY
            },
            3.0,
            1.0,
            torus_accuracy,
        )
        .unwrap();

        let union = boolean_brep(&cylinder, &torus, BooleanOp::Union, "union".into()).unwrap();
        assert_eq!(union.brep.solids.len(), 2);
        assert_eq!(union.brep.accuracy.geometric, torus_accuracy.geometric);
        assert_eq!(union.brep.topology.faces.len(), 4);
        assert_eq!(union.report.face_mappings.len(), 4);
        assert!(union
            .brep
            .topology
            .faces
            .iter()
            .all(|face| face.provenance.role == FaceRole::Preserved));

        let intersection = boolean_brep(
            &cylinder,
            &torus,
            BooleanOp::Intersection,
            "intersection".into(),
        )
        .unwrap();
        assert!(intersection.brep.solids.is_empty());
        assert!(intersection
            .report
            .face_mappings
            .iter()
            .all(|mapping| mapping.result_faces.is_empty()));

        let subtraction = boolean_brep(
            &cylinder,
            &torus,
            BooleanOp::Subtraction,
            "subtraction".into(),
        )
        .unwrap();
        assert_eq!(subtraction.brep.solids.len(), 1);
        assert_eq!(subtraction.brep.topology.faces.len(), 3);
        assert_eq!(
            subtraction.brep.topology.faces[0].provenance.sources[0].entity,
            "cylinder"
        );
    }
    #[test]
    fn positive_similarity_roundoff_does_not_reject_full_sphere_operands() {
        let rotation = std::f64::consts::PI / 7.0;
        let placement = Frame3 {
            origin: [0.2, 0.1, 0.2],
            x: [rotation.cos(), 0.0, -rotation.sin()],
            y: [0.0, 1.0, 0.0],
            z: [rotation.sin(), 0.0, rotation.cos()],
        };
        let frame = Frame3 {
            origin: [0.0, 1.2, 0.0],
            y: [0.0, 0.0, -1.0],
            z: [0.0, 1.0, 0.0],
            ..Frame3::IDENTITY
        };
        let first = primitives::sphere(
            "a".into(),
            frame,
            1.0,
            sphere("reference", [0.0; 3], 1.0).accuracy,
        )
        .unwrap();
        let second = primitives::sphere(
            "b".into(),
            Frame3 {
                origin: [1.0, 1.2, 0.0],
                ..frame
            },
            1.0,
            first.accuracy,
        )
        .unwrap();
        let first = super::super::placement::placed(&first, placement, 1.25).unwrap();
        let second = super::super::placement::placed(&second, placement, 1.25).unwrap();
        for op in [
            BooleanOp::Union,
            BooleanOp::Intersection,
            BooleanOp::Subtraction,
        ] {
            let result = boolean_spheres(&first, &second, op, "placed".into()).unwrap();
            result.brep.validate().unwrap();
            assert_eq!(result.brep.topology.faces.len(), 2);
            assert_eq!(result.report.face_mappings[0].source.entity, "a");
        }
    }
    #[test]
    fn internal_tangency_and_unresolved_small_caps_are_errors() {
        let a = sphere("a", [0.0; 3], 2.0);
        let b = sphere("b", [1.0, 0.0, 0.0], 1.0);
        assert!(matches!(
            boolean_spheres(&a, &b, BooleanOp::Subtraction, "r".into()),
            Err(GeometryError::UnresolvedIntersection(_))
        ));
        let a = sphere("a", [0.0; 3], 1.0);
        let b = sphere("b", [2.0 - 1e-10, 0.0, 0.0], 1.0);
        assert!(matches!(
            boolean_spheres(&a, &b, BooleanOp::Intersection, "r".into()),
            Err(GeometryError::UnresolvedIntersection(_))
        ));
    }
    #[test]
    fn rotated_unequal_spheres_roundtrip_and_keep_identical_boundary_positions() {
        let a = sphere("a", [5.0, -3.0, 8.0], 2.0);
        let b = sphere("b", [6.0, -2.0, 9.0], 1.25);
        let r = boolean_spheres(&a, &b, BooleanOp::Subtraction, "r".into()).unwrap();
        let decoded = BrepEnvelope::from_json(&r.brep.to_json().unwrap()).unwrap();
        let mesh = tessellate(&decoded, 0.02, 2_000_000).unwrap();
        let boundary = decoded.geometry.curves[0].elementary_point(0.0).unwrap();
        let count = mesh
            .positions
            .chunks_exact(3)
            .filter(|p| p == &boundary.as_slice())
            .count();
        assert!(count >= 2);
        assert!(mesh.indices.len() > 100);
    }
    #[test]
    fn sphere_booleans_obey_scaled_budgets_and_building_coordinates() {
        for size in [1e-6, 1.0, 1e6] {
            let accuracy = Accuracy {
                geometric: size * 1e-9,
                intersection: size * 1e-10,
                tessellation: size * 0.01,
                exchange: size * 1e-5,
            };
            let a = primitives::sphere("a".into(), Frame3::IDENTITY, size, accuracy).unwrap();
            let b = primitives::sphere(
                "b".into(),
                Frame3 {
                    origin: [size, 0.0, 0.0],
                    ..Frame3::IDENTITY
                },
                size,
                accuracy,
            )
            .unwrap();
            let r = boolean_spheres(&a, &b, BooleanOp::Subtraction, "r".into()).unwrap();
            assert_eq!(r.brep.topology.faces.len(), 2);
            tessellate(&r.brep, size * 0.02, 2_000_000).unwrap();
        }
        let accuracy = Accuracy {
            geometric: 1e-6,
            intersection: 5e-7,
            tessellation: 0.01,
            exchange: 1e-5,
        };
        let a = primitives::sphere(
            "a".into(),
            Frame3 {
                origin: [1e9; 3],
                ..Frame3::IDENTITY
            },
            1.0,
            accuracy,
        )
        .unwrap();
        let b = primitives::sphere(
            "b".into(),
            Frame3 {
                origin: [1e9 + 1.0, 1e9, 1e9],
                ..Frame3::IDENTITY
            },
            1.0,
            accuracy,
        )
        .unwrap();
        let r = boolean_spheres(&a, &b, BooleanOp::Intersection, "r".into()).unwrap();
        tessellate(&r.brep, 0.01, 2_000_000).unwrap();
    }

    #[test]
    fn concentric_torus_booleans_preserve_exact_cavity_supports() {
        let accuracy = sphere("accuracy", [0.0; 3], 1.0).accuracy;
        let host = primitives::torus("host".into(), Frame3::IDENTITY, 3.0, 1.0, accuracy).unwrap();
        let cutter =
            primitives::torus("cutter".into(), Frame3::IDENTITY, 3.0, 0.4, accuracy).unwrap();
        let subtraction =
            boolean_brep(&host, &cutter, BooleanOp::Subtraction, "torus-shell".into()).unwrap();
        subtraction.brep.validate().unwrap();
        assert_eq!(subtraction.brep.solids.len(), 1);
        assert_eq!(subtraction.brep.solids[0].cavity_shells.len(), 1);
        assert!(subtraction
            .brep
            .topology
            .faces
            .iter()
            .any(|face| { face.provenance.role == FaceRole::Cut && face.provenance.reversed }));
        assert!(subtraction
            .brep
            .geometry
            .surfaces
            .iter()
            .all(|surface| matches!(surface, SurfaceGeometry::Torus { .. })));
        tessellate(&subtraction.brep, 0.05, 2_000_000).unwrap();

        let intersection =
            boolean_brep(&host, &cutter, BooleanOp::Intersection, "torus-core".into()).unwrap();
        assert_eq!(intersection.brep.topology.faces.len(), 1);
        assert!(matches!(
            intersection.brep.geometry.surfaces[0],
            SurfaceGeometry::Torus {
                minor_radius: 0.4,
                ..
            }
        ));

        let coincident =
            boolean_brep(&host, &host, BooleanOp::Subtraction, "empty-torus".into()).unwrap();
        assert!(coincident.report.coincident);
        assert!(coincident.brep.solids.is_empty());
    }

    #[test]
    fn sphere_torus_containment_is_exact_in_both_directions() {
        let accuracy = sphere("accuracy", [0.0; 3], 1.0).accuracy;
        let torus =
            primitives::torus("torus".into(), Frame3::IDENTITY, 3.0, 1.0, accuracy).unwrap();
        let small_sphere = sphere("small", [3.0, 0.0, 0.0], 0.25);
        let torus_cut = boolean_brep(
            &torus,
            &small_sphere,
            BooleanOp::Subtraction,
            "torus-cut".into(),
        )
        .unwrap();
        assert_eq!(torus_cut.brep.solids[0].cavity_shells.len(), 1);
        assert!(torus_cut.brep.geometry.surfaces.iter().any(
            |surface| matches!(surface, SurfaceGeometry::Sphere { radius, .. } if *radius == 0.25)
        ));

        let large_sphere = sphere("large", [0.0; 3], 5.0);
        let sphere_cut = boolean_brep(
            &large_sphere,
            &torus,
            BooleanOp::Subtraction,
            "sphere-cut".into(),
        )
        .unwrap();
        sphere_cut.brep.validate().unwrap();
        assert_eq!(sphere_cut.brep.solids[0].cavity_shells.len(), 1);
        assert!(sphere_cut
            .brep
            .topology
            .faces
            .iter()
            .any(|face| face.provenance.role == FaceRole::Cut));
    }

    fn extrusion(id: &str, outer: Vec<[f64; 2]>, holes: Vec<Vec<[f64; 2]>>) -> BrepEnvelope {
        extrusion_span(id, 0.0, 3.0, outer, holes)
    }

    fn extrusion_span(
        id: &str,
        origin_z: f64,
        height: f64,
        outer: Vec<[f64; 2]>,
        holes: Vec<Vec<[f64; 2]>>,
    ) -> BrepEnvelope {
        primitives::linear_extrusion(
            id.into(),
            Frame3 {
                origin: [0.0, 0.0, origin_z],
                ..Frame3::IDENTITY
            },
            outer,
            holes,
            height,
            Accuracy {
                geometric: 1.0e-9,
                intersection: 1.0e-10,
                tessellation: 0.01,
                exchange: 1.0e-5,
            },
        )
        .unwrap()
    }

    #[test]
    fn planar_extrusion_round_through_cutter_keeps_analytic_brep() {
        let host = extrusion(
            "round-host",
            vec![[0.0, 0.0], [4.0, 0.0], [4.0, 0.3], [0.0, 0.3]],
            Vec::new(),
        );
        let cutter = primitives::cylinder(
            "round-opening".into(),
            Frame3 {
                origin: [2.0, -0.5, 1.5],
                x: [1.0, 0.0, 0.0],
                y: [0.0, 0.0, -1.0],
                z: [0.0, 1.0, 0.0],
            },
            0.5,
            1.3,
            host.accuracy,
        )
        .unwrap();
        let result = boolean_brep(
            &host,
            &cutter,
            BooleanOp::Subtraction,
            "round-host-cut".into(),
        )
        .unwrap();
        assert_eq!(result.brep.solids.len(), 1);
        result.brep.validate().unwrap();
        assert_eq!(
            classify_point(&result.brep, [2.0, 0.15, 1.5]).unwrap(),
            PointClassification::Outside
        );
        assert_eq!(
            classify_point(&result.brep, [2.0, 0.15, 2.2]).unwrap(),
            PointClassification::Inside
        );
        let cut_face = result
            .brep
            .topology
            .faces
            .iter()
            .find(|face| face.provenance.role == FaceRole::Cut)
            .unwrap();
        assert_eq!(
            face_contains_uv(&result.brep, cut_face, [0.0, 0.65]).unwrap(),
            Some(true)
        );
        tessellate(&result.brep, 0.01, 2_000_000).unwrap();
        crate::analytic::exchange::export_step(&result.brep, "metre").unwrap();
    }

    #[test]
    fn rectangular_cut_after_round_cut_never_loses_the_cylindrical_cap() {
        let host = extrusion(
            "arched-host",
            vec![[0.0, 0.0], [4.0, 0.0], [4.0, 0.3], [0.0, 0.3]],
            Vec::new(),
        );
        let round = primitives::cylinder(
            "arched-cap".into(),
            Frame3 {
                origin: [2.0, -0.5, 2.0],
                x: [1.0, 0.0, 0.0],
                y: [0.0, 0.0, -1.0],
                z: [0.0, 1.0, 0.0],
            },
            0.5,
            1.3,
            host.accuracy,
        )
        .unwrap();
        let lower = primitives::linear_extrusion(
            "arched-lower".into(),
            Frame3 {
                origin: [0.0, 0.8, 0.0],
                x: [1.0, 0.0, 0.0],
                y: [0.0, 0.0, 1.0],
                z: [0.0, -1.0, 0.0],
            },
            vec![[1.5, 0.5], [2.5, 0.5], [2.5, 2.0], [1.5, 2.0]],
            Vec::new(),
            1.3,
            host.accuracy,
        )
        .unwrap();
        let batch = subtract_planar_cutters(
            &host,
            &[round.clone(), lower.clone()],
            "arched-batch".into(),
        )
        .unwrap();
        batch.brep.validate().unwrap();
        assert_eq!(
            classify_point(&batch.brep, [2.0, 0.15, 2.2]).unwrap(),
            PointClassification::Outside
        );
        assert_eq!(
            classify_point(&batch.brep, [2.0, 0.15, 1.0]).unwrap(),
            PointClassification::Outside
        );
        assert!(batch.brep.topology.faces.iter().any(|face| {
            matches!(
                batch.brep.geometry.surfaces[face.surface as usize],
                SurfaceGeometry::Cylinder { .. }
            )
        }));
        for source in ["arched-cap", "arched-lower"] {
            assert!(batch.report.face_mappings.iter().any(|mapping| {
                mapping.source.entity == source && !mapping.result_faces.is_empty()
            }));
        }
        let expected = 4.0 * 0.3 * 3.0 - (1.0 * 1.5 + std::f64::consts::PI * 0.5 * 0.5 / 2.0) * 0.3;
        assert!((volume_at_deflection(&batch.brep, 0.0001).abs() - expected).abs() < 1.0e-4);
        crate::analytic::exchange::export_step(&batch.brep, "metre").unwrap();
        let cap = boolean_brep(&host, &round, BooleanOp::Subtraction, "cap".into()).unwrap();
        match boolean_brep(&cap.brep, &lower, BooleanOp::Subtraction, "arch".into()) {
            Ok(result) => {
                result.brep.validate().unwrap();
                assert!(result.brep.topology.faces.iter().any(|face| {
                    matches!(
                        result.brep.geometry.surfaces[face.surface as usize],
                        SurfaceGeometry::Cylinder { .. }
                    )
                }));
                assert_eq!(
                    classify_point(&result.brep, [2.0, 0.15, 2.2]).unwrap(),
                    PointClassification::Outside
                );
                assert_eq!(
                    classify_point(&result.brep, [2.0, 0.15, 1.0]).unwrap(),
                    PointClassification::Outside
                );
            }
            Err(GeometryError::CoverageGap { .. }) => {}
            Err(error) => panic!("unexpected chained arch failure: {error:?}"),
        }
    }

    #[test]
    fn disjoint_mixed_cutter_batch_preserves_both_openings_in_any_order() {
        let host = extrusion(
            "mixed-host",
            vec![[0.0, 0.0], [6.0, 0.0], [6.0, 0.3], [0.0, 0.3]],
            Vec::new(),
        );
        let round = primitives::cylinder(
            "round".into(),
            Frame3 {
                origin: [1.5, -0.5, 1.5],
                x: [1.0, 0.0, 0.0],
                y: [0.0, 0.0, -1.0],
                z: [0.0, 1.0, 0.0],
            },
            0.4,
            1.3,
            host.accuracy,
        )
        .unwrap();
        let rect = primitives::linear_extrusion(
            "rectangle".into(),
            Frame3 {
                origin: [0.0, 0.8, 0.0],
                x: [1.0, 0.0, 0.0],
                y: [0.0, 0.0, 1.0],
                z: [0.0, -1.0, 0.0],
            },
            vec![[3.5, 0.7], [4.5, 0.7], [4.5, 2.2], [3.5, 2.2]],
            Vec::new(),
            1.3,
            host.accuracy,
        )
        .unwrap();
        for cutters in [[round.clone(), rect.clone()], [rect.clone(), round.clone()]] {
            let result = subtract_planar_cutters(&host, &cutters, "mixed".into()).unwrap();
            result.brep.validate().unwrap();
            assert_eq!(
                classify_point(&result.brep, [1.5, 0.15, 1.5]).unwrap(),
                PointClassification::Outside
            );
            assert_eq!(
                classify_point(&result.brep, [4.0, 0.15, 1.5]).unwrap(),
                PointClassification::Outside
            );
            assert_eq!(
                classify_point(&result.brep, [2.5, 0.15, 1.5]).unwrap(),
                PointClassification::Inside
            );
            let expected_volume =
                6.0 * 0.3 * 3.0 - 1.0 * 0.3 * 1.5 - std::f64::consts::PI * 0.4 * 0.4 * 0.3;
            let measured_volume = volume_at_deflection(&result.brep, 0.0001).abs();
            assert!(
                (measured_volume - expected_volume).abs() < 1.0e-4,
                "mixed volume {measured_volume} differs from expected {expected_volume}",
            );
            assert!(result.brep.topology.faces.iter().any(|face| {
                matches!(
                    result.brep.geometry.surfaces[face.surface as usize],
                    SurfaceGeometry::Cylinder { .. }
                )
            }));
            for source in ["round", "rectangle"] {
                assert!(
                    result.brep.topology.faces.iter().any(|face| {
                        face.provenance
                            .sources
                            .iter()
                            .any(|origin| origin.entity == source)
                    }),
                    "missing source {source}: {:?}",
                    result
                        .brep
                        .topology
                        .faces
                        .iter()
                        .flat_map(|face| face
                            .provenance
                            .sources
                            .iter()
                            .map(|origin| &origin.entity))
                        .collect::<Vec<_>>()
                );
                assert!(result.report.face_mappings.iter().any(|mapping| {
                    mapping.source.entity == source && !mapping.result_faces.is_empty()
                }));
            }
            crate::analytic::exchange::export_step(&result.brep, "metre").unwrap();
        }
    }

    #[test]
    fn two_round_cutter_batch_keeps_both_analytic_voids() {
        let host = extrusion(
            "two-round-host",
            vec![[0.0, 0.0], [6.0, 0.0], [6.0, 0.3], [0.0, 0.3]],
            Vec::new(),
        );
        let make_round = |name: &str, station: f64| {
            primitives::cylinder(
                name.into(),
                Frame3 {
                    origin: [station, 0.0, 1.5],
                    x: [1.0, 0.0, 0.0],
                    y: [0.0, 0.0, -1.0],
                    z: [0.0, 1.0, 0.0],
                },
                0.4,
                0.3,
                host.accuracy,
            )
            .unwrap()
        };
        let first = make_round("round-a", 1.5);
        let second = make_round("round-b", 4.5);
        for cutters in [
            [first.clone(), second.clone()],
            [second.clone(), first.clone()],
        ] {
            let result = subtract_planar_cutters(&host, &cutters, "two-round".into()).unwrap();
            result.brep.validate().unwrap();
            assert_eq!(result.brep.solids.len(), 1);
            for station in [1.5, 4.5] {
                assert_eq!(
                    classify_point(&result.brep, [station, 0.15, 1.5]).unwrap(),
                    PointClassification::Outside
                );
            }
            assert_eq!(
                classify_point(&result.brep, [3.0, 0.15, 1.5]).unwrap(),
                PointClassification::Inside
            );
            let expected = 6.0 * 0.3 * 3.0 - 2.0 * std::f64::consts::PI * 0.4 * 0.4 * 0.3;
            assert!((volume_at_deflection(&result.brep, 0.0001).abs() - expected).abs() < 1.0e-4);
            for source in ["round-a", "round-b"] {
                assert!(result
                    .report
                    .face_mappings
                    .iter()
                    .any(|mapping| mapping.source.entity == source
                        && !mapping.result_faces.is_empty()));
            }
            let (_, report) =
                crate::analytic::exchange::export_step(&result.brep, "metre").unwrap();
            assert_eq!(report.solids, 1);
        }
    }

    #[test]
    fn mixed_profile_batch_preserves_disconnected_full_height_host_parts() {
        let host = extrusion(
            "split-mixed-host",
            vec![[0.0, 0.0], [6.0, 0.0], [6.0, 0.3], [0.0, 0.3]],
            Vec::new(),
        );
        let round = primitives::cylinder(
            "split-round".into(),
            Frame3 {
                origin: [1.5, -0.5, 1.5],
                x: [1.0, 0.0, 0.0],
                y: [0.0, 0.0, -1.0],
                z: [0.0, 1.0, 0.0],
            },
            0.4,
            1.3,
            host.accuracy,
        )
        .unwrap();
        let through = primitives::linear_extrusion(
            "full-height".into(),
            Frame3 {
                origin: [0.0, 0.3, 0.0],
                x: [1.0, 0.0, 0.0],
                y: [0.0, 0.0, 1.0],
                z: [0.0, -1.0, 0.0],
            },
            vec![[2.5, 0.0], [3.5, 0.0], [3.5, 3.0], [2.5, 3.0]],
            Vec::new(),
            0.3,
            host.accuracy,
        )
        .unwrap();
        for cutters in [
            [round.clone(), through.clone()],
            [through.clone(), round.clone()],
        ] {
            let result = subtract_planar_cutters(&host, &cutters, "split-mixed".into()).unwrap();
            result.brep.validate().unwrap();
            assert_eq!(result.brep.solids.len(), 2);
            for point in [[1.5, 0.15, 1.5], [3.0, 0.15, 1.5]] {
                assert_eq!(
                    classify_point(&result.brep, point).unwrap(),
                    PointClassification::Outside
                );
            }
            for point in [[0.5, 0.15, 1.5], [5.0, 0.15, 1.5]] {
                assert_eq!(
                    classify_point(&result.brep, point).unwrap(),
                    PointClassification::Inside
                );
            }
            let expected =
                6.0 * 0.3 * 3.0 - 1.0 * 0.3 * 3.0 - std::f64::consts::PI * 0.4 * 0.4 * 0.3;
            assert!((volume_at_deflection(&result.brep, 0.0001).abs() - expected).abs() < 1.0e-4);
            for source in ["split-round", "full-height"] {
                assert!(result
                    .report
                    .face_mappings
                    .iter()
                    .any(|mapping| mapping.source.entity == source
                        && !mapping.result_faces.is_empty()));
            }
            let (_, report) =
                crate::analytic::exchange::export_step(&result.brep, "metre").unwrap();
            assert_eq!(report.solids, 2);
        }
    }

    #[test]
    fn flush_mixed_cutters_cross_a_prior_planar_face_split_without_losing_the_round_hole() {
        let host = extrusion(
            "flush-mixed-host",
            vec![[0.0, 0.0], [6.0, 0.0], [6.0, 0.3], [0.0, 0.3]],
            Vec::new(),
        );
        let round = primitives::cylinder(
            "flush-round".into(),
            Frame3 {
                origin: [1.5, 0.0, 1.5],
                x: [1.0, 0.0, 0.0],
                y: [0.0, 0.0, -1.0],
                z: [0.0, 1.0, 0.0],
            },
            0.4,
            0.3,
            host.accuracy,
        )
        .unwrap();
        let rectangle = primitives::linear_extrusion(
            "flush-rectangle".into(),
            Frame3 {
                origin: [0.0, 0.3, 0.0],
                x: [1.0, 0.0, 0.0],
                y: [0.0, 0.0, 1.0],
                z: [0.0, -1.0, 0.0],
            },
            vec![[3.5, 0.7], [4.5, 0.7], [4.5, 1.5], [3.5, 1.5]],
            Vec::new(),
            0.3,
            host.accuracy,
        )
        .unwrap();
        for cutters in [
            [round.clone(), rectangle.clone()],
            [rectangle.clone(), round.clone()],
        ] {
            let result = subtract_planar_cutters(&host, &cutters, "flush-mixed".into()).unwrap();
            result.brep.validate().unwrap();
            assert_eq!(
                classify_point(&result.brep, [1.5, 0.15, 1.5]).unwrap(),
                PointClassification::Outside
            );
            assert_eq!(
                classify_point(&result.brep, [4.0, 0.15, 1.4]).unwrap(),
                PointClassification::Outside
            );
            assert_eq!(
                classify_point(&result.brep, [2.5, 0.15, 1.5]).unwrap(),
                PointClassification::Inside
            );
            assert!(result.brep.topology.faces.iter().any(|face| {
                matches!(
                    result.brep.geometry.surfaces[face.surface as usize],
                    SurfaceGeometry::Cylinder { .. }
                )
            }));
            for source in ["flush-round", "flush-rectangle"] {
                assert!(result.report.face_mappings.iter().any(|mapping| {
                    mapping.source.entity == source && !mapping.result_faces.is_empty()
                }));
            }
            crate::analytic::exchange::export_step(&result.brep, "metre").unwrap();
        }
    }

    #[test]
    fn coextensive_profile_extrusions_use_exact_planar_arrangements() {
        let a = extrusion(
            "a",
            vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]],
            Vec::new(),
        );
        let b = extrusion(
            "b",
            vec![[2.0, -1.0], [5.0, -1.0], [5.0, 2.0], [2.0, 2.0]],
            Vec::new(),
        );
        for (operation, expected_volume) in [
            (BooleanOp::Union, 63.0),
            (BooleanOp::Intersection, 12.0),
            (BooleanOp::Subtraction, 36.0),
        ] {
            let result = boolean_brep(&a, &b, operation, format!("{operation:?}")).unwrap();
            result.brep.validate().unwrap();
            assert!(matches!(result.brep.quality, GeometryQuality::Analytic));
            assert!((volume(&result.brep).abs() - expected_volume).abs() < 1.0e-6);
            assert!(result
                .report
                .face_mappings
                .iter()
                .any(|mapping| !mapping.result_faces.is_empty()));
        }
    }

    #[test]
    fn profile_subtraction_builds_holes_disconnected_solids_and_cut_provenance() {
        let host = extrusion(
            "host",
            vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]],
            Vec::new(),
        );
        let inner = extrusion(
            "inner",
            vec![[4.0, 4.0], [6.0, 4.0], [6.0, 6.0], [4.0, 6.0]],
            Vec::new(),
        );
        let cavity = boolean_brep(&host, &inner, BooleanOp::Subtraction, "cavity".into()).unwrap();
        assert_eq!(cavity.brep.solids.len(), 1);
        assert_eq!(cavity.brep.topology.faces[0].trim.holes.len(), 1);
        assert!((volume(&cavity.brep).abs() - 288.0).abs() < 1.0e-6);
        assert!(cavity
            .brep
            .topology
            .faces
            .iter()
            .any(|face| { face.provenance.role == FaceRole::Cut && face.provenance.reversed }));

        let band = extrusion(
            "band",
            vec![[-1.0, 4.0], [11.0, 4.0], [11.0, 6.0], [-1.0, 6.0]],
            Vec::new(),
        );
        let split = boolean_brep(&host, &band, BooleanOp::Subtraction, "split".into()).unwrap();
        assert_eq!(split.brep.solids.len(), 2);
        assert!((volume(&split.brep).abs() - 240.0).abs() < 1.0e-6);
    }

    #[test]
    fn matching_profile_extrusions_use_exact_axial_interval_booleans() {
        let profile = vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]];
        let a = extrusion_span("a", 0.0, 4.0, profile.clone(), Vec::new());
        let b = extrusion_span(
            "b",
            2.0,
            4.0,
            vec![[4.0, 4.0], [0.0, 4.0], [0.0, 0.0], [4.0, 0.0]],
            Vec::new(),
        );
        for (operation, expected_volume) in [
            (BooleanOp::Union, 96.0),
            (BooleanOp::Intersection, 32.0),
            (BooleanOp::Subtraction, 32.0),
        ] {
            let result = boolean_brep(&a, &b, operation, format!("axial-{operation:?}")).unwrap();
            result.brep.validate().unwrap();
            assert_eq!(result.brep.solids.len(), 1);
            assert!((volume(&result.brep).abs() - expected_volume).abs() < 1.0e-6);
            assert!(result
                .report
                .face_mappings
                .iter()
                .any(|mapping| !mapping.result_faces.is_empty()));
        }

        let embedded = extrusion_span("embedded", 1.0, 2.0, profile.clone(), Vec::new());
        let split =
            boolean_brep(&a, &embedded, BooleanOp::Subtraction, "axial-split".into()).unwrap();
        split.brep.validate().unwrap();
        assert_eq!(split.brep.solids.len(), 2);
        assert!((volume(&split.brep).abs() - 32.0).abs() < 1.0e-6);
        assert_eq!(
            split
                .brep
                .topology
                .faces
                .iter()
                .filter(|face| face.provenance.role == FaceRole::Cut && face.provenance.reversed)
                .count(),
            2
        );

        let disjoint = extrusion_span("disjoint", 6.0, 2.0, profile, Vec::new());
        let union = boolean_brep(&a, &disjoint, BooleanOp::Union, "axial-disjoint".into()).unwrap();
        assert_eq!(union.brep.solids.len(), 2);
        assert!((volume(&union.brep).abs() - 96.0).abs() < 1.0e-6);
        let intersection =
            boolean_brep(&a, &disjoint, BooleanOp::Intersection, "axial-empty".into()).unwrap();
        assert!(intersection.brep.solids.is_empty());
    }

    #[test]
    fn different_height_planar_openings_support_chained_flush_cuts() {
        let host = extrusion(
            "host",
            vec![[0.0, 0.0], [10.0, 0.0], [10.0, 0.3], [0.0, 0.3]],
            Vec::new(),
        );
        let first = extrusion_span(
            "lower_cutout",
            0.0,
            2.1,
            vec![[2.0, 0.0], [3.0, 0.0], [3.0, 0.3], [2.0, 0.3]],
            Vec::new(),
        );
        let cut = boolean_brep(
            &host,
            &first,
            BooleanOp::Subtraction,
            "one-lower_cutout".into(),
        )
        .unwrap();
        cut.brep.validate().unwrap();
        assert!((volume(&cut.brep).abs() - 8.37).abs() < 1.0e-6);
        assert_eq!(
            super::super::query::classify_point(&cut.brep, [2.5, 0.15, 1.0]).unwrap(),
            super::super::query::PointClassification::Outside
        );
        assert_eq!(
            super::super::query::classify_point(&cut.brep, [2.5, 0.15, 2.5]).unwrap(),
            super::super::query::PointClassification::Inside
        );
        let second = extrusion_span(
            "raised_cutout",
            0.8,
            1.2,
            vec![[6.0, 0.0], [7.0, 0.0], [7.0, 0.3], [6.0, 0.3]],
            Vec::new(),
        );
        let chained = boolean_brep(
            &cut.brep,
            &second,
            BooleanOp::Subtraction,
            "lower_cutout-raised_cutout".into(),
        )
        .unwrap();
        chained.brep.validate().unwrap();
        assert!((volume(&chained.brep).abs() - 8.01).abs() < 1.0e-6);
        assert_eq!(
            super::super::query::classify_point(&chained.brep, [6.5, 0.15, 1.2]).unwrap(),
            super::super::query::PointClassification::Outside
        );
    }

    #[test]
    fn chained_mitered_host_openings_keep_top_cap_provenance() {
        let host = extrusion_span(
            "mitered-host",
            0.0,
            3.8,
            vec![
                [-0.16, -0.14],
                [0.16, -0.46],
                [13.84, -0.46],
                [14.16, -0.14],
            ],
            Vec::new(),
        );
        let lower_cutout = extrusion_span(
            "lower_cutout",
            0.0,
            2.45,
            vec![[1.2, -0.14], [2.8, -0.14], [2.8, -0.46], [1.2, -0.46]],
            Vec::new(),
        );
        let raised_cutout = extrusion_span(
            "raised_cutout",
            0.3,
            2.8,
            vec![
                [3.625, -0.14],
                [5.175, -0.14],
                [5.175, -0.46],
                [3.625, -0.46],
            ],
            Vec::new(),
        );
        let first = boolean_brep(
            &host,
            &lower_cutout,
            BooleanOp::Subtraction,
            "lower_cutout-cut".into(),
        )
        .unwrap();
        let second = boolean_brep(
            &first.brep,
            &raised_cutout,
            BooleanOp::Subtraction,
            "raised_cutout-cut".into(),
        )
        .unwrap();
        second.brep.validate().unwrap();
        assert_eq!(
            super::super::query::classify_point(&second.brep, [2.0, -0.3, 1.0]).unwrap(),
            super::super::query::PointClassification::Outside,
        );
        assert_eq!(
            super::super::query::classify_point(&second.brep, [4.4, -0.3, 1.0]).unwrap(),
            super::super::query::PointClassification::Outside,
        );
    }

    #[test]
    fn ten_flush_planar_cutters_use_one_rectilinear_arrangement() {
        let host = extrusion(
            "host",
            vec![[0.0, 0.0], [20.0, 0.0], [20.0, 0.3], [0.0, 0.3]],
            Vec::new(),
        );
        let cutters = (0..10)
            .map(|index| {
                let left = 1.0 + index as f64 * 1.8;
                extrusion_span(
                    &format!("opening-{index}"),
                    0.0,
                    2.1,
                    vec![
                        [left, 0.0],
                        [left + 0.5, 0.0],
                        [left + 0.5, 0.3],
                        [left, 0.3],
                    ],
                    Vec::new(),
                )
            })
            .collect::<Vec<_>>();
        let result = subtract_planar_cutters(&host, &cutters, "batch".into()).unwrap();
        result.brep.validate().unwrap();
        assert!((volume(&result.brep).abs() - 14.85).abs() < 1.0e-6);
        for index in 0..10 {
            let station = 1.25 + index as f64 * 1.8;
            assert_eq!(
                super::super::query::classify_point(&result.brep, [station, 0.15, 1.0]).unwrap(),
                super::super::query::PointClassification::Outside,
            );
        }
        assert!(result
            .report
            .face_mappings
            .iter()
            .any(|mapping| mapping.source.entity == "opening-9"));
    }

    #[test]
    fn angled_host_cap_accepts_flush_lower_cutout_cutter() {
        let host = extrusion(
            "angled-host",
            vec![[0.0, 0.0], [10.0, 0.0], [10.0, 0.3], [0.3, 0.3], [0.0, 0.1]],
            Vec::new(),
        );
        let cutter = extrusion_span(
            "lower_cutout",
            0.0,
            2.1,
            vec![[2.0, 0.0], [3.0, 0.0], [3.0, 0.3], [2.0, 0.3]],
            Vec::new(),
        );
        let result =
            boolean_brep(&host, &cutter, BooleanOp::Subtraction, "angled-cut".into()).unwrap();
        result.brep.validate().unwrap();
        assert!((volume(&result.brep).abs() - (volume(&host).abs() - 0.63)).abs() < 1.0e-6);
        assert_eq!(
            super::super::query::classify_point(&result.brep, [2.5, 0.15, 1.0]).unwrap(),
            super::super::query::PointClassification::Outside
        );
        let raised_cutout = extrusion_span(
            "raised_cutout",
            0.8,
            1.2,
            vec![[6.0, 0.0], [7.0, 0.0], [7.0, 0.3], [6.0, 0.3]],
            Vec::new(),
        );
        let chained = boolean_brep(
            &result.brep,
            &raised_cutout,
            BooleanOp::Subtraction,
            "angled-chain".into(),
        )
        .unwrap();
        chained.brep.validate().unwrap();
        assert!((volume(&chained.brep).abs() - (volume(&host).abs() - 0.99)).abs() < 1.0e-6);
        assert_eq!(
            super::super::query::classify_point(&chained.brep, [6.5, 0.15, 1.2]).unwrap(),
            super::super::query::PointClassification::Outside
        );
        assert_eq!(
            super::super::query::classify_point(&chained.brep, [6.5, 0.15, 2.5]).unwrap(),
            super::super::query::PointClassification::Inside
        );
    }

    #[test]
    fn full_height_planar_opening_splits_angled_host_into_two_solids() {
        let host = extrusion(
            "angled-host",
            vec![[0.0, 0.0], [10.0, 0.0], [10.0, 0.3], [0.3, 0.3], [0.0, 0.1]],
            Vec::new(),
        );
        let cutter = extrusion(
            "through-lower_cutout",
            vec![[4.0, 0.0], [5.0, 0.0], [5.0, 0.3], [4.0, 0.3]],
            Vec::new(),
        );
        let result = boolean_brep(
            &host,
            &cutter,
            BooleanOp::Subtraction,
            "split-angled".into(),
        )
        .unwrap();
        result.brep.validate().unwrap();
        assert_eq!(result.brep.solids.len(), 2);
        assert!((volume(&result.brep).abs() - (volume(&host).abs() - 0.9)).abs() < 1.0e-6);
    }

    #[test]
    fn overlapping_flush_openings_on_angled_host_remove_the_union_once() {
        let host = extrusion(
            "angled-host",
            vec![[0.0, 0.0], [10.0, 0.0], [10.0, 0.3], [0.3, 0.3], [0.0, 0.1]],
            Vec::new(),
        );
        let first = extrusion_span(
            "first",
            0.0,
            2.1,
            vec![[2.0, 0.0], [3.0, 0.0], [3.0, 0.3], [2.0, 0.3]],
            Vec::new(),
        );
        let second = extrusion_span(
            "second",
            0.0,
            2.1,
            vec![[2.5, 0.0], [3.5, 0.0], [3.5, 0.3], [2.5, 0.3]],
            Vec::new(),
        );
        let once = boolean_brep(&host, &first, BooleanOp::Subtraction, "once".into()).unwrap();
        let twice =
            boolean_brep(&once.brep, &second, BooleanOp::Subtraction, "twice".into()).unwrap();
        twice.brep.validate().unwrap();
        assert!((volume(&twice.brep).abs() - (volume(&host).abs() - 0.945)).abs() < 1.0e-6);
    }

    #[test]
    fn layered_planar_cut_supports_an_internal_cavity_shell() {
        let host = extrusion(
            "angled-host",
            vec![
                [0.0, 0.0],
                [10.0, 0.0],
                [10.0, 10.0],
                [0.3, 10.0],
                [0.0, 9.8],
            ],
            Vec::new(),
        );
        let cutter = extrusion_span(
            "cavity",
            1.0,
            1.0,
            vec![[4.0, 4.0], [6.0, 4.0], [6.0, 6.0], [4.0, 6.0]],
            Vec::new(),
        );
        let result = boolean_brep(
            &host,
            &cutter,
            BooleanOp::Subtraction,
            "cavity-result".into(),
        )
        .unwrap();
        result.brep.validate().unwrap();
        assert_eq!(result.brep.solids.len(), 1);
        assert_eq!(result.brep.solids[0].cavity_shells.len(), 1);
        assert!((volume(&result.brep).abs() - (volume(&host).abs() - 4.0)).abs() < 1.0e-6);
    }

    #[test]
    fn angled_host_accepts_rotated_planar_cutter_frame() {
        let host = extrusion(
            "angled-host",
            vec![[0.0, 0.0], [10.0, 0.0], [10.0, 0.3], [0.3, 0.3], [0.0, 0.1]],
            Vec::new(),
        );
        let angle = std::f64::consts::FRAC_PI_6;
        let cutter = primitives::linear_extrusion(
            "rotated-cutter".into(),
            Frame3 {
                origin: [2.5, 0.15, 0.0],
                x: [angle.cos(), angle.sin(), 0.0],
                y: [-angle.sin(), angle.cos(), 0.0],
                z: [0.0, 0.0, 1.0],
            },
            vec![[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]],
            Vec::new(),
            2.1,
            host.accuracy,
        )
        .unwrap();
        let result =
            boolean_brep(&host, &cutter, BooleanOp::Subtraction, "rotated-cut".into()).unwrap();
        result.brep.validate().unwrap();
        assert_eq!(
            super::super::query::classify_point(&result.brep, [2.5, 0.15, 1.0]).unwrap(),
            super::super::query::PointClassification::Outside
        );
        assert_eq!(
            super::super::query::classify_point(&result.brep, [2.5, 0.15, 2.5]).unwrap(),
            super::super::query::PointClassification::Inside
        );
        assert!(volume(&result.brep).abs() < volume(&host).abs());
    }

    #[test]
    fn oblique_planar_cutter_crosses_a_host_without_tessellating_the_boolean() {
        let host = extrusion(
            "oblique-host",
            vec![[0.0, 0.0], [10.0, 0.0], [10.0, 0.3], [0.3, 0.3], [0.0, 0.1]],
            Vec::new(),
        );
        let angle = std::f64::consts::PI / 12.0;
        let cutter = primitives::linear_extrusion(
            "oblique-cutter".into(),
            Frame3 {
                origin: [2.5, 0.15, 0.0],
                x: [angle.cos(), 0.0, -angle.sin()],
                y: [0.0, 1.0, 0.0],
                z: [angle.sin(), 0.0, angle.cos()],
            },
            vec![[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]],
            Vec::new(),
            2.1,
            host.accuracy,
        )
        .unwrap();
        let result =
            boolean_brep(&host, &cutter, BooleanOp::Subtraction, "oblique-cut".into()).unwrap();
        result.brep.validate().unwrap();
        assert!(matches!(result.report.quality, GeometryQuality::Analytic));
        assert_eq!(
            super::super::query::classify_point(&result.brep, [2.8, 0.15, 1.0]).unwrap(),
            super::super::query::PointClassification::Outside
        );
        assert_eq!(
            super::super::query::classify_point(&result.brep, [2.8, 0.15, 2.5]).unwrap(),
            super::super::query::PointClassification::Inside
        );
        assert!(volume(&result.brep).abs() < volume(&host).abs());
    }

    #[test]
    fn chained_oblique_planar_cuts_keep_both_voids_and_analytic_faces() {
        let host = extrusion(
            "chained-oblique-host",
            vec![[0.0, 0.0], [10.0, 0.0], [10.0, 0.3], [0.3, 0.3], [0.0, 0.1]],
            Vec::new(),
        );
        let angle = std::f64::consts::PI / 12.0;
        let cutter = |name: &str, station: f64, sign: f64| {
            primitives::linear_extrusion(
                name.into(),
                Frame3 {
                    origin: [station, 0.15, 0.0],
                    x: [angle.cos(), 0.0, -sign * angle.sin()],
                    y: [0.0, 1.0, 0.0],
                    z: [sign * angle.sin(), 0.0, angle.cos()],
                },
                vec![[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]],
                Vec::new(),
                2.1,
                host.accuracy,
            )
            .unwrap()
        };
        let first = cutter("first-oblique", 2.5, 1.0);
        let second = cutter("second-oblique", 5.0, -1.0);
        let once = boolean_brep(&host, &first, BooleanOp::Subtraction, "once".into()).unwrap();
        let twice =
            boolean_brep(&once.brep, &second, BooleanOp::Subtraction, "twice".into()).unwrap();
        twice.brep.validate().unwrap();
        assert!(matches!(twice.report.quality, GeometryQuality::Analytic));
        for point in [[2.8, 0.15, 1.0], [4.7, 0.15, 1.0]] {
            assert_eq!(
                super::super::query::classify_point(&twice.brep, point).unwrap(),
                super::super::query::PointClassification::Outside
            );
        }
        assert_eq!(
            super::super::query::classify_point(&twice.brep, [3.8, 0.15, 1.0]).unwrap(),
            super::super::query::PointClassification::Inside
        );
        assert!(volume(&twice.brep).abs() < volume(&once.brep).abs());
    }

    #[test]
    fn oblique_cut_accepts_a_disconnected_planar_host() {
        let host = extrusion(
            "split-host",
            vec![[0.0, 0.0], [10.0, 0.0], [10.0, 0.3], [0.0, 0.3]],
            Vec::new(),
        );
        let through = extrusion_span(
            "through",
            -1.0,
            5.0,
            vec![[3.0, -1.0], [4.0, -1.0], [4.0, 1.0], [3.0, 1.0]],
            Vec::new(),
        );
        let split = boolean_brep(&host, &through, BooleanOp::Subtraction, "split".into()).unwrap();
        assert_eq!(split.brep.solids.len(), 2);
        let angle = std::f64::consts::PI / 12.0;
        let oblique = primitives::linear_extrusion(
            "split-oblique".into(),
            Frame3 {
                origin: [6.0, 0.15, 0.0],
                x: [angle.cos(), 0.0, -angle.sin()],
                y: [0.0, 1.0, 0.0],
                z: [angle.sin(), 0.0, angle.cos()],
            },
            vec![[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]],
            Vec::new(),
            2.1,
            host.accuracy,
        )
        .unwrap();
        let result = boolean_brep(
            &split.brep,
            &oblique,
            BooleanOp::Subtraction,
            "split-oblique-result".into(),
        )
        .unwrap();
        result.brep.validate().unwrap();
        assert_eq!(result.brep.solids.len(), 2);
        assert_eq!(
            super::super::query::classify_point(&result.brep, [6.3, 0.15, 1.0]).unwrap(),
            super::super::query::PointClassification::Outside
        );
        assert_eq!(
            super::super::query::classify_point(&result.brep, [1.0, 0.15, 1.0]).unwrap(),
            super::super::query::PointClassification::Inside
        );
    }

    #[test]
    fn planar_cutter_tilts_about_both_host_axes_keep_exact_occupancy() {
        let host = extrusion(
            "tilt-sweep-host",
            vec![[0.0, 0.0], [10.0, 0.0], [10.0, 0.3], [0.0, 0.3]],
            Vec::new(),
        );
        for axis in 0..2 {
            for degrees in [-25.0_f64, -15.0, -5.0, 5.0, 15.0, 25.0] {
                let angle = degrees.to_radians();
                let frame = if axis == 0 {
                    Frame3 {
                        origin: [2.5, 0.15, 0.0],
                        x: [angle.cos(), 0.0, -angle.sin()],
                        y: [0.0, 1.0, 0.0],
                        z: [angle.sin(), 0.0, angle.cos()],
                    }
                } else {
                    Frame3 {
                        origin: [2.5, 0.15, 0.0],
                        x: [1.0, 0.0, 0.0],
                        y: [0.0, angle.cos(), -angle.sin()],
                        z: [0.0, angle.sin(), angle.cos()],
                    }
                };
                let cutter = primitives::linear_extrusion(
                    format!("tilt-{axis}-{degrees}"),
                    frame,
                    vec![[-0.8, -0.8], [0.8, -0.8], [0.8, 0.8], [-0.8, 0.8]],
                    Vec::new(),
                    2.1,
                    host.accuracy,
                )
                .unwrap();
                let result = boolean_brep(
                    &host,
                    &cutter,
                    BooleanOp::Subtraction,
                    format!("tilt-result-{axis}-{degrees}"),
                )
                .unwrap();
                result.brep.validate().unwrap();
                assert_eq!(
                    super::super::query::classify_point(&result.brep, [2.5, 0.15, 1.0]).unwrap(),
                    super::super::query::PointClassification::Outside,
                    "axis={axis}, degrees={degrees}"
                );
                assert_eq!(
                    super::super::query::classify_point(&result.brep, [8.0, 0.15, 1.0]).unwrap(),
                    super::super::query::PointClassification::Inside,
                    "axis={axis}, degrees={degrees}"
                );
                assert!(volume(&result.brep).abs() < volume(&host).abs());
            }
        }
    }

    #[test]
    fn oblique_cut_preserves_a_planar_host_with_profile_hole_loops() {
        let host = extrusion(
            "profile-hole-host",
            vec![[0.0, 0.0], [10.0, 0.0], [10.0, 3.0], [0.0, 3.0]],
            vec![vec![[2.0, 1.0], [3.0, 1.0], [3.0, 2.0], [2.0, 2.0]]],
        );
        assert!(host
            .topology
            .faces
            .iter()
            .any(|face| !face.trim.holes.is_empty()));
        let angle = std::f64::consts::PI / 12.0;
        let oblique = primitives::linear_extrusion(
            "pocket-oblique".into(),
            Frame3 {
                origin: [6.0, 1.5, 0.0],
                x: [angle.cos(), 0.0, -angle.sin()],
                y: [0.0, 1.0, 0.0],
                z: [angle.sin(), 0.0, angle.cos()],
            },
            vec![[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]],
            Vec::new(),
            2.1,
            host.accuracy,
        )
        .unwrap();
        let result =
            boolean_brep(&host, &oblique, BooleanOp::Subtraction, "two-voids".into()).unwrap();
        result.brep.validate().unwrap();
        for point in [[2.5, 1.5, 1.5], [6.3, 1.5, 1.0]] {
            assert_eq!(
                super::super::query::classify_point(&result.brep, point).unwrap(),
                super::super::query::PointClassification::Outside
            );
        }
        for point in [[4.0, 1.5, 1.0], [6.3, 1.5, 2.5]] {
            assert_eq!(
                super::super::query::classify_point(&result.brep, point).unwrap(),
                super::super::query::PointClassification::Inside
            );
        }
    }

    #[test]
    fn oblique_nonconvex_planar_cutter_keeps_exact_host_material() {
        let host = extrusion(
            "nonconvex-host",
            vec![[0.0, 0.0], [10.0, 0.0], [10.0, 0.3], [0.0, 0.3]],
            Vec::new(),
        );
        let angle = std::f64::consts::PI / 12.0;
        let cutter = primitives::linear_extrusion(
            "nonconvex-cutter".into(),
            Frame3 {
                origin: [5.0, 0.15, 0.0],
                x: [angle.cos(), 0.0, -angle.sin()],
                y: [0.0, 1.0, 0.0],
                z: [angle.sin(), 0.0, angle.cos()],
            },
            vec![
                [-1.0, -1.0],
                [1.0, -1.0],
                [1.0, -0.05],
                [0.0, -0.05],
                [0.0, 1.0],
                [-1.0, 1.0],
            ],
            Vec::new(),
            2.1,
            host.accuracy,
        )
        .unwrap();
        let result = boolean_brep(
            &host,
            &cutter,
            BooleanOp::Subtraction,
            "nonconvex-oblique-cut".into(),
        )
        .unwrap();
        result.brep.validate().unwrap();
        assert!(matches!(result.report.quality, GeometryQuality::Analytic));
        assert_eq!(
            super::super::query::classify_point(&result.brep, [4.5, 0.15, 1.0]).unwrap(),
            super::super::query::PointClassification::Outside
        );
        assert_eq!(
            super::super::query::classify_point(&result.brep, [5.5, 0.15, 1.0]).unwrap(),
            super::super::query::PointClassification::Inside
        );
    }

    #[test]
    fn oblique_planar_cut_preserves_an_existing_internal_cavity_shell() {
        let host = extrusion(
            "cavity-host",
            vec![[0.0, 0.0], [10.0, 0.0], [10.0, 3.0], [0.0, 3.0]],
            Vec::new(),
        );
        let cavity = extrusion_span(
            "internal-cavity",
            1.0,
            1.0,
            vec![[2.0, 1.0], [3.0, 1.0], [3.0, 2.0], [2.0, 2.0]],
            Vec::new(),
        );
        let host = boolean_brep(&host, &cavity, BooleanOp::Subtraction, "cavity-host".into())
            .unwrap()
            .brep;
        assert_eq!(host.solids[0].cavity_shells.len(), 1);
        let angle = std::f64::consts::PI / 12.0;
        let cutter = primitives::linear_extrusion(
            "oblique-after-cavity".into(),
            Frame3 {
                origin: [6.0, 1.5, 0.0],
                x: [angle.cos(), 0.0, -angle.sin()],
                y: [0.0, 1.0, 0.0],
                z: [angle.sin(), 0.0, angle.cos()],
            },
            vec![[-0.5, -2.0], [0.5, -2.0], [0.5, 2.0], [-0.5, 2.0]],
            Vec::new(),
            2.1,
            host.accuracy,
        )
        .unwrap();
        let result = boolean_brep(
            &host,
            &cutter,
            BooleanOp::Subtraction,
            "oblique-cavity-cut".into(),
        )
        .unwrap();
        result.brep.validate().unwrap();
        assert_eq!(result.brep.solids.len(), 1);
        assert_eq!(result.brep.solids[0].cavity_shells.len(), 1);
        assert_eq!(
            super::super::query::classify_point(&result.brep, [2.5, 1.5, 1.5]).unwrap(),
            super::super::query::PointClassification::Outside
        );
        assert_eq!(
            super::super::query::classify_point(&result.brep, [6.2, 1.5, 1.0]).unwrap(),
            super::super::query::PointClassification::Outside
        );
        assert_eq!(
            super::super::query::classify_point(&result.brep, [6.2, 1.5, 2.6]).unwrap(),
            super::super::query::PointClassification::Inside
        );
    }

    #[test]
    fn oblique_cut_assigns_a_remote_cavity_to_its_original_material_component() {
        let left = extrusion(
            "left-cavity-host",
            vec![[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]],
            Vec::new(),
        );
        let cavity = extrusion_span(
            "left-cavity",
            1.0,
            1.0,
            vec![[1.0, 1.0], [2.0, 1.0], [2.0, 2.0], [1.0, 2.0]],
            Vec::new(),
        );
        let left_with_cavity = boolean_brep(
            &left,
            &cavity,
            BooleanOp::Subtraction,
            "left-with-cavity".into(),
        )
        .unwrap()
        .brep;
        let right = extrusion(
            "right-host",
            vec![[6.0, 0.0], [10.0, 0.0], [10.0, 3.0], [6.0, 3.0]],
            Vec::new(),
        );
        let host = boolean_brep(
            &left_with_cavity,
            &right,
            BooleanOp::Union,
            "two-components".into(),
        )
        .unwrap()
        .brep;
        assert_eq!(host.solids.len(), 2);
        let angle = std::f64::consts::PI / 12.0;
        let cutter = primitives::linear_extrusion(
            "right-oblique-cutter".into(),
            Frame3 {
                origin: [8.5, 1.5, 0.0],
                x: [angle.cos(), 0.0, -angle.sin()],
                y: [0.0, 1.0, 0.0],
                z: [angle.sin(), 0.0, angle.cos()],
            },
            vec![[-0.5, -2.0], [0.5, -2.0], [0.5, 2.0], [-0.5, 2.0]],
            Vec::new(),
            2.1,
            host.accuracy,
        )
        .unwrap();
        let result = boolean_brep(
            &host,
            &cutter,
            BooleanOp::Subtraction,
            "remote-cavity-cut".into(),
        )
        .unwrap();
        result.brep.validate().unwrap();
        assert_eq!(result.brep.solids.len(), 2);
        assert_eq!(
            result
                .brep
                .solids
                .iter()
                .map(|solid| solid.cavity_shells.len())
                .sum::<usize>(),
            1
        );
        assert_eq!(
            super::super::query::classify_point(&result.brep, [1.5, 1.5, 1.5]).unwrap(),
            super::super::query::PointClassification::Outside
        );
        assert_eq!(
            super::super::query::classify_point(&result.brep, [8.5, 1.5, 1.0]).unwrap(),
            super::super::query::PointClassification::Outside
        );
    }

    #[test]
    fn coaxial_conic_containment_builds_exact_cavities_and_ownership() {
        let accuracy = sphere("accuracy", [0.0; 3], 1.0).accuracy;
        let host =
            primitives::frustum("host".into(), Frame3::IDENTITY, 3.0, 4.0, 4.0, accuracy).unwrap();
        let cutter = primitives::frustum(
            "cutter".into(),
            Frame3 {
                origin: [0.0, 0.0, 1.0],
                ..Frame3::IDENTITY
            },
            1.0,
            1.5,
            1.0,
            accuracy,
        )
        .unwrap();
        let host_volume = std::f64::consts::PI * 4.0 * (9.0 + 12.0 + 16.0) / 3.0;
        let cutter_volume = std::f64::consts::PI * (1.0 + 1.5 + 2.25) / 3.0;
        for (operation, expected_volume, expected_faces) in [
            (BooleanOp::Union, host_volume, 3),
            (BooleanOp::Intersection, cutter_volume, 3),
            (BooleanOp::Subtraction, host_volume - cutter_volume, 6),
        ] {
            let result =
                boolean_brep(&host, &cutter, operation, format!("conic-{operation:?}")).unwrap();
            result.brep.validate().unwrap();
            assert_eq!(result.brep.topology.faces.len(), expected_faces);
            assert_eq!(result.report.face_mappings.len(), 6);
            let measured = volume(&result.brep).abs();
            assert!(
                (measured - expected_volume).abs() < 0.75,
                "{operation:?}: measured {measured}, expected {expected_volume}"
            );
            if operation == BooleanOp::Subtraction {
                assert_eq!(result.brep.solids[0].cavity_shells.len(), 1);
                assert_eq!(
                    result
                        .brep
                        .topology
                        .faces
                        .iter()
                        .filter(|face| face.provenance.role == FaceRole::Cut
                            && face.provenance.reversed)
                        .count(),
                    3
                );
            }
        }

        let touching =
            primitives::frustum("touching".into(), Frame3::IDENTITY, 1.0, 1.5, 1.0, accuracy)
                .unwrap();
        assert!(matches!(
            boolean_brep(
                &host,
                &touching,
                BooleanOp::Subtraction,
                "touching-result".into()
            ),
            Err(GeometryError::UnresolvedIntersection(_))
        ));
    }

    #[test]
    fn mixed_conic_containment_preserves_all_analytic_supports() {
        let accuracy = sphere("accuracy", [0.0; 3], 1.0).accuracy;
        let conic =
            primitives::frustum("frustum".into(), Frame3::IDENTITY, 3.0, 4.0, 4.0, accuracy)
                .unwrap();
        let inner_sphere = primitives::sphere(
            "inner-sphere".into(),
            Frame3 {
                origin: [0.0, 0.0, 2.0],
                ..Frame3::IDENTITY
            },
            0.5,
            accuracy,
        )
        .unwrap();
        let outer_sphere = primitives::sphere(
            "outer-sphere".into(),
            Frame3 {
                origin: [0.0, 0.0, 2.0],
                ..Frame3::IDENTITY
            },
            6.0,
            accuracy,
        )
        .unwrap();
        let inner_cylinder = primitives::cylinder(
            "inner-cylinder".into(),
            Frame3 {
                origin: [0.0, 0.0, 1.0],
                ..Frame3::IDENTITY
            },
            0.5,
            1.0,
            accuracy,
        )
        .unwrap();
        let outer_cylinder = primitives::cylinder(
            "outer-cylinder".into(),
            Frame3 {
                origin: [0.0, 0.0, -1.0],
                ..Frame3::IDENTITY
            },
            5.0,
            6.0,
            accuracy,
        )
        .unwrap();
        let inner_box = primitives::cuboid(
            "inner-box".into(),
            Frame3 {
                origin: [-0.25, -0.25, 1.75],
                ..Frame3::IDENTITY
            },
            [0.5, 0.5, 0.5],
            accuracy,
        )
        .unwrap();
        let outer_box = primitives::cuboid(
            "outer-box".into(),
            Frame3 {
                origin: [-5.0, -5.0, -1.0],
                ..Frame3::IDENTITY
            },
            [10.0, 10.0, 6.0],
            accuracy,
        )
        .unwrap();

        for (host, cutter, expected_faces) in [
            (&conic, &inner_sphere, 4),
            (&outer_sphere, &conic, 4),
            (&conic, &inner_cylinder, 6),
            (&outer_cylinder, &conic, 6),
            (&conic, &inner_box, 9),
            (&outer_box, &conic, 9),
        ] {
            let result = boolean_brep(
                host,
                cutter,
                BooleanOp::Subtraction,
                format!("{}-minus-{}", host.id, cutter.id),
            )
            .unwrap();
            result.brep.validate().unwrap();
            assert_eq!(result.brep.solids.len(), 1);
            assert_eq!(result.brep.solids[0].cavity_shells.len(), 1);
            assert_eq!(result.brep.topology.faces.len(), expected_faces);
            assert_eq!(
                result
                    .brep
                    .topology
                    .faces
                    .iter()
                    .filter(|face| face.provenance.role == FaceRole::Cut
                        && face.provenance.reversed)
                    .count(),
                cutter.topology.faces.len()
            );
            tessellate(&result.brep, 0.02, 2_000_000).unwrap();
        }
    }

    #[test]
    fn generic_closed_loop_imprints_an_off_axis_sphere_cone_intersection() {
        let accuracy = cylinder("accuracy", 0.0, 1.0, false).accuracy;
        let cone = primitives::cone("cone".into(), Frame3::IDENTITY, 2.0, 2.0, accuracy).unwrap();
        let sphere = primitives::sphere(
            "sphere".into(),
            Frame3 {
                origin: [1.42, 0.0, 1.0],
                ..Frame3::IDENTITY
            },
            0.35,
            accuracy,
        )
        .unwrap();
        for operation in [
            BooleanOp::Union,
            BooleanOp::Intersection,
            BooleanOp::Subtraction,
        ] {
            let result = boolean_brep(
                &cone,
                &sphere,
                operation,
                format!("sphere-cone-{operation:?}"),
            )
            .unwrap_or_else(|error| panic!("{operation:?}: {error}"));
            assert_eq!(result.brep.solids.len(), 1);
            assert!(result
                .brep
                .geometry
                .curves
                .iter()
                .any(|curve| matches!(curve, CurveGeometry::Intersection { .. })));
            assert!(result
                .brep
                .topology
                .faces
                .iter()
                .all(|face| !face.provenance.sources.is_empty()));
            result.brep.validate().unwrap();
            tessellate(&result.brep, 0.01, 2_000_000)
                .unwrap_or_else(|error| panic!("{operation:?} tessellation: {error}"));
        }
    }

    #[test]
    fn generic_closed_loop_handles_periodic_winding() {
        let accuracy = cylinder("accuracy", 0.0, 1.0, false).accuracy;
        let cone = primitives::cone("cone".into(), Frame3::IDENTITY, 2.0, 2.0, accuracy).unwrap();
        let sphere = primitives::sphere(
            "sphere".into(),
            Frame3 {
                origin: [1.1, 0.0, 0.8],
                ..Frame3::IDENTITY
            },
            0.35,
            accuracy,
        )
        .unwrap();
        let result =
            boolean_brep(&cone, &sphere, BooleanOp::Intersection, "winding".into()).unwrap();
        result.brep.validate().unwrap();
        tessellate(&result.brep, 0.01, 2_000_000).unwrap();
    }

    #[test]
    fn ring_torus_containment_uses_exact_host_support_bounds() {
        let accuracy = sphere("accuracy", [0.0; 3], 1.0).accuracy;
        let torus = primitives::torus(
            "torus".into(),
            Frame3 {
                origin: [0.0, 0.0, 2.0],
                ..Frame3::IDENTITY
            },
            2.0,
            0.5,
            accuracy,
        )
        .unwrap();
        let cylinder =
            primitives::cylinder("cylinder".into(), Frame3::IDENTITY, 4.0, 4.0, accuracy).unwrap();
        let box_ = primitives::cuboid(
            "box".into(),
            Frame3 {
                origin: [-4.0, -4.0, -1.0],
                ..Frame3::IDENTITY
            },
            [8.0, 8.0, 6.0],
            accuracy,
        )
        .unwrap();
        let conic =
            primitives::frustum("frustum".into(), Frame3::IDENTITY, 4.0, 5.0, 4.0, accuracy)
                .unwrap();
        for host in [&cylinder, &box_, &conic] {
            let result = boolean_brep(
                host,
                &torus,
                BooleanOp::Subtraction,
                format!("{}-minus-torus", host.id),
            )
            .unwrap();
            result.brep.validate().unwrap();
            assert_eq!(result.brep.solids.len(), 1);
            assert_eq!(result.brep.solids[0].cavity_shells.len(), 1);
            assert!(result.brep.topology.faces.iter().any(|face| {
                matches!(
                    result.brep.geometry.surfaces[face.surface as usize],
                    SurfaceGeometry::Torus { .. }
                ) && face.provenance.role == FaceRole::Cut
                    && face.provenance.reversed
            }));
            tessellate(&result.brep, 0.02, 2_000_000).unwrap();
        }
    }

    #[test]
    fn restricted_shell_offsets_supported_closed_families() {
        let accuracy = sphere("accuracy", [0.0; 3], 1.0).accuracy;
        let inputs = [
            (
                primitives::cuboid("box".into(), Frame3::IDENTITY, [2.0; 3], accuracy).unwrap(),
                12,
            ),
            (
                primitives::cylinder("cylinder".into(), Frame3::IDENTITY, 1.0, 2.0, accuracy)
                    .unwrap(),
                6,
            ),
            (
                primitives::sphere("sphere".into(), Frame3::IDENTITY, 1.0, accuracy).unwrap(),
                2,
            ),
            (
                primitives::torus("torus".into(), Frame3::IDENTITY, 3.0, 1.0, accuracy).unwrap(),
                2,
            ),
            (
                primitives::cone("cone".into(), Frame3::IDENTITY, 2.0, 3.0, accuracy).unwrap(),
                4,
            ),
            (
                primitives::frustum("frustum".into(), Frame3::IDENTITY, 2.0, 1.0, 3.0, accuracy)
                    .unwrap(),
                6,
            ),
            (
                extrusion(
                    "profile-shell",
                    vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]],
                    vec![vec![[1.0, 1.0], [1.0, 3.0], [3.0, 3.0], [3.0, 1.0]]],
                ),
                20,
            ),
        ];
        for (input, expected_faces) in &inputs {
            let result = shell_brep(&input, 0.2, format!("{}-shell", input.id)).unwrap();
            result.brep.validate().unwrap();
            assert_eq!(result.brep.solids.len(), 1);
            assert_eq!(result.brep.solids[0].cavity_shells.len(), 1);
            assert_eq!(result.brep.topology.faces.len(), *expected_faces);
            assert_eq!(
                result.report.face_mappings.len(),
                input.topology.faces.len()
            );
            assert!(result.brep.topology.faces.iter().any(|face| {
                face.provenance.role == FaceRole::Cut
                    && face.provenance.reversed
                    && face
                        .provenance
                        .sources
                        .iter()
                        .all(|source| source.entity == input.id)
            }));
            tessellate(&result.brep, 0.02, 2_000_000).unwrap();
        }
        assert!(matches!(
            shell_brep(&inputs[2].0, 1.0, "invalid".into()),
            Err(GeometryError::UnresolvedIntersection(_))
        ));
    }

    #[test]
    fn identical_general_analytic_body_uses_coincident_ownership() {
        let accuracy = sphere("accuracy", [0.0; 3], 1.0).accuracy;
        let host = primitives::annular_sector_extrusion(
            "host".into(),
            Frame3::IDENTITY,
            3.0,
            0.4,
            2.0,
            0.2,
            1.4,
            accuracy,
        )
        .unwrap();
        let union = boolean_brep(&host, &host, BooleanOp::Union, "same".into()).unwrap();
        assert!(union.report.coincident);
        assert_eq!(union.brep.topology.faces.len(), host.topology.faces.len());
        assert!(union
            .brep
            .topology
            .faces
            .iter()
            .all(|face| face.provenance.role == FaceRole::Coincident));
        let empty = boolean_brep(&host, &host, BooleanOp::Subtraction, "empty".into()).unwrap();
        assert!(empty.brep.solids.is_empty());
    }

    #[test]
    fn generic_pipeline_classifies_noncanonical_planar_solid_containment() {
        let accuracy = sphere("accuracy", [0.0; 3], 1.0).accuracy;
        let tetrahedron = primitives::planar_polyhedron(
            "tetrahedron".into(),
            vec![
                [-0.2, -0.2, 0.8],
                [0.2, -0.2, 0.8],
                [0.0, 0.2, 0.8],
                [0.0, 0.0, 1.2],
            ],
            vec![vec![0, 2, 1], vec![0, 1, 3], vec![0, 3, 2], vec![1, 2, 3]],
            accuracy,
        )
        .unwrap();
        let cylinder =
            primitives::cylinder("cylinder".into(), Frame3::IDENTITY, 2.0, 2.0, accuracy).unwrap();

        let union =
            boolean_brep(&cylinder, &tetrahedron, BooleanOp::Union, "union".into()).unwrap();
        assert_eq!(
            union.brep.topology.faces.len(),
            cylinder.topology.faces.len()
        );

        let intersection = boolean_brep(
            &cylinder,
            &tetrahedron,
            BooleanOp::Intersection,
            "intersection".into(),
        )
        .unwrap();
        assert_eq!(
            intersection.brep.topology.faces.len(),
            tetrahedron.topology.faces.len()
        );

        let subtraction = boolean_brep(
            &cylinder,
            &tetrahedron,
            BooleanOp::Subtraction,
            "subtraction".into(),
        )
        .unwrap();
        assert_eq!(subtraction.brep.solids.len(), 1);
        assert_eq!(subtraction.brep.solids[0].cavity_shells.len(), 1);
        subtraction.brep.validate().unwrap();
        tessellate(&subtraction.brep, 0.01, 2_000_000).unwrap();
    }

    #[test]
    fn annular_sector_accepts_an_exact_vertical_sector_opening() {
        let accuracy = sphere("accuracy", [0.0; 3], 1.0).accuracy;
        let host = primitives::annular_cylinder(
            "ring-host".into(),
            Frame3::IDENTITY,
            1.9,
            2.1,
            3.0,
            accuracy,
        )
        .unwrap();
        let reach = 2.3;
        let cutter = primitives::linear_extrusion(
            "ring-opening".into(),
            Frame3 {
                origin: [0.0, 0.0, 0.5],
                ..Frame3::IDENTITY
            },
            vec![
                [0.0, 0.0],
                [reach * (-0.1_f64).cos(), reach * (-0.1_f64).sin()],
                [reach * 0.1_f64.cos(), reach * 0.1_f64.sin()],
            ],
            vec![],
            2.0,
            accuracy,
        )
        .unwrap();
        let cut = boolean_brep(&host, &cutter, BooleanOp::Subtraction, "ring-cut".into()).unwrap();
        cut.brep.validate().unwrap();
        tessellate(&cut.brep, 0.01, 2_000_000).unwrap();
        assert_eq!(
            super::super::query::classify_point(&cut.brep, [2.0, 0.0, 1.0]).unwrap(),
            super::super::query::PointClassification::Outside
        );
        assert_eq!(
            super::super::query::classify_point(&cut.brep, [0.0, 2.0, 1.0]).unwrap(),
            super::super::query::PointClassification::Inside
        );
        assert_eq!(
            super::super::query::classify_point(&cut.brep, [0.0, 0.0, 1.0]).unwrap(),
            super::super::query::PointClassification::Outside
        );
        let second = primitives::linear_extrusion(
            "second-ring-opening".into(),
            Frame3 {
                origin: [0.0, 0.0, 0.8],
                ..Frame3::IDENTITY
            },
            vec![
                [0.0, 0.0],
                [
                    reach * (std::f64::consts::FRAC_PI_2 - 0.1).cos(),
                    reach * (std::f64::consts::FRAC_PI_2 - 0.1).sin(),
                ],
                [
                    reach * (std::f64::consts::FRAC_PI_2 + 0.1).cos(),
                    reach * (std::f64::consts::FRAC_PI_2 + 0.1).sin(),
                ],
            ],
            vec![],
            1.3,
            accuracy,
        )
        .unwrap();
        let chained = boolean_brep(
            &cut.brep,
            &second,
            BooleanOp::Subtraction,
            "ring-cut-chained".into(),
        )
        .unwrap();
        chained.brep.validate().unwrap();
        tessellate(&chained.brep, 0.01, 2_000_000).unwrap();
        for point in [[2.0, 0.0, 1.0], [0.0, 2.0, 1.0]] {
            assert_eq!(
                super::super::query::classify_point(&chained.brep, point).unwrap(),
                super::super::query::PointClassification::Outside
            );
        }
        assert_eq!(
            super::super::query::classify_point(&chained.brep, [-2.0, 0.0, 1.0]).unwrap(),
            super::super::query::PointClassification::Inside
        );
    }

    #[test]
    fn arc_edged_host_accepts_a_vertical_rectangular_opening_cut() {
        let accuracy = sphere("accuracy", [0.0; 3], 1.0).accuracy;
        let quarter = std::f64::consts::FRAC_PI_2;
        let host = primitives::arc_edged_extrusion(
            "arc-host".into(),
            Frame3::IDENTITY,
            vec![
                primitives::ProfileEdge::Arc {
                    center: [0.0, 0.0],
                    radius: 2.0,
                    start_angle: 0.0,
                    sweep_angle: quarter,
                },
                primitives::ProfileEdge::Line {
                    from: [0.0, 2.0],
                    to: [0.0, 1.5],
                },
                primitives::ProfileEdge::Arc {
                    center: [0.0, 0.0],
                    radius: 1.5,
                    start_angle: quarter,
                    sweep_angle: -quarter,
                },
                primitives::ProfileEdge::Line {
                    from: [1.5, 0.0],
                    to: [2.0, 0.0],
                },
            ],
            3.0,
            accuracy,
        )
        .unwrap();
        let cutter = primitives::cuboid(
            "opening".into(),
            Frame3 {
                origin: [1.1, 0.9, 0.5],
                ..Frame3::IDENTITY
            },
            [1.0, 0.35, 1.5],
            accuracy,
        )
        .unwrap();
        let assert_curved_normals = |brep: &BrepEnvelope| {
            for (radius, expected) in [(2.0, Orientation::Forward), (1.5, Orientation::Reverse)] {
                let faces = brep.topology.faces.iter().filter(|face| {
                    matches!(
                        brep.geometry.surfaces[face.surface as usize],
                        SurfaceGeometry::Cylinder { radius: value, .. } if (value - radius).abs() < 1e-9
                    )
                }).collect::<Vec<_>>();
                assert!(!faces.is_empty(), "missing curved face at radius {radius}");
                assert!(
                    faces.iter().all(|face| face.sense == expected),
                    "reconstructed curved face at radius {radius} points into material: {:?}",
                    faces
                        .iter()
                        .map(|face| (&face.key, face.sense))
                        .collect::<Vec<_>>()
                );
            }
        };
        let cut = boolean_brep(&host, &cutter, BooleanOp::Subtraction, "arc-cut".into()).unwrap();
        cut.brep.validate().unwrap();
        assert_curved_normals(&cut.brep);
        tessellate(&cut.brep, 0.01, 2_000_000).unwrap();
        assert_eq!(
            super::super::query::classify_point(&cut.brep, [1.5, 1.1, 1.2]).unwrap(),
            super::super::query::PointClassification::Outside
        );
        let second_cutter = primitives::cuboid(
            "second-opening".into(),
            Frame3 {
                origin: [0.6, 1.3, 0.8],
                ..Frame3::IDENTITY
            },
            [0.45, 0.8, 1.2],
            accuracy,
        )
        .unwrap();
        let batch = subtract_planar_cutters(
            &host,
            &[cutter, second_cutter.clone()],
            "arc-cut-batch".into(),
        )
        .unwrap();
        batch.brep.validate().unwrap();
        tessellate(&batch.brep, 0.01, 2_000_000).unwrap();
        assert_curved_normals(&batch.brep);
        for point in [[1.5, 1.1, 1.2], [0.9, 1.65, 1.2]] {
            assert_eq!(
                super::super::query::classify_point(&batch.brep, point).unwrap(),
                super::super::query::PointClassification::Outside
            );
        }
        let chained = boolean_brep(
            &cut.brep,
            &second_cutter,
            BooleanOp::Subtraction,
            "arc-cut-chained".into(),
        )
        .unwrap();
        chained.brep.validate().unwrap();
        assert_curved_normals(&chained.brep);
        for point in [[1.5, 1.1, 1.2], [0.9, 1.65, 1.2]] {
            assert_eq!(
                super::super::query::classify_point(&chained.brep, point).unwrap(),
                super::super::query::PointClassification::Outside
            );
        }
        let overlapping = primitives::cuboid(
            "overlapping-opening".into(),
            Frame3 {
                origin: [1.4, 0.95, 0.2],
                ..Frame3::IDENTITY
            },
            [0.5, 0.4, 2.2],
            accuracy,
        )
        .unwrap();
        let third = boolean_brep(
            &chained.brep,
            &overlapping,
            BooleanOp::Subtraction,
            "arc-cut-third".into(),
        )
        .unwrap();
        third.brep.validate().unwrap();
        tessellate(&third.brep, 0.01, 2_000_000).unwrap();
        assert_curved_normals(&third.brep);
        for point in [
            [1.5, 1.1, 1.2],
            [0.9, 1.65, 1.2],
            [1.45, 1.2, 0.3],
            [1.45, 1.2, 2.2],
        ] {
            assert_eq!(
                super::super::query::classify_point(&third.brep, point).unwrap(),
                super::super::query::PointClassification::Outside
            );
        }
    }

    #[test]
    fn curved_internal_cut_creates_one_cavity_shell_in_its_host_solid() {
        let accuracy = sphere("accuracy", [0.0; 3], 1.0).accuracy;
        let quarter = std::f64::consts::FRAC_PI_2;
        let host = primitives::arc_edged_extrusion(
            "curved-cavity-host".into(),
            Frame3::IDENTITY,
            vec![
                primitives::ProfileEdge::Arc {
                    center: [0.0, 0.0],
                    radius: 2.0,
                    start_angle: 0.0,
                    sweep_angle: quarter,
                },
                primitives::ProfileEdge::Line {
                    from: [0.0, 2.0],
                    to: [0.0, 1.0],
                },
                primitives::ProfileEdge::Arc {
                    center: [0.0, 0.0],
                    radius: 1.0,
                    start_angle: quarter,
                    sweep_angle: -quarter,
                },
                primitives::ProfileEdge::Line {
                    from: [1.0, 0.0],
                    to: [2.0, 0.0],
                },
            ],
            3.0,
            accuracy,
        )
        .unwrap();
        let cutter = primitives::linear_extrusion(
            "curved-interior-cutter".into(),
            Frame3 {
                origin: [0.0, 0.0, 1.0],
                ..Frame3::IDENTITY
            },
            vec![[1.0, 1.0], [1.2, 1.0], [1.2, 1.2], [1.0, 1.2]],
            vec![],
            1.0,
            accuracy,
        )
        .unwrap();
        let result = boolean_brep(
            &host,
            &cutter,
            BooleanOp::Subtraction,
            "curved-cavity".into(),
        )
        .unwrap();
        result.brep.validate().unwrap();
        tessellate(&result.brep, 0.01, 2_000_000).unwrap();
        assert_eq!(result.brep.solids.len(), 1);
        assert_eq!(result.brep.topology.shells.len(), 2);
        assert_eq!(result.brep.solids[0].cavity_shells.len(), 1);
        assert_eq!(
            super::super::query::classify_point(&result.brep, [1.1, 1.1, 1.5]).unwrap(),
            super::super::query::PointClassification::Outside
        );
        assert_eq!(
            super::super::query::classify_point(&result.brep, [1.5, 0.5, 1.5]).unwrap(),
            super::super::query::PointClassification::Inside
        );

        let split_sector = primitives::linear_extrusion(
            "curved-split-sector".into(),
            Frame3 {
                origin: [0.0, 0.0, -1.0],
                ..Frame3::IDENTITY
            },
            vec![
                [0.0, 0.0],
                [3.0 * 0.2_f64.cos(), 3.0 * 0.2_f64.sin()],
                [3.0 * 0.3_f64.cos(), 3.0 * 0.3_f64.sin()],
            ],
            vec![],
            5.0,
            accuracy,
        )
        .unwrap();
        let split = boolean_brep(
            &host,
            &split_sector,
            BooleanOp::Subtraction,
            "curved-split".into(),
        )
        .unwrap();
        assert_eq!(split.brep.solids.len(), 2);
        let split_with_cavity = boolean_brep(
            &split.brep,
            &cutter,
            BooleanOp::Subtraction,
            "curved-split-with-cavity".into(),
        )
        .unwrap();
        split_with_cavity.brep.validate().unwrap();
        assert_eq!(split_with_cavity.brep.solids.len(), 2);
        assert_eq!(split_with_cavity.brep.topology.shells.len(), 3);
        assert_eq!(
            split_with_cavity
                .brep
                .solids
                .iter()
                .map(|solid| solid.cavity_shells.len())
                .sum::<usize>(),
            1
        );
        assert_eq!(
            super::super::query::classify_point(&split_with_cavity.brep, [1.1, 1.1, 1.5]).unwrap(),
            super::super::query::PointClassification::Outside
        );

        let second_cutter = primitives::linear_extrusion(
            "curved-other-component-cutter".into(),
            Frame3 {
                origin: [0.0, 0.0, 1.1],
                ..Frame3::IDENTITY
            },
            vec![[1.55, 0.1], [1.67, 0.1], [1.67, 0.22], [1.55, 0.22]],
            vec![],
            0.8,
            accuracy,
        )
        .unwrap();
        let same_component_cavities = boolean_brep(
            &result.brep,
            &second_cutter,
            BooleanOp::Subtraction,
            "curved-same-component-cavities".into(),
        )
        .unwrap();
        same_component_cavities.brep.validate().unwrap();
        assert_eq!(same_component_cavities.brep.solids.len(), 1);
        assert_eq!(same_component_cavities.brep.topology.shells.len(), 3);
        assert_eq!(
            same_component_cavities.brep.solids[0].cavity_shells.len(),
            2
        );
        let two_cavities = boolean_brep(
            &split_with_cavity.brep,
            &second_cutter,
            BooleanOp::Subtraction,
            "curved-two-cavities".into(),
        )
        .unwrap();
        two_cavities.brep.validate().unwrap();
        assert_eq!(two_cavities.brep.solids.len(), 2);
        assert_eq!(two_cavities.brep.topology.shells.len(), 4);
        assert!(two_cavities
            .brep
            .solids
            .iter()
            .all(|solid| solid.cavity_shells.len() == 1));
        for point in [[1.1, 1.1, 1.5], [1.6, 0.16, 1.5]] {
            assert_eq!(
                super::super::query::classify_point(&two_cavities.brep, point).unwrap(),
                super::super::query::PointClassification::Outside
            );
        }
    }

    #[test]
    fn curved_cut_provenance_selects_the_trimmed_coaxial_source_face() {
        let accuracy = sphere("accuracy", [0.0; 3], 1.0).accuracy;
        let quarter = std::f64::consts::FRAC_PI_2;
        let host = primitives::arc_edged_extrusion(
            "split-arc-host".into(),
            Frame3::IDENTITY,
            vec![
                primitives::ProfileEdge::Arc {
                    center: [0.0, 0.0],
                    radius: 2.0,
                    start_angle: 0.0,
                    sweep_angle: quarter / 2.0,
                },
                primitives::ProfileEdge::Arc {
                    center: [0.0, 0.0],
                    radius: 2.0,
                    start_angle: quarter / 2.0,
                    sweep_angle: quarter / 2.0,
                },
                primitives::ProfileEdge::Line {
                    from: [0.0, 2.0],
                    to: [0.0, 1.5],
                },
                primitives::ProfileEdge::Arc {
                    center: [0.0, 0.0],
                    radius: 1.5,
                    start_angle: quarter,
                    sweep_angle: -quarter,
                },
                primitives::ProfileEdge::Line {
                    from: [1.5, 0.0],
                    to: [2.0, 0.0],
                },
            ],
            3.0,
            accuracy,
        )
        .unwrap();
        let cutter = primitives::cuboid(
            "split-arc-opening".into(),
            Frame3 {
                origin: [1.1, 0.9, 0.5],
                ..Frame3::IDENTITY
            },
            [1.0, 0.35, 1.5],
            accuracy,
        )
        .unwrap();
        let cut = boolean_brep(
            &host,
            &cutter,
            BooleanOp::Subtraction,
            "split-arc-cut".into(),
        )
        .unwrap();
        cut.brep.validate().unwrap();
        let mut outer_faces = 0;
        for face in &cut.brep.topology.faces {
            let SurfaceGeometry::Cylinder { radius, .. } =
                &cut.brep.geometry.surfaces[face.surface as usize]
            else {
                continue;
            };
            if (*radius - 2.0).abs() > accuracy.geometric {
                continue;
            }
            outer_faces += 1;
            assert_eq!(
                face.provenance
                    .sources
                    .iter()
                    .filter(|source| source.entity == host.id)
                    .count(),
                1,
                "output face {} should belong to exactly one trimmed outer arc",
                face.id
            );
        }
        assert!(outer_faces >= 2);
    }

    #[test]
    fn chained_curved_cut_keeps_each_split_top_cap_on_its_source_region() {
        let accuracy = sphere("accuracy", [0.0; 3], 1.0).accuracy;
        let quarter = std::f64::consts::FRAC_PI_2;
        let host = primitives::arc_edged_extrusion(
            "split-cap-arc-host".into(),
            Frame3::IDENTITY,
            vec![
                primitives::ProfileEdge::Arc {
                    center: [0.0, 0.0],
                    radius: 2.0,
                    start_angle: 0.0,
                    sweep_angle: quarter,
                },
                primitives::ProfileEdge::Line {
                    from: [0.0, 2.0],
                    to: [0.0, 1.5],
                },
                primitives::ProfileEdge::Arc {
                    center: [0.0, 0.0],
                    radius: 1.5,
                    start_angle: quarter,
                    sweep_angle: -quarter,
                },
                primitives::ProfileEdge::Line {
                    from: [1.5, 0.0],
                    to: [2.0, 0.0],
                },
            ],
            3.0,
            accuracy,
        )
        .unwrap();
        let sector = |id: &str, start: f64, end: f64, bottom: f64, height: f64| {
            let reach = 3.0;
            primitives::linear_extrusion(
                id.into(),
                Frame3 {
                    origin: [0.0, 0.0, bottom],
                    ..Frame3::IDENTITY
                },
                vec![
                    [0.0, 0.0],
                    [reach * start.cos(), reach * start.sin()],
                    [reach * end.cos(), reach * end.sin()],
                ],
                Vec::new(),
                height,
                accuracy,
            )
            .unwrap()
        };
        let first_cutter = sector("full-height-sector", 0.7, 0.9, -1.0, 5.0);
        let first = boolean_brep(
            &host,
            &first_cutter,
            BooleanOp::Subtraction,
            "split-cap-first".into(),
        )
        .unwrap();
        first.brep.validate().unwrap();
        assert_eq!(first.brep.solids.len(), 2);
        let second_cutter = sector("lower_cutout-sector", 0.2, 0.3, 0.5, 1.5);
        let second = boolean_brep(
            &first.brep,
            &second_cutter,
            BooleanOp::Subtraction,
            "split-cap-second".into(),
        )
        .unwrap();
        second.brep.validate().unwrap();
        let mut caps = 0;
        for face in &second.brep.topology.faces {
            let SurfaceGeometry::Plane { frame } =
                &second.brep.geometry.surfaces[face.surface as usize]
            else {
                continue;
            };
            if dot(frame.z, [0.0, 0.0, 1.0]) < 1.0 - 1.0e-10
                || (frame.origin[2] - 3.0).abs() > accuracy.intersection
            {
                continue;
            }
            caps += 1;
            assert_eq!(
                face.provenance
                    .sources
                    .iter()
                    .filter(|source| source.entity == first.brep.id)
                    .count(),
                1,
                "top cap {} should come from one prior top region",
                face.id
            );
        }
        assert_eq!(caps, 2);
    }

    #[test]
    fn wide_curved_opening_uses_multiple_planar_sector_cutters() {
        let accuracy = sphere("accuracy", [0.0; 3], 1.0).accuracy;
        let start = -std::f64::consts::FRAC_PI_2;
        let sweep = 3.0 * std::f64::consts::FRAC_PI_2;
        let point = |radius: f64, angle: f64| [radius * angle.cos(), radius * angle.sin()];
        let host = primitives::arc_edged_extrusion(
            "wide-arc".into(),
            Frame3::IDENTITY,
            vec![
                primitives::ProfileEdge::Arc {
                    center: [0.0, 0.0],
                    radius: 2.1,
                    start_angle: start,
                    sweep_angle: sweep,
                },
                primitives::ProfileEdge::Line {
                    from: point(2.1, start + sweep),
                    to: point(1.9, start + sweep),
                },
                primitives::ProfileEdge::Arc {
                    center: [0.0, 0.0],
                    radius: 1.9,
                    start_angle: start + sweep,
                    sweep_angle: -sweep,
                },
                primitives::ProfileEdge::Line {
                    from: point(1.9, start),
                    to: point(2.1, start),
                },
            ],
            3.0,
            accuracy,
        )
        .unwrap();
        let opening_start = start + 1.2 / 2.0;
        let opening_end = start + 8.2 / 2.0;
        let cutters = (0..3)
            .map(|index| {
                let from = opening_start + (opening_end - opening_start) * index as f64 / 3.0;
                let to = opening_start + (opening_end - opening_start) * (index + 1) as f64 / 3.0;
                let reach = 2.1 / ((to - from) / 2.0).cos() + 0.2;
                primitives::linear_extrusion(
                    format!("sector-{index}"),
                    Frame3::IDENTITY,
                    vec![[0.0, 0.0], point(reach, from), point(reach, to)],
                    vec![],
                    2.0,
                    accuracy,
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let cut = subtract_planar_cutters(&host, &cutters, "wide-cut".into()).unwrap();
        cut.brep.validate().unwrap();
        tessellate(&cut.brep, 0.01, 2_000_000).unwrap();
        let angle = start + 4.7 / 2.0;
        assert_eq!(
            super::super::query::classify_point(
                &cut.brep,
                [2.0 * angle.cos(), 2.0 * angle.sin(), 1.0]
            )
            .unwrap(),
            super::super::query::PointClassification::Outside
        );
    }
}
