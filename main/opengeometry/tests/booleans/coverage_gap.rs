use opengeometry::primitives::{ProfileSegment, SegmentArc};
use opengeometry::world_graph::{
    CreateOptions, CreatingOperation, EditScope, ErrorCode, ErrorDetails, ModifyingOperation,
    Plane, Primitive, Transform, WorldGraph,
};
use opengeometry_test_support::scenes::acceptance::{attempt_rail_minus_wall, recut_scene};
use opengeometry_test_support::scenes::coverage_gap::rail_minus_wall_clearance;
use opengeometry_test_support::world_graph::{graph, named};

const GENERIC_HANDLERS: [&str; 5] = [
    "subtract_vertical_arc_extrusion",
    "generic_two_sided_cutter_band_subtraction",
    "generic_multiple_closed_loops_boolean",
    "generic_single_closed_loop_boolean",
    "generic_non_intersecting_boolean",
];

struct Gap {
    attempted: Vec<String>,
    families: Option<Box<[String; 2]>>,
}

fn gap(world: &mut WorldGraph, operation: ModifyingOperation, tool: &str) -> Gap {
    let error = world
        .operate("box", operation, &[tool.to_string()], EditScope::Node)
        .unwrap_err();
    assert_eq!(error.error_code(), ErrorCode::CoverageGap, "{error:?}");
    match error.details() {
        ErrorDetails::Operate {
            handlers,
            attempted,
            families,
            tool_index,
            og_ids,
        } => {
            assert!(handlers.is_empty(), "{handlers:?}");
            assert_eq!(tool_index, 0);
            assert_eq!(og_ids, vec!["box".to_string(), tool.to_string()]);
            Gap {
                attempted,
                families,
            }
        }
        details => panic!("{details:?}"),
    }
}

fn pair(first: &str, second: &str) -> Option<Box<[String; 2]>> {
    Some(Box::new([first.into(), second.into()]))
}

fn box_and_leg(degrees: f64) -> WorldGraph {
    let mut world = graph();
    world
        .create_primitive(
            Primitive::Cuboid {
                width: 1.0,
                height: 0.5,
                depth: 1.0,
            },
            named("box"),
        )
        .unwrap();
    world
        .create_primitive(
            Primitive::Cylinder {
                radius: 0.1,
                height: 0.7,
            },
            named("leg"),
        )
        .unwrap();
    world
        .transform(
            "leg",
            Transform::Rotate {
                axis: [1.0, 0.0, 0.0],
                degrees,
                pivot: Some([0.0, 0.35, 0.0]),
            },
        )
        .unwrap();
    world
        .transform(
            "leg",
            Transform::Translate {
                offset: [0.5, -0.1, 0.0],
            },
        )
        .unwrap();
    world
}

fn d_pocket(world: &mut WorldGraph) {
    let edge = |to: [f64; 2]| ProfileSegment { to, arc: None };
    let profile = Primitive::Profile {
        start: [0.0, 0.0],
        segments: vec![
            edge([0.325, 0.0]),
            ProfileSegment {
                to: [0.325, 0.35],
                arc: Some(SegmentArc {
                    center: [0.325, 0.175],
                    counter_clockwise: true,
                }),
            },
            edge([0.0, 0.35]),
            edge([0.0, 0.0]),
        ],
        closed: true,
    };
    let options = CreateOptions {
        plane: Some(Plane {
            origin: Some([-0.25, 0.3, 0.175]),
            normal: None,
            x_direction: None,
        }),
        ..named("pocket-profile")
    };
    world.create_primitive(profile, options).unwrap();
    world
        .create_operation(
            CreatingOperation::Extrude {
                profile: "pocket-profile".into(),
                holes: Vec::new(),
                distance: 0.4,
            },
            named("pocket"),
        )
        .unwrap();
}

#[test]
fn rail_minus_wall_returns_coverage_gap_with_clearance_and_leaves_graph_unchanged() {
    let mut scene = recut_scene();
    let clearance = rail_minus_wall_clearance(&scene.graph, &scene.rail, &scene.wall);
    let accuracy = scene.graph.accuracy().geometric;
    assert!(clearance > 10.0 * accuracy, "clearance {clearance}");
    let revision = scene.graph.revision();
    let node_count = scene.graph.node_count();
    let rail_before = scene.graph.brep(&scene.rail).unwrap().to_json().unwrap();
    let wall_before = scene.graph.brep(&scene.wall).unwrap().to_json().unwrap();
    let error = attempt_rail_minus_wall(&mut scene);
    assert_eq!(error.error_code(), ErrorCode::CoverageGap, "{error:?}");
    let mut attempted = vec![
        "separate_boolean".to_string(),
        "boolean_spheres".to_string(),
    ];
    attempted.extend(GENERIC_HANDLERS.map(String::from));
    assert_eq!(
        error.details(),
        ErrorDetails::Operate {
            handlers: Vec::new(),
            attempted,
            families: pair("planar/compound solid", "planar/compound solid"),
            tool_index: 0,
            og_ids: vec!["rail-1".into(), "wall-1".into()],
        }
    );
    assert_eq!(scene.graph.revision(), revision);
    assert_eq!(scene.graph.node_count(), node_count);
    let rail_after = scene.graph.brep(&scene.rail).unwrap().to_json().unwrap();
    let wall_after = scene.graph.brep(&scene.wall).unwrap().to_json().unwrap();
    assert_eq!(rail_after, rail_before);
    assert_eq!(wall_after, wall_before);
}

#[test]
fn cuboid_and_side_crossing_upright_cylinder_report_the_cylinder_box_gap_for_every_operation() {
    for operation in [
        ModifyingOperation::Subtract,
        ModifyingOperation::Union,
        ModifyingOperation::Intersect,
    ] {
        let mut world = box_and_leg(0.0);
        let gap = gap(&mut world, operation, "leg");
        assert_eq!(
            gap.families,
            pair("cylinder", "side-intersecting cuboid"),
            "{operation:?}"
        );
        let mut attempted = vec![
            "separate_boolean".to_string(),
            "cylinder_box_boolean".to_string(),
        ];
        attempted.extend(GENERIC_HANDLERS.map(String::from));
        assert_eq!(gap.attempted, attempted, "{operation:?}");
    }
}

#[test]
fn cuboid_minus_oblique_cylinder_reports_the_oblique_cuboid_gap() {
    let mut world = box_and_leg(10.0);
    let gap = gap(&mut world, ModifyingOperation::Subtract, "leg");
    assert_eq!(
        gap.families,
        pair("cylinder", "oblique cuboid intersection")
    );
    assert!(gap.attempted.contains(&"cylinder_box_boolean".to_string()));
}

#[test]
fn cuboid_minus_blind_arc_edged_pocket_reports_the_fallback_gap_and_the_generic_handlers() {
    let mut world = graph();
    world
        .create_primitive(
            Primitive::Cuboid {
                width: 1.0,
                height: 0.5,
                depth: 1.0,
            },
            named("box"),
        )
        .unwrap();
    d_pocket(&mut world);
    let gap = gap(&mut world, ModifyingOperation::Subtract, "pocket");
    assert_eq!(
        gap.families,
        pair("planar/compound solid", "planar/compound solid")
    );
    assert!(gap.attempted.contains(&"boolean_spheres".to_string()));
    for handler in GENERIC_HANDLERS {
        assert!(gap.attempted.contains(&handler.to_string()), "{handler}");
    }
}
