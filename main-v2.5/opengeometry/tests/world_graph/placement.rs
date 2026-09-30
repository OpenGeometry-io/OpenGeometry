use super::support::{box_primitive, near};
use opengeometry::brep::Similarity3;
use opengeometry::world_graph::{
    CopyOptions, CreateOptions, ErrorCode, Plane, Primitive, Transform,
};
use opengeometry_test_support::world_graph::{graph, named};

#[test]
fn place_round_trip_and_rotation_preserve_shape_revision() {
    let mut world = graph();
    world
        .create_primitive(box_primitive(), named("body"))
        .unwrap();
    let before = world.brep("body").unwrap().revision;
    world
        .transform(
            "body",
            Transform::Place {
                origin: None,
                x_direction: None,
                normal: None,
                scale: None,
            },
        )
        .unwrap();
    assert_eq!(world.node("body").unwrap().local, Similarity3::IDENTITY);
    let placement = world.placement("body").unwrap();
    world
        .transform(
            "body",
            Transform::Place {
                origin: Some(placement.origin),
                x_direction: Some(placement.x_direction),
                normal: Some(placement.normal),
                scale: Some(placement.scale),
            },
        )
        .unwrap();
    assert_eq!(world.node("body").unwrap().local, Similarity3::IDENTITY);
    world
        .transform(
            "body",
            Transform::Rotate {
                axis: [0.0, 1.0, 0.0],
                degrees: 90.0,
                pivot: Some([0.0; 3]),
            },
        )
        .unwrap();
    near(
        world
            .world_placement("body")
            .unwrap()
            .apply_point([1.0, 0.0, 0.0]),
        [0.0, 0.0, -1.0],
    );
    world
        .transform(
            "body",
            Transform::Rotate {
                axis: [0.1, 0.2, 0.3],
                degrees: 13.37,
                pivot: None,
            },
        )
        .unwrap();
    let before_round_trip = world.node("body").unwrap().local;
    let placement = world.placement("body").unwrap();
    world
        .transform(
            "body",
            Transform::Place {
                origin: Some(placement.origin),
                x_direction: Some(placement.x_direction),
                normal: Some(placement.normal),
                scale: Some(placement.scale),
            },
        )
        .unwrap();
    assert_eq!(world.node("body").unwrap().local, before_round_trip);
    assert_eq!(world.brep("body").unwrap().revision, before);
}

#[test]
fn primitive_plane_and_precision_guard_are_applied_at_commit() {
    let mut world = graph();
    world.create_system_assembly(named("parent")).unwrap();
    world
        .transform(
            "parent",
            Transform::Translate {
                offset: [10.0, 0.0, 0.0],
            },
        )
        .unwrap();
    let (id, _) = world
        .create_primitive(
            box_primitive(),
            CreateOptions {
                og_id: Some("planned".into()),
                parent: Some("parent".into()),
                plane: Some(Plane {
                    origin: Some([4.0, 5.0, 6.0]),
                    ..Plane::default()
                }),
            },
        )
        .unwrap();
    near(
        world.world_placement(&id).unwrap().frame.origin,
        [14.0, 5.0, 6.0],
    );
    let revision = world.revision();
    let error = world
        .create_primitive(
            Primitive::Cuboid {
                width: 2e6,
                height: 2.0,
                depth: 2.0,
            },
            CreateOptions::default(),
        )
        .unwrap_err();
    assert_eq!(error.error_code(), ErrorCode::LimitExceeded);
    assert_eq!(world.revision(), revision);
}

#[test]
fn one_hundred_thousand_incremental_rotations_keep_a_valid_frame() {
    let mut world = graph();
    world.create_system_assembly(named("pivot")).unwrap();
    for _ in 0..100_000 {
        world
            .transform(
                "pivot",
                Transform::Rotate {
                    axis: [0.1, 0.2, 0.3],
                    degrees: 0.001,
                    pivot: None,
                },
            )
            .unwrap();
    }
    world.node("pivot").unwrap().local.validate().unwrap();
}

#[test]
fn copies_under_another_parent_keep_their_world_placement() {
    let mut world = graph();
    world.create_system_assembly(named("a")).unwrap();
    world.create_system_assembly(named("b")).unwrap();
    world
        .transform(
            "a",
            Transform::Translate {
                offset: [7.0, 0.0, 0.0],
            },
        )
        .unwrap();
    world
        .transform(
            "b",
            Transform::Translate {
                offset: [-3.0, 0.0, 0.0],
            },
        )
        .unwrap();
    world
        .create_primitive(
            box_primitive(),
            CreateOptions {
                og_id: Some("body".into()),
                parent: Some("a".into()),
                plane: None,
            },
        )
        .unwrap();
    let options = CopyOptions {
        og_id: Some("copy".into()),
        parent: Some(Some("b".into())),
    };
    world.instance("body", options).unwrap();
    near(
        world.world_placement("body").unwrap().frame.origin,
        world.world_placement("copy").unwrap().frame.origin,
    );
    near(
        world
            .relative_placement("copy", "body")
            .unwrap()
            .frame
            .origin,
        [0.0; 3],
    );
    assert_eq!(world.parent("copy").unwrap(), Some("b"));
    world
        .duplicate(
            "body",
            CopyOptions {
                og_id: Some("root-copy".into()),
                parent: Some(None),
            },
        )
        .unwrap();
    assert_eq!(world.parent("root-copy").unwrap(), None);
    near(
        world.world_placement("root-copy").unwrap().frame.origin,
        world.world_placement("body").unwrap().frame.origin,
    );
}
