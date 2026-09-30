use super::support::box_primitive;
use opengeometry::world_graph::{CreateOptions, Transform};
use opengeometry_test_support::world_graph::{graph, named};

fn close(actual: [f64; 6], expected: [f64; 6]) {
    for index in 0..6 {
        assert!(
            (actual[index] - expected[index]).abs() < 1e-7,
            "{actual:?} != {expected:?}"
        );
    }
}

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
    close(
        world.local_bounds("child").unwrap().unwrap(),
        [-1.0, 0.0, -1.0, 1.0, 2.0, 1.0],
    );
    close(
        world.bounds("child").unwrap().unwrap(),
        [10.0, 2.0, 2.0, 12.0, 4.0, 4.0],
    );
    assert_eq!(world.local_bounds("parent").unwrap(), None);
}
