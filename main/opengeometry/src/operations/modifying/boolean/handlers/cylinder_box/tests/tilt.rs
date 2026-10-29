use super::support::{assert_kept, cylinder_with, Cut, R};
use crate::brep::{fine_accuracy, BrepEnvelope, Frame3, GeometryError};
use crate::math::{scale, sub};
use crate::operations::modifying::boolean::dispatch::{
    boolean_brep_outcome_with_handlers, boolean_brep_with_handlers,
};
use crate::operations::modifying::boolean::types::BooleanOp;
use crate::primitives;
use std::f64::consts::PI;

fn host() -> BrepEnvelope {
    cylinder_with([0.0; 3], [0.0, 1.0, 0.0], R, 2.0)
}

fn tilted_cuboid(degrees: f64) -> BrepEnvelope {
    let t = degrees.to_radians();
    let x = [t.cos(), t.sin(), 0.0];
    let y = [-t.sin(), t.cos(), 0.0];
    let z = [0.0, 0.0, 1.0];
    let origin = sub(sub([0.0, 1.0, 0.0], scale(x, 5.0)), scale(z, 5.0));
    primitives::cuboid(
        "box".into(),
        Frame3 { origin, x, y, z },
        [10.0; 3],
        fine_accuracy(),
    )
    .unwrap()
}

fn side(degrees: f64, sign: f64) -> Cut {
    let t = degrees.to_radians();
    Cut {
        point: [0.0, 1.0, 0.0],
        normal: [-sign * t.sin(), sign * t.cos(), 0.0],
        reference: [0.0, 0.0, 1.0],
    }
}

fn assert_half(a: &BrepEnvelope, b: &BrepEnvelope, operation: BooleanOp, handler: &str, side: Cut) {
    let (result, handlers) = boolean_brep_with_handlers(a, b, operation, "out".into())
        .unwrap_or_else(|error| panic!("{operation:?}: {error:?}"));
    assert_eq!(handlers, [handler], "{operation:?}");
    result.brep.validate().unwrap();
    assert_eq!(result.brep.solids.len(), 1);
    assert!(result.brep.topology.shells[0].is_closed);
    let expected = PI * R * R;
    assert_kept(&result.brep, &side, expected, 1e-3 * expected);
}

#[test]
fn cuboid_tilted_against_the_axis_cuts_an_exact_half_from_the_cylinder() {
    let cylinder = host();
    for degrees in [20.0, 45.0, 70.0] {
        let cuboid = tilted_cuboid(degrees);
        assert_half(
            &cylinder,
            &cuboid,
            BooleanOp::Subtraction,
            "cylinder_box_boolean",
            side(degrees, 1.0),
        );
        assert_half(
            &cylinder,
            &cuboid,
            BooleanOp::Intersection,
            "cylinder_box_boolean",
            side(degrees, -1.0),
        );
        assert_half(
            &cuboid,
            &cylinder,
            BooleanOp::Intersection,
            "cylinder_box_boolean",
            side(degrees, -1.0),
        );
    }
}

#[test]
fn cuboid_across_or_along_the_axis_keeps_its_handler() {
    let cylinder = host();
    assert_half(
        &cylinder,
        &tilted_cuboid(0.0),
        BooleanOp::Subtraction,
        "cylinder_box_boolean",
        side(0.0, 1.0),
    );
    assert_half(
        &cylinder,
        &tilted_cuboid(90.0),
        BooleanOp::Subtraction,
        "subtract_vertical_arc_extrusion",
        side(90.0, 1.0),
    );
}

#[test]
fn tilted_cuboid_minus_cylinder_and_their_union_stay_a_coverage_gap() {
    let cylinder = host();
    let cuboid = tilted_cuboid(45.0);
    for (a, b, operation) in [
        (&cuboid, &cylinder, BooleanOp::Subtraction),
        (&cylinder, &cuboid, BooleanOp::Union),
    ] {
        let (result, handlers) = boolean_brep_outcome_with_handlers(a, b, operation, "out".into());
        assert!(handlers.is_empty(), "{handlers:?}");
        assert!(
            matches!(result, Err(GeometryError::CoverageGap { .. })),
            "{operation:?}: {:?}",
            result.map(|_| ())
        );
    }
}
