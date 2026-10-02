use opengeometry::world_graph::{
    CreatingOperation, EditScope, ErrorCode, ErrorDetails, ModifyingOperation, Primitive,
    Transform, WorldGraph,
};
use opengeometry_test_support::volume;
use opengeometry_test_support::world_graph::{graph, named};

struct Tool {
    og_id: &'static str,
    size: [f64; 3],
    axis: [f64; 3],
    degrees: f64,
    offset: [f64; 3],
}

const OVERLAPPING_DOORS: [Tool; 2] = [
    Tool {
        og_id: "first",
        size: [1.0, 2.1, 1.0],
        axis: [0.0, 0.0, 1.0],
        degrees: 0.0,
        offset: [-2.5, -0.5, 0.0],
    },
    Tool {
        og_id: "second",
        size: [1.0, 2.1, 1.0],
        axis: [0.0, 0.0, 1.0],
        degrees: 0.0,
        offset: [-2.0, -0.5, 0.0],
    },
];

const OBLIQUE_OPENINGS: [Tool; 2] = [
    Tool {
        og_id: "first",
        size: [1.0, 1.0, 1.0],
        axis: [0.0, 0.0, 1.0],
        degrees: 30.0,
        offset: [-2.5, 1.5, 0.0],
    },
    Tool {
        og_id: "second",
        size: [1.0, 1.0, 1.0],
        axis: [0.0, 0.0, 1.0],
        degrees: 30.0,
        offset: [2.5, 1.5, 0.0],
    },
];

fn wall_with(tools: &[Tool]) -> WorldGraph {
    let mut world = graph();
    world
        .create_primitive(
            Primitive::Rectangle {
                width: 10.0,
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
    for tool in tools {
        let [width, height, depth] = tool.size;
        world
            .create_primitive(
                Primitive::Cuboid {
                    width,
                    height,
                    depth,
                },
                named(tool.og_id),
            )
            .unwrap();
        world
            .transform(
                tool.og_id,
                Transform::Rotate {
                    axis: tool.axis,
                    degrees: tool.degrees,
                    pivot: Some([0.0; 3]),
                },
            )
            .unwrap();
        world
            .transform(
                tool.og_id,
                Transform::Translate {
                    offset: tool.offset,
                },
            )
            .unwrap();
    }
    world
}

fn subtract(world: &mut WorldGraph, tools: &[&str]) -> Result<(), ErrorCode> {
    world
        .operate(
            "wall",
            ModifyingOperation::Subtract,
            &tools
                .iter()
                .map(|tool| tool.to_string())
                .collect::<Vec<_>>(),
            EditScope::Node,
        )
        .map(|_| ())
        .map_err(|error| error.error_code())
}

fn handlers(world: &WorldGraph) -> Vec<String> {
    serde_json::from_value(world.report("wall").unwrap().unwrap()["handlers"].clone()).unwrap()
}

#[test]
fn plain_coverage_gap_batch_falls_back_serially() {
    for (tools, expected_volume) in [(OVERLAPPING_DOORS, 8.28), (OBLIQUE_OPENINGS, 8.4)] {
        let mut serial = wall_with(&tools);
        let mut expected_handlers = Vec::new();
        for tool in &tools {
            subtract(&mut serial, &[tool.og_id]).unwrap();
            expected_handlers.extend(handlers(&serial));
        }
        let mut world = wall_with(&tools);
        subtract(&mut world, &["first", "second"]).unwrap();
        let result = world.brep("wall").unwrap();
        result.validate().unwrap();
        assert_eq!(result.revision, 1);
        assert_eq!(handlers(&world), expected_handlers);
        let measured = volume::estimate(&result, 0.01);
        assert!(
            (measured.value - expected_volume).abs() <= measured.error_bound,
            "{} against {expected_volume}",
            measured.value
        );
    }
}

#[test]
fn mixed_cutter_batch_gap_never_falls_back() {
    let rotated = |og_id, x| Tool {
        og_id,
        size: [0.6, 1.0, 1.0],
        axis: [0.0, 1.0, 0.0],
        degrees: 30.0,
        offset: [x, 1.5, 0.0],
    };
    let mut world = wall_with(&[rotated("first", -2.5), rotated("second", 0.0)]);
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
                offset: [2.5, 1.5, -1.0],
            },
        )
        .unwrap();
    let revision = world.revision();
    let node_count = world.node_count();
    let brep_before = world.brep("wall").unwrap().to_json().unwrap();
    let report_before = world.report("wall").unwrap().cloned();
    let tools = ["first", "second", "round"].map(String::from);
    let error = world
        .operate(
            "wall",
            ModifyingOperation::Subtract,
            &tools,
            EditScope::Node,
        )
        .unwrap_err();
    assert_eq!(error.error_code(), ErrorCode::CoverageGap);
    assert!(
        error.message().contains("mixed cutter batch"),
        "{}",
        error.message()
    );
    assert_eq!(
        error.details(),
        ErrorDetails::Operate {
            handlers: Vec::new(),
            tool_index: 0,
            og_ids: vec![
                "wall".into(),
                "first".into(),
                "second".into(),
                "round".into()
            ],
        }
    );
    assert_eq!(world.revision(), revision);
    assert_eq!(world.node_count(), node_count);
    assert_eq!(world.brep("wall").unwrap().to_json().unwrap(), brep_before);
    assert_eq!(world.report("wall").unwrap().cloned(), report_before);
}

#[test]
fn intermediate_empty_two_tool_intersect_gives_empty_result() {
    let mut world = graph();
    for (og_id, x) in [("target", 0.0), ("far", 10.0), ("near", 0.5)] {
        world
            .create_primitive(
                Primitive::Cuboid {
                    width: 2.0,
                    height: 2.0,
                    depth: 2.0,
                },
                named(og_id),
            )
            .unwrap();
        world
            .transform(
                og_id,
                Transform::Translate {
                    offset: [x, 0.0, 0.0],
                },
            )
            .unwrap();
    }
    let revision = world.revision();
    let brep_before = world.brep("target").unwrap().to_json().unwrap();
    let error = world
        .operate(
            "target",
            ModifyingOperation::Intersect,
            &["far".into(), "near".into()],
            EditScope::Node,
        )
        .unwrap_err();
    assert_eq!(error.error_code(), ErrorCode::EmptyResult, "{error:?}");
    assert_eq!(world.revision(), revision);
    assert_eq!(
        world.brep("target").unwrap().to_json().unwrap(),
        brep_before
    );
}
