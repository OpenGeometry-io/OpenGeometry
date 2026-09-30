use super::errors::dto;
use super::OGWorldGraph;
use crate::brep::{Accuracy, GeometryError};
use crate::math::MathError;
use crate::tessellation::display::{bucket_floor, static_bucket};
use crate::world_graph::{
    ChangeSet, CopyOptions, CreateOptions, ErrorCode, ErrorDetails, GraphError, Plane, Primitive,
    StepOptions, Transform, WorldGraph,
};
use serde_json::{json, Value};

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

fn assert_camel_case_keys(value: &Value) {
    match value {
        Value::Object(map) => {
            for (key, inner) in map {
                let mut bytes = key.bytes();
                let first = bytes.next().unwrap_or(b'_');
                assert!(
                    first.is_ascii_lowercase() && bytes.all(|byte| byte.is_ascii_alphanumeric()),
                    "{key} in {value}"
                );
                assert_camel_case_keys(inner);
            }
        }
        Value::Array(items) => items.iter().for_each(assert_camel_case_keys),
        _ => {}
    }
}

fn binding_accuracy() -> Accuracy {
    Accuracy {
        geometric: 1e-8,
        intersection: 1e-9,
        tessellation: 0.01,
        exchange: 1e-6,
    }
}

fn unit_cube(graph: &mut WorldGraph) -> (String, ChangeSet) {
    graph
        .create_primitive(
            Primitive::Cuboid {
                width: 1.0,
                height: 1.0,
                depth: 1.0,
            },
            CreateOptions {
                og_id: Some("cube".into()),
                ..CreateOptions::default()
            },
        )
        .unwrap()
}

#[test]
fn bindings_payloads_are_camel_case() {
    let accuracy = binding_accuracy();
    let mut graph = WorldGraph::new(accuracy).unwrap();
    let created = unit_cube(&mut graph);
    let plane = Plane {
        origin: Some([0.0; 3]),
        normal: Some([0.0, 0.0, 1.0]),
        x_direction: Some([1.0, 0.0, 0.0]),
    };
    let error = GraphError::Code {
        code: ErrorCode::InvalidOperand,
        message: "bad tool".into(),
        details: ErrorDetails::Operate {
            handlers: vec!["box".into()],
            tool_index: 0,
            og_ids: vec!["cube".into()],
        },
    };
    let payloads = [
        serde_json::to_value(&created).unwrap(),
        serde_json::to_value(graph.placement("cube").unwrap()).unwrap(),
        serde_json::to_value(Transform::Place {
            origin: Some([0.0; 3]),
            x_direction: Some([1.0, 0.0, 0.0]),
            normal: Some([0.0, 0.0, 1.0]),
            scale: Some(1.0),
        })
        .unwrap(),
        serde_json::to_value(plane).unwrap(),
        serde_json::to_value(CreateOptions {
            og_id: Some("cube".into()),
            parent: Some("root".into()),
            plane: Some(plane),
            body_type: None,
        })
        .unwrap(),
        serde_json::to_value(CopyOptions {
            og_id: Some("copy".into()),
            parent: Some(Some("root".into())),
        })
        .unwrap(),
        serde_json::to_value(StepOptions::default()).unwrap(),
        serde_json::to_value(accuracy).unwrap(),
        serde_json::to_value(dto(error)).unwrap(),
    ];
    for payload in &payloads {
        assert_camel_case_keys(payload);
    }
}

#[test]
fn place_accepts_camel_case_x_direction_and_rejects_snake_case() {
    assert!(serde_json::from_str::<Transform>(r#"{"kind":"Place","xDirection":[1,0,0]}"#).is_ok());
    assert!(
        serde_json::from_str::<Transform>(r#"{"kind":"Place","x_direction":[1,0,0]}"#).is_err()
    );
    assert!(serde_json::from_str::<CreateOptions>(r#"{"ogId":"a"}"#).is_ok());
    assert!(serde_json::from_str::<CopyOptions>(r#"{"ogId":"a"}"#).is_ok());
    assert!(serde_json::from_str::<CreateOptions>(r#"{"og_id":"a"}"#).is_err());
    assert!(serde_json::from_str::<CopyOptions>(r#"{"og_id":"a"}"#).is_err());
    assert!(serde_json::from_str::<Plane>(r#"{"xDirection":[1,0,0]}"#).is_ok());
    assert!(serde_json::from_str::<Plane>(r#"{"x_direction":[1,0,0]}"#).is_err());
}

#[test]
fn display_buckets_reports_the_kernel_floor_and_static_bucket() {
    let mut binding = OGWorldGraph::new(
        r#"{"geometric":1e-8,"intersection":1e-9,"tessellation":0.01,"exchange":1e-6}"#,
    )
    .unwrap();
    binding
        .create_primitive(
            r#"{"kind":"Cuboid","width":1,"height":1,"depth":1}"#,
            r#"{"ogId":"cube"}"#,
        )
        .unwrap();
    let buckets: Value = serde_json::from_str(&binding.display_buckets("cube").unwrap()).unwrap();
    let keys: Vec<&str> = buckets
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, ["floor", "static"]);
    let mut graph = WorldGraph::new(binding_accuracy()).unwrap();
    unit_cube(&mut graph);
    let brep = graph.brep("cube").unwrap();
    let floor = bucket_floor(&brep).unwrap();
    let bucket = static_bucket(&brep).unwrap();
    assert_eq!(floor.to_bits(), 2.0_f64.powi(-21).to_bits());
    assert_eq!(bucket.to_bits(), 2.0_f64.powi(-9).to_bits());
    assert_eq!(
        buckets["floor"].as_f64().unwrap().to_bits(),
        floor.to_bits()
    );
    assert_eq!(
        buckets["static"].as_f64().unwrap().to_bits(),
        bucket.to_bits()
    );
}
