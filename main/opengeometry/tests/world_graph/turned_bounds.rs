use super::support::near_within;
use opengeometry::brep::{Accuracy, CurveGeometry, Frame3};
use opengeometry::math::Point3;
use opengeometry::primitives::{arc_wire, ProfileSegment, SegmentArc};
use opengeometry::world_graph::{CreatingOperation, EditScope, MirrorPlane, Primitive, WorldGraph};
use opengeometry_test_support::world_graph::{graph, named};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

const EMPTY: [f64; 6] = [
    f64::INFINITY,
    f64::INFINITY,
    f64::INFINITY,
    -f64::INFINITY,
    -f64::INFINITY,
    -f64::INFINITY,
];

fn along(center: Point3, a: Point3, b: Point3, t: f64) -> Point3 {
    std::array::from_fn(|i| center[i] + a[i] * t.cos() + b[i] * t.sin())
}

fn arc_box(center: Point3, a: Point3, b: Point3, range: [f64; 2]) -> [f64; 6] {
    let mut angles = range.to_vec();
    for i in 0..3 {
        let critical = b[i].atan2(a[i]);
        for k in -4..=4 {
            let t = critical + k as f64 * PI;
            if range[0] <= t && t <= range[1] {
                angles.push(t);
            }
        }
    }
    let mut result = EMPTY;
    for t in angles {
        let p = along(center, a, b, t);
        for i in 0..3 {
            result[i] = result[i].min(p[i]);
            result[i + 3] = result[i + 3].max(p[i]);
        }
    }
    result
}

fn hull(a: [f64; 6], b: [f64; 6]) -> [f64; 6] {
    std::array::from_fn(|i| {
        if i < 3 {
            a[i].min(b[i])
        } else {
            a[i].max(b[i])
        }
    })
}

fn assert_exact(actual: [f64; 6], expected: [f64; 6]) {
    let size = (0..3)
        .map(|i| expected[i + 3] - expected[i])
        .fold(0.0, f64::max);
    near_within(&actual, &expected, 1e-10 * size);
}

fn curve_box(curve: CurveGeometry, start: f64, sweep: f64) -> [f64; 6] {
    let bounds = arc_wire("arc".into(), curve, start, sweep, Accuracy::STANDARD)
        .unwrap()
        .bounds()
        .unwrap()
        .unwrap();
    std::array::from_fn(|i| {
        if i < 3 {
            bounds.axes[i].lo
        } else {
            bounds.axes[i - 3].hi
        }
    })
}

fn about_z() -> Frame3 {
    Frame3::from_axis([0.5, -1.0, 2.0], [0.0, 0.0, 1.0], [1.0, 1.0, 0.0]).unwrap()
}

fn skew() -> Frame3 {
    Frame3::from_axis([0.3, -0.2, 0.5], [1.0, 2.0, 3.0], [1.0, -0.4, 0.2]).unwrap()
}

fn circle_box(frame: Frame3, radius: f64, range: [f64; 2]) -> [f64; 6] {
    let a = frame.x.map(|v| v * radius);
    let b = frame.y.map(|v| v * radius);
    arc_box(frame.origin, a, b, range)
}

#[test]
fn a_full_circle_turned_about_the_vertical_gets_its_exact_box() {
    let curve = CurveGeometry::Circle {
        frame: about_z(),
        radius: 1.0,
    };
    assert_exact(
        curve_box(curve, 0.0, TAU),
        circle_box(about_z(), 1.0, [0.0, TAU]),
    );
}

#[test]
fn a_full_circle_turned_about_a_skew_axis_gets_its_exact_box() {
    let curve = CurveGeometry::Circle {
        frame: skew(),
        radius: 1.0,
    };
    assert_exact(
        curve_box(curve, 0.0, TAU),
        circle_box(skew(), 1.0, [0.0, TAU]),
    );
}

#[test]
fn an_arc_on_a_skew_frame_gets_its_exact_box() {
    let curve = CurveGeometry::Circle {
        frame: skew(),
        radius: 0.8,
    };
    assert_exact(
        curve_box(curve, 0.4, 2.1),
        circle_box(skew(), 0.8, [0.4, 2.5]),
    );
}

#[test]
fn an_ellipse_on_a_skew_frame_gets_its_exact_box() {
    let frame = skew();
    let curve = CurveGeometry::Ellipse {
        frame,
        major_radius: 2.0,
        minor_radius: 0.7,
    };
    let a = frame.x.map(|v| v * 2.0);
    let b = frame.y.map(|v| v * 0.7);
    assert_exact(
        curve_box(curve.clone(), 0.0, TAU),
        arc_box(frame.origin, a, b, [0.0, TAU]),
    );
    assert_exact(
        curve_box(curve, -1.0, 2.6),
        arc_box(frame.origin, a, b, [-1.0, 1.6]),
    );
}

fn reflect(plane: MirrorPlane, p: Point3, offset: Point3) -> Point3 {
    let length = plane.normal.iter().map(|v| v * v).sum::<f64>().sqrt();
    let n = plane.normal.map(|v| v / length);
    let d: f64 = (0..3).map(|i| (p[i] - offset[i]) * n[i]).sum();
    std::array::from_fn(|i| p[i] - 2.0 * d * n[i])
}

fn arm_box(
    point: impl Fn(Point3) -> Point3,
    vector: impl Fn(Point3) -> Point3,
    heights: &[f64],
) -> [f64; 6] {
    let a = vector([0.04, 0.0, 0.0]);
    let b = vector([0.0, 0.0, -0.04]);
    let mut result = EMPTY;
    for &y in heights {
        let center = point([0.55, y, -0.04]);
        result = hull(result, arc_box(center, a, b, [-FRAC_PI_2, FRAC_PI_2]));
        for corner in [[0.0, y, 0.0], [0.0, y, -0.08]] {
            let p = point(corner);
            result = hull(result, [p[0], p[1], p[2], p[0], p[1], p[2]]);
        }
    }
    result
}

fn arm_outline() -> Primitive {
    let line = |to: [f64; 2]| ProfileSegment { to, arc: None };
    let arc = ProfileSegment {
        to: [0.55, 0.08],
        arc: Some(SegmentArc {
            center: [0.55, 0.04],
            counter_clockwise: true,
        }),
    };
    Primitive::Profile {
        start: [0.0, 0.0],
        segments: vec![line([0.55, 0.0]), arc, line([0.0, 0.08]), line([0.0, 0.0])],
        closed: true,
    }
}

fn slanted() -> MirrorPlane {
    MirrorPlane {
        origin: [0.3, -0.2, 0.5],
        normal: [1.0, 2.0, -0.5],
    }
}

fn mirrored_box(world: &mut WorldGraph, og_id: &str) -> [f64; 6] {
    world.mirror(og_id, slanted(), EditScope::Node).unwrap();
    world.bounds(og_id).unwrap().unwrap()
}

fn slanted_point(p: Point3) -> Point3 {
    reflect(slanted(), p, slanted().origin)
}

fn slanted_vector(v: Point3) -> Point3 {
    reflect(slanted(), v, [0.0; 3])
}

#[test]
fn a_mirrored_chair_arm_outline_gets_its_exact_box() {
    let mut world = graph();
    world
        .create_primitive(arm_outline(), named("outline"))
        .unwrap();
    assert_exact(
        world.bounds("outline").unwrap().unwrap(),
        arm_box(|p| p, |v| v, &[0.0]),
    );
    let expected = arm_box(slanted_point, slanted_vector, &[0.0]);
    assert_exact(mirrored_box(&mut world, "outline"), expected);
}

#[test]
fn a_mirrored_chair_arm_gets_its_exact_box() {
    let mut world = graph();
    world
        .create_primitive(arm_outline(), named("outline"))
        .unwrap();
    let extrude = CreatingOperation::Extrude {
        profile: "outline".into(),
        holes: Vec::new(),
        distance: 0.06,
    };
    world.create_operation(extrude, named("arm")).unwrap();
    let expected = arm_box(slanted_point, slanted_vector, &[0.0, 0.06]);
    near_within(&mirrored_box(&mut world, "arm"), &expected, 3e-8);
}

fn rings_box(rings: &[(f64, f64)]) -> [f64; 6] {
    let mut result = EMPTY;
    for &(y, radius) in rings {
        let a = slanted_vector([radius, 0.0, 0.0]);
        let b = slanted_vector([0.0, 0.0, radius]);
        result = hull(
            result,
            arc_box(slanted_point([0.0, y, 0.0]), a, b, [0.0, TAU]),
        );
    }
    result
}

#[test]
fn a_cylinder_and_a_cone_on_a_slanted_frame_get_their_exact_boxes() {
    let cases = [
        (
            Primitive::Cylinder {
                radius: 0.3,
                height: 0.6,
            },
            [(0.0, 0.3), (0.6, 0.3)],
        ),
        (
            Primitive::Cone {
                radius_bottom: 1.0,
                radius_top: 0.5,
                height: 2.0,
            },
            [(0.0, 1.0), (2.0, 0.5)],
        ),
        (
            Primitive::Cone {
                radius_bottom: 0.0,
                radius_top: 0.4,
                height: 1.5,
            },
            [(0.0, 0.0), (1.5, 0.4)],
        ),
    ];
    for (primitive, rings) in cases {
        let mut world = graph();
        world.create_primitive(primitive, named("body")).unwrap();
        assert_exact(mirrored_box(&mut world, "body"), rings_box(&rings));
    }
}

#[test]
fn a_torus_on_a_slanted_frame_gets_its_exact_box() {
    let mut world = graph();
    let torus = Primitive::Torus {
        major_radius: 2.0,
        minor_radius: 0.5,
    };
    world.create_primitive(torus, named("ring")).unwrap();
    let center = slanted_point([0.0, 0.5, 0.0]);
    let a = slanted_vector([1.0, 0.0, 0.0]);
    let b = slanted_vector([0.0, 0.0, 1.0]);
    let half: [f64; 3] = std::array::from_fn(|i| 2.0 * a[i].hypot(b[i]) + 0.5);
    let expected: [f64; 6] = std::array::from_fn(|i| {
        if i < 3 {
            center[i] - half[i]
        } else {
            center[i - 3] + half[i - 3]
        }
    });
    assert_exact(mirrored_box(&mut world, "ring"), expected);
}
