use opengeometry::brep::{BrepEnvelope, GeometryError};
use opengeometry::operations::modifying::boolean::{multi_tool_boolean, BooleanOp};
use opengeometry::world_graph::{
    EditScope, ErrorCode, ErrorDetails, GraphError, ModifyingOperation, Primitive, Transform,
};
use opengeometry_test_support::world_graph::{graph, named};

const APEX_UP: (f64, f64) = (0.5, 0.0);
const APEX_DOWN: (f64, f64) = (0.0, 0.5);
const THROUGH_AXIS: [f64; 3] = [0.0, -1.0, -1.0];
const OFF_AXIS: [f64; 3] = [0.0, -1.0, -0.8];

fn cone((radius_bottom, radius_top): (f64, f64)) -> Primitive {
    Primitive::Cone {
        radius_bottom,
        radius_top,
        height: 1.0,
    }
}

fn subtract_half_space(radii: (f64, f64), offset: [f64; 3], degrees: f64) -> GraphError {
    let mut world = graph();
    world.create_primitive(cone(radii), named("cone")).unwrap();
    world
        .create_primitive(
            Primitive::Cuboid {
                width: 4.0,
                height: 4.0,
                depth: 2.0,
            },
            named("half-space"),
        )
        .unwrap();
    world
        .transform("half-space", Transform::Translate { offset })
        .unwrap();
    world
        .transform(
            "half-space",
            Transform::Rotate {
                axis: [0.0, 1.0, 0.0],
                degrees,
                pivot: Some([0.0; 3]),
            },
        )
        .unwrap();
    world
        .operate(
            "cone",
            ModifyingOperation::Subtract,
            &["half-space".into()],
            EditScope::Node,
        )
        .unwrap_err()
}

fn assert_cone_cuboid_gap(error: &GraphError, label: &str) {
    assert_eq!(
        error.error_code(),
        ErrorCode::CoverageGap,
        "{label}: {error:?}"
    );
    let ErrorDetails::Operate {
        handlers, families, ..
    } = error.details()
    else {
        panic!("{label}: {error:?}");
    };
    assert!(handlers.is_empty(), "{label}: {handlers:?}");
    assert_eq!(
        families,
        Some(Box::new(["cone or frustum".into(), "cuboid".into()])),
        "{label}"
    );
}

#[test]
fn pointed_cone_cut_along_its_axis_refuses_with_coverage_gap_and_families() {
    for (name, radii) in [("apex up", APEX_UP), ("apex down", APEX_DOWN)] {
        for degrees in [0.0, 90.0, 180.0, 270.0] {
            let error = subtract_half_space(radii, THROUGH_AXIS, degrees);
            let label = format!("{name} at {degrees} degrees");
            assert_cone_cuboid_gap(&error, &label);
            assert!(
                error.message().contains("singular surface point"),
                "{label}: {}",
                error.message()
            );
        }
    }
}

#[test]
fn pointed_cone_cut_off_its_axis_keeps_the_generic_coverage_gap() {
    for (name, radii) in [("apex up", APEX_UP), ("apex down", APEX_DOWN)] {
        let error = subtract_half_space(radii, OFF_AXIS, 0.0);
        assert_cone_cuboid_gap(&error, name);
        assert_eq!(
            error.message(),
            GeometryError::CoverageGap {
                families: [
                    "generic analytic solid".into(),
                    "intersecting analytic solid".into(),
                ],
            }
            .to_string(),
            "{name}"
        );
    }
}

#[test]
fn pointed_cone_minus_floorkit_section_clipper_refuses_with_coverage_gap_and_families() {
    let clipper =
        BrepEnvelope::from_json(include_str!("fixtures/floorkit-section-clipper.json")).unwrap();
    for (name, radii) in [("apex up", APEX_UP), ("apex down", APEX_DOWN)] {
        let mut world = graph();
        world.create_primitive(cone(radii), named("cone")).unwrap();
        world
            .transform(
                "cone",
                Transform::Translate {
                    offset: [1.12, 0.2, 0.0],
                },
            )
            .unwrap();
        let host = (*world.placed_brep("cone").unwrap()).clone();
        let outcome = multi_tool_boolean(
            &host,
            std::slice::from_ref(&clipper),
            BooleanOp::Subtraction,
            "cut".into(),
        );
        assert!(
            matches!(outcome.result, Err(GeometryError::CoverageGap { .. })),
            "{name}: {:?}",
            outcome.result.err()
        );
        assert_eq!(
            outcome.families,
            Some(["cone or frustum".into(), "cuboid".into()]),
            "{name}"
        );
    }
}
