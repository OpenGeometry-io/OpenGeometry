use super::errors::dto;
use crate::brep::GeometryError;
use crate::math::MathError;
use crate::world_graph::{ErrorCode, ErrorDetails, GraphError};
use serde_json::json;

fn rendered(error: GraphError) -> String {
    serde_json::to_value(dto(error)).unwrap().to_string()
}

#[test]
fn dto_carries_the_code_and_details_of_every_geometry_error_variant() {
    let cases = [
        (
            GeometryError::Math(MathError::DivisionByZero),
            "InvalidGeometry",
            json!({"math": "DivisionByZero"}),
        ),
        (
            GeometryError::InvalidGeometry("bad".into()),
            "InvalidGeometry",
            json!({}),
        ),
        (
            GeometryError::InvalidTopology("bad".into()),
            "InvalidTopology",
            json!({}),
        ),
        (
            GeometryError::UnsupportedGeometry("bad".into()),
            "UnsupportedGeometry",
            json!({}),
        ),
        (
            GeometryError::AmbiguousProfileAlignment("bad".into()),
            "InvalidParameter",
            json!({}),
        ),
        (
            GeometryError::CoverageGap {
                families: ["plane".into(), "torus".into()],
            },
            "CoverageGap",
            json!({"families": ["plane", "torus"]}),
        ),
        (
            GeometryError::SingularParameterization,
            "InvalidGeometry",
            json!({}),
        ),
        (
            GeometryError::MissingReference {
                kind: "edge".into(),
                index: 3,
            },
            "InvalidTopology",
            json!({"kind": "edge", "index": 3}),
        ),
        (
            GeometryError::UnsupportedSchema { found: 9 },
            "InvalidTopology",
            json!({}),
        ),
        (
            GeometryError::UnresolvedIntersection("bad".into()),
            "UnresolvedIntersection",
            json!({}),
        ),
        (
            GeometryError::UnresolvedTessellation("bad".into()),
            "UnresolvedTessellation",
            json!({}),
        ),
        (
            GeometryError::LimitExceeded("bad".into()),
            "LimitExceeded",
            json!({}),
        ),
    ];
    for (error, code, details) in cases {
        let value = serde_json::to_value(dto(GraphError::Geometry(error.clone()))).unwrap();
        assert_eq!(
            value,
            json!({"code": code, "message": error.to_string(), "details": details}),
            "{error:?}"
        );
    }
}

#[test]
fn dto_renders_shared_shape_errors_byte_identical_to_the_untyped_form() {
    let error = GraphError::Code {
        code: ErrorCode::SharedShape,
        message: "shape has multiple instances".into(),
        details: ErrorDetails::SharedShape {
            shape_id: Some("shape-1".into()),
            instance_count: 2,
            sharing: vec!["host".into(), "host-copy".into()],
        },
    };
    assert_eq!(
        rendered(error),
        r#"{"code":"SharedShape","details":{"instanceCount":2,"shapeId":"shape-1","sharing":["host","host-copy"]},"message":"shape has multiple instances"}"#
    );
}

#[test]
fn dto_renders_coverage_gap_errors_byte_identical_to_the_untyped_form() {
    let error = GraphError::Geometry(GeometryError::CoverageGap {
        families: ["plane".into(), "torus".into()],
    });
    assert_eq!(
        rendered(error),
        r#"{"code":"CoverageGap","details":{"families":["plane","torus"]},"message":"CoverageGap { families: [\"plane\", \"torus\"] }"}"#
    );
}
