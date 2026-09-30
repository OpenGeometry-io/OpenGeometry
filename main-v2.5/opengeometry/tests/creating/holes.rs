use opengeometry::world_graph::{
    CreateOptions, CreatingOperation, ErrorCode, Plane, Primitive, WorldGraph,
};
use opengeometry_test_support::world_graph::{graph, named};

fn square_profile() -> WorldGraph {
    let mut world = graph();
    world
        .create_primitive(
            Primitive::Rectangle {
                width: 4.0,
                breadth: 4.0,
            },
            named("outer"),
        )
        .unwrap();
    world
}

fn circle_at(world: &mut WorldGraph, id: &str, radius: f64, origin: [f64; 3]) {
    world
        .create_primitive(
            Primitive::Circle { radius },
            CreateOptions {
                plane: Some(Plane {
                    origin: Some(origin),
                    ..Plane::default()
                }),
                ..named(id)
            },
        )
        .unwrap();
}

fn assert_rejected(mut world: WorldGraph, holes: &[&str], message: &str) {
    let revision = world.revision();
    let nodes = world.node_count();
    let error = world
        .create_operation(
            CreatingOperation::Extrude {
                profile: "outer".into(),
                holes: holes.iter().map(|hole| (*hole).into()).collect(),
                distance: 2.0,
            },
            named("solid"),
        )
        .unwrap_err();
    assert_eq!(error.error_code(), ErrorCode::InvalidParameter);
    assert_eq!(error.message(), message);
    assert_eq!(world.revision(), revision);
    assert_eq!(world.node_count(), nodes);
}

#[test]
fn hole_outside_the_profile_is_invalid_parameter() {
    let mut world = square_profile();
    circle_at(&mut world, "hole", 0.5, [5.0, 0.0, 0.0]);
    assert_rejected(world, &["hole"], "hole lies outside the profile");
}

#[test]
fn hole_touching_the_profile_is_invalid_parameter() {
    let mut world = square_profile();
    circle_at(&mut world, "hole", 1.0, [0.0, 0.0, 1.0]);
    assert_rejected(world, &["hole"], "hole touches the profile");
}

#[test]
fn overlapping_holes_are_invalid_parameter() {
    let mut world = square_profile();
    circle_at(&mut world, "a", 0.5, [-0.3, 0.0, 0.0]);
    circle_at(&mut world, "b", 0.5, [0.3, 0.0, 0.0]);
    assert_rejected(world, &["a", "b"], "holes overlap");
}

#[test]
fn nested_holes_are_invalid_parameter() {
    let mut world = square_profile();
    circle_at(&mut world, "a", 1.5, [0.0; 3]);
    circle_at(&mut world, "b", 0.5, [0.0; 3]);
    assert_rejected(world, &["a", "b"], "holes are nested");
}

#[test]
fn annular_hole_not_smaller_than_the_profile_is_invalid_parameter() {
    let mut world = graph();
    circle_at(&mut world, "outer", 2.0, [0.0; 3]);
    circle_at(&mut world, "hole", 2.0, [0.0; 3]);
    assert_rejected(
        world,
        &["hole"],
        "hole radius is not smaller than the profile",
    );
}

#[test]
fn tilted_circle_hole_rim_off_the_plane_is_invalid_parameter() {
    let mut world = square_profile();
    world
        .create_primitive(
            Primitive::Circle { radius: 1.0 },
            CreateOptions {
                plane: Some(Plane {
                    normal: Some([0.0, (1e-5_f64).cos(), (1e-5_f64).sin()]),
                    ..Plane::default()
                }),
                ..named("hole")
            },
        )
        .unwrap();
    assert_rejected(world, &["hole"], "hole is not coplanar");
}
