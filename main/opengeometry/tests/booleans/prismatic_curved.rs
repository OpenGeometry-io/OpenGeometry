use crate::support::{assert_valid_closed, assert_volume};
use opengeometry::brep::{BrepEnvelope, Frame3};
use opengeometry::operations::modifying::boolean::{boolean_brep_with_handlers, BooleanOp};
use opengeometry::primitives;
use opengeometry::world_graph::{EditScope, ErrorCode, ModifyingOperation, WorldGraph};
use opengeometry_test_support::accuracy::accuracy;
use opengeometry_test_support::scenes::prismatic::{
    box_and_leg, box_and_pocket, d_shape_area, pan_and_inset, pan_volume, BOX_VOLUME, LEG_HEIGHT,
    LEG_RADIUS,
};
use opengeometry_test_support::volume::{self, VolumeEstimate};
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
