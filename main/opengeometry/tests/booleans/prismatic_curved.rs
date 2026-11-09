use crate::support::{assert_valid_closed, assert_volume};
use opengeometry::brep::{BrepEnvelope, Frame3};
use opengeometry::operations::modifying::boolean::{boolean_brep_with_handlers, BooleanOp};
use opengeometry::primitives;
use opengeometry::primitives::{ProfileSegment, SegmentArc};
use opengeometry::world_graph::Primitive;
use opengeometry::world_graph::{EditScope, ErrorCode, ModifyingOperation, WorldGraph};
use opengeometry_test_support::accuracy::accuracy;
use opengeometry_test_support::scenes::prismatic::{
    box_and_leg, box_and_pocket, d_shape_area, extrude_profile, pan_and_inset, pan_volume,
    BOX_VOLUME, LEG_HEIGHT, LEG_RADIUS,
};
use opengeometry_test_support::volume::{self, VolumeEstimate};
use opengeometry_test_support::world_graph::graph;
use serde_json::json;
use std::f64::consts::PI;

fn operate_with_prismatic_handler(
    world: &mut WorldGraph,
    target: &str,
    operation: ModifyingOperation,
    tool: &str,
) -> VolumeEstimate {
    world
        .operate(target, operation, &[tool.to_string()], EditScope::Node)
        .unwrap_or_else(|error| panic!("{operation:?}: {error:?}"));
    assert_eq!(
        world.report(target).unwrap().unwrap()["handlers"],
        json!(["prismatic_curved_boolean"]),
        "{operation:?}"
    );
    let body = world.brep(target).unwrap();
    assert_valid_closed(&body, 1, &format!("{operation:?}"));
    volume::estimate(&body, 1e-5)
}

#[test]
fn an_arc_edged_extrusion_minus_a_blind_inset_builds_one_closed_pan_with_no_sliver_at_a_flush_top()
{
    for inset_height in [0.4, 0.35] {
        let mut world = pan_and_inset(inset_height);
        let measured = operate_with_prismatic_handler(
            &mut world,
            "pan",
            ModifyingOperation::Subtract,
            "inset",
        );
        assert_volume(measured, pan_volume(), &format!("inset {inset_height}"));
        let body = serde_json::to_value(world.brep("pan").unwrap().as_ref()).unwrap();
        assert_eq!(
            body["topology"]["faces"].as_array().unwrap().len(),
            11,
            "inset {inset_height}"
        );
    }
}

#[test]
fn a_cuboid_and_a_side_crossing_upright_cylinder_build_for_every_operation() {
    let disc = PI * LEG_RADIUS * LEG_RADIUS;
    let inside = disc / 2.0 * 0.5;
    for (operation, expected) in [
        (ModifyingOperation::Subtract, BOX_VOLUME - inside),
        (
            ModifyingOperation::Union,
            BOX_VOLUME + disc * LEG_HEIGHT - inside,
        ),
        (ModifyingOperation::Intersect, inside),
    ] {
        let mut world = box_and_leg(0.0, [0.5, -0.1, 0.0]);
        let measured = operate_with_prismatic_handler(&mut world, "box", operation, "leg");
        assert_volume(measured, expected, &format!("{operation:?}"));
    }
}

#[test]
fn a_cuboid_minus_a_blind_arc_edged_pocket_builds_with_the_pocket_floor() {
    let mut world = box_and_pocket(0.5);
    let measured =
        operate_with_prismatic_handler(&mut world, "box", ModifyingOperation::Subtract, "pocket");
    assert_volume(
        measured,
        BOX_VOLUME - d_shape_area(0.5, 0.35, 0.0) * 0.2,
        "pocket",
    );
}

#[test]
fn a_pocket_arc_tangent_to_a_cuboid_side_is_an_unresolved_intersection() {
    let mut world = box_and_pocket(0.75);
    let error = world
        .operate(
            "box",
            ModifyingOperation::Subtract,
            &["pocket".to_string()],
            EditScope::Node,
        )
        .unwrap_err();
    assert_eq!(
        error.error_code(),
        ErrorCode::UnresolvedIntersection,
        "{error:?}"
    );
}

fn block_and_leg(x: f64, z: f64) -> (BrepEnvelope, BrepEnvelope) {
    let at = |origin| Frame3 {
        origin,
        ..Frame3::IDENTITY
    };
    let block = primitives::cuboid(
        "block".into(),
        at([-0.5, -0.5, 0.0]),
        [1.0, 1.0, 0.5],
        accuracy(1e-6),
    )
    .unwrap();
    let leg = primitives::cylinder("leg".into(), at([x, 0.0, z]), 0.1, 0.7, accuracy(1e-6));
    (block, leg.unwrap())
}

#[test]
fn an_intersection_of_a_stacked_leg_empty_in_every_layer_is_the_separate_empty_result() {
    let (block, stacked) = block_and_leg(0.5, 0.5);
    let (result, handlers) =
        boolean_brep_with_handlers(&block, &stacked, BooleanOp::Intersection, "empty".into())
            .unwrap();
    assert_eq!(handlers, ["prismatic_curved_boolean"]);
    let (block, far) = block_and_leg(5.0, -0.1);
    let (separate, handlers) =
        boolean_brep_with_handlers(&block, &far, BooleanOp::Intersection, "empty".into()).unwrap();
    assert_eq!(handlers, ["separate_boolean"]);
    assert_eq!(
        serde_json::to_value(&result.brep).unwrap(),
        serde_json::to_value(&separate.brep).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&result.report).unwrap(),
        serde_json::to_value(&separate.report).unwrap()
    );
    let mut world = box_and_leg(0.0, [0.5, 0.5, 0.0]);
    let error = world
        .operate(
            "box",
            ModifyingOperation::Intersect,
            &["leg".to_string()],
            EditScope::Node,
        )
        .unwrap_err();
    assert_eq!(error.error_code(), ErrorCode::EmptyResult, "{error:?}");
}

const INNER: f64 = 2.9;
const OUTER: f64 = 3.1;
const HEIGHT: f64 = 3.0;

fn line(to: [f64; 2]) -> ProfileSegment {
    ProfileSegment { to, arc: None }
}

fn quarter_ring() -> Primitive {
    Primitive::Profile {
        start: [INNER, 0.0],
        segments: vec![
            line([OUTER, 0.0]),
            ProfileSegment {
                to: [0.0, OUTER],
                arc: Some(SegmentArc {
                    center: [0.0, 0.0],
                    counter_clockwise: true,
                }),
            },
            line([0.0, INNER]),
            ProfileSegment {
                to: [INNER, 0.0],
                arc: Some(SegmentArc {
                    center: [0.0, 0.0],
                    counter_clockwise: false,
                }),
            },
        ],
        closed: true,
    }
}

fn rectangle(low: [f64; 2], high: [f64; 2]) -> Primitive {
    Primitive::Profile {
        start: low,
        segments: vec![
            line([high[0], low[1]]),
            line(high),
            line([low[0], high[1]]),
            line(low),
        ],
        closed: true,
    }
}

fn ring_overlap(reach: f64) -> f64 {
    let square = OUTER * OUTER;
    (reach * (square - reach * reach).sqrt() + square * (reach / OUTER).asin()) / 2.0
        - INNER * reach
}

fn ring_area() -> f64 {
    PI / 4.0 * (OUTER * OUTER - INNER * INNER)
}

fn ring_and(tool: Primitive) -> WorldGraph {
    let mut world = graph();
    extrude_profile(&mut world, "ring", quarter_ring(), [0.0, 0.0, 0.0], HEIGHT);
    extrude_profile(&mut world, "tool", tool, [0.0, 0.0, 0.0], HEIGHT);
    world
}

#[test]
fn a_prism_whose_sides_run_on_along_the_tangents_at_the_arcs_ends_unions_with_the_ring() {
    for reach in [0.5, 0.9] {
        let mut world = ring_and(rectangle([INNER, -2.0], [OUTER, reach]));
        let measured =
            operate_with_prismatic_handler(&mut world, "ring", ModifyingOperation::Union, "tool");
        let tangent = (OUTER - INNER) * (2.0 + reach);
        assert_volume(
            measured,
            HEIGHT * (ring_area() + tangent - ring_overlap(reach)),
            &format!("reach {reach}"),
        );
    }
}

#[test]
fn a_prism_whose_side_runs_on_along_the_tangent_at_the_inner_arcs_end_unions_with_the_ring() {
    let mut world = ring_and(rectangle([-0.2, INNER], [0.05, INNER + 2.0]));
    let measured =
        operate_with_prismatic_handler(&mut world, "ring", ModifyingOperation::Union, "tool");
    assert_volume(
        measured,
        HEIGHT * (ring_area() + 0.25 * 2.0 - ring_overlap(0.05)),
        "side",
    );
}

fn ring_at(centre: [f64; 2], inner: f64, outer: f64) -> Primitive {
    let at = |radius: f64, angle: f64| {
        [
            centre[0] + radius * angle.cos(),
            centre[1] + radius * angle.sin(),
        ]
    };
    let arc = |to| ProfileSegment {
        to,
        arc: Some(SegmentArc {
            center: centre,
            counter_clockwise: true,
        }),
    };
    let mut segments = vec![
        line(at(outer, 0.0)),
        arc(at(outer, PI / 2.0)),
        line(at(inner, PI / 2.0)),
    ];
    segments.push(ProfileSegment {
        to: at(inner, 0.0),
        arc: Some(SegmentArc {
            center: centre,
            counter_clockwise: false,
        }),
    });
    Primitive::Profile {
        start: at(inner, 0.0),
        segments,
        closed: true,
    }
}

fn parallel_jamb_box(centre: [f64; 2], angle: f64, half: f64, near: f64, far: f64) -> Primitive {
    let (radial, side) = ([angle.cos(), angle.sin()], [-angle.sin(), angle.cos()]);
    let corner = |reach: f64, offset: f64| {
        [
            centre[0] + reach * radial[0] + offset * side[0],
            centre[1] + reach * radial[1] + offset * side[1],
        ]
    };
    let points = [
        corner(near, -half),
        corner(far, -half),
        corner(far, half),
        corner(near, half),
    ];
    Primitive::Profile {
        start: points[0],
        segments: points[1..]
            .iter()
            .chain(std::iter::once(&points[0]))
            .map(|&to| line(to))
            .collect(),
        closed: true,
    }
}

#[test]
fn two_boxes_through_a_curved_wall_that_ends_on_the_cylinder_seam_cut_both_holes() {
    let (centre, inner, outer, thickness) = ([0.0, -10.0], 3.8425, 4.1575, 0.315);
    let hole = |angle: f64, half: f64| {
        let near = (inner * inner - half * half).sqrt() - thickness;
        parallel_jamb_box(centre, angle, half, near, outer + thickness)
    };
    let mut world = graph();
    extrude_profile(
        &mut world,
        "ring",
        ring_at(centre, inner, outer),
        [0.0; 3],
        3.0,
    );
    extrude_profile(
        &mut world,
        "door",
        hole(PI / 2.0 - 0.375, 0.5),
        [0.0; 3],
        2.15,
    );
    extrude_profile(
        &mut world,
        "window",
        hole(PI / 2.0 - 1.0, 0.8),
        [0.0, 0.85, 0.0],
        1.3,
    );
    world
        .operate(
            "ring",
            ModifyingOperation::Subtract,
            &["door".to_string(), "window".to_string()],
            EditScope::Node,
        )
        .unwrap_or_else(|error| panic!("{error:?}"));
    assert_valid_closed(&world.brep("ring").unwrap(), 1, "ring");
}
