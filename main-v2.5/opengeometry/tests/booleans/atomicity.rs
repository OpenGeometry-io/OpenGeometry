use super::support::cuboid;
use opengeometry::brep::FaceRole;
use opengeometry::world_graph::{
    CopyOptions, EditScope, ErrorCode, ErrorDetails, GraphError, ModifyingOperation, Primitive,
    Transform,
};
use opengeometry_test_support::volume;
use opengeometry_test_support::world_graph::{graph, named};

#[test]
fn box_subtraction_is_analytic_atomic_and_reports_handler() {
    let mut world = graph();
    world.create_primitive(cuboid(2.0), named("host")).unwrap();
    world.create_primitive(cuboid(2.0), named("tool")).unwrap();
    world
        .transform(
            "tool",
            Transform::Translate {
                offset: [1.0, 0.0, 0.0],
            },
        )
        .unwrap();
    let tool_before = world.brep("tool").unwrap().to_json().unwrap();
    world
        .operate(
            "host",
            ModifyingOperation::Subtract,
            &["tool".into()],
            EditScope::Node,
        )
        .unwrap();
    let host = world.brep("host").unwrap();
    let measured = volume::estimate(&host, 0.01);
    assert!((measured.value - 4.0).abs() <= measured.error_bound);
    assert!(host
        .topology
        .faces
        .iter()
        .any(|face| face.provenance.role == FaceRole::Cut));
    assert_eq!(
        world.report("host").unwrap().unwrap()["handlers"],
        serde_json::json!(["boolean_boxes"])
    );
    assert_eq!(world.brep("tool").unwrap().to_json().unwrap(), tool_before);
    assert_eq!(host.revision, 1);
}

#[test]
fn shared_target_requires_explicit_all_instances_and_tool_instance_is_allowed() {
    let mut world = graph();
    world.create_primitive(cuboid(2.0), named("host")).unwrap();
    world.create_primitive(cuboid(2.0), named("tool")).unwrap();
    world
        .transform(
            "tool",
            Transform::Translate {
                offset: [1.0, 0.0, 0.0],
            },
        )
        .unwrap();
    world
        .instance(
            "host",
            CopyOptions {
                og_id: Some("host-copy".into()),
                ..CopyOptions::default()
            },
        )
        .unwrap();
    world
        .instance(
            "tool",
            CopyOptions {
                og_id: Some("tool-copy".into()),
                ..CopyOptions::default()
            },
        )
        .unwrap();
    let error = world
        .operate(
            "host",
            ModifyingOperation::Subtract,
            &["tool-copy".into()],
            EditScope::Node,
        )
        .unwrap_err();
    assert_eq!(error.error_code(), ErrorCode::SharedShape);
    if let GraphError::Code {
        details:
            ErrorDetails::SharedShape {
                instance_count,
                sharing,
                ..
            },
        ..
    } = error
    {
        assert_eq!(instance_count, 2);
        assert_eq!(sharing.len(), 2);
    } else {
        panic!("expected SharedShape details");
    }
    world
        .operate(
            "host",
            ModifyingOperation::Subtract,
            &["tool-copy".into()],
            EditScope::AllInstances,
        )
        .unwrap();
    assert_eq!(world.brep("host").unwrap().revision, 1);
    assert_eq!(world.brep("host-copy").unwrap().revision, 1);
    assert_eq!(
        world
            .operate(
                "host",
                ModifyingOperation::Subtract,
                &["host-copy".into()],
                EditScope::AllInstances
            )
            .unwrap_err()
            .error_code(),
        ErrorCode::ToolSharesTargetShape
    );
}

#[test]
fn empty_intersection_and_tool_limit_leave_graph_unchanged() {
    let mut world = graph();
    world.create_primitive(cuboid(2.0), named("host")).unwrap();
    world.create_primitive(cuboid(2.0), named("tool")).unwrap();
    world
        .transform(
            "tool",
            Transform::Translate {
                offset: [10.0, 0.0, 0.0],
            },
        )
        .unwrap();
    let revision = world.revision();
    let before = world.brep("host").unwrap().to_json().unwrap();
    assert_eq!(
        world
            .operate(
                "host",
                ModifyingOperation::Intersect,
                &["tool".into()],
                EditScope::Node
            )
            .unwrap_err()
            .error_code(),
        ErrorCode::EmptyResult
    );
    assert_eq!(
        world
            .operate("host", ModifyingOperation::Union, &[], EditScope::Node)
            .unwrap_err()
            .error_code(),
        ErrorCode::LimitExceeded
    );
    assert_eq!(world.revision(), revision);
    assert_eq!(world.brep("host").unwrap().to_json().unwrap(), before);
}

#[test]
fn empty_subtraction_maps_to_empty_result_without_a_commit() {
    let mut world = graph();
    world
        .create_primitive(
            Primitive::Cuboid {
                width: 1.0,
                height: 1.0,
                depth: 1.0,
            },
            named("small"),
        )
        .unwrap();
    world
        .create_primitive(
            Primitive::Cuboid {
                width: 2.0,
                height: 2.0,
                depth: 2.0,
            },
            named("large"),
        )
        .unwrap();
    let before = world.brep("small").unwrap().to_json().unwrap();
    let revision = world.revision();
    assert_eq!(
        world
            .operate(
                "small",
                ModifyingOperation::Subtract,
                &["large".into()],
                EditScope::Node
            )
            .unwrap_err()
            .error_code(),
        ErrorCode::EmptyResult,
    );
    assert_eq!(world.brep("small").unwrap().to_json().unwrap(), before);
    assert_eq!(world.revision(), revision);
}

#[test]
fn make_unique_then_operate_uses_new_shape_provenance() {
    let mut world = graph();
    world.create_primitive(cuboid(2.0), named("host")).unwrap();
    world.create_primitive(cuboid(2.0), named("tool")).unwrap();
    world
        .transform(
            "tool",
            Transform::Translate {
                offset: [1.0, 0.0, 0.0],
            },
        )
        .unwrap();
    world
        .instance(
            "host",
            CopyOptions {
                og_id: Some("instance".into()),
                ..CopyOptions::default()
            },
        )
        .unwrap();
    let old_shape = world.node("host").unwrap().shape.clone().unwrap();
    world.make_unique("host").unwrap();
    let new_shape = world.node("host").unwrap().shape.clone().unwrap();
    assert_ne!(old_shape, new_shape);
    world
        .operate(
            "host",
            ModifyingOperation::Subtract,
            &["tool".into()],
            EditScope::Node,
        )
        .unwrap();
    let body = world.brep("host").unwrap();
    assert!(body
        .topology
        .faces
        .iter()
        .flat_map(|face| &face.provenance.sources)
        .any(|source| source.body == new_shape));
    assert_eq!(world.brep("instance").unwrap().revision, 0);
}
