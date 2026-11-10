use opengeometry::primitives::{PolysolidJustify, PolysolidPath, ProfileSegment};
use opengeometry::world_graph::{
    CreateOptions, EditScope, ErrorCode, ModifyingOperation, Plane, Primitive, StepOptions,
    Transform, WorldGraph,
};
use opengeometry_test_support::part21::Document;
use opengeometry_test_support::world_graph::{graph, named};
use serde_json::json;

fn path(start: [f64; 2], points: &[[f64; 2]], closed: bool) -> PolysolidPath {
    PolysolidPath {
        start,
        segments: points
            .iter()
            .map(|&to| ProfileSegment { to, arc: None })
            .collect(),
        closed,
    }
}

fn wall(path: PolysolidPath, width: f64, justify: PolysolidJustify) -> Primitive {
    Primitive::Polysolid {
        path,
        width,
        height: 1.0,
        justify,
    }
}

fn l_path() -> PolysolidPath {
    path([0.0, 0.0], &[[3.0, 0.0], [3.0, 2.0]], false)
}

fn loop_path() -> PolysolidPath {
    path(
        [0.0, 0.0],
        &[[3.0, 0.0], [3.0, 2.0], [0.0, 2.0], [0.0, 0.0]],
        true,
    )
}

fn built(primitive: Primitive) -> WorldGraph {
    let mut world = graph();
    world.create_primitive(primitive, named("wall")).unwrap();
    world
}

fn assert_valid_closed(world: &WorldGraph, og_id: &str, label: &str) {
    let validity = world.validity(og_id).unwrap();
    assert!(validity.valid && validity.closed, "{label}: {validity:?}");
}

fn assert_flat_volume(world: &WorldGraph, expected: f64) {
    let measured = world.measure("wall", Some(1e-4)).unwrap();
    assert!(
        (measured.volume - expected).abs() <= 1e-9,
        "{} versus {expected}",
        measured.volume
    );
}

fn face_count(world: &WorldGraph) -> usize {
    world.brep("wall").unwrap().topology.faces.len()
}

fn assert_bounds(world: &WorldGraph, expected: [f64; 6]) {
    let bounds = world.bounds("wall").unwrap().unwrap();
    assert!(
        (0..6).all(|axis| (bounds[axis] - expected[axis]).abs() <= 1e-9),
        "{bounds:?} versus {expected:?}"
    );
}

fn refused(world: &mut WorldGraph, primitive: Primitive) -> String {
    let label = format!("{primitive:?}");
    let error = world
        .create_primitive(primitive, named("wall"))
        .expect_err(&label);
    assert_eq!(error.error_code(), ErrorCode::InvalidParameter, "{label}");
    assert_eq!(world.node_count(), 0);
    error.message().to_string()
}

fn refusal(primitive: Primitive) -> String {
    refused(&mut graph(), primitive)
}

#[test]
fn l_shaped_polysolid_is_a_closed_wall_of_volume_one_with_ten_faces() {
    let world = built(wall(l_path(), 0.2, PolysolidJustify::Centre));
    assert_valid_closed(&world, "wall", "l path");
    assert_flat_volume(&world, 1.0);
    assert_eq!(face_count(&world), 10);
    assert_bounds(&world, [0.0, 0.0, -2.0, 3.1, 1.0, 0.1]);
}

#[test]
fn closed_polysolid_loop_is_a_closed_ring_wall_of_volume_two() {
    let world = built(wall(loop_path(), 0.2, PolysolidJustify::Centre));
    assert_valid_closed(&world, "wall", "loop");
    assert_flat_volume(&world, 2.0);
    assert_eq!(face_count(&world), 18);
    assert_bounds(&world, [-0.1, 0.0, -2.1, 3.1, 1.0, 0.1]);
}

#[test]
fn left_justified_polysolid_lies_inside_the_l_corner() {
    let world = built(wall(l_path(), 0.2, PolysolidJustify::Left));
    assert_valid_closed(&world, "wall", "left");
    assert_flat_volume(&world, (2.9 + 1.9) * 0.2);
    assert_bounds(&world, [0.0, 0.0, -2.0, 3.0, 1.0, 0.0]);
}

#[test]
fn right_justified_polysolid_lies_outside_the_l_corner() {
    let world = built(wall(l_path(), 0.2, PolysolidJustify::Right));
    assert_valid_closed(&world, "wall", "right");
    assert_flat_volume(&world, (3.1 + 2.1) * 0.2);
    assert_bounds(&world, [0.0, 0.0, -2.0, 3.2, 1.0, 0.2]);
}

#[test]
fn polysolid_with_fewer_than_two_points_is_refused() {
    let message = refusal(wall(
        path([0.0, 0.0], &[], false),
        0.2,
        PolysolidJustify::Centre,
    ));
    assert!(message.contains("at least two points"), "{message}");
}

#[test]
fn polysolid_with_a_zero_length_leg_is_refused() {
    let repeated = path([0.0, 0.0], &[[3.0, 0.0], [3.0, 0.0], [3.0, 2.0]], false);
    let message = refusal(wall(repeated, 0.2, PolysolidJustify::Centre));
    assert!(message.contains("leg 1 has zero length"), "{message}");
}

#[test]
fn polysolid_with_a_leg_shorter_than_half_its_width_is_refused() {
    let short = path([0.0, 0.0], &[[3.0, 0.0], [3.0, 0.09], [5.0, 0.09]], false);
    let message = refusal(wall(short, 0.2, PolysolidJustify::Centre));
    assert!(
        message.contains("leg 1 is shorter than half the width"),
        "{message}"
    );
}

#[test]
fn polysolid_path_that_crosses_itself_is_refused() {
    let crossing = path([0.0, 0.0], &[[3.0, 0.0], [3.0, 2.0], [1.5, -1.0]], false);
    let message = refusal(wall(crossing, 0.2, PolysolidJustify::Centre));
    assert!(
        message.contains("crosses itself at legs 0 and 2"),
        "{message}"
    );
}

#[test]
fn polysolid_whose_stroke_gives_no_region_is_refused() {
    let message = refusal(wall(l_path(), 3e-6, PolysolidJustify::Centre));
    assert!(message.contains("0 regions"), "{message}");
}

#[test]
fn closed_polysolid_path_that_does_not_end_at_its_start_is_refused() {
    let open_end = path([0.0, 0.0], &[[3.0, 0.0], [3.0, 2.0], [0.0, 2.0]], true);
    let message = refusal(wall(open_end, 0.2, PolysolidJustify::Centre));
    assert!(message.contains("must end at its start"), "{message}");
}

#[test]
fn polysolid_with_a_bad_width_or_height_is_refused() {
    for (width, height) in [
        (0.0, 1.0),
        (-0.2, 1.0),
        (f64::NAN, 1.0),
        (0.2, 0.0),
        (0.2, f64::INFINITY),
    ] {
        refusal(Primitive::Polysolid {
            path: l_path(),
            width,
            height,
            justify: PolysolidJustify::Centre,
        });
    }
}

#[test]
fn polysolid_json_is_camel_case_and_justify_defaults_to_centre() {
    let parsed: Primitive = serde_json::from_value(json!({
        "kind": "Polysolid",
        "path": {"start": [0, 0], "segments": [{"to": [3, 0]}, {"to": [3, 2]}], "closed": false},
        "width": 0.2,
        "height": 1
    }))
    .unwrap();
    let Primitive::Polysolid { justify, .. } = parsed else {
        panic!("{parsed:?}");
    };
    assert_eq!(justify, PolysolidJustify::Centre);
    let right = json!({
        "kind": "Polysolid",
        "path": {"start": [0, 0], "segments": [{"to": [3, 0]}], "closed": false},
        "width": 0.2,
        "height": 1,
        "justify": "right"
    });
    let world = built(serde_json::from_value(right.clone()).unwrap());
    assert_valid_closed(&world, "wall", "json");
    let mut unknown = right;
    unknown["path"]["isClosed"] = json!(false);
    assert!(serde_json::from_value::<Primitive>(unknown).is_err());
}

#[test]
fn polysolid_on_a_plane_is_placed_and_keeps_its_volume() {
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
        .create_primitive(wall(l_path(), 0.2, PolysolidJustify::Centre), options)
        .unwrap();
    assert_valid_closed(&world, "wall", "placed");
    assert_flat_volume(&world, 1.0);
    assert_bounds(&world, [1.0, 0.0, 3.0, 2.0, 2.1, 6.1]);
}

#[test]
fn polysolid_shell_and_step_export_are_valid() {
    let mut world = built(wall(l_path(), 0.2, PolysolidJustify::Centre));
    let (text, report) = world
        .export_step(&["wall".into()], &StepOptions::default())
        .unwrap();
    assert_eq!(report.pcurveless_edges, 0);
    let document = Document::parse(&text).unwrap();
    assert_eq!(document.count("MANIFOLD_SOLID_BREP"), 1);
    assert_eq!(document.pcurveless_edges().unwrap(), 0);
    assert_eq!(document.count("PLANE"), face_count(&world));
    assert_eq!(document.count("CYLINDRICAL_SURFACE"), 0);
    world.shell("wall", 0.03, EditScope::Node).unwrap();
    assert_valid_closed(&world, "wall", "shell");
}

fn hairpin_bounds(justify: PolysolidJustify) -> [f64; 6] {
    let hairpin = path([0.0, 0.0], &[[3.0, 0.0], [0.0, 0.3]], false);
    let world = built(wall(hairpin, 0.2, justify));
    assert_valid_closed(&world, "wall", &format!("{justify:?}"));
    world.bounds("wall").unwrap().unwrap()
}

#[test]
fn justified_near_hairpin_polysolid_stays_within_the_mitre_limit_of_its_corner() {
    for justify in [PolysolidJustify::Left, PolysolidJustify::Right] {
        let bounds = hairpin_bounds(justify);
        let reach = 0.2 * 4.0;
        assert!(bounds[3] <= 3.0 + reach, "{justify:?} {bounds:?}");
        assert!(bounds[2] >= -0.3 - reach, "{justify:?} {bounds:?}");
        assert!(bounds[5] <= reach, "{justify:?} {bounds:?}");
    }
}

#[test]
fn open_polysolid_path_that_ends_at_its_start_is_refused() {
    let returning = path([0.0, 0.0], &[[3.0, 0.0], [3.0, 2.0], [0.0, 0.0]], false);
    let message = refusal(wall(returning, 0.2, PolysolidJustify::Centre));
    assert!(message.contains("must not end at its start"), "{message}");
}

#[test]
fn closed_polysolid_path_with_two_segments_is_refused() {
    let there_and_back = path([0.0, 0.0], &[[3.0, 0.0], [0.0, 0.0]], true);
    let message = refusal(wall(there_and_back, 0.2, PolysolidJustify::Centre));
    assert!(message.contains("at least three points"), "{message}");
}

#[test]
fn closed_bow_tie_polysolid_path_is_refused() {
    let bow_tie = path(
        [0.0, 0.0],
        &[[2.0, 0.0], [0.0, 2.0], [2.0, 2.0], [0.0, 0.0]],
        true,
    );
    let message = refusal(wall(bow_tie, 0.2, PolysolidJustify::Centre));
    assert!(
        message.contains("crosses itself at legs 1 and 3"),
        "{message}"
    );
}

#[test]
fn polysolid_with_collinear_legs_is_one_straight_wall() {
    let straight = path([0.0, 0.0], &[[1.0, 0.0], [2.0, 0.0]], false);
    let world = built(wall(straight, 0.2, PolysolidJustify::Centre));
    assert_valid_closed(&world, "wall", "collinear");
    assert_flat_volume(&world, 2.0 * 0.2 * 1.0);
}

#[test]
fn polysolid_path_that_turns_straight_back_is_refused() {
    let u_turn = path([0.0, 0.0], &[[3.0, 0.0], [1.0, 0.0]], false);
    let message = refusal(wall(u_turn, 0.2, PolysolidJustify::Centre));
    assert!(message.contains("crosses itself"), "{message}");
}

const OPERATIONS: [ModifyingOperation; 3] = [
    ModifyingOperation::Subtract,
    ModifyingOperation::Union,
    ModifyingOperation::Intersect,
];

const PLACEMENTS: [(&str, [f64; 3], f64); 3] = [
    ("A", [1.5, 0.75, -0.1], 0.5),
    ("B", [1.5, 0.75, 0.0], 0.5),
    ("T", [1.5, -0.25, 0.0], 1.5),
];

fn tools(height: f64) -> [(&'static str, Primitive); 2] {
    [
        (
            "box",
            Primitive::Cuboid {
                width: 0.08,
                height,
                depth: 0.08,
            },
        ),
        (
            "cylinder",
            Primitive::Cylinder {
                radius: 0.04,
                height,
            },
        ),
    ]
}

fn bar_cell(tool: &Primitive, offset: [f64; 3], operation: ModifyingOperation) -> String {
    let mut world = built(wall(l_path(), 0.2, PolysolidJustify::Centre));
    world.create_primitive(tool.clone(), named("tool")).unwrap();
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

#[test]
fn l_shaped_polysolid_passes_the_boolean_bar_with_box_and_cylinder_tools() {
    let mut seen = Vec::new();
    let mut expected = Vec::new();
    for (place, offset, height) in PLACEMENTS {
        for (name, tool) in tools(height) {
            for operation in OPERATIONS {
                let handlers = bar_cell(&tool, offset, operation);
                seen.push(format!("{place}/{name}/{operation:?} {handlers}"));
                let handler = match (place, name, operation) {
                    (_, "box", _) => "boolean_rectilinear",
                    ("T", _, ModifyingOperation::Subtract) => {
                        "generic_two_sided_cutter_band_subtraction"
                    }
                    _ => "prismatic_curved_boolean",
                };
                expected.push(format!(
                    "{place}/{name}/{operation:?} valid true closed true [\"{handler}\"]"
                ));
            }
        }
    }
    assert_eq!(seen, expected);
}
