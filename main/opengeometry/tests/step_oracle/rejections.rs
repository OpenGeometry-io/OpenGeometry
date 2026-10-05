use crate::support::{add_cuboid, fixture_text};
use opengeometry::brep::{BrepEnvelope, GeometryError};
use opengeometry::exchange::export_step;
use opengeometry::world_graph::{CreateOptions, ErrorCode, Primitive, StepOptions, WorldGraph};
use opengeometry_test_support::world_graph::{graph, named};
use serde_json::{json, Value};
use std::f64::consts::TAU;

#[derive(Debug, PartialEq)]
struct GraphState {
    revision: u64,
    nodes: usize,
    shapes: usize,
    breps: Vec<String>,
}

fn graph_state(world: &WorldGraph, bodies: &[&str]) -> GraphState {
    GraphState {
        revision: world.revision(),
        nodes: world.node_count(),
        shapes: world.shape_count(),
        breps: bodies
            .iter()
            .map(|og_id| world.brep(og_id).unwrap().to_json().unwrap())
            .collect(),
    }
}

fn assert_export_rejected_unchanged(
    world: &WorldGraph,
    nodes: &[&str],
    code: ErrorCode,
    bodies: &[&str],
) {
    let before = graph_state(world, bodies);
    let nodes = nodes
        .iter()
        .map(|node| node.to_string())
        .collect::<Vec<_>>();
    let error = world
        .export_step(&nodes, &StepOptions::default())
        .unwrap_err();
    assert_eq!(error.error_code(), code, "{error}");
    assert_eq!(graph_state(world, bodies), before);
}

#[test]
fn empty_node_list_is_invalid_parameter_and_leaves_the_graph_unchanged() {
    let mut world = graph();
    add_cuboid(&mut world, "body", [1.0; 3], [0.0; 3]);
    assert_export_rejected_unchanged(&world, &[], ErrorCode::InvalidParameter, &["body"]);
}

#[test]
fn unknown_node_is_unknown_node_and_leaves_the_graph_unchanged() {
    let mut world = graph();
    add_cuboid(&mut world, "body", [1.0; 3], [0.0; 3]);
    assert_export_rejected_unchanged(&world, &["missing"], ErrorCode::UnknownNode, &["body"]);
}

#[test]
fn assembly_holding_only_wires_is_empty_result_and_leaves_the_graph_unchanged() {
    let mut world = graph();
    world.create_system_assembly(named("level")).unwrap();
    for (og_id, primitive) in [
        ("profile", Primitive::Circle { radius: 0.5 }),
        (
            "rail",
            Primitive::Polyline {
                points: vec![[0.0; 3], [1.0, 0.0, 0.0]],
                closed: false,
            },
        ),
    ] {
        world
            .create_primitive(
                primitive,
                CreateOptions {
                    parent: Some("level".into()),
                    ..named(og_id)
                },
            )
            .unwrap();
    }
    assert_export_rejected_unchanged(
        &world,
        &["level"],
        ErrorCode::EmptyResult,
        &["profile", "rail"],
    );
}

#[test]
fn empty_assembly_is_empty_result_and_leaves_the_graph_unchanged() {
    let mut world = graph();
    world.create_system_assembly(named("level")).unwrap();
    assert_export_rejected_unchanged(&world, &["level"], ErrorCode::EmptyResult, &[]);
}

fn edited_fixture(name: &str, edit: impl FnOnce(&mut Value)) -> BrepEnvelope {
    let mut body: Value =
        serde_json::from_str(&fixture_text(&format!("{name}.brep.json")).unwrap()).unwrap();
    edit(&mut body);
    BrepEnvelope::from_json(&body.to_string()).unwrap_or_else(|error| panic!("{error}"))
}

fn assert_export_rejected(brep: &BrepEnvelope, matches: fn(&GeometryError) -> bool) {
    let before = brep.to_json().unwrap();
    let error = export_step(brep, "metre").unwrap_err();
    assert!(matches(&error), "{error}");
    assert_eq!(brep.to_json().unwrap(), before);
}

#[test]
fn face_key_over_4_kib_is_limit_exceeded() {
    let over = edited_fixture("cuboid", |body| {
        body["topology"]["faces"][0]["key"] = json!("k".repeat(4097));
    });
    assert_export_rejected(&over, |error| {
        matches!(error, GeometryError::LimitExceeded(_))
    });
    let at_limit = edited_fixture("cuboid", |body| {
        body["topology"]["faces"][0]["key"] = json!("k".repeat(4096));
    });
    assert!(export_step(&at_limit, "metre").is_ok());
}

#[test]
fn seam_edge_with_a_projected_pcurve_is_unsupported_geometry() {
    let brep = edited_fixture("cylinder", |body| {
        assert_eq!(body["topology"]["edges"][2]["chart_seam"], true);
        assert_eq!(body["topology"]["halfedges"][1]["edge"], 2);
        body["geometry"]["pcurves"][1] = json!({
            "kind": "ProjectedCurve",
            "curve": 2,
            "surface": 0,
            "chart": 0,
            "uv_hint": [TAU, 0.0],
            "uv_rate": [0.0, 1.0],
            "parameter_origin": 0.0,
        });
    });
    assert_export_rejected(
        &brep,
        |error| matches!(error, GeometryError::UnsupportedGeometry(message) if message.contains("seam")),
    );
}
