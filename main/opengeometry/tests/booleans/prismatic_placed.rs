use crate::support::{assert_valid_closed, assert_volume};
use opengeometry::brep::{BrepEnvelope, Frame3, GeometryError};
use opengeometry::operations::modifying::boolean::{
    boolean_brep_outcome_with_handlers, boolean_brep_with_handlers, BooleanOp,
};
use opengeometry::primitives::{self, ProfileEdge};
use opengeometry_test_support::accuracy::accuracy;
use opengeometry_test_support::scenes::prismatic::d_shape_area;
use opengeometry_test_support::volume;
use std::f64::consts::PI;

const OPERATIONS: [BooleanOp; 3] = [
    BooleanOp::Union,
    BooleanOp::Intersection,
    BooleanOp::Subtraction,
];

struct Volumes {
    a: f64,
    b: f64,
    common: f64,
}

fn assert_prismatic(a: &BrepEnvelope, b: &BrepEnvelope, operation: BooleanOp, expected: f64) {
    let label = format!("{} {} {operation:?}", a.id, b.id);
    let (result, handlers) = boolean_brep_with_handlers(a, b, operation, "placed".into())
        .unwrap_or_else(|error| panic!("{label}: {error:?}"));
    assert_eq!(handlers, ["prismatic_curved_boolean"], "{label}");
    assert_valid_closed(&result.brep, 1, &label);
    assert_volume(volume::estimate(&result.brep, 1e-5), expected, &label);
}

fn assert_every_operation(a: &BrepEnvelope, b: &BrepEnvelope, volumes: Volumes) {
    let Volumes {
        a: va,
        b: vb,
        common,
    } = volumes;
    for (operation, expected) in OPERATIONS
        .into_iter()
        .zip([va + vb - common, common, va - common])
    {
        assert_prismatic(a, b, operation, expected);
    }
}

fn turned(degrees: f64) -> impl Fn([f64; 3]) -> Frame3 {
    let (sin, cos) = degrees.to_radians().sin_cos();
    move |origin| Frame3 {
        origin: [
            cos * origin[0] - sin * origin[1],
            sin * origin[0] + cos * origin[1],
            origin[2],
        ],
        x: [cos, sin, 0.0],
        y: [-sin, cos, 0.0],
        z: [0.0, 0.0, 1.0],
    }
}

fn spun(origin: [f64; 3]) -> Frame3 {
    let axis = [1.0, 2.0, 3.0].map(|value: f64| value / 14.0_f64.sqrt());
    let (sin, cos) = 0.7_f64.sin_cos();
    let rotate = |v: [f64; 3]| -> [f64; 3] {
        let along = axis[0] * v[0] + axis[1] * v[1] + axis[2] * v[2];
        let cross = [
            axis[1] * v[2] - axis[2] * v[1],
            axis[2] * v[0] - axis[0] * v[2],
            axis[0] * v[1] - axis[1] * v[0],
        ];
        [0, 1, 2].map(|i| v[i] * cos + cross[i] * sin + axis[i] * along * (1.0 - cos))
    };
    Frame3 {
        origin: rotate(origin),
        x: rotate([1.0, 0.0, 0.0]),
        y: rotate([0.0, 1.0, 0.0]),
        z: rotate([0.0, 0.0, 1.0]),
    }
}

fn d_shape(name: &str, frame: Frame3, inset: f64, height: f64) -> BrepEnvelope {
    let (straight, far, radius) = (0.325, 0.35 - inset, 0.175 - inset);
    let edges = vec![
        ProfileEdge::Line {
            from: [inset, inset],
            to: [straight, inset],
        },
        ProfileEdge::Arc {
            center: [straight, 0.175],
            radius,
            start_angle: -PI / 2.0,
            sweep_angle: PI,
        },
        ProfileEdge::Line {
            from: [straight, far],
            to: [inset, far],
        },
        ProfileEdge::Line {
            from: [inset, far],
            to: [inset, inset],
        },
    ];
    primitives::arc_edged_extrusion(name.into(), frame, edges, height, accuracy(1e-6)).unwrap()
}

fn oval(frame: Frame3) -> BrepEnvelope {
    let line = |from, to| ProfileEdge::Line { from, to };
    let arc = |center, start_angle| ProfileEdge::Arc {
        center,
        radius: 0.3,
        start_angle,
        sweep_angle: PI,
    };
    let edges = vec![
        line([-0.5, -0.3], [0.5, -0.3]),
        arc([0.5, 0.0], -PI / 2.0),
        line([0.5, 0.3], [-0.5, 0.3]),
        arc([-0.5, 0.0], PI / 2.0),
    ];
    primitives::arc_edged_extrusion("oval".into(), frame, edges, 0.4, accuracy(1e-6)).unwrap()
}

fn block(frame: Frame3) -> BrepEnvelope {
    primitives::cuboid("block".into(), frame, [1.0, 1.0, 0.5], accuracy(1e-6)).unwrap()
}

fn round(frame: Frame3, radius: f64, height: f64) -> BrepEnvelope {
    primitives::cylinder("round".into(), frame, radius, height, accuracy(1e-6)).unwrap()
}

fn lens(distance: f64, first: f64, second: f64) -> f64 {
    let part = |near: f64, far: f64| {
        near * near
            * ((distance * distance + near * near - far * far) / (2.0 * distance * near)).acos()
    };
    let kite = (-distance + first + second)
        * (distance + first - second)
        * (distance - first + second)
        * (distance + first + second);
    part(first, second) + part(second, first) - kite.sqrt() / 2.0
}

#[test]
fn a_pan_turned_thirty_degrees_in_plan_builds_for_every_operation() {
    let at = turned(30.0);
    let pan = d_shape("pan", at([0.0; 3]), 0.0, 0.4);
    let inset = d_shape("inset", at([0.0, 0.0, 0.05]), 0.03, 0.4);
    let (outer, inner) = (d_shape_area(0.5, 0.35, 0.0), d_shape_area(0.5, 0.35, 0.03));
    let volumes = Volumes {
        a: outer * 0.4,
        b: inner * 0.4,
        common: inner * 0.35,
    };
    assert_every_operation(&pan, &inset, volumes);
}

#[test]
fn a_block_minus_a_blind_pocket_turned_thirty_degrees_in_plan_builds() {
    let at = turned(30.0);
    let pocket = d_shape("pocket", at([-0.25, -0.175, 0.3]), 0.0, 0.4);
    let expected = 0.5 - d_shape_area(0.5, 0.35, 0.0) * 0.2;
    assert_prismatic(
        &block(at([-0.5, -0.5, 0.0])),
        &pocket,
        BooleanOp::Subtraction,
        expected,
    );
}

#[test]
fn a_block_and_a_side_crossing_cylinder_turned_in_space_build_for_every_operation() {
    let leg = round(spun([0.5, 0.0, -0.1]), 0.2, 0.8);
    let volumes = Volumes {
        a: 0.5,
        b: PI * 0.04 * 0.8,
        common: PI * 0.04 / 2.0 * 0.5,
    };
    assert_every_operation(&block(spun([-0.5, -0.5, 0.0])), &leg, volumes);
}

#[test]
fn an_oval_and_a_blind_cylinder_across_its_arc_build_for_every_operation() {
    let bore = round(turned(0.0)([0.75, 0.0, 0.2]), 0.2, 0.4);
    let volumes = Volumes {
        a: (0.6 + PI * 0.09) * 0.4,
        b: PI * 0.04 * 0.4,
        common: lens(0.25, 0.3, 0.2) * 0.2,
    };
    assert_every_operation(&oval(turned(0.0)([0.0; 3])), &bore, volumes);
}

#[test]
fn an_equal_radius_bore_at_the_round_end_of_a_d_shape_builds_for_every_operation() {
    let at = turned(0.0);
    let bore = round(at([0.325, 0.175, 0.2]), 0.175, 0.3);
    let disc = PI * 0.175 * 0.175;
    let volumes = Volumes {
        a: d_shape_area(0.5, 0.35, 0.0) * 0.4,
        b: disc * 0.3,
        common: disc * 0.2,
    };
    assert_every_operation(&d_shape("d", at([0.0; 3]), 0.0, 0.4), &bore, volumes);
}

#[test]
fn a_cylinder_inscribed_in_the_straight_part_of_a_d_shape_is_still_refused() {
    let at = turned(0.0);
    let inscribed = round(at([0.175, 0.175, 0.2]), 0.175, 0.3);
    let (result, _) = boolean_brep_outcome_with_handlers(
        &d_shape("d", at([0.0; 3]), 0.0, 0.4),
        &inscribed,
        BooleanOp::Subtraction,
        "inscribed".into(),
    );
    assert!(
        matches!(result, Err(GeometryError::UnresolvedIntersection(_))),
        "{:?}",
        result.err()
    );
}
