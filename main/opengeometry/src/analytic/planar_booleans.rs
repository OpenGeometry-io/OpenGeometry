use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
};

use super::{
    booleans::{
        analytic_face_mappings, brep_face_source, full_planar_extrusion, BooleanOp, BooleanReport,
        BooleanResult, PrismaticInput,
    },
    geometry::{cross, dot, norm, scale, sub, unit},
    primitives::{plane_boundary, Builder, Use},
    query::face_contains_uv,
    topology::{
        Accuracy, BrepEnvelope, EdgeGeometry, FaceProvenance, FaceRole, FaceSource, Orientation,
        Shell, SolidRegion,
    },
    CurveGeometry, Frame3, GeometryError, Point3, SurfaceGeometry,
};
use crate::{
    geometry::{
        boolean2d::{boolean_oriented_regions, regions_from_edges_by, PlanarBooleanOp, RingRegion},
        poly2d::{winding_number2, Pt2},
    },
    math::interval::Interval,
};

fn coverage() -> GeometryError {
    GeometryError::CoverageGap {
        families: [
            "layered planar profile extrusion".into(),
            "layered planar profile extrusion".into(),
        ],
    }
}

fn contours(input: &PrismaticInput<'_>, frame: Frame3) -> Vec<Vec<Pt2>> {
    input
        .contours
        .iter()
        .map(|ring| {
            ring.iter()
                .map(|point| {
                    let local = frame.local(*point);
                    Pt2::new(local[0], local[1])
                })
                .collect()
        })
        .collect()
}

fn region_contours(regions: &[RingRegion]) -> Vec<Vec<Pt2>> {
    regions
        .iter()
        .flat_map(|region| {
            std::iter::once(region.outer.clone()).chain(region.holes.iter().cloned())
        })
        .collect()
}

fn region_points(regions: &[RingRegion], out: &mut Vec<Pt2>) {
    for region in regions {
        out.extend(region.outer.iter().copied());
        for hole in &region.holes {
            out.extend(hole.iter().copied());
        }
    }
}

fn loop_positions(brep: &BrepEnvelope, loop_id: u32) -> Result<Vec<Point3>, GeometryError> {
    let start = brep
        .topology
        .loops
        .get(loop_id as usize)
        .ok_or_else(coverage)?
        .start_halfedge;
    let mut current = start;
    let mut points = Vec::new();
    loop {
        let halfedge = brep
            .topology
            .halfedges
            .get(current as usize)
            .ok_or_else(coverage)?;
        points.push(
            brep.topology
                .vertices
                .get(halfedge.from as usize)
                .ok_or_else(coverage)?
                .position,
        );
        current = halfedge.next.ok_or_else(coverage)?;
        if current == start {
            break;
        }
        if points.len() > brep.topology.halfedges.len() {
            return Err(coverage());
        }
    }
    Ok(points)
}

fn slice_regions(
    brep: &BrepEnvelope,
    frame: Frame3,
    level: f64,
    tolerance: f64,
) -> Result<Vec<RingRegion>, GeometryError> {
    let mut segments = Vec::<(Pt2, Pt2)>::new();
    for face in &brep.topology.faces {
        let SurfaceGeometry::Plane { frame: face_frame } = brep
            .geometry
            .surfaces
            .get(face.surface as usize)
            .ok_or_else(coverage)?
        else {
            return Err(coverage());
        };
        if dot(face_frame.z, frame.z).abs() > 1.0e-10 {
            continue;
        }
        if !face.trim.holes.is_empty() {
            return Err(coverage());
        }
        let points = loop_positions(brep, face.trim.outer)?;
        let mut crossings = Vec::<Pt2>::new();
        for index in 0..points.len() {
            let from = frame.local(points[index]);
            let to = frame.local(points[(index + 1) % points.len()]);
            if (from[2] < level && to[2] > level) || (from[2] > level && to[2] < level) {
                let fraction = (level - from[2]) / (to[2] - from[2]);
                let point = Pt2::new(
                    from[0] + fraction * (to[0] - from[0]),
                    from[1] + fraction * (to[1] - from[1]),
                );
                if crossings
                    .iter()
                    .all(|existing| (existing.x - point.x).hypot(existing.z - point.z) > tolerance)
                {
                    crossings.push(point);
                }
            }
        }
        if crossings.is_empty() {
            continue;
        }
        if crossings.len() != 2 {
            return Err(coverage());
        }
        let mut from = crossings[0];
        let mut to = crossings[1];
        let edge = sub(
            frame.point([to.x, to.z, level]),
            frame.point([from.x, from.z, level]),
        );
        let outward = cross(edge, frame.z);
        let actual = scale(face_frame.z, face.sense.multiplier());
        if dot(outward, actual) < 0.0 {
            std::mem::swap(&mut from, &mut to);
        }
        segments.push((from, to));
    }
    let mut loops = Vec::new();
    while let Some((from, mut to)) = segments.pop() {
        let mut ring = vec![from];
        while (to.x - from.x).hypot(to.z - from.z) > tolerance {
            ring.push(to);
            let Some(next) = segments
                .iter()
                .position(|(start, _)| (start.x - to.x).hypot(start.z - to.z) <= tolerance)
            else {
                return Err(coverage());
            };
            let (_, end) = segments.swap_remove(next);
            to = end;
            if ring.len() > brep.topology.halfedges.len() {
                return Err(coverage());
            }
        }
        if ring.len() < 3 {
            return Err(coverage());
        }
        loops.push(ring);
    }
    Ok(boolean_oriented_regions(
        &loops,
        &[],
        PlanarBooleanOp::Union,
        tolerance,
    ))
}

fn side_sources(
    brep: &BrepEnvelope,
    point: Point3,
    outward: Point3,
    tolerance: f64,
) -> Result<Vec<FaceSource>, GeometryError> {
    let mut sources = Vec::new();
    for face in &brep.topology.faces {
        let SurfaceGeometry::Plane { frame } = brep
            .geometry
            .surfaces
            .get(face.surface as usize)
            .ok_or_else(coverage)?
        else {
            return Err(coverage());
        };
        let local = frame.local(point);
        if local[2].abs() > tolerance || dot(frame.z, outward).abs() < 1.0 - 1.0e-10 {
            continue;
        }
        if face_contains_uv(brep, face, [local[0], local[1]])? == Some(true) {
            sources.push(brep_face_source(brep, face.id));
        }
    }
    Ok(sources)
}

fn region_sample(region: &RingRegion) -> Result<Pt2, GeometryError> {
    let mut coordinates = Vec::new();
    let mut holes = Vec::new();
    for point in &region.outer {
        coordinates.extend([point.x, point.z]);
    }
    for hole in &region.holes {
        holes.push(coordinates.len() / 2);
        for point in hole {
            coordinates.extend([point.x, point.z]);
        }
    }
    let triangles = earcutr::earcut(&coordinates, &holes, 2);
    // Earcut may put a nearly collinear boundary triangle first. Its centroid
    // can round onto the host face and fail cap provenance after a chained cut.
    // The largest triangle gives a stable point strictly inside this region.
    let triangle = triangles
        .chunks_exact(3)
        .max_by(|left, right| {
            let area = |triangle: &[usize]| {
                let point =
                    |index: usize| Pt2::new(coordinates[index * 2], coordinates[index * 2 + 1]);
                let a = point(triangle[0]);
                let b = point(triangle[1]);
                let c = point(triangle[2]);
                ((b.x - a.x) * (c.z - a.z) - (b.z - a.z) * (c.x - a.x)).abs()
            };
            area(left).total_cmp(&area(right))
        })
        .ok_or_else(coverage)?;
    Ok(Pt2::new(
        triangle
            .iter()
            .map(|&index| coordinates[index * 2])
            .sum::<f64>()
            / 3.0,
        triangle
            .iter()
            .map(|&index| coordinates[index * 2 + 1])
            .sum::<f64>()
            / 3.0,
    ))
}

fn cap_sources(
    brep: &BrepEnvelope,
    point: Point3,
    outward: Point3,
    cut: bool,
    tolerance: f64,
) -> Result<Vec<FaceSource>, GeometryError> {
    let mut sources = Vec::new();
    for face in &brep.topology.faces {
        let SurfaceGeometry::Plane { frame } = brep
            .geometry
            .surfaces
            .get(face.surface as usize)
            .ok_or_else(coverage)?
        else {
            return Err(coverage());
        };
        let local = frame.local(point);
        if local[2].abs() > tolerance {
            continue;
        }
        let actual = scale(frame.z, face.sense.multiplier());
        let alignment = dot(actual, outward);
        if (cut && alignment > -1.0 + 1.0e-10) || (!cut && alignment < 1.0 - 1.0e-10) {
            continue;
        }
        if face_contains_uv(brep, face, [local[0], local[1]])? == Some(true) {
            sources.push(brep_face_source(brep, face.id));
        }
    }
    Ok(sources)
}

fn split_ring(ring: &[Pt2], vertices: &[Pt2], tolerance: f64) -> Vec<Pt2> {
    let mut split = Vec::new();
    for index in 0..ring.len() {
        let from = ring[index];
        let to = ring[(index + 1) % ring.len()];
        let delta = Pt2::new(to.x - from.x, to.z - from.z);
        let length2 = delta.x * delta.x + delta.z * delta.z;
        if length2 <= tolerance * tolerance {
            continue;
        }
        split.push(from);
        let mut interior = Vec::new();
        for &point in vertices {
            let offset = Pt2::new(point.x - from.x, point.z - from.z);
            let t = (offset.x * delta.x + offset.z * delta.z) / length2;
            let distance = (offset.x * delta.z - offset.z * delta.x).abs() / length2.sqrt();
            if t > tolerance / length2.sqrt()
                && t < 1.0 - tolerance / length2.sqrt()
                && distance <= tolerance
            {
                interior.push((t, point));
            }
        }
        interior.sort_by(|left, right| left.0.total_cmp(&right.0));
        for (_, point) in interior {
            if split
                .last()
                .is_none_or(|last| (last.x - point.x).hypot(last.z - point.z) > tolerance)
            {
                split.push(point);
            }
        }
    }
    split
}

struct Facets {
    builder: Builder,
    vertices: Vec<(Point3, u32)>,
    edges: BTreeMap<(u32, u32), u32>,
    accuracy: Accuracy,
}

impl Facets {
    fn new(id: String, accuracy: Accuracy) -> Result<Self, GeometryError> {
        Ok(Self {
            builder: Builder::new(id, accuracy)?,
            vertices: Vec::new(),
            edges: BTreeMap::new(),
            accuracy,
        })
    }

    fn vertex(&mut self, point: Point3) -> u32 {
        if let Some((_, id)) = self
            .vertices
            .iter()
            .find(|(existing, _)| norm(sub(*existing, point)) <= self.accuracy.geometric / 4.0)
        {
            return *id;
        }
        let id = self.builder.vertex(point);
        self.vertices.push((point, id));
        id
    }

    fn uses(&mut self, ring: &[Point3], frame: Frame3) -> Result<Vec<Use>, GeometryError> {
        let ids = ring
            .iter()
            .map(|point| self.vertex(*point))
            .collect::<Vec<_>>();
        let mut uses = Vec::with_capacity(ids.len());
        for index in 0..ids.len() {
            let next = (index + 1) % ids.len();
            let from = ids[index];
            let to = ids[next];
            if from == to {
                return Err(GeometryError::UnresolvedIntersection(
                    "layered planar face has a sub-tolerance edge".into(),
                ));
            }
            let key = (from.min(to), from.max(to));
            let edge = if let Some(&edge) = self.edges.get(&key) {
                edge
            } else {
                let delta = sub(ring[next], ring[index]);
                let length = norm(delta);
                let edge = self.builder.edge(
                    CurveGeometry::Line {
                        origin: ring[index],
                        direction: unit(delta)?,
                    },
                    Interval::new(0.0, length)?,
                    false,
                );
                self.edges.insert(key, edge);
                edge
            };
            let EdgeGeometry::Curve { curve, .. } =
                self.builder.brep.topology.edges[edge as usize].geometry
            else {
                return Err(coverage());
            };
            let CurveGeometry::Line { origin, direction } =
                self.builder.brep.geometry.curves[curve as usize]
            else {
                return Err(coverage());
            };
            let sense = if norm(sub(origin, ring[index])) <= self.accuracy.geometric / 4.0
                && dot(direction, sub(ring[next], ring[index])) > 0.0
            {
                Orientation::Forward
            } else {
                Orientation::Reverse
            };
            uses.push(plane_boundary(&self.builder, frame, edge, from, to, sense)?);
        }
        Ok(uses)
    }

    fn face(
        &mut self,
        key: &str,
        frame: Frame3,
        outer: Vec<Point3>,
        holes: Vec<Vec<Point3>>,
        provenance: FaceProvenance,
    ) -> Result<(), GeometryError> {
        let mut bounds = [[f64::INFINITY, f64::NEG_INFINITY]; 2];
        for point in outer.iter().chain(holes.iter().flatten()) {
            let local = frame.local(*point);
            if local[2].abs() > 4.0 * self.accuracy.geometric {
                return Err(GeometryError::UnresolvedIntersection(
                    "layered planar face is not coplanar".into(),
                ));
            }
            for axis in 0..2 {
                bounds[axis][0] = bounds[axis][0].min(local[axis]);
                bounds[axis][1] = bounds[axis][1].max(local[axis]);
            }
        }
        for bound in &mut bounds {
            bound[0] -= self.accuracy.geometric;
            bound[1] += self.accuracy.geometric;
        }
        let outer_uses = self.uses(&outer, frame)?;
        let hole_uses = holes
            .iter()
            .map(|hole| self.uses(hole, frame))
            .collect::<Result<Vec<_>, _>>()?;
        let face = self.builder.brep.topology.faces.len();
        self.builder.face_with_holes(
            &format!("{key}-{face}"),
            super::SurfaceGeometry::Plane { frame },
            bounds,
            outer_uses,
            hole_uses,
        )?;
        self.builder.brep.topology.faces[face].provenance = provenance;
        Ok(())
    }
}

fn cap(
    facets: &mut Facets,
    base: Frame3,
    level: f64,
    up: bool,
    regions: &[RingRegion],
    vertices: &[Pt2],
    source_breps: &[(&BrepEnvelope, FaceRole)],
) -> Result<(), GeometryError> {
    let origin = base.point([0.0, 0.0, level]);
    let frame = if up {
        Frame3 { origin, ..base }
    } else {
        Frame3 {
            origin,
            x: base.x,
            y: scale(base.y, -1.0),
            z: scale(base.z, -1.0),
        }
    };
    for region in regions {
        let sample = region_sample(region)?;
        let mut selected = None;
        for &(brep, role) in source_breps {
            let sources = cap_sources(
                brep,
                base.point([sample.x, sample.z, level]),
                frame.z,
                role == FaceRole::Cut,
                facets.accuracy.intersection,
            )?;
            if !sources.is_empty() {
                selected = Some((sources, role));
                break;
            }
        }
        let (sources, role) = selected.ok_or_else(|| {
            GeometryError::InvalidTopology(format!(
                "layered planar cap has no source face at level {level}"
            ))
        })?;
        let mut outer = split_ring(&region.outer, vertices, facets.accuracy.geometric / 4.0)
            .into_iter()
            .map(|point| base.point([point.x, point.z, level]))
            .collect::<Vec<_>>();
        let mut holes = region
            .holes
            .iter()
            .map(|ring| {
                split_ring(ring, vertices, facets.accuracy.geometric / 4.0)
                    .into_iter()
                    .map(|point| base.point([point.x, point.z, level]))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        if !up {
            outer.reverse();
            for hole in &mut holes {
                hole.reverse();
            }
        }
        facets.face(
            if up { "upper-cap" } else { "lower-cap" },
            frame,
            outer,
            holes,
            FaceProvenance {
                sources,
                role,
                reversed: role == FaceRole::Cut,
            },
        )?;
    }
    Ok(())
}

fn side_faces(
    facets: &mut Facets,
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    base: Frame3,
    regions: &[RingRegion],
    vertices: &[Pt2],
    lo: f64,
    hi: f64,
) -> Result<(), GeometryError> {
    for region in regions {
        for ring in std::iter::once(&region.outer).chain(&region.holes) {
            let ring = split_ring(ring, vertices, facets.accuracy.geometric / 4.0);
            for index in 0..ring.len() {
                let from = ring[index];
                let to = ring[(index + 1) % ring.len()];
                let lower_from = base.point([from.x, from.z, lo]);
                let lower_to = base.point([to.x, to.z, lo]);
                let upper_to = base.point([to.x, to.z, hi]);
                let upper_from = base.point([from.x, from.z, hi]);
                let normal = unit(cross(
                    sub(lower_to, lower_from),
                    sub(upper_from, lower_from),
                ))?;
                let face_frame = Frame3::from_axis(lower_from, normal, sub(lower_to, lower_from))?;
                let midpoint = scale(
                    [
                        lower_from[0] + lower_to[0] + upper_to[0] + upper_from[0],
                        lower_from[1] + lower_to[1] + upper_to[1] + upper_from[1],
                        lower_from[2] + lower_to[2] + upper_to[2] + upper_from[2],
                    ],
                    0.25,
                );
                let a_sources = side_sources(a, midpoint, normal, facets.accuracy.intersection)?;
                let b_sources = side_sources(b, midpoint, normal, facets.accuracy.intersection)?;
                let is_cut = a_sources.is_empty() && !b_sources.is_empty();
                let sources = if is_cut { b_sources } else { a_sources };
                if sources.is_empty() {
                    return Err(GeometryError::InvalidTopology(
                        "layered planar side has no source face".into(),
                    ));
                }
                facets.face(
                    if is_cut { "cut-side" } else { "host-side" },
                    face_frame,
                    vec![lower_from, lower_to, upper_to, upper_from],
                    Vec::new(),
                    FaceProvenance {
                        sources,
                        role: if is_cut {
                            FaceRole::Cut
                        } else {
                            FaceRole::Split
                        },
                        reversed: is_cut,
                    },
                )?;
            }
        }
    }
    Ok(())
}

fn shell_volume(brep: &BrepEnvelope, faces: &[u32]) -> Result<f64, GeometryError> {
    let mut volume = 0.0;
    for &face_id in faces {
        let face = &brep.topology.faces[face_id as usize];
        for &loop_id in std::iter::once(&face.trim.outer).chain(&face.trim.holes) {
            let start = brep.topology.loops[loop_id as usize].start_halfedge;
            let mut current = start;
            let mut points = Vec::new();
            loop {
                let halfedge = &brep.topology.halfedges[current as usize];
                points.push(brep.topology.vertices[halfedge.from as usize].position);
                current = halfedge.next.ok_or_else(|| {
                    GeometryError::InvalidTopology("layered planar face has open loop".into())
                })?;
                if current == start {
                    break;
                }
                if points.len() > brep.topology.halfedges.len() {
                    return Err(coverage());
                }
            }
            for index in 1..points.len() - 1 {
                volume += dot(points[0], cross(points[index], points[index + 1])) / 6.0;
            }
        }
    }
    Ok(volume)
}

fn shell_contains_point(
    brep: &BrepEnvelope,
    faces: &[u32],
    point: Point3,
    tolerance: f64,
) -> Result<bool, GeometryError> {
    for direction in [
        unit([1.0, 0.371, 0.127])?,
        unit([0.193, 1.0, 0.419])?,
        unit([0.311, 0.233, 1.0])?,
    ] {
        let mut hits: Vec<f64> = Vec::new();
        let mut uncertain = false;
        for &face_id in faces {
            let face = &brep.topology.faces[face_id as usize];
            let SurfaceGeometry::Plane { frame } = brep.geometry.surface(face.surface)? else {
                return Err(coverage());
            };
            let denominator = dot(frame.z, direction);
            let numerator = dot(frame.z, sub(frame.origin, point));
            if denominator.abs() <= 64.0 * f64::EPSILON {
                if numerator.abs() <= tolerance {
                    uncertain = true;
                    break;
                }
                continue;
            }
            let distance = numerator / denominator;
            if distance <= tolerance {
                continue;
            }
            let position = [
                point[0] + direction[0] * distance,
                point[1] + direction[1] * distance,
                point[2] + direction[2] * distance,
            ];
            let local = frame.local(position);
            match face_contains_uv(brep, face, [local[0], local[1]])? {
                Some(true) if hits.iter().any(|hit| (hit - distance).abs() <= tolerance) => {
                    uncertain = true;
                    break;
                }
                Some(true) => hits.push(distance),
                Some(false) => {}
                None => {
                    uncertain = true;
                    break;
                }
            }
        }
        if !uncertain {
            return Ok(hits.len() % 2 == 1);
        }
    }
    Err(coverage())
}

fn finish(
    mut facets: Facets,
    a: &BrepEnvelope,
    b: &BrepEnvelope,
) -> Result<BooleanResult, GeometryError> {
    let mut assigned = vec![false; facets.builder.brep.topology.faces.len()];
    let mut outer_shells = Vec::new();
    let mut cavity_shells = Vec::new();
    for start in 0..assigned.len() {
        if assigned[start] {
            continue;
        }
        let shell = facets.builder.brep.topology.shells.len() as u32;
        let mut faces = Vec::new();
        let mut queue = VecDeque::from([start]);
        assigned[start] = true;
        while let Some(face) = queue.pop_front() {
            facets.builder.brep.topology.faces[face].shell_ref = Some(shell);
            faces.push(face as u32);
            for halfedge in facets
                .builder
                .brep
                .topology
                .halfedges
                .iter()
                .filter(|edge| edge.face == Some(face as u32))
            {
                let twin = halfedge.twin.ok_or_else(|| {
                    GeometryError::InvalidTopology(
                        "layered planar Boolean has an open boundary".into(),
                    )
                })?;
                let adjacent = facets.builder.brep.topology.halfedges[twin as usize]
                    .face
                    .ok_or_else(|| {
                        GeometryError::InvalidTopology("layered planar twin has no face".into())
                    })? as usize;
                if !assigned[adjacent] {
                    assigned[adjacent] = true;
                    queue.push_back(adjacent);
                }
            }
        }
        let volume = shell_volume(&facets.builder.brep, &faces)?;
        if volume.abs() <= facets.accuracy.geometric.powi(3) {
            return Err(coverage());
        }
        facets.builder.brep.topology.shells.push(Shell {
            id: shell,
            faces,
            is_closed: true,
        });
        if volume > 0.0 {
            outer_shells.push((shell, volume));
        } else {
            cavity_shells.push(shell);
        }
    }
    for &(outer_shell, _) in &outer_shells {
        facets.builder.brep.solids.push(SolidRegion {
            outer_shell,
            cavity_shells: Vec::new(),
        });
    }
    for cavity_shell in cavity_shells {
        let shell = &facets.builder.brep.topology.shells[cavity_shell as usize];
        let face = &facets.builder.brep.topology.faces[shell.faces[0] as usize];
        let loop_start =
            facets.builder.brep.topology.loops[face.trim.outer as usize].start_halfedge;
        let vertex = facets.builder.brep.topology.halfedges[loop_start as usize].from;
        let point = facets.builder.brep.topology.vertices[vertex as usize].position;
        let mut enclosing = Vec::new();
        for (index, &(outer_shell, volume)) in outer_shells.iter().enumerate() {
            let outer = &facets.builder.brep.topology.shells[outer_shell as usize];
            if shell_contains_point(
                &facets.builder.brep,
                &outer.faces,
                point,
                facets.accuracy.intersection,
            )? {
                enclosing.push((index, volume));
            }
        }
        let Some(&(index, _)) = enclosing
            .iter()
            .min_by(|left, right| left.1.total_cmp(&right.1))
        else {
            return Err(coverage());
        };
        facets.builder.brep.solids[index]
            .cavity_shells
            .push(cavity_shell);
    }
    let mut out = facets.builder.brep;
    out.revision =
        a.revision.max(b.revision).checked_add(1).ok_or_else(|| {
            GeometryError::LimitExceeded("planar Boolean revision overflow".into())
        })?;
    out.validate()?;
    let face_mappings = analytic_face_mappings(&out, [a, b]);
    Ok(BooleanResult {
        report: BooleanReport {
            operation: BooleanOp::Subtraction,
            quality: out.quality.clone(),
            contacts: Vec::new(),
            coincident: true,
            face_mappings,
        },
        brep: out,
    })
}

pub(super) fn subtract_layered_extrusions(
    host: &BrepEnvelope,
    cutter: &BrepEnvelope,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    host.validate()?;
    let b = full_planar_extrusion(cutter)?;
    if host
        .geometry
        .surfaces
        .iter()
        .any(|surface| !matches!(surface, SurfaceGeometry::Plane { .. }))
        || host
            .geometry
            .curves
            .iter()
            .any(|curve| !matches!(curve, CurveGeometry::Line { .. }))
        || host.solids.is_empty()
    {
        return Err(coverage());
    }
    let accuracy = Accuracy {
        geometric: host.accuracy.geometric.max(cutter.accuracy.geometric),
        intersection: host.accuracy.intersection.max(cutter.accuracy.intersection),
        tessellation: host.accuracy.tessellation.max(cutter.accuracy.tessellation),
        exchange: host.accuracy.exchange.max(cutter.accuracy.exchange),
    };
    let host_positions = host
        .topology
        .vertices
        .iter()
        .map(|vertex| b.frame.local(vertex.position)[2])
        .collect::<Vec<_>>();
    let host_lo = host_positions.iter().copied().fold(f64::INFINITY, f64::min);
    let host_hi = host_positions
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    if !host_lo.is_finite() || host_hi - host_lo <= 4.0 * accuracy.geometric {
        return Err(coverage());
    }
    let frame = Frame3 {
        origin: b.frame.point([0.0, 0.0, host_lo]),
        ..b.frame
    };
    for face in &host.topology.faces {
        let SurfaceGeometry::Plane { frame: face_frame } = host
            .geometry
            .surfaces
            .get(face.surface as usize)
            .ok_or_else(coverage)?
        else {
            return Err(coverage());
        };
        let alignment = dot(face_frame.z, frame.z).abs();
        if alignment > 1.0e-10 && alignment < 1.0 - 1.0e-10 {
            return Err(coverage());
        }
    }
    let b_contours = contours(&b, frame);
    let b_lo = dot(sub(b.frame.origin, frame.origin), frame.z);
    let b_hi = b_lo + b.height;
    let host_height = host_hi - host_lo;
    if b_hi <= accuracy.geometric || b_lo >= host_height - accuracy.geometric {
        return Err(coverage());
    }
    let mut levels = host_positions
        .iter()
        .map(|level| level - host_lo)
        .collect::<Vec<_>>();
    for level in [b_lo, b_hi] {
        if level > accuracy.geometric && level < host_height - accuracy.geometric {
            levels.push(level);
        }
    }
    levels.sort_by(f64::total_cmp);
    levels.dedup_by(|left, right| (*left - *right).abs() <= accuracy.geometric);
    let layers = levels
        .windows(2)
        .map(|span| -> Result<Vec<RingRegion>, GeometryError> {
            let middle = (span[0] + span[1]) / 2.0;
            let host_regions = slice_regions(host, frame, middle, accuracy.intersection)?;
            Ok(boolean_oriented_regions(
                &region_contours(&host_regions),
                if middle > b_lo && middle < b_hi {
                    &b_contours
                } else {
                    &[]
                },
                PlanarBooleanOp::Subtraction,
                accuracy.intersection,
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut exposed_up = Vec::new();
    let mut exposed_down = Vec::new();
    for index in 1..layers.len() {
        let below = region_contours(&layers[index - 1]);
        let above = region_contours(&layers[index]);
        exposed_up.push(boolean_oriented_regions(
            &below,
            &above,
            PlanarBooleanOp::Subtraction,
            accuracy.intersection,
        ));
        exposed_down.push(boolean_oriented_regions(
            &above,
            &below,
            PlanarBooleanOp::Subtraction,
            accuracy.intersection,
        ));
    }
    let mut vertices = Vec::new();
    for layer in &layers {
        region_points(layer, &mut vertices);
    }
    for regions in exposed_up.iter().chain(&exposed_down) {
        region_points(regions, &mut vertices);
    }
    let mut facets = Facets::new(id, accuracy)?;
    cap(
        &mut facets,
        frame,
        0.0,
        false,
        &layers[0],
        &vertices,
        &[(host, FaceRole::Split)],
    )?;
    for (index, layer) in layers.iter().enumerate() {
        side_faces(
            &mut facets,
            host,
            cutter,
            frame,
            layer,
            &vertices,
            levels[index],
            levels[index + 1],
        )?;
        if index + 1 < layers.len() {
            cap(
                &mut facets,
                frame,
                levels[index + 1],
                true,
                &exposed_up[index],
                &vertices,
                &[(host, FaceRole::Split), (cutter, FaceRole::Cut)],
            )?;
            cap(
                &mut facets,
                frame,
                levels[index + 1],
                false,
                &exposed_down[index],
                &vertices,
                &[(host, FaceRole::Split), (cutter, FaceRole::Cut)],
            )?;
        }
    }
    cap(
        &mut facets,
        frame,
        host_height,
        true,
        layers.last().ok_or_else(coverage)?,
        &vertices,
        &[(host, FaceRole::Split)],
    )?;
    finish(facets, host, cutter)
}

struct PlanarFace {
    id: u32,
    frame: Frame3,
    polygon: Vec<Point3>,
    holes: Vec<Vec<Point3>>,
}

fn planar_faces(
    brep: &BrepEnvelope,
    tolerance: f64,
) -> Result<Option<Vec<PlanarFace>>, GeometryError> {
    if brep.solids.is_empty()
        || brep
            .geometry
            .curves
            .iter()
            .any(|curve| !matches!(curve, CurveGeometry::Line { .. }))
    {
        return Ok(None);
    }
    let mut faces = Vec::with_capacity(brep.topology.faces.len());
    for face in &brep.topology.faces {
        let Some(SurfaceGeometry::Plane { frame: surface }) =
            brep.geometry.surfaces.get(face.surface as usize)
        else {
            return Ok(None);
        };
        let frame = if face.sense == Orientation::Forward {
            *surface
        } else {
            Frame3 {
                origin: surface.origin,
                x: surface.x,
                y: scale(surface.y, -1.0),
                z: scale(surface.z, -1.0),
            }
        };
        let polygon = loop_positions(brep, face.trim.outer)?;
        if polygon.len() < 3 {
            return Ok(None);
        }
        let mut holes = face
            .trim
            .holes
            .iter()
            .map(|&loop_id| loop_positions(brep, loop_id))
            .collect::<Result<Vec<_>, _>>()?;
        let orient = |mut ring: Vec<Point3>, outer: bool| -> Option<Vec<Point3>> {
            let area = ring
                .iter()
                .enumerate()
                .map(|(index, point)| {
                    let a = frame.local(*point);
                    let b = frame.local(ring[(index + 1) % ring.len()]);
                    a[0] * b[1] - b[0] * a[1]
                })
                .sum::<f64>();
            if ring.len() < 3 || area.abs() <= tolerance * tolerance {
                return None;
            }
            if (area > 0.0) != outer {
                ring.reverse();
            }
            Some(ring)
        };
        let Some(polygon) = orient(polygon, true) else {
            return Ok(None);
        };
        for hole in &mut holes {
            let Some(oriented) = orient(std::mem::take(hole), false) else {
                return Ok(None);
            };
            *hole = oriented;
        }
        faces.push(PlanarFace {
            id: face.id,
            frame,
            polygon,
            holes,
        });
    }
    Ok(Some(faces))
}

fn convex_faces(
    brep: &BrepEnvelope,
    tolerance: f64,
) -> Result<Option<Vec<PlanarFace>>, GeometryError> {
    let Some(faces) = planar_faces(brep, tolerance)? else {
        return Ok(None);
    };
    if brep.solids.len() != 1
        || brep.topology.shells.len() != 1
        || faces.iter().any(|face| !face.holes.is_empty())
    {
        return Ok(None);
    }
    for face in &faces {
        if brep
            .topology
            .vertices
            .iter()
            .any(|vertex| dot(face.frame.z, sub(vertex.position, face.frame.origin)) > tolerance)
        {
            return Ok(None);
        }
    }
    Ok(Some(faces))
}

fn planar_section_segments(
    brep: &BrepEnvelope,
    faces: &[PlanarFace],
    section: Frame3,
    tolerance: f64,
) -> Result<Vec<(Pt2, Pt2)>, GeometryError> {
    let mut segments = Vec::<(Pt2, Pt2)>::new();
    for source in faces {
        let direction = cross(source.frame.z, section.z);
        if norm(direction) <= 1.0e-10 {
            continue;
        }
        let direction = unit(direction)?;
        let mut crossings = Vec::new();
        for ring in std::iter::once(&source.polygon).chain(&source.holes) {
            for index in 0..ring.len() {
                let from = ring[index];
                let to = ring[(index + 1) % ring.len()];
                let from_distance = dot(section.z, sub(from, section.origin));
                let to_distance = dot(section.z, sub(to, section.origin));
                if from_distance.abs() <= tolerance {
                    crossings.push(from);
                }
                if (from_distance < -tolerance && to_distance > tolerance)
                    || (from_distance > tolerance && to_distance < -tolerance)
                {
                    let fraction = from_distance / (from_distance - to_distance);
                    crossings.push([
                        from[0] + fraction * (to[0] - from[0]),
                        from[1] + fraction * (to[1] - from[1]),
                        from[2] + fraction * (to[2] - from[2]),
                    ]);
                }
            }
        }
        crossings.sort_by(|left, right| dot(*left, direction).total_cmp(&dot(*right, direction)));
        crossings.dedup_by(|left, right| norm(sub(*left, *right)) <= tolerance);
        let face = &brep.topology.faces[source.id as usize];
        let SurfaceGeometry::Plane { frame: surface } =
            &brep.geometry.surfaces[face.surface as usize]
        else {
            return Err(coverage());
        };
        for pair in crossings.windows(2) {
            if norm(sub(pair[0], pair[1])) <= tolerance {
                continue;
            }
            let midpoint = scale(
                [
                    pair[0][0] + pair[1][0],
                    pair[0][1] + pair[1][1],
                    pair[0][2] + pair[1][2],
                ],
                0.5,
            );
            let local = surface.local(midpoint);
            let on_trim_edge = std::iter::once(&source.polygon)
                .chain(&source.holes)
                .any(|ring| {
                    ring.iter().enumerate().any(|(index, &from)| {
                        let to = ring[(index + 1) % ring.len()];
                        let edge = sub(to, from);
                        let length_squared = dot(edge, edge);
                        if length_squared <= tolerance * tolerance {
                            return false;
                        }
                        let fraction = dot(sub(midpoint, from), edge) / length_squared;
                        fraction >= -tolerance
                            && fraction <= 1.0 + tolerance
                            && norm(sub(sub(midpoint, from), scale(edge, fraction))) <= tolerance
                    })
                });
            if face_contains_uv(brep, face, [local[0], local[1]])? != Some(true) && !on_trim_edge {
                continue;
            }
            let a = section.local(pair[0]);
            let b = section.local(pair[1]);
            let mut from = Pt2::new(a[0], a[1]);
            let mut to = Pt2::new(b[0], b[1]);
            let edge = sub(pair[1], pair[0]);
            if dot(cross(edge, section.z), source.frame.z) < 0.0 {
                std::mem::swap(&mut from, &mut to);
            }
            segments.push((from, to));
        }
    }
    Ok(segments)
}

fn clip_convex_face(polygon: &[Point3], planes: &[PlanarFace], tolerance: f64) -> Vec<Point3> {
    let mut result = polygon.to_vec();
    for plane in planes {
        if result.len() < 3 {
            return Vec::new();
        }
        let mut next = Vec::new();
        let mut previous = *result.last().unwrap();
        let mut previous_distance = dot(plane.frame.z, sub(previous, plane.frame.origin));
        for &current in &result {
            let distance = dot(plane.frame.z, sub(current, plane.frame.origin));
            let previous_inside = previous_distance <= tolerance;
            let current_inside = distance <= tolerance;
            if previous_inside != current_inside {
                let fraction = previous_distance / (previous_distance - distance);
                next.push([
                    previous[0] + fraction * (current[0] - previous[0]),
                    previous[1] + fraction * (current[1] - previous[1]),
                    previous[2] + fraction * (current[2] - previous[2]),
                ]);
            }
            if current_inside {
                next.push(current);
            }
            previous = current;
            previous_distance = distance;
        }
        next.dedup_by(|left, right| norm(sub(*left, *right)) <= tolerance / 4.0);
        if next.len() > 1 && norm(sub(next[0], *next.last().unwrap())) <= tolerance / 4.0 {
            next.pop();
        }
        result = next;
    }
    if result.len() < 3 {
        Vec::new()
    } else {
        result
    }
}

fn convex_section_polygon(
    cutter: &BrepEnvelope,
    faces: &[PlanarFace],
    section: Frame3,
    tolerance: f64,
) -> Vec<Pt2> {
    let mut limits = [[f64::INFINITY, f64::NEG_INFINITY]; 2];
    for vertex in &cutter.topology.vertices {
        let local = section.local(vertex.position);
        for axis in 0..2 {
            limits[axis][0] = limits[axis][0].min(local[axis]);
            limits[axis][1] = limits[axis][1].max(local[axis]);
        }
    }
    if limits.iter().any(|bound| !bound[0].is_finite()) {
        return Vec::new();
    }
    let span = (limits[0][1] - limits[0][0])
        .max(limits[1][1] - limits[1][0])
        .max(1.0);
    let corners = [
        [limits[0][0] - span, limits[1][0] - span],
        [limits[0][1] + span, limits[1][0] - span],
        [limits[0][1] + span, limits[1][1] + span],
        [limits[0][0] - span, limits[1][1] + span],
    ]
    .map(|point| section.point([point[0], point[1], 0.0]));
    let clipped = clip_convex_face(&corners, faces, tolerance);
    if clipped.len() < 3 {
        return Vec::new();
    }
    let result = clipped
        .iter()
        .map(|point| {
            let local = section.local(*point);
            Pt2::new(local[0], local[1])
        })
        .collect::<Vec<_>>();
    let twice_area = result
        .iter()
        .enumerate()
        .map(|(index, point)| {
            let next = result[(index + 1) % result.len()];
            point.x * next.z - next.x * point.z
        })
        .sum::<f64>();
    if twice_area.abs() <= tolerance * tolerance {
        Vec::new()
    } else {
        result
    }
}

fn split_planar_ring(ring: &[Point3], vertices: &[Point3], tolerance: f64) -> Vec<Point3> {
    let mut split = Vec::new();
    for index in 0..ring.len() {
        let from = ring[index];
        let to = ring[(index + 1) % ring.len()];
        let delta = sub(to, from);
        let length = norm(delta);
        if length <= tolerance {
            continue;
        }
        split.push(from);
        let mut interior = Vec::new();
        for &point in vertices {
            let offset = sub(point, from);
            let fraction = dot(offset, delta) / (length * length);
            if fraction > tolerance / length
                && fraction < 1.0 - tolerance / length
                && norm(sub(offset, scale(delta, fraction))) <= tolerance
            {
                interior.push((fraction, point));
            }
        }
        interior.sort_by(|left, right| left.0.total_cmp(&right.0));
        for (_, point) in interior {
            if split
                .last()
                .is_none_or(|last| norm(sub(*last, point)) > tolerance)
            {
                split.push(point);
            }
        }
    }
    split
}

struct PlanarFacet {
    key: String,
    frame: Frame3,
    outer: Vec<Point3>,
    holes: Vec<Vec<Point3>>,
    provenance: FaceProvenance,
}

pub(super) fn subtract_planar_polyhedra(
    host: &BrepEnvelope,
    cutter: &BrepEnvelope,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let accuracy = Accuracy {
        geometric: host.accuracy.geometric.max(cutter.accuracy.geometric),
        intersection: host.accuracy.intersection.max(cutter.accuracy.intersection),
        tessellation: host.accuracy.tessellation.max(cutter.accuracy.tessellation),
        exchange: host.accuracy.exchange.max(cutter.accuracy.exchange),
    };
    let Some(host_faces) = planar_faces(host, accuracy.intersection)? else {
        return Err(coverage());
    };
    let Some(cutter_faces) = planar_faces(cutter, accuracy.intersection)? else {
        return Err(coverage());
    };
    let convex_cutter_faces = convex_faces(cutter, accuracy.intersection)?;
    if cutter.topology.vertices.iter().all(|vertex| {
        matches!(
            super::query::classify_point(host, vertex.position),
            Ok(super::query::PointClassification::Inside)
        )
    }) || host.topology.vertices.iter().all(|vertex| {
        matches!(
            super::query::classify_point(cutter, vertex.position),
            Ok(super::query::PointClassification::Inside)
        )
    }) {
        return Err(coverage());
    }
    let project = |points: &[Point3], frame: Frame3| {
        points
            .iter()
            .map(|point| {
                let local = frame.local(*point);
                Pt2::new(local[0], local[1])
            })
            .collect::<Vec<_>>()
    };
    let mut pieces = Vec::new();
    for face in &host_faces {
        let mut base = vec![project(&face.polygon, face.frame)];
        base.extend(face.holes.iter().map(|ring| project(ring, face.frame)));
        let regions = if let Some(convex_faces) = &convex_cutter_faces {
            let section =
                convex_section_polygon(cutter, convex_faces, face.frame, accuracy.intersection);
            let cut = if section.is_empty() {
                Vec::new()
            } else {
                vec![section]
            };
            boolean_oriented_regions(
                &base,
                &cut,
                PlanarBooleanOp::Subtraction,
                accuracy.intersection,
            )
        } else {
            let mut edges =
                planar_section_segments(cutter, &cutter_faces, face.frame, accuracy.intersection)?;
            let coplanar = cutter_faces
                .iter()
                .filter(|other| {
                    dot(other.frame.z, face.frame.z).abs() >= 1.0 - 1.0e-10
                        && dot(other.frame.z, sub(face.frame.origin, other.frame.origin)).abs()
                            <= accuracy.intersection
                })
                .flat_map(|other| {
                    std::iter::once(&other.polygon)
                        .chain(other.holes.iter())
                        .map(|ring| project(ring, face.frame))
                })
                .collect::<Vec<_>>();
            for ring in base.iter().chain(&coplanar) {
                edges.extend(
                    ring.iter()
                        .enumerate()
                        .map(|(index, &point)| (point, ring[(index + 1) % ring.len()])),
                );
            }
            let failure = RefCell::new(None);
            let regions = regions_from_edges_by(&edges, accuracy.intersection, |point| {
                if base
                    .iter()
                    .map(|ring| winding_number2(point, ring))
                    .sum::<i32>()
                    == 0
                {
                    return false;
                }
                let position = face.frame.point([point.x, point.z, 0.0]);
                let classification = super::query::classify_point(cutter, position);
                let classification = match classification {
                    Ok(super::query::PointClassification::Boundary) => {
                        super::query::classify_point(
                            cutter,
                            sub(
                                position,
                                scale(
                                    face.frame.z,
                                    accuracy.geometric.max(accuracy.intersection) * 4.0,
                                ),
                            ),
                        )
                    }
                    other => other,
                };
                match classification {
                    Ok(super::query::PointClassification::Outside) => true,
                    Ok(super::query::PointClassification::Inside) => false,
                    Ok(super::query::PointClassification::Boundary)
                    | Ok(super::query::PointClassification::Unknown) => {
                        *failure.borrow_mut() = Some(coverage());
                        false
                    }
                    Err(error) => {
                        *failure.borrow_mut() = Some(error);
                        false
                    }
                }
            });
            if let Some(error) = failure.into_inner() {
                return Err(error);
            }
            regions
        };
        for region in regions {
            pieces.push(PlanarFacet {
                key: format!("planar-host-{}", face.id),
                frame: face.frame,
                outer: region
                    .outer
                    .iter()
                    .map(|point| face.frame.point([point.x, point.z, 0.0]))
                    .collect(),
                holes: region
                    .holes
                    .iter()
                    .map(|ring| {
                        ring.iter()
                            .map(|point| face.frame.point([point.x, point.z, 0.0]))
                            .collect()
                    })
                    .collect(),
                provenance: FaceProvenance {
                    sources: vec![brep_face_source(host, face.id)],
                    role: FaceRole::Split,
                    reversed: false,
                },
            });
        }
    }
    for face in &cutter_faces {
        let mut edges =
            planar_section_segments(host, &host_faces, face.frame, accuracy.intersection)?;
        let coplanar = host_faces
            .iter()
            .filter(|other| {
                dot(other.frame.z, face.frame.z).abs() >= 1.0 - 1.0e-10
                    && dot(other.frame.z, sub(face.frame.origin, other.frame.origin)).abs()
                        <= accuracy.intersection
            })
            .flat_map(|other| {
                std::iter::once(&other.polygon)
                    .chain(other.holes.iter())
                    .map(|ring| project(ring, face.frame))
            })
            .collect::<Vec<_>>();
        let mut cutter_region = vec![project(&face.polygon, face.frame)];
        cutter_region.extend(face.holes.iter().map(|ring| project(ring, face.frame)));
        for ring in cutter_region.iter().chain(&coplanar) {
            edges.extend(
                ring.iter()
                    .enumerate()
                    .map(|(index, &point)| (point, ring[(index + 1) % ring.len()])),
            );
        }
        let failure = RefCell::new(None);
        let regions = regions_from_edges_by(&edges, accuracy.intersection, |point| {
            if cutter_region
                .iter()
                .map(|ring| winding_number2(point, ring))
                .sum::<i32>()
                == 0
                || coplanar
                    .iter()
                    .map(|ring| winding_number2(point, ring))
                    .sum::<i32>()
                    != 0
            {
                return false;
            }
            match super::query::classify_point(host, face.frame.point([point.x, point.z, 0.0])) {
                Ok(super::query::PointClassification::Inside)
                | Ok(super::query::PointClassification::Boundary) => true,
                Ok(super::query::PointClassification::Outside) => false,
                Ok(super::query::PointClassification::Unknown) => {
                    *failure.borrow_mut() = Some(coverage());
                    false
                }
                Err(error) => {
                    *failure.borrow_mut() = Some(error);
                    false
                }
            }
        });
        if let Some(error) = failure.into_inner() {
            return Err(error);
        }
        let reversed = Frame3 {
            origin: face.frame.origin,
            x: face.frame.x,
            y: scale(face.frame.y, -1.0),
            z: scale(face.frame.z, -1.0),
        };
        for region in regions {
            let mut outer = region
                .outer
                .iter()
                .map(|point| face.frame.point([point.x, point.z, 0.0]))
                .collect::<Vec<_>>();
            outer.reverse();
            let holes = region
                .holes
                .iter()
                .map(|ring| {
                    let mut points = ring
                        .iter()
                        .map(|point| face.frame.point([point.x, point.z, 0.0]))
                        .collect::<Vec<_>>();
                    points.reverse();
                    points
                })
                .collect();
            pieces.push(PlanarFacet {
                key: format!("planar-cut-{}", face.id),
                frame: reversed,
                outer,
                holes,
                provenance: FaceProvenance {
                    sources: vec![brep_face_source(cutter, face.id)],
                    role: FaceRole::Cut,
                    reversed: true,
                },
            });
        }
    }
    if pieces.is_empty() {
        return Err(coverage());
    }
    let vertices = pieces
        .iter()
        .flat_map(|piece| {
            piece
                .outer
                .iter()
                .chain(piece.holes.iter().flatten())
                .copied()
        })
        .collect::<Vec<_>>();
    let mut facets = Facets::new(id, accuracy)?;
    for piece in pieces {
        let outer = split_planar_ring(&piece.outer, &vertices, accuracy.geometric / 4.0);
        let holes = piece
            .holes
            .iter()
            .map(|ring| split_planar_ring(ring, &vertices, accuracy.geometric / 4.0))
            .collect();
        facets.face(&piece.key, piece.frame, outer, holes, piece.provenance)?;
    }
    let mut result = finish(facets, host, cutter)?;
    result.report.coincident = false;
    Ok(result)
}
