use super::planar::all_planar;
use crate::brep::{
    coverage_gap, BrepEnvelope, CurveGeometry, EdgeGeometry, FaceRole, FaceSource, Frame3,
    GeometryError, GeometryQuality, PcurveGeometry, SurfaceGeometry, Topology,
};
use crate::math::{dot, norm, scale, sub, Point3};
use crate::primitives::cuboid;
use crate::query::{classify_point_validated, face_contains_uv, PointClassification};

pub(crate) struct BoxInput<'a> {
    pub(crate) brep: &'a BrepEnvelope,
    pub(crate) frame: Frame3,
    pub(crate) size: Point3,
}

pub(crate) type GridPoint = [usize; 3];

pub(crate) fn source(input: &BoxInput<'_>, face: usize) -> FaceSource {
    let authored = &input.brep.topology.faces[face];
    if matches!(authored.provenance.role, FaceRole::Preserved)
        && authored.provenance.sources.len() == 1
    {
        return authored.provenance.sources[0].clone();
    }
    FaceSource {
        entity: input.brep.id.clone(),
        body: input.brep.id.clone(),
        key: authored.key.clone(),
        face: face as u32,
    }
}

pub(crate) fn rectilinear_input<'a>(
    brep: &'a BrepEnvelope,
    axes: Frame3,
) -> Result<BoxInput<'a>, GeometryError> {
    brep.validate()?;
    if !matches!(brep.quality, GeometryQuality::Analytic)
        || brep.geometry.surfaces.is_empty()
        || brep.solids.is_empty()
        || !all_planar(brep)
        || brep
            .geometry
            .curves
            .iter()
            .any(|curve| !matches!(curve, CurveGeometry::Line { .. }))
    {
        return Err(coverage());
    }
    let directions = [axes.x, axes.y, axes.z];
    for surface in &brep.geometry.surfaces {
        let normal = surface.frame().z;
        if directions
            .iter()
            .all(|axis| dot(normal, *axis).abs() < 1.0 - 1.0e-10)
        {
            return Err(coverage());
        }
    }
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for vertex in &brep.topology.vertices {
        let point = axes.local(vertex.position);
        for axis in 0..3 {
            lo[axis] = lo[axis].min(point[axis]);
            hi[axis] = hi[axis].max(point[axis]);
        }
    }
    if (0..3).any(|axis| hi[axis] - lo[axis] <= 4.0 * brep.accuracy.geometric) {
        return Err(coverage());
    }
    for edge in &brep.topology.edges {
        let EdgeGeometry::Curve { curve, .. } = edge.geometry else {
            return Err(coverage());
        };
        let CurveGeometry::Line { direction, .. } = brep.geometry.curves[curve as usize] else {
            return Err(coverage());
        };
        if directions
            .iter()
            .all(|axis| dot(direction, *axis).abs() < 1.0 - 1.0e-10)
        {
            return Err(coverage());
        }
    }
    Ok(BoxInput {
        brep,
        frame: Frame3 {
            origin: axes.point(lo),
            ..axes
        },
        size: std::array::from_fn(|axis| hi[axis] - lo[axis]),
    })
}

pub(crate) fn coverage() -> GeometryError {
    coverage_gap("canonical aligned cuboid", "canonical aligned cuboid")
}

pub(crate) fn classified_inside(brep: &BrepEnvelope, point: Point3) -> Result<bool, GeometryError> {
    match classify_point_validated(brep, point)? {
        PointClassification::Inside => Ok(true),
        PointClassification::Outside => Ok(false),
        _ => Err(GeometryError::UnresolvedIntersection(
            "rectilinear arrangement cell classification is unresolved".into(),
        )),
    }
}

pub(crate) fn containing_faces(
    input: &BoxInput<'_>,
    point: Point3,
    outward: Point3,
    tolerance: f64,
) -> Result<Vec<(usize, bool)>, GeometryError> {
    let mut matches = Vec::new();
    for (index, face) in input.brep.topology.faces.iter().enumerate() {
        let SurfaceGeometry::Plane { frame } = input.brep.geometry.surfaces[face.surface as usize]
        else {
            return Err(coverage());
        };
        let local = frame.local(point);
        if local[2].abs() > tolerance {
            continue;
        }
        if face_contains_uv(input.brep, face, [local[0], local[1]])? != Some(true) {
            continue;
        }
        let normal = scale(frame.z, face.sense.multiplier());
        let alignment = dot(normal, outward);
        if alignment.abs() < 1.0 - 1.0e-10 {
            continue;
        }
        matches.push((index, alignment < 0.0));
    }
    Ok(matches)
}

pub(crate) fn full_box(brep: &BrepEnvelope) -> Result<BoxInput<'_>, GeometryError> {
    brep.validate()?;
    let t = &brep.topology;
    if !matches!(brep.quality, GeometryQuality::Analytic)
        || [
            t.vertices.len(),
            t.edges.len(),
            t.halfedges.len(),
            t.loops.len(),
            t.faces.len(),
            t.shells.len(),
        ] != [8, 12, 24, 6, 6, 1]
        || !t.wires.is_empty()
        || brep.solids.len() != 1
        || brep.geometry.surfaces.len() != 6
        || brep.geometry.curves.len() != 12
        || brep.geometry.pcurves.len() != 24
        || !brep.geometry.intersections.is_empty()
    {
        return Err(coverage());
    }
    let mut axes = [[0.0; 3]; 3];
    let mut size = [0.0; 3];
    for i in 0..3 {
        let EdgeGeometry::Curve { curve, range } = t.edges[i].geometry else {
            return Err(coverage());
        };
        let CurveGeometry::Line { direction, .. } = brep.geometry.curves[curve as usize] else {
            return Err(coverage());
        };
        if range.lo != 0.0 {
            return Err(coverage());
        }
        axes[i] = direction;
        size[i] = range.hi;
    }
    let frame = Frame3 {
        origin: t.vertices[0].position,
        x: axes[0],
        y: axes[1],
        z: axes[2],
    };
    frame.validate().map_err(|_| coverage())?;
    let canonical = cuboid(brep.id.clone(), frame, size, brep.accuracy)?;
    let magnitude = size.into_iter().fold(0.0_f64, f64::max).max(
        t.vertices
            .iter()
            .map(|v| norm(v.position))
            .fold(0.0_f64, f64::max),
    );
    let noise = (256.0 * f64::EPSILON * magnitude).min(brep.accuracy.intersection / 8.0);
    let close = |a: Point3, b: Point3| norm(sub(a, b)) <= noise;
    let direction_close =
        |a: Point3, b: Point3| norm(sub(a, b)) * size.into_iter().fold(0.0_f64, f64::max) <= noise;
    check_canonical_surfaces(brep, &canonical, close, direction_close)?;
    check_canonical_curves(brep, &canonical, close, direction_close)?;
    check_canonical_pcurves(brep, &canonical, noise, size)?;
    let topology = snapped_topology(brep, &canonical, close, noise)?;
    if topology != canonical.topology || brep.solids != canonical.solids {
        return Err(coverage());
    }
    Ok(BoxInput { brep, frame, size })
}

fn check_canonical_surfaces(
    brep: &BrepEnvelope,
    canonical: &BrepEnvelope,
    close: impl Fn(Point3, Point3) -> bool,
    direction_close: impl Fn(Point3, Point3) -> bool,
) -> Result<(), GeometryError> {
    for (a, b) in brep
        .geometry
        .surfaces
        .iter()
        .zip(&canonical.geometry.surfaces)
    {
        let (SurfaceGeometry::Plane { frame: a }, SurfaceGeometry::Plane { frame: b }) = (a, b)
        else {
            return Err(coverage());
        };
        if !close(a.origin, b.origin)
            || !direction_close(a.x, b.x)
            || !direction_close(a.y, b.y)
            || !direction_close(a.z, b.z)
        {
            return Err(coverage());
        }
    }
    Ok(())
}

fn check_canonical_curves(
    brep: &BrepEnvelope,
    canonical: &BrepEnvelope,
    close: impl Fn(Point3, Point3) -> bool,
    direction_close: impl Fn(Point3, Point3) -> bool,
) -> Result<(), GeometryError> {
    for (a, b) in brep.geometry.curves.iter().zip(&canonical.geometry.curves) {
        let (
            CurveGeometry::Line {
                origin: ao,
                direction: ad,
            },
            CurveGeometry::Line {
                origin: bo,
                direction: bd,
            },
        ) = (a, b)
        else {
            return Err(coverage());
        };
        if !close(*ao, *bo) || !direction_close(*ad, *bd) {
            return Err(coverage());
        }
    }
    Ok(())
}

fn check_canonical_pcurves(
    brep: &BrepEnvelope,
    canonical: &BrepEnvelope,
    noise: f64,
    size: Point3,
) -> Result<(), GeometryError> {
    for (a, b) in brep
        .geometry
        .pcurves
        .iter()
        .zip(&canonical.geometry.pcurves)
    {
        let (
            PcurveGeometry::Line2 {
                origin: ao,
                direction: ad,
            },
            PcurveGeometry::Line2 {
                origin: bo,
                direction: bd,
            },
        ) = (a, b)
        else {
            return Err(coverage());
        };
        for i in 0..2 {
            if (ao[i] - bo[i]).abs() > noise
                || (ad[i] - bd[i]).abs() * size.into_iter().fold(0.0_f64, f64::max) > noise
            {
                return Err(coverage());
            }
        }
    }
    Ok(())
}

fn snapped_topology(
    brep: &BrepEnvelope,
    canonical: &BrepEnvelope,
    close: impl Fn(Point3, Point3) -> bool,
    noise: f64,
) -> Result<Topology, GeometryError> {
    let mut topology = brep.topology.clone();
    for (v, expected) in topology
        .vertices
        .iter_mut()
        .zip(&canonical.topology.vertices)
    {
        if !close(v.position, expected.position) {
            return Err(coverage());
        }
        v.position = expected.position;
        v.tolerance = expected.tolerance;
    }
    for e in &mut topology.edges {
        e.tolerance = brep.accuracy.geometric;
    }
    for (f, expected) in topology.faces.iter_mut().zip(&canonical.topology.faces) {
        for i in 0..2 {
            if (f.trim.uv_bounds[i].lo - expected.trim.uv_bounds[i].lo).abs() > noise
                || (f.trim.uv_bounds[i].hi - expected.trim.uv_bounds[i].hi).abs() > noise
            {
                return Err(coverage());
            }
        }
        f.trim.uv_bounds = expected.trim.uv_bounds;
        f.key = expected.key.clone();
        f.provenance = expected.provenance.clone();
    }
    Ok(topology)
}
