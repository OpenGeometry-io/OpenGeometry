use super::support::cuboid;
use opengeometry::operations::modifying::boolean::{boolean_brep_outcome_with_handlers, BooleanOp};
use opengeometry::world_graph::{
    CreateOptions, CreatingOperation, EditScope, ModifyingOperation, Primitive, Transform,
};
use opengeometry_test_support::world_graph::{graph, named};

#[test]
fn sweep_result_boolean_routing_is_pinned() {
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
        .create_primitive(
            Primitive::Polyline {
                points: vec![[0.0; 3], [0.0, 3.0, 0.0], [3.0, 3.0, 0.0]],
                closed: false,
            },
            named("path"),
        )
        .unwrap();
    world
        .create_operation(
            CreatingOperation::Sweep {
                profile: "profile".into(),
                path: "path".into(),
            },
            named("sweep"),
        )
        .unwrap();
    world
        .create_primitive(
            Primitive::Cuboid {
                width: 1.0,
                height: 1.0,
                depth: 1.0,
            },
            named("tool"),
        )
        .unwrap();
    let sweep = world.brep("sweep").unwrap();
    let tool = world.brep("tool").unwrap();
    let (result, handlers) = boolean_brep_outcome_with_handlers(
        &sweep,
        &tool,
        BooleanOp::Subtraction,
        "sweep-cut".into(),
    );
    assert_eq!(handlers, ["subtract_planar_polyhedra"]);
    result.unwrap().brep.validate().unwrap();
}

#[test]
fn corotated_levels_keep_box_handler_and_replay_bytes() {
    let recipe = |degrees: f64| {
        let mut world = graph();
        world.create_system_assembly(named("level")).unwrap();
        world
            .create_primitive(
                cuboid(2.0),
                CreateOptions {
                    parent: Some("level".into()),
                    ..named("host")
                },
            )
            .unwrap();
        world
            .create_primitive(
                cuboid(2.0),
                CreateOptions {
                    parent: Some("level".into()),
                    ..named("tool")
                },
            )
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
            .transform(
                "level",
                Transform::Rotate {
                    axis: [0.0, 1.0, 0.0],
                    degrees,
                    pivot: Some([0.0, 0.0, 0.0]),
                },
            )
            .unwrap();
        world
            .operate(
                "host",
                ModifyingOperation::Subtract,
                &["tool".into()],
                EditScope::Node,
            )
            .unwrap();
        (
            world.brep("host").unwrap().to_json().unwrap(),
            world.report("host").unwrap().unwrap()["handlers"].clone(),
        )
    };
    let original = recipe(0.0);
    assert_eq!(original, recipe(90.0));
    assert_eq!(original, recipe(30.0));
    assert_eq!(original.1, serde_json::json!(["boolean_boxes"]));
}

#[test]
fn corotated_levels_keep_wall_handler_and_replay_bytes() {
    let recipe = |degrees: f64| {
        let mut world = graph();
        world.create_system_assembly(named("level")).unwrap();
        world
            .create_primitive(
                Primitive::Rectangle {
                    width: 4.0,
                    breadth: 0.5,
                },
                CreateOptions {
                    parent: Some("level".into()),
                    ..named("profile")
                },
            )
            .unwrap();
        world
            .create_operation(
                CreatingOperation::Extrude {
                    profile: "profile".into(),
                    holes: Vec::new(),
                    distance: 3.0,
                },
                CreateOptions {
                    parent: Some("level".into()),
                    ..named("wall")
                },
            )
            .unwrap();
        world
            .create_primitive(
                Primitive::Cuboid {
                    width: 0.8,
                    height: 3.0,
                    depth: 1.0,
                },
                CreateOptions {
                    parent: Some("level".into()),
                    ..named("cutter")
                },
            )
            .unwrap();
        world
            .transform(
                "cutter",
                Transform::Translate {
                    offset: [0.0, 0.0, 0.5],
                },
            )
            .unwrap();
        world
            .transform(
                "level",
                Transform::Rotate {
                    axis: [0.0, 1.0, 0.0],
                    degrees,
                    pivot: Some([0.0; 3]),
                },
            )
            .unwrap();
        world
            .operate(
                "wall",
                ModifyingOperation::Subtract,
                &["cutter".into()],
                EditScope::Node,
            )
            .unwrap();
        (
            world.brep("wall").unwrap().to_json().unwrap(),
            world.report("wall").unwrap().unwrap()["handlers"].clone(),
        )
    };
    let original = recipe(0.0);
    assert_eq!(original, recipe(90.0));
    assert_eq!(original, recipe(30.0));
    assert_eq!(original.1, serde_json::json!(["boolean_rectilinear"]));
}
