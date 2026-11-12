use super::support::{arc, line};
use opengeometry::primitives::{PolysolidJustify, PolysolidPath, ProfileSegment};
use opengeometry::world_graph::{
    CreateOptions, EditScope, ErrorCode, ModifyingOperation, Plane, Primitive, StepOptions,
    Transform, WorldGraph,
};
use opengeometry_test_support::part21::Document;
use opengeometry_test_support::volume;
use opengeometry_test_support::world_graph::{graph, named};
use serde_json::json;
use std::f64::consts::{FRAC_1_SQRT_2, PI};

fn arc_path(start: [f64; 2], segments: Vec<ProfileSegment>, closed: bool) -> PolysolidPath {
    PolysolidPath {
        start,
        segments,
        closed,
    }
}

fn leg_and_arc() -> PolysolidPath {
    arc_path(
        [0.0, 0.0],
        vec![line([2.0, 0.0]), arc([3.0, 1.0], [2.0, 1.0], true)],
        false,
    )
}

fn circle_path() -> PolysolidPath {
    arc_path(
        [1.0, 0.0],
        vec![
            arc([-1.0, 0.0], [0.0, 0.0], true),
            arc([1.0, 0.0], [0.0, 0.0], true),
        ],
        true,
    )
}

fn curved_wall(path: PolysolidPath, justify: PolysolidJustify) -> Primitive {
    Primitive::Polysolid {
        path,
        width: 0.2,
        height: 1.0,
        justify,
    }
}

fn curved(path: PolysolidPath, justify: PolysolidJustify) -> WorldGraph {
    let mut world = graph();
    world
        .create_primitive(curved_wall(path, justify), named("wall"))
        .unwrap();
    world
}

fn assert_closed_body(world: &WorldGraph, label: &str) {
    let validity = world.validity("wall").unwrap();
    assert!(validity.valid && validity.closed, "{label}: {validity:?}");
}

fn assert_exact_volume(world: &WorldGraph, expected: f64) {
    let measured = volume::estimate(&world.brep("wall").unwrap(), 1e-4);
    assert!(
        (measured.value - expected).abs() <= measured.error_bound,
        "{} versus {expected} with bound {}",
        measured.value,
        measured.error_bound
    );
}

fn assert_step_cylinders(world: &mut WorldGraph, cylinders: usize) {
    let (text, report) = world
        .export_step(&["wall".into()], &StepOptions::default())
        .unwrap();
    assert_eq!(report.pcurveless_edges, 0);
    let document = Document::parse(&text).unwrap();
    assert_eq!(document.count("MANIFOLD_SOLID_BREP"), 1);
    assert_eq!(document.pcurveless_edges().unwrap(), 0);
    assert_eq!(document.count("CYLINDRICAL_SURFACE"), cylinders);
}

fn assert_shell_valid(world: &mut WorldGraph) {
    world.shell("wall", 0.03, EditScope::Node).unwrap();
    assert_closed_body(world, "shell");
}

fn refused_message(path: PolysolidPath) -> String {
    let mut world = graph();
    let error = world
        .create_primitive(curved_wall(path, PolysolidJustify::Centre), named("wall"))
        .expect_err("refused");
    assert_eq!(error.error_code(), ErrorCode::InvalidParameter);
    assert_eq!(world.node_count(), 0);
    error.message().to_string()
}

#[test]
fn leg_and_tangent_arc_polysolid_is_a_closed_wall_with_eight_faces() {
    let world = curved(leg_and_arc(), PolysolidJustify::Centre);
    assert_closed_body(&world, "leg and arc");
    assert_exact_volume(&world, 0.4 + 0.1 * PI);
    assert_eq!(world.brep("wall").unwrap().topology.faces.len(), 8);
    let bounds = world.bounds("wall").unwrap().unwrap();
    let expected = [0.0, 0.0, -1.0, 3.1, 1.0, 0.1];
    assert!(
        (0..6).all(|axis| (bounds[axis] - expected[axis]).abs() <= 1e-6),
        "{bounds:?}"
    );
}

#[test]
fn leg_and_arc_polysolid_exports_two_cylinders_to_step_and_shells() {
    let mut world = curved(leg_and_arc(), PolysolidJustify::Centre);
    assert_step_cylinders(&mut world, 2);
    assert_shell_valid(&mut world);
}

#[test]
fn closed_path_of_two_half_circles_is_a_tube_wall_of_four_half_cylinder_faces() {
    let mut world = curved(circle_path(), PolysolidJustify::Centre);
    assert_closed_body(&world, "tube");
    assert_exact_volume(&world, 0.4 * PI);
    assert_step_cylinders(&mut world, 4);
    assert_shell_valid(&mut world);
}

#[test]
fn clockwise_tube_wall_keeps_its_outer_ring_outside() {
    let path = arc_path(
        [1.0, 0.0],
        vec![
            arc([-1.0, 0.0], [0.0, 0.0], false),
            arc([1.0, 0.0], [0.0, 0.0], false),
        ],
        true,
    );
    let world = curved(path, PolysolidJustify::Left);
    assert_closed_body(&world, "clockwise tube");
    assert_exact_volume(&world, 0.44 * PI);
}

#[test]
fn arc_that_does_not_end_on_its_circle_is_refused() {
    let mut path = leg_and_arc();
    path.segments[1].to = [3.0, 1.2];
    let message = refused_message(path);
    assert!(message.contains("does not end on its circle"), "{message}");
}

#[test]
fn left_and_right_justified_leg_and_arc_keep_the_path_on_one_side() {
    for (justify, expected) in [
        (PolysolidJustify::Left, 0.4 + 0.09 * PI),
        (PolysolidJustify::Right, 0.4 + 0.11 * PI),
    ] {
        let world = curved(leg_and_arc(), justify);
        assert_closed_body(&world, &format!("{justify:?}"));
        assert_exact_volume(&world, expected);
    }
}

#[test]
fn straight_corner_before_a_tangent_arc_is_mitred() {
    let path = arc_path(
        [0.0, 0.0],
        vec![
            line([2.0, 0.0]),
            line([2.0, 2.0]),
            arc([0.0, 2.0], [1.0, 2.0], true),
        ],
        false,
    );
    let world = curved(path, PolysolidJustify::Centre);
    assert_closed_body(&world, "mitre");
    assert_exact_volume(&world, 0.8 + 0.2 * PI);
}

#[test]
fn line_into_an_arc_at_a_sharp_corner_is_refused_naming_the_joint() {
    let path = arc_path(
        [0.0, 0.0],
        vec![line([2.0, 0.0]), arc([4.0, 0.0], [3.0, 0.0], true)],
        false,
    );
    let message = refused_message(path);
    assert!(
        message.contains("joint 1 between segment 0 and segment 1 is not tangent"),
        "{message}"
    );
}

#[test]
fn arc_into_a_line_at_a_sharp_corner_is_refused_naming_the_joint() {
    let mut path = leg_and_arc();
    path.segments.push(line([4.0, 1.0]));
    let message = refused_message(path);
    assert!(
        message.contains("joint 2 between segment 1 and segment 2 is not tangent"),
        "{message}"
    );
}

#[test]
fn closed_path_checks_the_joint_at_its_start() {
    let path = arc_path(
        [1.0, 0.0],
        vec![
            arc([0.0, 1.0], [0.0, 0.0], true),
            line([-2.0, 1.0]),
            line([1.0, 0.0]),
        ],
        true,
    );
    let message = refused_message(path);
    assert!(
        message.contains("joint 0 between segment 2 and segment 0 is not tangent"),
        "{message}"
    );
}

#[test]
fn arc_tighter_than_half_the_width_is_refused() {
    let path = arc_path(
        [0.0, 0.0],
        vec![line([1.0, 0.0]), arc([1.1, 0.1], [1.0, 0.1], true)],
        false,
    );
    let message = refused_message(path);
    assert!(message.contains("segment 1"), "{message}");
    assert!(message.contains("offset radius"), "{message}");
}

#[test]
fn arc_path_whose_wall_overlaps_itself_is_refused_naming_the_segments() {
    let path = arc_path(
        [0.0, 0.0],
        vec![
            line([2.0, 0.0]),
            arc([2.0, 0.3], [2.0, 0.15], true),
            line([1.0, 0.3]),
            line([0.0, 0.12]),
        ],
        false,
    );
    let message = refused_message(path);
    assert!(
        message.contains("overlaps itself between segment 0 and segment 3"),
        "{message}"
    );
}

#[test]
fn acute_straight_corner_next_to_an_arc_is_refused_as_an_overlap() {
    let turn = 160f64.to_radians();
    let corner = [0.3 + 0.3 * turn.cos(), 0.3 * turn.sin()];
    let center = [corner[0] - 0.5 * turn.sin(), corner[1] + 0.5 * turn.cos()];
    let end = [center[0] + 0.5 * turn.cos(), center[1] + 0.5 * turn.sin()];
    let path = arc_path(
        [0.0, 0.0],
        vec![line([0.3, 0.0]), line(corner), arc(end, center, true)],
        false,
    );
    let message = refused_message(path);
    assert!(
        message.contains("overlaps itself between segment"),
        "{message}"
    );
}

#[test]
fn short_leg_at_a_straight_corner_in_an_arc_path_is_refused_naming_the_segment() {
    let path = arc_path(
        [0.0, 0.0],
        vec![
            line([2.0, 0.0]),
            line([2.0, 0.05]),
            arc([0.0, 0.05], [1.0, 0.05], true),
        ],
        false,
    );
    let message = refused_message(path);
    assert!(
        message.contains("segment 1 is a leg shorter than half the width"),
        "{message}"
    );
}

#[test]
fn arc_path_that_crosses_itself_is_refused() {
    let path = arc_path(
        [0.0, 0.0],
        vec![
            line([2.0, 0.0]),
            arc([1.0, 1.0], [2.0, 1.0], true),
            line([1.0, -1.0]),
        ],
        false,
    );
    refused_message(path);
}

#[test]
fn arc_polysolid_json_is_camel_case() {
    let parsed: Primitive = serde_json::from_value(json!({
        "kind": "Polysolid",
        "path": {
            "start": [0, 0],
            "segments": [{"to": [2, 0]}, {"to": [3, 1], "arc": {"center": [2, 1], "counterClockwise": true}}],
            "closed": false
        },
        "width": 0.2,
        "height": 1,
        "justify": "left"
    }))
    .unwrap();
    let mut world = graph();
    world.create_primitive(parsed, named("wall")).unwrap();
    assert_exact_volume(&world, 0.4 + 0.09 * PI);
}

#[test]
fn arc_polysolid_on_a_plane_is_placed() {
    let mut world = graph();
    let options = CreateOptions {
        plane: Some(Plane {
            origin: Some([1.0, 2.0, 3.0]),
            normal: Some([1.0, 0.0, 0.0]),
            x_direction: Some([0.0, 0.0, 1.0]),
        }),
        ..named("wall")
    };
    world
        .create_primitive(
            curved_wall(leg_and_arc(), PolysolidJustify::Centre),
            options,
        )
        .unwrap();
    assert_closed_body(&world, "placed");
    assert_exact_volume(&world, 0.4 + 0.1 * PI);
    let bounds = world.bounds("wall").unwrap().unwrap();
    let expected = [1.0, 1.0, 3.0, 2.0, 2.1, 6.1];
    assert!(
        (0..6).all(|axis| (bounds[axis] - expected[axis]).abs() <= 1e-6),
        "{bounds:?}"
    );
}

const BAR_OPERATIONS: [ModifyingOperation; 3] = [
    ModifyingOperation::Subtract,
    ModifyingOperation::Union,
    ModifyingOperation::Intersect,
];

const BAR_PLACEMENTS: [(&str, [f64; 3], f64); 4] = [
    ("A", [1.0, 0.75, 0.1], 0.5),
    ("B", [1.0, 0.75, 0.0], 0.5),
    ("T", [1.0, -0.25, 0.0], 1.5),
    ("C", [2.0 + FRAC_1_SQRT_2, 0.75, FRAC_1_SQRT_2 - 1.0], 0.5),
];

fn bar_tool(name: &str, height: f64) -> Primitive {
    if name == "box" {
        Primitive::Cuboid {
            width: 0.08,
            height,
            depth: 0.08,
        }
    } else {
        Primitive::Cylinder {
            radius: 0.04,
            height,
        }
    }
}

fn arc_bar_cell(tool: Primitive, offset: [f64; 3], operation: ModifyingOperation) -> String {
    let mut world = curved(leg_and_arc(), PolysolidJustify::Centre);
    world.create_primitive(tool, named("tool")).unwrap();
    world
        .transform("tool", Transform::Translate { offset })
        .unwrap();
    if let Err(error) = world.operate("wall", operation, &["tool".into()], EditScope::Node) {
        return format!("refused {:?}", error.error_code());
    }
    let validity = world.validity("wall").unwrap();
    format!(
        "valid {} closed {} {}",
        validity.valid,
        validity.closed,
        world.report("wall").unwrap().unwrap()["handlers"]
    )
}

fn arc_bar_handler(place: &str, name: &str, operation: ModifyingOperation) -> &'static str {
    match (place, name, operation) {
        (_, "box", ModifyingOperation::Subtract) => "subtract_vertical_arc_extrusion",
        ("T", _, ModifyingOperation::Subtract) => "generic_two_sided_cutter_band_subtraction",
        _ => "prismatic_curved_boolean",
    }
}

#[test]
fn leg_and_arc_polysolid_passes_the_boolean_bar_with_box_and_cylinder_tools() {
    let mut seen = Vec::new();
    let mut expected = Vec::new();
    for (place, offset, height) in BAR_PLACEMENTS {
        for name in ["box", "cylinder"] {
            for operation in BAR_OPERATIONS {
                let cell = format!("{place}/{name}/{operation:?}");
                let result = arc_bar_cell(bar_tool(name, height), offset, operation);
                seen.push(format!("{cell} {result}"));
                let handler = arc_bar_handler(place, name, operation);
                expected.push(format!("{cell} valid true closed true [\"{handler}\"]"));
            }
        }
    }
    assert_eq!(seen, expected);
}
