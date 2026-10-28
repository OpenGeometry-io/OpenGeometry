use super::support::assert_volume;
use opengeometry::world_graph::{
    EditScope, ErrorCode, ErrorDetails, ModifyingOperation, Primitive, Transform, WorldGraph,
};
use opengeometry_test_support::world_graph::{graph, named};
use serde_json::{json, Value};
use std::f64::consts::PI;

fn kind(value: Value) -> Primitive {
    serde_json::from_value(value.clone()).unwrap_or_else(|error| panic!("{value}: {error}"))
}

fn sphere(radius: f64) -> Value {
    json!({"kind": "Sphere", "radius": radius})
}

fn cone(bottom: f64, top: f64, height: f64) -> Value {
    json!({"kind": "Cone", "radiusBottom": bottom, "radiusTop": top, "height": height})
}

fn torus(major: f64, minor: f64) -> Value {
    json!({"kind": "Torus", "majorRadius": major, "minorRadius": minor})
}

fn created(value: Value, og_id: &str) -> WorldGraph {
    let mut world = graph();
    world.create_primitive(kind(value), named(og_id)).unwrap();
    world
}

fn assert_vertical_span(world: &WorldGraph, og_id: &str, [low, high]: [f64; 2]) {
    let bounds = world.bounds(og_id).unwrap().unwrap();
    assert!((bounds[1] - low).abs() <= 1e-9, "{og_id}: {bounds:?}");
    assert!((bounds[4] - high).abs() <= 1e-9, "{og_id}: {bounds:?}");
}

fn frustum_volume(bottom: f64, top: f64, height: f64) -> f64 {
    PI * height * (bottom * bottom + bottom * top + top * top) / 3.0
}

#[test]
fn sphere_volume_is_four_thirds_pi_r_cubed_centred_on_the_plane_origin() {
    let world = created(sphere(0.5), "knob");
    assert_volume(&world, "knob", 4.0 / 3.0 * PI * 0.125);
    assert_vertical_span(&world, "knob", [-0.5, 0.5]);
}

#[test]
fn cone_volume_follows_the_frustum_formula_for_every_radius_pair() {
    for (index, [bottom, top, height]) in [
        [1.0, 0.0, 2.0],
        [0.0, 0.6, 1.0],
        [1.0, 0.5, 2.0],
        [0.4, 0.9, 1.5],
        [0.5, 0.5, 1.0],
    ]
    .into_iter()
    .enumerate()
    {
        let og_id = format!("cone-{index}");
        let world = created(cone(bottom, top, height), &og_id);
        assert_volume(&world, &og_id, frustum_volume(bottom, top, height));
        assert_vertical_span(&world, &og_id, [0.0, height]);
    }
}

#[test]
fn cone_radii_within_four_geometric_tolerances_build_the_cylinder() {
    let world = created(cone(0.5, 0.5 + 1e-12, 1.0), "post");
    assert_volume(&world, "post", PI * 0.25);
    let brep: Value =
        serde_json::from_str(&world.brep("post").unwrap().to_json().unwrap()).unwrap();
    let kinds = brep["geometry"]["surfaces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|surface| surface["kind"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(kinds.contains(&"Cylinder"), "{kinds:?}");
    assert!(!kinds.contains(&"Cone"), "{kinds:?}");
}

#[test]
fn torus_volume_is_two_pi_squared_r_r_squared_standing_on_the_plane() {
    let world = created(torus(1.0, 0.25), "ring");
    assert_volume(&world, "ring", 2.0 * PI * PI * 1.0 * 0.0625);
    assert_vertical_span(&world, "ring", [0.0, 0.5]);
}

#[test]
fn invalid_spheres_cones_and_tori_are_invalid_parameter() {
    for value in [
        sphere(0.0),
        sphere(1e-9),
        cone(0.0, 0.0, 1.0),
        cone(-1.0, 0.5, 1.0),
        cone(1.0, 1e-9, 1.0),
        cone(1.0, 0.5, 0.0),
        torus(1.0, 1.0),
        torus(1.0, 1.5),
        torus(1.0, 0.0),
    ] {
        let mut world = graph();
        let label = value.to_string();
        let error = world
            .create_primitive(kind(value), named("solid"))
            .expect_err(&label);
        assert_eq!(error.error_code(), ErrorCode::InvalidParameter, "{label}");
        assert_eq!(world.node_count(), 0);
    }
}

#[test]
fn cone_and_torus_json_use_camel_case_names() {
    let snake = json!({"kind": "Cone", "radius_bottom": 1, "radius_top": 0, "height": 1});
    assert!(serde_json::from_value::<Primitive>(snake).is_err());
    let snake = json!({"kind": "Torus", "major_radius": 1, "minor_radius": 0.25});
    assert!(serde_json::from_value::<Primitive>(snake).is_err());
}

fn add_tool(world: &mut WorldGraph, size: [f64; 3], offset: [f64; 3]) {
    let tool = Primitive::Cuboid {
        width: size[0],
        height: size[1],
        depth: size[2],
    };
    world.create_primitive(tool, named("tool")).unwrap();
    world
        .transform("tool", Transform::Translate { offset })
        .unwrap();
}

#[test]
fn sphere_minus_a_cuboid_cuts_a_spherical_cap_through_operate() {
    let mut world = created(sphere(1.0), "ball");
    add_tool(&mut world, [4.0, 1.0, 4.0], [0.0, 0.5, 0.0]);
    world
        .operate(
            "ball",
            ModifyingOperation::Subtract,
            &["tool".into()],
            EditScope::Node,
        )
        .unwrap();
    let cap = PI * 0.25 * (3.0 - 0.5) / 3.0;
    assert_volume(&world, "ball", 4.0 / 3.0 * PI - cap);
    assert_eq!(
        world.report("ball").unwrap().unwrap()["handlers"],
        json!(["sphere_box_boolean"])
    );
}

#[test]
fn torus_minus_a_crossing_cuboid_is_a_coverage_gap_naming_the_torus_and_cuboid() {
    let mut world = created(torus(1.0, 0.25), "ring");
    add_tool(&mut world, [0.8, 1.5, 0.8], [1.0, -0.5, 0.0]);
    let error = world
        .operate(
            "ring",
            ModifyingOperation::Subtract,
            &["tool".into()],
            EditScope::Node,
        )
        .unwrap_err();
    assert_eq!(error.error_code(), ErrorCode::CoverageGap, "{error:?}");
    let ErrorDetails::Operate {
        handlers,
        attempted,
        families,
        ..
    } = error.details()
    else {
        panic!("{error:?}");
    };
    assert!(handlers.is_empty(), "{handlers:?}");
    assert_eq!(
        families.map(|pair| pair.to_vec()),
        Some(vec!["ring torus".to_string(), "cuboid".to_string()])
    );
    assert_eq!(
        attempted,
        [
            "separate_boolean",
            "torus_box_containment",
            "subtract_vertical_arc_extrusion",
            "generic_two_sided_cutter_band_subtraction",
            "generic_multiple_closed_loops_boolean",
            "generic_single_closed_loop_boolean",
            "generic_non_intersecting_boolean",
            "prismatic_curved_boolean",
        ]
    );
}
