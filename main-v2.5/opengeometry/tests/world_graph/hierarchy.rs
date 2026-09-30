use super::support::{box_primitive, copy_named, near};
use opengeometry::world_graph::{
    CreateOptions, EditScope, ErrorCode, ModifyingOperation, Primitive, Transform,
};
use opengeometry_test_support::world_graph::{graph, named};
use std::sync::Arc;

#[test]
fn instances_share_one_immutable_shape_and_duplicates_do_not() {
    let mut world = graph();
    world
        .create_primitive(box_primitive(), named("body"))
        .unwrap();
    world.instance("body", copy_named("instance")).unwrap();
    assert_eq!(world.shape_count(), 1);
    assert_eq!(world.instance_count("body").unwrap(), 2);
    assert_eq!(
        world
            .ensure_editable("body", EditScope::Node)
            .unwrap_err()
            .error_code(),
        ErrorCode::SharedShape
    );
    world
        .ensure_editable("body", EditScope::AllInstances)
        .unwrap();
    assert!(Arc::ptr_eq(
        &world.brep("body").unwrap(),
        &world.brep("instance").unwrap()
    ));
    world.duplicate("body", copy_named("duplicate")).unwrap();
    assert_eq!(world.shape_count(), 2);
    assert!(!Arc::ptr_eq(
        &world.brep("body").unwrap(),
        &world.brep("duplicate").unwrap()
    ));
    let copied_id = world.brep("duplicate").unwrap().id.clone();
    assert_ne!(copied_id, world.brep("body").unwrap().id);
    assert!(world
        .brep("duplicate")
        .unwrap()
        .topology
        .faces
        .iter()
        .all(|face| face
            .provenance
            .sources
            .iter()
            .all(|source| source.entity == copied_id && source.body == copied_id)));
    world.make_unique("instance").unwrap().unwrap();
    assert_eq!(world.instance_count("body").unwrap(), 1);
    assert_eq!(world.shape_count(), 3);
    assert!(world.make_unique("body").unwrap().is_none());
}

#[test]
fn copies_of_an_operated_body_carry_no_report() {
    let mut world = graph();
    world
        .create_primitive(box_primitive(), named("body"))
        .unwrap();
    world
        .create_primitive(box_primitive(), named("tool"))
        .unwrap();
    world
        .transform(
            "tool",
            Transform::Translate {
                offset: [1.0, 0.0, 0.0],
            },
        )
        .unwrap();
    world
        .operate(
            "body",
            ModifyingOperation::Subtract,
            &["tool".into()],
            EditScope::Node,
        )
        .unwrap();
    world.duplicate("body", copy_named("duplicate")).unwrap();
    world.instance("body", copy_named("instance")).unwrap();
    world.make_unique("instance").unwrap().unwrap();
    assert!(world.report("body").unwrap().is_some());
    assert!(world.report("duplicate").unwrap().is_none());
    assert!(world.report("instance").unwrap().is_none());
}

#[test]
fn parent_motion_and_reparenting_obey_local_and_world_rules() {
    let mut world = graph();
    world.create_system_assembly(named("a")).unwrap();
    world.create_system_assembly(named("b")).unwrap();
    world
        .create_primitive(
            box_primitive(),
            CreateOptions {
                og_id: Some("child".into()),
                parent: Some("a".into()),
                plane: None,
            },
        )
        .unwrap();
    world
        .transform(
            "a",
            Transform::Translate {
                offset: [10.0, 0.0, 0.0],
            },
        )
        .unwrap();
    near(
        world.world_placement("child").unwrap().frame.origin,
        [10.0, 0.0, 0.0],
    );
    world.add_child("b", &["child".into()], false).unwrap();
    near(
        world.world_placement("child").unwrap().frame.origin,
        [0.0, 0.0, 0.0],
    );
    world.add_child("a", &["child".into()], true).unwrap();
    near(
        world.world_placement("child").unwrap().frame.origin,
        [0.0, 0.0, 0.0],
    );
    near(world.placement("child").unwrap().origin, [-10.0, 0.0, 0.0]);
    world.remove_child("a", "child", true).unwrap();
    near(
        world.world_placement("child").unwrap().frame.origin,
        [0.0, 0.0, 0.0],
    );
    assert_eq!(world.parent("child").unwrap(), None);
}

#[test]
fn i6_failed_calls_do_not_consume_ids_or_change_the_graph() {
    let mut world = graph();
    let reserved = world.reserve(1, &[]).unwrap();
    assert_eq!(reserved.shape_ids, ["shape-0"]);
    world.create_system_assembly(named("a")).unwrap();
    world
        .create_system_assembly(CreateOptions {
            og_id: Some("b".into()),
            parent: Some("a".into()),
            plane: None,
        })
        .unwrap();
    let revision = world.revision();
    let cycle = world.add_child("b", &["a".into()], false).unwrap_err();
    assert_eq!(cycle.error_code(), ErrorCode::CycleDetected);
    assert_eq!(world.revision(), revision);
    assert_eq!(world.parent("a").unwrap(), None);
    assert_eq!(
        world
            .create_system_assembly(named("a"))
            .unwrap_err()
            .error_code(),
        ErrorCode::DuplicateNode
    );
    assert_eq!(
        world
            .transform(
                "a",
                Transform::Scale {
                    factor: -1.0,
                    pivot: None
                }
            )
            .unwrap_err()
            .error_code(),
        ErrorCode::InvalidTransform
    );
    assert_eq!(
        world
            .create_primitive(
                Primitive::Cylinder {
                    radius: 0.0,
                    height: 2.0
                },
                CreateOptions::default()
            )
            .unwrap_err()
            .error_code(),
        ErrorCode::InvalidParameter
    );
    assert_eq!(
        world
            .create_primitive(
                Primitive::Polyline {
                    points: vec![[0.0; 3], [0.0; 3]],
                    closed: false
                },
                CreateOptions::default(),
            )
            .unwrap_err()
            .error_code(),
        ErrorCode::InvalidParameter
    );
    let (generated, _) = world
        .create_primitive(box_primitive(), CreateOptions::default())
        .unwrap();
    assert_eq!(generated, "node-0");
    assert_eq!(world.brep(&generated).unwrap().id, reserved.shape_ids[0]);
    let body = world.node(&generated).unwrap();
    let handle = body.handle;
    let generation = body.generation;
    world.dispose(&generated).unwrap();
    assert_eq!(
        world
            .node_by_handle(handle, generation)
            .unwrap_err()
            .error_code(),
        ErrorCode::Disposed
    );
    let (next, _) = world
        .create_primitive(box_primitive(), CreateOptions::default())
        .unwrap();
    assert_eq!(next, "node-1");
}
