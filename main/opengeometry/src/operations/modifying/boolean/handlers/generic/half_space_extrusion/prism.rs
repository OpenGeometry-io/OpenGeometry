use crate::brep::{
    unit, BrepEnvelope, CurveGeometry, EdgeGeometry, FaceRole, FaceSource, Frame3, GeometryError,
    HalfEdge, Orientation, SurfaceGeometry,
};
use crate::math::{cross, dot, scale, sub, Point3};
use crate::operations::modifying::boolean::operands::brep_face_source;
use crate::primitives::ProfileEdge;
use std::f64::consts::{PI, TAU};

#[derive(Clone, Copy)]
pub(super) struct Affine {
    pub(super) k: f64,
    pub(super) g: [f64; 2],
}

impl Affine {
    pub(super) fn at(self, q: [f64; 2]) -> f64 {
        self.k + self.g[0] * q[0] + self.g[1] * q[1]
    }

    pub(super) fn minus(self, other: Self) -> Self {
        Self {
            k: self.k - other.k,
            g: [self.g[0] - other.g[0], self.g[1] - other.g[1]],
        }
    }
}

#[derive(Clone)]
pub(super) struct Cap {
    pub(super) point: Point3,
    pub(super) normal: Point3,
    pub(super) height: Affine,
    pub(super) source: FaceSource,
    pub(super) role: FaceRole,
    pub(super) reversed: bool,
}

pub(super) struct Span {
    pub(super) edge: ProfileEdge,
    pub(super) source: FaceSource,
}

pub(super) struct Prism {
    pub(super) frame: Frame3,
    pub(super) loops: Vec<Vec<Span>>,
    pub(super) caps: [Cap; 2],
}

struct ArcSource {
    center: [f64; 2],
    radius: f64,
    counter_clockwise: bool,
}

pub(super) fn gap() -> GeometryError {
    GeometryError::CoverageGap {
        families: ["arc-edged extrusion".into(), "half-space cutter".into()],
    }
}

pub(super) fn outward(normal: Point3, sense: Orientation) -> Point3 {
    if sense == Orientation::Reverse {
        scale(normal, -1.0)
    } else {
        normal
    }
}

pub(super) fn cap_height(frame: Frame3, point: Point3, normal: Point3) -> Affine {
    let along = dot(normal, frame.z);
    Affine {
        k: dot(normal, sub(point, frame.origin)) / along,
        g: [-dot(normal, frame.x) / along, -dot(normal, frame.y) / along],
    }
}

pub(super) fn prism(host: &BrepEnvelope) -> Result<Prism, GeometryError> {
    if host.solids.len() != 1
        || host.topology.shells.len() != 1
        || !host.solids[0].cavity_shells.is_empty()
    {
        return Err(gap());
    }
    let frame = section_frame(host)?;
    let [low, high] = cap_faces(host, frame)?;
    let loops = cap_loops(host, frame, high)?;
    check_low_cap(host, frame, low, &loops)?;
    let caps = [low, high].map(|face| {
        let face_ref = &host.topology.faces[face as usize];
        let plane = host.geometry.surfaces[face_ref.surface as usize].frame();
        let normal = outward(plane.z, face_ref.sense);
        Cap {
            point: plane.origin,
            normal,
            height: cap_height(frame, plane.origin, normal),
            source: brep_face_source(host, face),
            role: FaceRole::Preserved,
            reversed: false,
        }
    });
    Ok(Prism { frame, loops, caps })
}

fn section_frame(host: &BrepEnvelope) -> Result<Frame3, GeometryError> {
    let axis = host
        .geometry
        .surfaces
        .iter()
        .find_map(|surface| match surface {
            SurfaceGeometry::Cylinder { frame, .. } => Some(*frame),
            _ => None,
        })
        .ok_or_else(gap)?;
    let x = unit(sub(axis.x, scale(axis.z, dot(axis.x, axis.z))))?;
    Ok(Frame3 {
        origin: axis.origin,
        x,
        y: cross(axis.z, x),
        z: axis.z,
    })
}

fn cap_faces(host: &BrepEnvelope, frame: Frame3) -> Result<[u32; 2], GeometryError> {
    let mut caps = [None, None];
    for face in &host.topology.faces {
        let along = match &host.geometry.surfaces[face.surface as usize] {
            SurfaceGeometry::Plane { frame: plane } => dot(outward(plane.z, face.sense), frame.z),
            SurfaceGeometry::Cylinder { frame: axis, .. }
                if 1.0 - dot(axis.z, frame.z).abs() <= 1.0e-12 =>
            {
                0.0
            }
            _ => return Err(gap()),
        };
        if along.abs() <= 1.0e-12 {
            continue;
        }
        if along.abs() < 1.0e-6 {
            return Err(gap());
        }
        let slot = &mut caps[usize::from(along > 0.0)];
        if slot.replace(face.id).is_some() {
            return Err(gap());
        }
    }
    match caps {
        [Some(low), Some(high)] => Ok([low, high]),
        _ => Err(gap()),
    }
}

fn loop_halfedges(host: &BrepEnvelope, start: u32) -> Result<Vec<&HalfEdge>, GeometryError> {
    let mut halfedges = Vec::new();
    let mut current = start;
    loop {
        let halfedge = &host.topology.halfedges[current as usize];
        halfedges.push(halfedge);
        current = halfedge.next.ok_or_else(gap)?;
        if current == start {
            return Ok(halfedges);
        }
        if halfedges.len() > host.topology.halfedges.len() {
            return Err(gap());
        }
    }
}

fn face_loops(host: &BrepEnvelope, face: u32) -> Vec<u32> {
    let trim = &host.topology.faces[face as usize].trim;
    std::iter::once(trim.outer)
        .chain(trim.holes.iter().copied())
        .map(|id| host.topology.loops[id as usize].start_halfedge)
        .collect()
}

fn section(frame: Frame3, point: Point3) -> [f64; 2] {
    let local = frame.local(point);
    [local[0], local[1]]
}

fn cap_loops(
    host: &BrepEnvelope,
    frame: Frame3,
    face: u32,
) -> Result<Vec<Vec<Span>>, GeometryError> {
    let mut loops = Vec::new();
    for (index, start) in face_loops(host, face).into_iter().enumerate() {
        let mut spans = Vec::new();
        for halfedge in loop_halfedges(host, start)? {
            let twin = halfedge.twin.ok_or_else(gap)?;
            let side = host.topology.halfedges[twin as usize]
                .face
                .ok_or_else(gap)?;
            for edge in span_edges(host, frame, halfedge)? {
                spans.push(Span {
                    edge,
                    source: brep_face_source(host, side),
                });
            }
        }
        let area = area_twice(&spans);
        if (index == 0) != (area > 0.0) {
            return Err(gap());
        }
        loops.push(spans);
    }
    Ok(loops)
}

fn span_edges(
    host: &BrepEnvelope,
    frame: Frame3,
    halfedge: &HalfEdge,
) -> Result<Vec<ProfileEdge>, GeometryError> {
    let EdgeGeometry::Curve { curve, range } =
        &host.topology.edges[halfedge.edge as usize].geometry
    else {
        return Err(gap());
    };
    let geometry = &host.geometry.curves[*curve as usize];
    let position = |vertex: u32| host.topology.vertices[vertex as usize].position;
    let from = section(frame, position(halfedge.from));
    let to = section(frame, position(halfedge.to));
    let (center, radius) = match geometry {
        CurveGeometry::Line { .. } => return Ok(vec![ProfileEdge::Line { from, to }]),
        CurveGeometry::Circle { frame: c, radius } => (c.origin, *radius),
        CurveGeometry::Ellipse {
            frame: c,
            minor_radius,
            ..
        } => (c.origin, *minor_radius),
        _ => return Err(gap()),
    };
    let center = section(frame, center);
    let middle = 0.5 * (range.lo + range.hi);
    let radial = sub2(section(frame, geometry.elementary_point(middle)?), center);
    let tangent = geometry.elementary_tangent(middle)?;
    let turn = radial[0] * dot(tangent, frame.y) - radial[1] * dot(tangent, frame.x);
    let arc = ArcSource {
        center,
        radius,
        counter_clockwise: (turn > 0.0) == (halfedge.geometry_use.sense == Orientation::Forward),
    };
    arc_edges(&arc, from, to, halfedge.from == halfedge.to)
}

fn sub2(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

fn angle(center: [f64; 2], point: [f64; 2]) -> f64 {
    let d = sub2(point, center);
    d[1].atan2(d[0])
}

fn arc_edges(
    arc: &ArcSource,
    from: [f64; 2],
    to: [f64; 2],
    closed: bool,
) -> Result<Vec<ProfileEdge>, GeometryError> {
    let start = angle(arc.center, from);
    let sign = if arc.counter_clockwise { 1.0 } else { -1.0 };
    let edge = |start_angle: f64, sweep_angle: f64| ProfileEdge::Arc {
        center: arc.center,
        radius: arc.radius,
        start_angle,
        sweep_angle,
    };
    if closed {
        return Ok(vec![
            edge(start, sign * PI),
            edge(start + sign * PI, sign * PI),
        ]);
    }
    let sweep = sign * (sign * (angle(arc.center, to) - start)).rem_euclid(TAU);
    if sweep.abs() <= 1.0e-9 {
        return Err(gap());
    }
    Ok(vec![edge(start, sweep)])
}

fn area_twice(spans: &[Span]) -> f64 {
    spans
        .iter()
        .map(|span| {
            let (p, q) = span.edge.endpoints();
            let chord = p[0] * q[1] - q[0] * p[1];
            match span.edge {
                ProfileEdge::Line { .. } => chord,
                ProfileEdge::Arc {
                    center,
                    radius,
                    sweep_angle,
                    ..
                } => {
                    let fan = center[0] * (q[1] - p[1]) - center[1] * (q[0] - p[0]);
                    fan + radius * radius * sweep_angle
                }
            }
        })
        .sum()
}

fn check_low_cap(
    host: &BrepEnvelope,
    frame: Frame3,
    face: u32,
    loops: &[Vec<Span>],
) -> Result<(), GeometryError> {
    let tolerance = 4.0 * host.accuracy.geometric;
    let points = loops
        .iter()
        .flatten()
        .map(|span| span.edge.endpoints().0)
        .collect::<Vec<_>>();
    let mut sides = loops
        .iter()
        .flatten()
        .map(|span| span.source.face)
        .collect::<Vec<_>>();
    let mut low_sides = Vec::new();
    for start in face_loops(host, face) {
        for halfedge in loop_halfedges(host, start)? {
            let twin = halfedge.twin.ok_or_else(gap)?;
            low_sides.push(
                host.topology.halfedges[twin as usize]
                    .face
                    .ok_or_else(gap)?,
            );
            let point = section(
                frame,
                host.topology.vertices[halfedge.from as usize].position,
            );
            if !points.iter().any(|known| {
                let d = sub2(point, *known);
                d[0].hypot(d[1]) <= tolerance
            }) {
                return Err(gap());
            }
        }
    }
    sides.sort_unstable();
    sides.dedup();
    low_sides.sort_unstable();
    low_sides.dedup();
    if sides != low_sides || sides.len() + 2 != host.topology.faces.len() {
        return Err(gap());
    }
    Ok(())
}
