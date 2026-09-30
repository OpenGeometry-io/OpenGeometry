#![cfg(not(target_arch = "wasm32"))]
use opengeometry::brep::{Accuracy, GeometryError};
use opengeometry::math::MathError;
use opengeometry::operations::OperationError;
use opengeometry::world_graph::{
    CreatingOperation, EditScope, ErrorCode, ErrorContext, ErrorDetails, GraphError,
    ModifyingOperation, Primitive, StepOptions, Transform, WorldGraph,
};
use opengeometry_test_support::world_graph::{graph, named};

const CONTEXTS: [(ErrorContext, ErrorCode); 5] = [
    (ErrorContext::Create, ErrorCode::InvalidParameter),
    (ErrorContext::Rebuild, ErrorCode::InvalidParameter),
    (ErrorContext::Transform, ErrorCode::InvalidTransform),
    (ErrorContext::Operate, ErrorCode::InvalidGeometry),
    (ErrorContext::Export, ErrorCode::InvalidGeometry),
];

fn coded(code: ErrorCode, message: &str) -> GraphError {
    GraphError::Code {
        code,
        message: message.into(),
        details: ErrorDetails::None,
    }
}

#[test]
fn singular_parameterization_takes_the_code_of_its_context() {
    for (context, expected) in CONTEXTS {
        let error = GraphError::from(GeometryError::SingularParameterization).in_context(context);
        assert_eq!(error.error_code(), expected, "{context:?}");
        let gap = GraphError::from(GeometryError::CoverageGap {
            families: ["plane".into(), "torus".into()],
        });
        assert_eq!(gap.clone().in_context(context), gap, "{context:?}");
    }
}

#[test]
fn operation_errors_keep_their_code_and_message_as_graph_errors() {
    let cases = [
        (
            OperationError::InvalidParameter("hole is not coplanar".into()),
            coded(ErrorCode::InvalidParameter, "hole is not coplanar"),
        ),
        (
            OperationError::SweepSelfIntersection("sweep path reverses".into()),
            coded(ErrorCode::SweepSelfIntersection, "sweep path reverses"),
        ),
        (
            OperationError::InvalidTopology("seam use is missing".into()),
            coded(ErrorCode::InvalidTopology, "seam use is missing"),
        ),
        (
            OperationError::Geometry(GeometryError::SingularParameterization),
            GraphError::Geometry(GeometryError::SingularParameterization),
        ),
    ];
    for (operation, expected) in cases {
        assert_eq!(GraphError::from(operation), expected);
    }
}

#[test]
fn extrusion_below_resolution_reports_invalid_parameter_through_the_graph() {
    let mut world = graph();
    world
        .create_primitive(
            Primitive::Rectangle {
                width: 2.0,
                breadth: 3.0,
            },
            named("profile"),
        )
        .unwrap();
    let operation = CreatingOperation::Extrude {
        profile: "profile".into(),
        holes: Vec::new(),
        distance: 0.0,
    };
    assert_eq!(
        world
            .create_operation(operation, named("solid"))
            .unwrap_err(),
        coded(
            ErrorCode::InvalidParameter,
            "extrusion distance is below geometric resolution"
        ),
    );
}

fn geometry_errors() -> [(GeometryError, ErrorCode); 12] {
    [
        (
            GeometryError::Math(MathError::DivisionByZero),
            ErrorCode::InvalidGeometry,
        ),
        (
            GeometryError::InvalidGeometry("bad".into()),
            ErrorCode::InvalidGeometry,
        ),
        (
            GeometryError::InvalidTopology("bad".into()),
            ErrorCode::InvalidTopology,
        ),
        (
            GeometryError::UnsupportedGeometry("bad".into()),
            ErrorCode::UnsupportedGeometry,
        ),
        (
            GeometryError::AmbiguousProfileAlignment("bad".into()),
            ErrorCode::InvalidParameter,
        ),
        (
            GeometryError::CoverageGap {
                families: ["plane".into(), "torus".into()],
            },
            ErrorCode::CoverageGap,
        ),
        (
            GeometryError::SingularParameterization,
            ErrorCode::InvalidGeometry,
        ),
        (
            GeometryError::MissingReference {
                kind: "edge".into(),
                index: 3,
            },
            ErrorCode::InvalidTopology,
        ),
        (
            GeometryError::UnsupportedSchema { found: 9 },
            ErrorCode::InvalidTopology,
        ),
        (
            GeometryError::UnresolvedIntersection("bad".into()),
            ErrorCode::UnresolvedIntersection,
        ),
        (
            GeometryError::UnresolvedTessellation("bad".into()),
            ErrorCode::UnresolvedTessellation,
        ),
        (
            GeometryError::LimitExceeded("bad".into()),
            ErrorCode::LimitExceeded,
        ),
    ]
}

#[test]
fn every_geometry_error_variant_keeps_its_code_in_every_context() {
    for (error, code) in geometry_errors() {
        for (context, singular_code) in CONTEXTS {
            let actual = GraphError::from(error.clone()).in_context(context);
            let expected = match (&error, context) {
                (
                    GeometryError::SingularParameterization,
                    ErrorContext::Create | ErrorContext::Rebuild | ErrorContext::Transform,
                ) => coded(singular_code, "SingularParameterization"),
                _ => GraphError::Geometry(error.clone()),
            };
            assert_eq!(actual, expected, "{error:?} {context:?}");
            if expected == GraphError::Geometry(error.clone()) {
                assert_eq!(actual.error_code(), code, "{error:?} {context:?}");
            }
        }
    }
}

fn unit_cuboid(world: &mut WorldGraph, og_id: &str, size: f64) {
    world
        .create_primitive(
            Primitive::Cuboid {
                width: size,
                height: size,
                depth: size,
            },
            named(og_id),
        )
        .unwrap();
}

fn translate(world: &mut WorldGraph, og_id: &str, offset: [f64; 3]) {
    world
        .transform(og_id, Transform::Translate { offset })
        .unwrap();
}

#[test]
fn multi_tool_operate_failure_carries_handlers_tool_index_and_og_ids() {
    let mut world = graph();
    unit_cuboid(&mut world, "target", 2.0);
    unit_cuboid(&mut world, "tool-1", 2.0);
    translate(&mut world, "tool-1", [1.0, 0.0, 0.0]);
    unit_cuboid(&mut world, "tool-2", 1.0);
    translate(&mut world, "tool-2", [1.5, 0.5, 0.0]);
    world
        .transform(
            "tool-2",
            Transform::Rotate {
                axis: [0.0, 1.0, 0.0],
                degrees: 30.0,
                pivot: Some([1.5, 0.5, 0.0]),
            },
        )
        .unwrap();
    let mut step_one = graph();
    unit_cuboid(&mut step_one, "target", 2.0);
    unit_cuboid(&mut step_one, "tool-1", 2.0);
    translate(&mut step_one, "tool-1", [1.0, 0.0, 0.0]);
    step_one
        .operate(
            "target",
            ModifyingOperation::Union,
            &["tool-1".into()],
            EditScope::Node,
        )
        .unwrap();
    let step_one_handlers: Vec<String> =
        serde_json::from_value(step_one.report("target").unwrap().unwrap()["handlers"].clone())
            .unwrap();
    let revision = world.revision();
    let brep_before = world.brep("target").unwrap().to_json().unwrap();
    let error = world
        .operate(
            "target",
            ModifyingOperation::Union,
            &["tool-1".into(), "tool-2".into()],
            EditScope::Node,
        )
        .unwrap_err();
    assert_eq!(error.error_code(), ErrorCode::CoverageGap);
    assert_eq!(
        error.details(),
        ErrorDetails::Operate {
            handlers: step_one_handlers,
            tool_index: 1,
            og_ids: vec!["target".into(), "tool-1".into(), "tool-2".into()],
        }
    );
    assert_eq!(world.revision(), revision);
    assert_eq!(
        world.brep("target").unwrap().to_json().unwrap(),
        brep_before
    );
}

#[test]
fn two_body_export_failing_on_the_second_body_carries_its_og_id() {
    let mut world = WorldGraph::new(Accuracy {
        geometric: 1e-8,
        intersection: 1e-9,
        tessellation: 0.01,
        exchange: 1.001e-8,
    })
    .unwrap();
    unit_cuboid(&mut world, "near", 1.0);
    unit_cuboid(&mut world, "far", 1.0);
    translate(&mut world, "far", [10_000.0, 0.0, 0.0]);
    let options = StepOptions {
        unit: "metre".into(),
        up_axis: "Y".into(),
        ..StepOptions::default()
    };
    let error = world
        .export_step(&["near".into(), "far".into()], &options)
        .unwrap_err();
    assert_eq!(
        error,
        GraphError::Code {
            code: ErrorCode::LimitExceeded,
            message: GeometryError::LimitExceeded(
                "analytic exchange exchange budget is below geometric tolerance or coordinate precision"
                    .into()
            )
            .to_string(),
            details: ErrorDetails::Export {
                og_id: "far".into()
            },
        }
    );
}
