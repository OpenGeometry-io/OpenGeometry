use super::support::{bits, box_primitive, near, place};
use opengeometry::brep::Similarity3;
use opengeometry::world_graph::{
    CopyOptions, CreateOptions, ErrorCode, Placement, Plane, Primitive, Transform,
};
use opengeometry_test_support::world_graph::{graph, named};

#[test]
fn place_round_trip_runs_the_formula_and_preserves_shape_revision() {
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
    assert_eq!(
        bits(&world.node("body").unwrap().local),
        bits(&Similarity3::IDENTITY)
    );
    let placement = world.placement("body").unwrap();
    world.transform("body", place(placement)).unwrap();
    assert_eq!(
        bits(&world.node("body").unwrap().local),
        bits(&Similarity3::IDENTITY)
    );
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
    let placement = world.placement("body").unwrap();
    world.transform("body", place(placement)).unwrap();
    let after = bits(&world.node("body").unwrap().local);
    assert_eq!(bits(&world.placement("body").unwrap()), bits(&placement));
    world.transform("body", place(placement)).unwrap();
    assert_eq!(bits(&world.node("body").unwrap().local), after);
    let mut fresh = graph();
    fresh
        .create_primitive(box_primitive(), named("body"))
        .unwrap();
    fresh.transform("body", place(placement)).unwrap();
    assert_eq!(bits(&fresh.node("body").unwrap().local), after);
    assert_eq!(world.brep("body").unwrap().revision, before);
}

#[test]
fn quarter_turn_rotations_are_bitwise_exact() {
    let exact = [0.0_f64, 1.0, -1.0].map(f64::to_bits);
    for axis in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]] {
        for degrees in [90.0, 180.0, 270.0, -90.0, 1440.0] {
            let mut world = graph();
            world.create_system_assembly(named("pivot")).unwrap();
            world
                .transform(
                    "pivot",
                    Transform::Rotate {
                        axis,
                        degrees,
                        pivot: None,
                    },
                )
                .unwrap();
            let frame = world.node("pivot").unwrap().local.frame;
            for entry in [frame.x, frame.y, frame.z].concat() {
                assert!(
                    exact.contains(&entry.to_bits()),
                    "{degrees} about {axis:?} gave {entry:e}"
                );
            }
        }
    }
}

#[test]
fn place_returns_its_input_and_replays_bitwise_in_a_fresh_graph() {
    let input = Placement {
        origin: [1.0, 2.0, 3.0],
        x_direction: [3.0, 4.0, 0.0],
        normal: [0.0, 0.0, 1.0],
        scale: 2.0,
    };
    let mut a = graph();
    a.create_primitive(box_primitive(), named("body")).unwrap();
    a.transform("body", place(input)).unwrap();
    assert_eq!(bits(&a.placement("body").unwrap()), bits(&input));
    let mut b = graph();
    b.create_primitive(box_primitive(), named("body")).unwrap();
    b.transform("body", place(input)).unwrap();
    let local = bits(&a.node("body").unwrap().local);
    assert_eq!(bits(&b.node("body").unwrap().local), local);
    b.transform("body", place(a.placement("body").unwrap()))
        .unwrap();
    assert_eq!(bits(&b.node("body").unwrap().local), local);
}

#[test]
fn place_applied_twice_is_bitwise_stable() {
    let input = Placement {
        origin: [1.0, 2.0, 3.0],
        x_direction: [3.0, 4.0, 0.0],
        normal: [0.0, 0.0, 1.0],
        scale: 2.0,
    };
    let mut world = graph();
    world
        .create_primitive(box_primitive(), named("body"))
        .unwrap();
    world.transform("body", place(input)).unwrap();
    let first = bits(&world.node("body").unwrap().local);
    world.transform("body", place(input)).unwrap();
    assert_eq!(bits(&world.node("body").unwrap().local), first);
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
    let placement = world.placement("body").unwrap();
    world.transform("body", place(placement)).unwrap();
    let once = bits(&world.node("body").unwrap().local);
    world.transform("body", place(placement)).unwrap();
    assert_eq!(bits(&world.node("body").unwrap().local), once);
    assert_eq!(bits(&world.placement("body").unwrap()), bits(&placement));
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
fn world_placement_form_composes_the_parent_chain() {
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
    world
        .create_primitive(
            box_primitive(),
            CreateOptions {
                og_id: Some("body".into()),
                parent: Some("parent".into()),
                plane: Some(Plane {
                    origin: Some([4.0, 5.0, 6.0]),
                    ..Plane::default()
                }),
            },
        )
        .unwrap();
    assert_eq!(
        world.world_placement_form("body").unwrap(),
        Placement {
            origin: [14.0, 5.0, 6.0],
            x_direction: [1.0, 0.0, 0.0],
            normal: [0.0, 1.0, 0.0],
            scale: 1.0,
        }
    );
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
