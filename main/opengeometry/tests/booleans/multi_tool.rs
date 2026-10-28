use crate::support::assert_valid_closed;
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

fn add_round(world: &mut WorldGraph, x: f64, degrees: f64) {
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
                degrees,
                pivot: Some([0.0; 3]),
            },
        )
        .unwrap();
    world
        .transform(
            "round",
            Transform::Translate {
                offset: [x, 1.5, -1.0],
            },
        )
        .unwrap();
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
    add_round(&mut world, 2.5, 90.0);
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
            attempted: vec![
                "subtract_planar_cutters".into(),
                "subtract_planar_cutters".into()
            ],
            families: Some(Box::new([
                "mixed cutter batch".into(),
                "planar stage".into()
            ])),
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
fn mixed_cutter_batch_gap_names_the_failing_cylindrical_step_gap() {
    let mut world = wall_with(&OVERLAPPING_DOORS[..1]);
    add_round(&mut world, 4.8, 80.0);
    let tools = ["first", "round"].map(String::from);
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
        error.message().contains("cylindrical stage"),
        "{}",
        error.message()
    );
    let ErrorDetails::Operate {
        attempted,
        families,
        ..
    } = error.details()
    else {
        panic!("{error:?}");
    };
    assert_eq!(
        families,
        Some(Box::new([
            "canonical aligned cuboid".into(),
            "canonical aligned cuboid".into()
        ]))
    );
    let expected = [
        "subtract_planar_cutters",
        "subtract_planar_cutters",
        "separate_boolean",
        "boolean_boxes",
        "boolean_planar_extrusions",
        "boolean_rectilinear",
        "separate_boolean",
        "cylinder_box_boolean",
        "subtract_vertical_arc_extrusion",
        "generic_two_sided_cutter_band_subtraction",
        "generic_multiple_closed_loops_boolean",
        "generic_single_closed_loop_boolean",
        "generic_non_intersecting_boolean",
        "prismatic_curved_boolean",
    ];
    assert_eq!(attempted, expected.map(String::from).to_vec());
}

#[test]
fn a_round_opening_crossing_the_end_of_a_notched_wall_builds_with_the_prismatic_handler() {
    let mut world = wall_with(&OVERLAPPING_DOORS[..1]);
    add_round(&mut world, 4.8, 90.0);
    subtract(&mut world, &["first", "round"]).unwrap();
    assert!(handlers(&world).contains(&"prismatic_curved_boolean".to_string()));
    let body = world.brep("wall").unwrap();
    assert_valid_closed(&body, 1, "notched wall");
    let segment = 0.16 * (0.5_f64).acos() - 0.2 * 0.12_f64.sqrt();
    let opening = (std::f64::consts::PI * 0.16 - segment) * 0.3;
    let measured = volume::estimate(&body, 1e-4);
    assert!(
        (measured.value - (9.0 - 0.48 - opening)).abs() <= measured.error_bound,
        "{} within {}",
        measured.value,
        measured.error_bound
    );
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
