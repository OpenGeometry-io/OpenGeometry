use super::support::extrude;
use opengeometry::world_graph::{CopyOptions, EditScope, ErrorCode, Primitive, Transform};
use opengeometry_test_support::world_graph::{graph, named};

#[test]
fn rebuild_maps_profile_to_body_and_preserves_placement() {
    let mut world = graph();
    world
        .create_primitive(
            Primitive::Rectangle {
                width: 2.0,
                breadth: 2.0,
            },
            named("profile"),
        )
        .unwrap();
    world
        .create_operation(extrude("profile", 2.0), named("solid"))
        .unwrap();
    world
        .transform(
            "solid",
            Transform::Translate {
                offset: [5.0, 0.0, 0.0],
            },
        )
        .unwrap();
    let before = world.world_placement("solid").unwrap();
    world
        .rebuild_operation("solid", extrude("profile", 3.0), EditScope::Node)
        .unwrap();
    assert_eq!(before, world.world_placement("solid").unwrap());
    let bounds = world.bounds("solid").unwrap().unwrap();
    assert!((bounds[0] + 1.0).abs() < 2e-8, "{bounds:?}");
    assert!((bounds[3] - 1.0).abs() < 2e-8, "{bounds:?}");
}

#[test]
fn rebuild_obeys_shared_shape_scope() {
    let mut world = graph();
    world
        .create_primitive(
            Primitive::Rectangle {
                width: 2.0,
                breadth: 2.0,
            },
            named("profile"),
        )
        .unwrap();
    world
        .create_operation(extrude("profile", 2.0), named("solid"))
        .unwrap();
    world
        .instance(
            "solid",
            CopyOptions {
                og_id: Some("instance".into()),
                ..CopyOptions::default()
            },
        )
        .unwrap();
    assert_eq!(
        world
            .rebuild_operation("solid", extrude("profile", 3.0), EditScope::Node)
            .unwrap_err()
            .error_code(),
        ErrorCode::SharedShape
    );
    world
        .rebuild_operation("solid", extrude("profile", 3.0), EditScope::AllInstances)
        .unwrap();
    assert_eq!(world.brep("solid").unwrap().revision, 1);
    assert_eq!(world.brep("instance").unwrap().revision, 1);
}

#[test]
fn rebuild_after_180_about_x_extrudes_circle_and_rectangle_the_same_way() {
    let mut world = graph();
    world
        .create_primitive(Primitive::Circle { radius: 1.0 }, named("circle"))
        .unwrap();
    world
        .create_primitive(
            Primitive::Rectangle {
                width: 2.0,
                breadth: 2.0,
            },
            named("rectangle"),
        )
        .unwrap();
    for (profile, body) in [("circle", "circle-solid"), ("rectangle", "rectangle-solid")] {
        world
            .create_operation(extrude(profile, 2.0), named(body))
            .unwrap();
        world
            .transform(
                body,
                Transform::Rotate {
                    axis: [1.0, 0.0, 0.0],
                    degrees: 180.0,
                    pivot: Some([0.0; 3]),
                },
            )
            .unwrap();
        world
            .rebuild_operation(body, extrude(profile, 2.0), EditScope::Node)
            .unwrap();
    }
    for body in ["circle-solid", "rectangle-solid"] {
        let bounds = world.bounds(body).unwrap().unwrap();
        assert!((bounds[1] + 2.0).abs() < 2e-8, "{body} {bounds:?}");
        assert!(bounds[4] < 2e-8, "{body} {bounds:?}");
    }
}
