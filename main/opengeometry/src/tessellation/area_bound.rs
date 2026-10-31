use super::mesh::Tessellation;
use crate::brep::{
    BrepEnvelope, CurveGeometry, EdgeGeometry, Frame3, GeometryError, SurfaceGeometry,
};
use crate::math::{cross, dot, norm, scale, sub, Point3};
use std::f64::consts::PI;

const ROUNDING: f64 = 1e-10;

#[derive(Clone, Copy)]
struct Corners {
    points: [Point3; 3],
    normal: Point3,
    area: f64,
}

#[derive(Clone, Copy)]
struct Deviation {
    cosine: f64,
    offset: f64,
}

#[derive(Clone, Copy)]
struct Turn {
    frame: Frame3,
    cos: f64,
    sin: f64,
    apex_tolerance: f64,
}

#[derive(Clone, Copy)]
struct Arc {
    lo: [f64; 2],
    hi: [f64; 2],
}

pub(super) fn area_error_bound(
    body: &BrepEnvelope,
    mesh: &Tessellation,
    deflection: f64,
    area: f64,
) -> Result<f64, GeometryError> {
    let mut bound = ROUNDING * area + boundary_bound(body, mesh, deflection)?;
    for (index, ids) in mesh.indices.chunks_exact(3).enumerate() {
        let face = &body.topology.faces[mesh.triangle_face_ids[index] as usize];
        let surface = body.geometry.surface(face.surface)?;
        let corners = corners(mesh, ids);
        let sense = face.sense.multiplier();
        match deviation(surface, corners, sense, deflection, body.accuracy.geometric)
            .and_then(excess)
        {
            Some(excess) => bound += excess * corners.area,
            None => return Ok(f64::INFINITY),
        }
    }
    Ok(bound)
}

fn corners(mesh: &Tessellation, ids: &[u32]) -> Corners {
    let points = [mesh.point(ids[0]), mesh.point(ids[1]), mesh.point(ids[2])];
    let direction = cross(sub(points[1], points[0]), sub(points[2], points[0]));
    let length = norm(direction);
    Corners {
        points,
        normal: scale(direction, 1.0 / length),
        area: length * 0.5,
    }
}

fn excess(deviation: Deviation) -> Option<f64> {
    let Deviation { cosine, offset } = deviation;
    if !(0.0..1.0).contains(&offset) || !cosine.is_finite() {
        return None;
    }
    let lower = if cosine >= 0.0 {
        cosine / (1.0 + offset).powi(2)
    } else {
        cosine / (1.0 - offset).powi(2)
    };
    let upper = 1.0 / (1.0 - offset).powi(2);
    Some((1.0 - lower).max(upper - 1.0))
}

fn deviation(
    surface: &SurfaceGeometry,
    corners: Corners,
    sense: f64,
    deflection: f64,
    geometric: f64,
) -> Option<Deviation> {
    match *surface {
        SurfaceGeometry::Plane { .. } => Some(Deviation {
            cosine: 1.0,
            offset: 0.0,
        }),
        SurfaceGeometry::Sphere { frame, radius } => {
            let height = sense * dot(corners.normal, sub(corners.points[0], frame.origin));
            let ratio = (height / radius).min(1.0);
            (ratio > 0.0).then_some(Deviation {
                cosine: ratio,
                offset: 1.0 - ratio,
            })
        }
        SurfaceGeometry::Cylinder { frame, .. } => turned(
            Turn {
                frame,
                cos: 1.0,
                sin: 0.0,
                apex_tolerance: -1.0,
            },
            corners,
            sense,
        ),
        SurfaceGeometry::Cone { frame, semi_angle } => turned(
            Turn {
                frame,
                cos: semi_angle.cos(),
                sin: semi_angle.sin(),
                apex_tolerance: geometric,
            },
            corners,
            sense,
        ),
        SurfaceGeometry::Torus {
            frame,
            major_radius,
            minor_radius,
        } => torus(
            frame,
            [major_radius, minor_radius],
            corners,
            sense,
            deflection,
        ),
    }
}

fn turned(turn: Turn, corners: Corners, sense: f64) -> Option<Deviation> {
    let local = corners.points.map(|point| turn.frame.local(point));
    let nappe = if turn.sin == 0.0 { 1.0 } else { nappe(&local)? };
    let mut radial = [[0.0; 2]; 3];
    let mut count = 0;
    for p in local {
        let length = p[0].hypot(p[1]);
        if length > turn.apex_tolerance {
            radial[count] = [p[0] / length, p[1] / length];
            count += 1;
        }
    }
    let arc = arc(&radial[..count])?;
    let n = [
        dot(corners.normal, turn.frame.x),
        dot(corners.normal, turn.frame.y),
        dot(corners.normal, turn.frame.z),
    ];
    let along = sense * nappe * turn.cos;
    let across = -sense * turn.sin * n[2];
    let cosine_at = |u: [f64; 2]| along * (n[0] * u[0] + n[1] * u[1]) + across;
    let mut cosine = cosine_at(arc.lo).min(cosine_at(arc.hi));
    let toward = if along > 0.0 {
        [-n[0], -n[1]]
    } else {
        [n[0], n[1]]
    };
    if arc.holds(toward) {
        cosine = cosine.min(across - (along * n[0].hypot(n[1])).abs());
    }
    let half = ((1.0 + planar_dot(arc.lo, arc.hi)) / 2.0)
        .clamp(0.0, 1.0)
        .sqrt();
    Some(Deviation {
        cosine,
        offset: 1.0 - half,
    })
}

fn nappe(local: &[Point3; 3]) -> Option<f64> {
    if local.iter().all(|p| p[2] >= 0.0) {
        Some(1.0)
    } else if local.iter().all(|p| p[2] <= 0.0) {
        Some(-1.0)
    } else {
        None
    }
}

fn arc(radial: &[[f64; 2]]) -> Option<Arc> {
    let reference = *radial.first()?;
    let turn = |u: [f64; 2]| {
        let side = planar_cross(reference, u);
        if planar_dot(reference, u) >= 0.0 {
            side
        } else if side >= 0.0 {
            2.0 - side
        } else {
            -2.0 - side
        }
    };
    let mut arc = Arc {
        lo: reference,
        hi: reference,
    };
    for &u in &radial[1..] {
        if turn(u) < turn(arc.lo) {
            arc.lo = u;
        }
        if turn(u) > turn(arc.hi) {
            arc.hi = u;
        }
    }
    let cross = planar_cross(arc.lo, arc.hi);
    let opened = cross > 0.0 || (cross >= -f64::EPSILON && planar_dot(arc.lo, arc.hi) > 0.0);
    opened.then_some(arc)
}

impl Arc {
    fn holds(self, direction: [f64; 2]) -> bool {
        planar_cross(self.lo, direction) >= 0.0 && planar_cross(direction, self.hi) >= 0.0
    }
}

fn planar_dot(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

fn planar_cross(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

fn torus(
    frame: Frame3,
    [major, minor]: [f64; 2],
    corners: Corners,
    sense: f64,
    deflection: f64,
) -> Option<Deviation> {
    if major <= minor {
        return None;
    }
    let curvature = (1.0 / minor).max(1.0 / (major - minor));
    let mut widest: f64 = 0.0;
    for point in corners.points {
        let local = frame.local(point);
        let radial = local[0].hypot(local[1]);
        let ring = scale([local[0], local[1], 0.0], major / radial);
        let tube = sub(local, ring);
        let normal = scale(frame.vector(tube), sense / norm(tube));
        widest = widest.max(dot(corners.normal, normal).clamp(-1.0, 1.0).acos());
    }
    let [a, b, c] = corners.points;
    let longest = norm(sub(b, a)).max(norm(sub(c, b))).max(norm(sub(a, c)));
    let reach = curvature * (longest / 3f64.sqrt() + deflection);
    let cosine = if reach > 1.0 {
        -1.0
    } else {
        (widest + 2.0 * (reach / 2.0).asin()).min(PI).cos()
    };
    Some(Deviation {
        cosine,
        offset: curvature * deflection,
    })
}

fn boundary_bound(
    body: &BrepEnvelope,
    mesh: &Tessellation,
    deflection: f64,
) -> Result<f64, GeometryError> {
    let mut chords = vec![0.0; body.topology.edges.len()];
    for (segment, &edge) in mesh
        .outline_positions
        .chunks_exact(6)
        .zip(&mesh.outline_edge_ids)
    {
        let start = [segment[0], segment[1], segment[2]];
        let end = [segment[3], segment[4], segment[5]];
        chords[edge as usize] += norm(sub(end, start));
    }
    let mut bound = 0.0;
    for (edge, chord) in body.topology.edges.iter().zip(chords) {
        let EdgeGeometry::Curve { curve, .. } = edge.geometry else {
            continue;
        };
        if matches!(
            body.geometry.curves[curve as usize],
            CurveGeometry::Line { .. }
        ) {
            continue;
        }
        for halfedge in std::iter::once(edge.halfedge).chain(edge.twin_halfedge) {
            let Some(face) = body.topology.halfedges[halfedge as usize].face else {
                continue;
            };
            let surface = body
                .geometry
                .surface(body.topology.faces[face as usize].surface)?;
            let width = if matches!(surface, SurfaceGeometry::Plane { .. }) {
                deflection
            } else {
                2.0 * deflection
            };
            bound += width * chord;
        }
    }
    Ok(bound)
}
