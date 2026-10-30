use crate::brep::{
    fine_accuracy, unit, BrepEnvelope, CurveGeometry, Face, Frame3, GeometryError, Orientation,
    SurfaceGeometry,
};
use crate::math::{add, dot, norm, scale, sub, Point3};
use crate::operations::modifying::boolean::dispatch::boolean_brep_with_handlers;
use crate::operations::modifying::boolean::types::BooleanOp;
use crate::primitives;
use crate::tessellation::{measure, tessellate};
use std::f64::consts::PI;

pub(super) const R: f64 = 0.5;

pub(super) struct Cyl {
    pub(super) origin: Point3,
    pub(super) axis: Point3,
    pub(super) height: f64,
}

#[derive(Clone, Copy)]
pub(super) struct Cut {
    pub(super) point: Point3,
    pub(super) normal: Point3,
    pub(super) reference: Point3,
}

pub(super) struct Expected {
    pub(super) faces: [usize; 2],
    pub(super) handlers: [&'static str; 2],
}

pub(super) const HALF_SPACE: [&str; 2] = ["cylinder_box_boolean", "cylinder_box_boolean"];

pub(super) fn cylinder_with(
    origin: Point3,
    axis: Point3,
    radius: f64,
    height: f64,
) -> BrepEnvelope {
    let reference = if axis[2].abs() < 0.9 {
        [0.0, 0.0, 1.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let frame = Frame3::from_axis(origin, axis, reference).unwrap();
    primitives::cylinder("cyl".into(), frame, radius, height, fine_accuracy()).unwrap()
}

pub(super) fn half_space(cut: &Cut, big: f64) -> BrepEnvelope {
    let frame = Frame3::from_axis([0.0; 3], unit(cut.normal).unwrap(), cut.reference).unwrap();
    let origin = sub(
        sub(cut.point, scale(frame.x, big / 2.0)),
        scale(frame.y, big / 2.0),
    );
    primitives::cuboid(
        "half".into(),
        Frame3 { origin, ..frame },
        [big, big, big / 2.0],
        fine_accuracy(),
    )
    .unwrap()
}

pub(super) fn along_axis(c: &Cyl, t: f64) -> Point3 {
    add(c.origin, scale(unit(c.axis).unwrap(), t))
}

pub(super) fn kept_volume(c: &Cyl, point: Point3, normal: Point3) -> f64 {
    let z = unit(c.axis).unwrap();
    let rate = dot(normal, z);
    let rho = norm(sub(normal, scale(z, rate)));
    let offset = dot(normal, sub(c.origin, point));
    let length = |u: f64| {
        let rhs = -(offset + rho * u);
        if rate.abs() < 1e-14 {
            if rhs >= 0.0 {
                c.height
            } else {
                0.0
            }
        } else if rate > 0.0 {
            (rhs / rate).clamp(0.0, c.height)
        } else {
            c.height - (rhs / rate).clamp(0.0, c.height)
        }
    };
    let mut breaks = vec![-R, R];
    if rho > 0.0 {
        breaks.push(-offset / rho);
        breaks.push(-(offset + rate * c.height) / rho);
    }
    let mut breaks: Vec<f64> = breaks.into_iter().map(|u| u.clamp(-R, R)).collect();
    breaks.sort_by(f64::total_cmp);
    breaks
        .windows(2)
        .map(|pair| piece_volume(pair[0], pair[1], &length))
        .sum()
}

fn piece_volume(lo: f64, hi: f64, length: &dyn Fn(f64) -> f64) -> f64 {
    if hi <= lo {
        return 0.0;
    }
    let (u1, u2) = (lo + (hi - lo) / 3.0, lo + 2.0 * (hi - lo) / 3.0);
    let slope = (length(u2) - length(u1)) / (u2 - u1);
    let level = length(u1) - slope * u1;
    let f = |u: f64| u * (R * R - u * u).max(0.0).sqrt() + R * R * (u / R).asin();
    let g = |u: f64| -2.0 / 3.0 * (R * R - u * u).max(0.0).powf(1.5);
    level * (f(hi) - f(lo)) + slope * (g(hi) - g(lo))
}

pub(super) fn assert_kept(brep: &BrepEnvelope, side: &Cut, volume: f64, tolerance: f64) {
    let measured = measure(brep, 5e-4).unwrap();
    assert!(
        (measured.volume - volume).abs() <= measured.error_bound.min(tolerance),
        "volume {} expected {volume}, bound {}",
        measured.volume,
        measured.error_bound
    );
    let faces = cut_faces(brep, side);
    assert!(!faces.is_empty(), "a cut face");
    for face in faces {
        let SurfaceGeometry::Plane { frame } = &brep.geometry.surfaces[face.surface as usize]
        else {
            unreachable!()
        };
        assert!(
            dot(scale(frame.z, face.sense.multiplier()), side.normal) > 0.0,
            "shell must face outward"
        );
    }
}

pub(super) fn section_area(c: &Cyl, point: Point3, normal: Point3) -> f64 {
    let z = unit(c.axis).unwrap();
    let rate = dot(normal, z).abs();
    let sine = (1.0 - rate * rate).max(0.0).sqrt();
    if rate < 1e-14 {
        let d = dot(normal, sub(point, c.origin)).abs();
        return c.height * 2.0 * (R * R - d * d).sqrt();
    }
    if sine < 1e-14 {
        return PI * R * R;
    }
    let major = R / rate;
    let center = dot(normal, sub(point, c.origin)) / dot(normal, z);
    let f = |x: f64| {
        let x = x.clamp(-1.0, 1.0);
        x * (1.0 - x * x).sqrt() + x.asin()
    };
    let s1 = -center / sine;
    let s2 = (c.height - center) / sine;
    R * major * (f(s2 / major) - f(s1 / major))
}

pub(super) fn face_area(brep: &BrepEnvelope, face: &Face) -> f64 {
    let mut points = Vec::new();
    let start = brep.topology.loops[face.trim.outer as usize].start_halfedge;
    let mut h = start;
    loop {
        let half = &brep.topology.halfedges[h as usize];
        let range = brep.topology.edges[half.edge as usize].geometry.range();
        let pcurve = half.geometry_use.pcurve.unwrap();
        let samples = 20_000;
        for i in 0..samples {
            let s = i as f64 / samples as f64;
            let t = if half.geometry_use.sense == Orientation::Forward {
                range.lo + (range.hi - range.lo) * s
            } else {
                range.hi - (range.hi - range.lo) * s
            };
            points.push(brep.geometry.pcurve_at(pcurve, t).unwrap());
        }
        h = half.next.unwrap();
        if h == start {
            break;
        }
    }
    let n = points.len();
    let twice: f64 = (0..n)
        .map(|i| {
            let (a, b) = (points[i], points[(i + 1) % n]);
            a[0] * b[1] - a[1] * b[0]
        })
        .sum();
    twice / 2.0
}

fn cut_faces<'a>(brep: &'a BrepEnvelope, cut: &Cut) -> Vec<&'a Face> {
    brep.topology
        .faces
        .iter()
        .filter(
            |face| match &brep.geometry.surfaces[face.surface as usize] {
                SurfaceGeometry::Plane { frame } => {
                    1.0 - dot(frame.z, cut.normal).abs() < 1e-12
                        && dot(cut.normal, sub(frame.origin, cut.point)).abs() < 1e-9
                }
                _ => false,
            },
        )
        .collect()
}

fn assert_closed(brep: &BrepEnvelope, operation: BooleanOp) {
    brep.validate().unwrap();
    assert_eq!(brep.topology.shells.len(), 1, "{operation:?}");
    assert!(brep.topology.shells[0].is_closed, "{operation:?}");
    assert!(brep
        .topology
        .edges
        .iter()
        .all(|edge| edge.twin_halfedge.is_some()));
    BrepEnvelope::from_json(&brep.to_json().unwrap()).unwrap();
}

fn assert_ellipse(brep: &BrepEnvelope, c: &Cyl, normal: Point3) {
    let rate = dot(unit(c.axis).unwrap(), normal).abs();
    if rate > 1e-12 && rate < 1.0 - 1e-12 {
        assert!(brep.geometry.curves.iter().any(|curve| matches!(
            curve,
            CurveGeometry::Ellipse { major_radius, minor_radius, .. }
                if (major_radius - R / rate).abs() < 1e-9 && (minor_radius - R).abs() < 1e-12
        )));
    }
}

pub(super) fn assert_cut(c: Cyl, cut: Cut, expected: Expected) {
    let host = cylinder_with(c.origin, c.axis, R, c.height);
    let cutter = half_space(&cut, 100.0);
    let full = PI * R * R * c.height;
    let expected_area = section_area(&c, cut.point, cut.normal);
    for (index, (operation, outward)) in [
        (BooleanOp::Subtraction, cut.normal),
        (BooleanOp::Intersection, scale(cut.normal, -1.0)),
    ]
    .into_iter()
    .enumerate()
    {
        let (result, handlers) =
            boolean_brep_with_handlers(&host, &cutter, operation, "out".into())
                .unwrap_or_else(|error| panic!("{operation:?}: {error:?}"));
        assert_eq!(handlers, [expected.handlers[index]], "{operation:?}");
        let brep = result.brep;
        assert_closed(&brep, operation);
        assert_eq!(
            brep.topology.faces.len(),
            expected.faces[index],
            "{operation:?} face count"
        );
        let side = Cut {
            normal: outward,
            ..cut
        };
        assert_kept(
            &brep,
            &side,
            kept_volume(&c, cut.point, outward),
            1e-3 * full,
        );
        let faces = cut_faces(&brep, &cut);
        assert_eq!(faces.len(), 1, "{operation:?}: one cut face");
        assert_ellipse(&brep, &c, cut.normal);
        let area = face_area(&brep, faces[0]).abs();
        assert!(
            (area - expected_area).abs() <= 1e-6 * expected_area,
            "{operation:?}: cut area {area} expected {expected_area}"
        );
    }
}

pub(super) struct Probe {
    pub(super) origin: Point3,
    pub(super) axis: Point3,
    pub(super) radius: f64,
    pub(super) height: f64,
}

pub(super) struct Built {
    pub(super) handlers: Vec<String>,
    pub(super) cut_by_cuboid: bool,
}

pub(super) fn robust(label: &str, probe: &Probe, cut: Cut) -> [Option<Built>; 2] {
    let host = cylinder_with(probe.origin, probe.axis, probe.radius, probe.height);
    let cutter = half_space(&cut, 100.0 * probe.height.max(probe.radius));
    [BooleanOp::Subtraction, BooleanOp::Intersection].map(|operation| {
        match boolean_brep_with_handlers(&host, &cutter, operation, "out".into()) {
            Ok((result, handlers)) => {
                result.brep.validate().unwrap();
                if !result.brep.topology.faces.is_empty() {
                    tessellate(&result.brep, 2e-3 * probe.radius, 4_000_000).unwrap_or_else(
                        |error| panic!("{label} {operation:?} {:?}: {error:?}", cut.point),
                    );
                }
                let cut_by_cuboid = result.brep.topology.faces.iter().any(|face| {
                    face.provenance
                        .sources
                        .iter()
                        .any(|source| source.entity == cutter.id)
                });
                Some(Built {
                    handlers,
                    cut_by_cuboid,
                })
            }
            Err(
                error @ (GeometryError::InvalidGeometry(_) | GeometryError::InvalidTopology(_)),
            ) => panic!("{label} {operation:?}: {error:?}"),
            Err(_) => None,
        }
    })
}

pub(super) fn sloped(degrees: f64, height: f64) -> Cyl {
    let a = degrees.to_radians();
    Cyl {
        origin: [0.3, -0.2, 1.0],
        axis: [a.cos(), 0.0, a.sin()],
        height,
    }
}

pub(super) fn sloped_probe(radius: f64, height: f64) -> Probe {
    let c = sloped(40.0, height);
    Probe {
        origin: c.origin,
        axis: c.axis,
        radius,
        height,
    }
}
