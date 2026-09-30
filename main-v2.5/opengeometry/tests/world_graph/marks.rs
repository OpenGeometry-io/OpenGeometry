use super::support::{box_primitive, copy_named};
use opengeometry::world_graph::{CreateOptions, EditScope, ErrorCode, Primitive, Transform};
use opengeometry_test_support::world_graph::{graph, named};
use std::sync::Arc;

#[test]
fn marks_restore_nodes_and_shape_pointers_without_reusing_counters() {
    let mut world = graph();
    world
        .create_primitive(box_primitive(), named("body"))
        .unwrap();
    let original = world.brep("body").unwrap();
    let original_handle = world.node("body").unwrap().handle;
    let original_generation = world.node("body").unwrap().generation;
    let mark = world.mark().unwrap();
    world.instance("body", copy_named("instance")).unwrap();
    world.make_unique("instance").unwrap().unwrap();
    let later = world.mark().unwrap();
    world.dispose("body").unwrap();
    assert_eq!(
        world
            .node_by_handle(original_handle, original_generation)
            .unwrap_err()
            .error_code(),
        ErrorCode::Disposed
    );
    let rollback = world.rollback(mark).unwrap();
    assert!(rollback.added.iter().any(|node| node.og_id == "body"));
    assert!(Arc::ptr_eq(&world.brep("body").unwrap(), &original));
    assert_eq!(
        world
            .node_by_handle(original_handle, original_generation)
            .unwrap()
            .og_id,
        "body"
    );
    assert_eq!(
        world.rollback(later).unwrap_err().error_code(),
        ErrorCode::InvalidMark
    );
    let (_, next) = world
        .create_primitive(box_primitive(), CreateOptions::default())
        .unwrap();
    assert!(next.added[0].handle > original_handle);
    assert_eq!(world.mark_stats().live_marks, 1);
    world.release(mark).unwrap();
    assert_eq!(world.mark_stats().live_marks, 0);
    assert_eq!(world.mark_stats().retained_revisions, 0);
}

#[test]
fn changes_since_compacts_updates_and_reports_subtree_motion() {
    let mut world = graph();
    world.create_system_assembly(named("parent")).unwrap();
    world
        .create_primitive(
            box_primitive(),
            CreateOptions {
                og_id: Some("child".into()),
                parent: Some("parent".into()),
                plane: None,
            },
        )
        .unwrap();
    let revision = world.revision();
    world
        .transform(
            "parent",
            Transform::Translate {
                offset: [1.0, 0.0, 0.0],
            },
        )
        .unwrap();
    world
        .transform(
            "parent",
            Transform::Translate {
                offset: [2.0, 0.0, 0.0],
            },
        )
        .unwrap();
    let changes = world.changes_since(revision).unwrap();
    assert_eq!(changes.changed.len(), 2);
    assert!(changes.changed.iter().any(|node| node.og_id == "child"
        && node.shape_revision == Some(world.brep("child").unwrap().revision)));
    let bounds = world.bounds("parent").unwrap().unwrap();
    assert!(bounds[0] <= 2.0 && bounds[3] >= 4.0);
    assert!(bounds[1] <= 0.0 && bounds[4] >= 2.0);
    assert!(bounds[2] <= -1.0 && bounds[5] >= 1.0);
}

#[test]
fn nested_marks_and_compacted_changes_preserve_live_history() {
    let mut world = graph();
    let baseline = world.revision();
    let outer = world.mark().unwrap();
    world
        .create_primitive(box_primitive(), named("body"))
        .unwrap();
    let inner = world.mark().unwrap();
    world.instance("body", copy_named("temporary")).unwrap();
    assert!(world.mark_stats().retained_revisions >= 1);
    world.rollback(inner).unwrap();
    assert_eq!(world.instance_count("body").unwrap(), 1);
    world.release(inner).unwrap();
    let revision_before_outer_rollback = world.revision();
    world.rollback(outer).unwrap();
    assert!(world.node("body").is_err());
    assert!(world.revision() > revision_before_outer_rollback);
    let compact = world.changes_since(baseline).unwrap();
    assert!(compact.added.is_empty());
    assert!(compact.changed.is_empty());
    assert!(compact.removed.is_empty());
    world.release(outer).unwrap();
}

#[test]
fn shared_rebuild_and_rollback_keep_shape_revision_high_water() {
    let mut world = graph();
    world
        .create_primitive(box_primitive(), named("body"))
        .unwrap();
    world.instance("body", copy_named("instance")).unwrap();
    let denied = world
        .rebuild_primitive("body", box_primitive(), EditScope::Node)
        .unwrap_err();
    assert_eq!(denied.error_code(), ErrorCode::SharedShape);
    let original = world.brep("body").unwrap();
    let mark = world.mark().unwrap();
    let changed = world
        .rebuild_primitive(
            "body",
            Primitive::Cuboid {
                width: 3.0,
                height: 2.0,
                depth: 2.0,
            },
            EditScope::AllInstances,
        )
        .unwrap();
    assert_eq!(changed.changed.len(), 2);
    assert_eq!(world.brep("body").unwrap().revision, 1);
    assert!(changed
        .changed
        .iter()
        .all(|node| node.shape_revision == Some(world.brep("body").unwrap().revision)));
    assert!(Arc::ptr_eq(
        &world.brep("body").unwrap(),
        &world.brep("instance").unwrap()
    ));
    world.rollback(mark).unwrap();
    assert!(Arc::ptr_eq(&world.brep("body").unwrap(), &original));
    assert_eq!(world.brep("body").unwrap().revision, 0);
    world
        .rebuild_primitive(
            "body",
            Primitive::Cuboid {
                width: 4.0,
                height: 2.0,
                depth: 2.0,
            },
            EditScope::AllInstances,
        )
        .unwrap();
    assert_eq!(world.brep("body").unwrap().revision, 2);
    assert_eq!(
        world
            .rebuild_primitive(
                "body",
                Primitive::Rectangle {
                    width: 2.0,
                    breadth: 2.0
                },
                EditScope::AllInstances
            )
            .unwrap_err()
            .error_code(),
        ErrorCode::BodyTypeMismatch
    );
    world.release(mark).unwrap();
}

#[test]
fn subtree_dispose_and_rollback_preserve_counters_and_handles() {
    let mut world = graph();
    world.create_system_assembly(named("parent")).unwrap();
    assert!(world.bounds("parent").unwrap().is_none());
    let mark = world.mark().unwrap();
    let (generated, _) = world
        .create_primitive(
            box_primitive(),
            CreateOptions {
                og_id: None,
                parent: Some("parent".into()),
                plane: None,
            },
        )
        .unwrap();
    let handle = world.node(&generated).unwrap().handle;
    let generation = world.node(&generated).unwrap().generation;
    world.dispose("parent").unwrap();
    assert_eq!(
        world
            .node_by_handle(handle, generation)
            .unwrap_err()
            .error_code(),
        ErrorCode::Disposed
    );
    world.rollback(mark).unwrap();
    assert!(world.node("parent").is_ok());
    assert!(world.node(&generated).is_err());
    let (next, _) = world
        .create_primitive(box_primitive(), CreateOptions::default())
        .unwrap();
    assert_ne!(generated, next);
    world.release(mark).unwrap();
}
