use opengeometry::brep::FaceRole;
use opengeometry::world_graph::{
    CreatingOperation, EditScope, ModifyingOperation, Primitive, Transform,
};
use opengeometry_test_support::volume;
use opengeometry_test_support::world_graph::{graph, named};

#[test]
fn planar_batch_subtraction_commits_one_shape_revision() {
    let mut world = graph();
    world
        .create_primitive(
            Primitive::Cuboid {
                width: 6.0,
                height: 2.0,
                depth: 2.0,
            },
            named("host"),
        )
        .unwrap();
    for (name, x) in [("left", -1.5), ("right", 1.5)] {
        world
            .create_primitive(
                Primitive::Cuboid {
                    width: 1.0,
                    height: 2.0,
                    depth: 2.0,
                },
                named(name),
            )
            .unwrap();
        world
            .transform(
                name,
                Transform::Translate {
                    offset: [x, 0.0, 0.0],
                },
            )
            .unwrap();
    }
    world
        .operate(
            "host",
            ModifyingOperation::Subtract,
            &["left".into(), "right".into()],
            EditScope::Node,
        )
        .unwrap();
    assert_eq!(world.brep("host").unwrap().revision, 1);
    let measured = volume::estimate(&world.brep("host").unwrap(), 0.01);
    assert!((measured.value - 16.0).abs() <= measured.error_bound);
    assert_eq!(
        world.report("host").unwrap().unwrap()["handlers"][0],
        "subtract_planar_cutters"
    );
}

#[test]
fn mixed_cutter_batch_keeps_cut_faces_and_one_revision() {
    let mut world = graph();
    world
        .create_primitive(
            Primitive::Rectangle {
                width: 6.0,
                breadth: 0.3,
            },
            named("profile"),
        )
        .unwrap();
    world
        .create_operation(
            CreatingOperation::Extrude {
                profile: "profile".into(),
                holes: Vec::new(),
                distance: 3.0,
            },
            named("wall"),
        )
        .unwrap();
    world
        .create_primitive(
            Primitive::Cuboid {
                width: 0.8,
                height: 3.0,
                depth: 1.0,
            },
            named("rect"),
        )
        .unwrap();
    world
        .transform(
            "rect",
            Transform::Translate {
                offset: [1.5, 0.0, 0.5],
            },
        )
        .unwrap();
    world
        .create_primitive(
            Primitive::Cylinder {
                radius: 0.4,
                height: 2.0,
            },
            named("round"),
        )
        .unwrap();
    world
        .transform(
            "round",
            Transform::Rotate {
                axis: [1.0, 0.0, 0.0],
                degrees: 90.0,
                pivot: Some([0.0; 3]),
            },
        )
        .unwrap();
    world
        .transform(
            "round",
            Transform::Translate {
                offset: [-1.5, 1.5, -1.0],
            },
        )
        .unwrap();
    world
        .operate(
            "wall",
            ModifyingOperation::Subtract,
            &["rect".into(), "round".into()],
            EditScope::Node,
        )
        .unwrap();
    let result = world.brep("wall").unwrap();
    result.validate().unwrap();
    assert_eq!(result.revision, 1);
    assert!(result
        .topology
        .faces
        .iter()
        .any(|face| face.provenance.role == FaceRole::Cut));
    let handlers = &world.report("wall").unwrap().unwrap()["handlers"];
    assert_eq!(handlers[0], "subtract_planar_cutters");
}
