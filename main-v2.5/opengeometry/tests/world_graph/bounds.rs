use super::support::{box_primitive, near_within};
use opengeometry::world_graph::{CreateOptions, Transform};
use opengeometry_test_support::world_graph::{graph, named};

#[test]
fn local_bounds_ignore_placement_and_parents() {
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
                parent: Some("parent".into()),
                ..named("child")
            },
        )
        .unwrap();
    world
        .transform(
            "child",
            Transform::Translate {
                offset: [1.0, 2.0, 3.0],
            },
        )
        .unwrap();
    near_within(
        &world.local_bounds("child").unwrap().unwrap(),
        &[-1.0, 0.0, -1.0, 1.0, 2.0, 1.0],
        1e-7,
    );
    near_within(
        &world.bounds("child").unwrap().unwrap(),
        &[10.0, 2.0, 2.0, 12.0, 4.0, 4.0],
        1e-7,
    );
    assert_eq!(world.local_bounds("parent").unwrap(), None);
}
