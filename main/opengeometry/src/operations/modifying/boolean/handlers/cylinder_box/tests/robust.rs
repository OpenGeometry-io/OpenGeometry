use super::support::{cylinder_with, robust, sloped_probe, Built, Cut, Probe, R};
use crate::brep::{fine_accuracy, unit, Frame3, GeometryError};
use crate::math::{add, dot, scale, sub};
use crate::operations::modifying::boolean::dispatch::boolean_brep;
use crate::operations::modifying::boolean::types::BooleanOp;
use crate::primitives;

fn left_to_other_paths(outcomes: [Option<Built>; 2], label: &str) {
    for built in outcomes.into_iter().flatten() {
        assert!(
            !(built.cut_by_cuboid && built.handlers == ["cylinder_box_boolean"]),
            "{label}: {:?}",
            built.handlers
        );
    }
}

#[test]
fn near_perpendicular_cuts_are_exact_or_refused() {
    let probe = sloped_probe(R, 6.0);
    let axis = unit(probe.axis).unwrap();
    let mid = add(probe.origin, scale(axis, 3.0));
    for eps in [1e-1, 1e-3, 1e-4, 1e-6, 1e-7, 1e-9] {
        let normal = unit(add(axis, scale([0.0, 1.0, 0.0], eps))).unwrap();
        robust(
            "perpendicular",
            &probe,
            Cut {
                point: mid,
                normal,
                reference: [0.0, 1.0, 0.0],
            },
        );
    }
}

#[test]
fn near_parallel_cuts_are_exact_or_unresolved() {
    let probe = sloped_probe(R, 6.0);
    let axis = probe.axis;
    let mid = add(probe.origin, scale(unit(axis).unwrap(), 3.0));
    for delta in [
        1e-13, 1e-12, 5e-12, 2e-11, 5e-11, 1e-10, 1e-9, 1e-8, 9e-8, 1.1e-7, 3e-7, 1e-6, 1e-5, 1e-4,
        1e-3, 1e-2,
    ] {
        let normal = unit(add([0.0, 1.0, 0.0], scale(axis, delta))).unwrap();
        robust(
            "tilt",
            &probe,
            Cut {
                point: add(mid, [0.0, 0.1, 0.0]),
                normal,
                reference: [1.0, 0.0, 0.0],
            },
        );
    }
}

#[test]
fn near_parallel_cuts_at_large_and_small_scale_are_exact_or_unresolved() {
    for (radius, height) in [(500.0, 6000.0), (0.001, 0.01)] {
        let probe = sloped_probe(radius, height);
        let axis = unit(probe.axis).unwrap();
        let mid = add(probe.origin, scale(axis, height / 2.0));
        for delta in [1e-6, 1e-3, 0.3] {
            let normal = unit(add([0.0, 1.0, 0.0], scale(axis, delta))).unwrap();
            robust(
                "scale",
                &probe,
                Cut {
                    point: add(mid, [0.0, 0.2 * radius, 0.0]),
                    normal,
                    reference: [1.0, 0.0, 0.0],
                },
            );
        }
    }
}

#[test]
fn slivers_at_a_side_are_left_to_other_paths() {
    let probe = sloped_probe(R, 6.0);
    for d in [
        0.0,
        0.3,
        0.499,
        0.5 - 1e-6,
        0.5 - 8e-9,
        0.5 - 3e-9,
        0.5,
        0.5 + 3e-9,
        0.5 + 1e-6,
        -0.5,
        -0.5 + 1e-7,
    ] {
        let outcomes = robust(
            "parallel",
            &probe,
            Cut {
                point: [0.0, -0.2 + d, 0.0],
                normal: [0.0, 1.0, 0.0],
                reference: [1.0, 0.0, 0.0],
            },
        );
        if (R - f64::abs(d)).abs() <= 1e-3 * R {
            left_to_other_paths(outcomes, "parallel sliver");
        }
    }
    let aligned = Probe {
        origin: [0.0, 0.0, 1.0],
        axis: [1.0, 0.0, 0.0],
        radius: R,
        height: 6.0,
    };
    for d in [0.0, 0.2, 0.5 - 1e-6, 0.5] {
        robust(
            "aligned",
            &aligned,
            Cut {
                point: [0.0, d, 1.0],
                normal: [0.0, 1.0, 0.0],
                reference: [1.0, 0.0, 0.0],
            },
        );
    }
}

#[test]
fn slivers_at_a_rim_are_left_to_other_paths() {
    let probe = sloped_probe(R, 6.0);
    let z = unit(probe.axis).unwrap();
    let b = 30f64.to_radians();
    let normal = [b.cos(), b.sin(), 0.0];
    let across = unit(sub(normal, scale(z, dot(normal, z)))).unwrap();
    let top = add(probe.origin, scale(z, 6.0));
    for (center, sign) in [
        (top, -1.0),
        (top, 1.0),
        (probe.origin, -1.0),
        (probe.origin, 1.0),
    ] {
        let rim = add(center, scale(across, sign * R));
        for s in [
            0.0, 2e-9, -2e-9, 1e-8, -1e-8, 1e-6, -1e-6, 1e-4, -1e-4, 1e-3, -1e-3,
        ] {
            let outcomes = robust(
                "rim",
                &probe,
                Cut {
                    point: add(rim, scale(normal, s)),
                    normal,
                    reference: [0.0, 0.0, 1.0],
                },
            );
            if f64::abs(s) <= 1e-6 {
                left_to_other_paths(outcomes, "rim sliver");
            }
        }
    }
}

#[test]
fn two_crossing_cuboid_faces_are_not_a_half_space() {
    let probe = sloped_probe(R, 6.0);
    let host = cylinder_with(probe.origin, probe.axis, R, 6.0);
    let mid = add(probe.origin, scale(unit(probe.axis).unwrap(), 3.0));
    for (ox, oy) in [(0.0, 0.0), (0.0, -0.2), (-50.0, 0.1)] {
        let cutter = primitives::cuboid(
            "box".into(),
            Frame3 {
                origin: add(mid, [ox, oy, -10.0]),
                ..Frame3::IDENTITY
            },
            [50.0, 50.0, 50.0],
            fine_accuracy(),
        )
        .unwrap();
        for operation in [BooleanOp::Subtraction, BooleanOp::Intersection] {
            assert!(matches!(
                boolean_brep(&host, &cutter, operation, "out".into()),
                Err(GeometryError::CoverageGap { .. })
            ));
        }
    }
}
