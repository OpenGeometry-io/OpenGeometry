#![cfg(not(target_arch = "wasm32"))]
use opengeometry::brep::GeometryError;
use opengeometry::operations::OperationError;
use opengeometry::world_graph::{
    CreatingOperation, ErrorCode, ErrorContext, GraphError, Primitive,
};
use opengeometry_test_support::world_graph::{graph, named};
use serde_json::Value;

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
        details: Value::Null,
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
