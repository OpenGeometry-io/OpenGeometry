use crate::support::{fixture_names, fixture_text};
use opengeometry::brep::{Accuracy, BrepEnvelope, Frame3, GeometryError};
use opengeometry::exchange::ifc_body::{export_ifc_body, AnalyticExchangeBodyV2};
use opengeometry::primitives;
use opengeometry::world_graph::{
    CreateOptions, CreatingOperation, ErrorCode, Primitive, StepOptions, Transform, WorldGraph,
};
use opengeometry_test_support::stored;
use opengeometry_test_support::world_graph::{graph, named};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const Z_UP: Frame3 = Frame3 {
    origin: [0.0; 3],
    x: [1.0, 0.0, 0.0],
    y: [0.0, 0.0, 1.0],
    z: [0.0, -1.0, 0.0],
};

fn exported(body: &BrepEnvelope) -> Result<(String, Value), GeometryError> {
    let output = export_ifc_body(body, Z_UP)?;
    let text = serde_json::to_string(&output).unwrap();
    let value = serde_json::from_str(&text).unwrap();
    Ok((text, value))
}

fn entity_types(value: &Value) -> BTreeMap<String, u64> {
    let mut counts = BTreeMap::new();
    for entity in value["entities"].as_array().unwrap() {
        *counts
            .entry(entity["entity_type"].as_str().unwrap().to_string())
            .or_insert(0) += 1;
    }
    counts
}

fn references_point_backwards(value: &Value, before: u64) {
    match value {
        Value::Object(fields) if fields.get("kind") == Some(&Value::from("Ref")) => {
            let id = fields["id"].as_u64().unwrap();
            assert!(id > 0 && id < before, "{id} >= {before}");
        }
        Value::Object(fields) => {
            for value in fields.values() {
                references_point_backwards(value, before);
            }
        }
        Value::Array(values) => {
            for value in values {
                references_point_backwards(value, before);
            }
        }
        _ => {}
    }
}

fn single_body_fixtures() -> Vec<(String, BrepEnvelope)> {
    fixture_names("")
        .into_iter()
        .filter_map(|file| {
            let name = file.strip_suffix(".brep.json")?.to_string();
            Some((
                name,
                BrepEnvelope::from_json(&fixture_text(&file).unwrap()).unwrap(),
            ))
        })
        .collect()
}

#[test]
fn prepared_ifc_has_shared_geometry_backreferences_and_single_axis_conversion() {
    let accuracy = Accuracy {
        geometric: 1e-8,
        intersection: 1e-9,
        tessellation: 0.01,
        exchange: 1e-6,
    };
    let frame = Frame3 {
        origin: [1.0, 2.0, 3.0],
        y: [0.0, 0.0, -1.0],
        z: [0.0, 1.0, 0.0],
        ..Frame3::IDENTITY
    };
    let bodies = [
        primitives::cylinder("c".into(), frame, 1.0, 2.0, accuracy).unwrap(),
        primitives::sphere("s".into(), frame, 1.0, accuracy).unwrap(),
        primitives::cone("cone".into(), frame, 1.0, 2.0, accuracy).unwrap(),
        primitives::torus("t".into(), frame, 2.0, 0.5, accuracy).unwrap(),
    ];
    for input in bodies {
        let original = input.to_json().unwrap();
        let (text, value) = exported(&input).unwrap();
        assert_eq!(
            value["faces"].as_array().unwrap().len(),
            input.topology.faces.len()
        );
        assert_eq!(value["solids"].as_array().unwrap().len(), 1);
        assert_eq!(value["up_axis"], "Z");
        assert_eq!(value["length_unit"], "metre");
        for entity in value["entities"].as_array().unwrap() {
            references_point_backwards(&entity["attributes"], entity["id"].as_u64().unwrap());
        }
        assert!(value["entities"].as_array().unwrap().iter().any(|entity| {
            entity["entity_type"] == "IfcCartesianPoint"
                && entity["attributes"][0]["values"]
                    == serde_json::json!([
                        {"kind": "Real", "value": 1.0},
                        {"kind": "Real", "value": -3.0},
                        {"kind": "Real", "value": 2.0}
                    ])
        }));
        assert!(!entity_types(&value).contains_key("IfcSurfaceCurve"));
        assert_eq!(input.to_json().unwrap(), original);
        serde_json::from_str::<AnalyticExchangeBodyV2>(&text).unwrap();
    }
}

#[test]
fn ifc_bodies_of_the_fixture_bodies_match_the_2_0_18_oracle() {
    let fixtures = single_body_fixtures();
    assert_eq!(fixtures.len(), 17);
    for (name, body) in fixtures {
        let Some(stored_text) = fixture_text(&format!("ifc-body/{name}.ifc.json")) else {
            let error = exported(&body).unwrap_err();
            assert!(
                matches!(error, GeometryError::UnsupportedGeometry(_)),
                "{name}: {error:?}"
            );
            continue;
        };
        assert_matches_oracle(&name, &body, &stored_text);
    }
}

#[test]
fn ellipse_edges_and_a_union_of_solids_match_the_2_0_18_oracle() {
    for (name, entity) in [
        ("circle-sweep-elbow", "IfcEllipse"),
        ("hollow-and-disjoint-union", "IfcBooleanResult"),
    ] {
        let input = fixture_text(&format!("ifc-body/extra/{name}.brep.json")).unwrap();
        let stored_text = fixture_text(&format!("ifc-body/extra/{name}.ifc.json")).unwrap();
        let body = BrepEnvelope::from_json(&input).unwrap();
        assert_matches_oracle(name, &body, &stored_text);
        assert!(stored_text.contains(&format!(r#""entity_type":"{entity}""#)));
    }
    assert!(
        fixture_text("ifc-body/extra/hollow-and-disjoint-union.ifc.json")
            .unwrap()
            .contains(r#""value":"UNION""#)
    );
}

fn assert_matches_oracle(name: &str, body: &BrepEnvelope, stored_text: &str) {
    let stored_value: Value = serde_json::from_str(stored_text).unwrap();
    let (text, value) = exported(body).unwrap_or_else(|error| panic!("{name}: {error:?}"));
    assert_eq!(
        value["representation_type"], stored_value["representation_type"],
        "{name}"
    );
    assert_eq!(entity_types(&value), entity_types(&stored_value), "{name}");
    assert_eq!(value["solids"], stored_value["solids"], "{name}");
    assert_eq!(value["faces"], stored_value["faces"], "{name}");
    for entity in value["entities"].as_array().unwrap() {
        references_point_backwards(&entity["attributes"], entity["id"].as_u64().unwrap());
    }
    if stored::compared() {
        assert_eq!(text, stored_text.trim_end(), "{name}");
    }
}

#[test]
fn ifc_bodies_of_the_matrix_results_match_the_2_0_18_digests() {
    let digests: Value =
        serde_json::from_str(&fixture_text("ifc-body/matrix.json").unwrap()).unwrap();
    let digests = digests.as_object().unwrap();
    assert_eq!(digests.len(), 35);
    for (name, expected) in digests {
        let (folder, case) = name.split_once("--").unwrap();
        let fixture: Value =
            serde_json::from_str(&fixture_text(&format!("{folder}/{case}.json")).unwrap()).unwrap();
        let body: BrepEnvelope = serde_json::from_value(fixture["result"]["brep"].clone()).unwrap();
        let outcome = exported(&body);
        if let Some(error) = expected.get("error") {
            let error_kind = format!("{:?}", outcome.unwrap_err());
            assert!(error_kind.starts_with(error.as_str().unwrap()), "{name}");
            continue;
        }
        let (text, value) = outcome.unwrap_or_else(|error| panic!("{name}: {error:?}"));
        assert_eq!(
            value["representation_type"], expected["representation_type"],
            "{name}"
        );
        assert_eq!(
            serde_json::to_value(entity_types(&value)).unwrap(),
            expected["entity_types"],
            "{name}"
        );
        assert!(value["exchange_error_bound"].as_f64().unwrap() <= body.accuracy.exchange);
        if stored::compared() {
            let digest = format!("{:x}", Sha256::digest(text.as_bytes()));
            assert_eq!(digest, expected["sha256"].as_str().unwrap(), "{name}");
        }
    }
}

fn matrix_body(name: &str) -> BrepEnvelope {
    let fixture: Value =
        serde_json::from_str(&fixture_text(&format!("boolean-matrix/{name}.json")).unwrap())
            .unwrap();
    serde_json::from_value(fixture["result"]["brep"].clone()).unwrap()
}

fn refusal(body: &BrepEnvelope, exchange: f64) -> String {
    let mut body = body.clone();
    body.accuracy.exchange = exchange;
    format!("{:?}", exported(&body).unwrap_err())
}

#[test]
fn the_smallest_budget_past_preflight_leaves_no_room_for_fitted_curves() {
    let body = matrix_body("cylinder-cross");
    let preflight = "exchange budget is below geometric tolerance";
    let mut low = body.accuracy.geometric.to_bits();
    let mut high = (body.accuracy.geometric * 1.001).to_bits();
    assert!(refusal(&body, f64::from_bits(low)).contains(preflight));
    assert!(!refusal(&body, f64::from_bits(high)).contains(preflight));
    while high - low > 1 {
        let middle = low + (high - low) / 2;
        if refusal(&body, f64::from_bits(middle)).contains(preflight) {
            low = middle;
        } else {
            high = middle;
        }
    }
    assert_eq!(
        refusal(&body, f64::from_bits(high)),
        r#"LimitExceeded("IFC exchange budget leaves no room for numerical curve fitting")"#
    );
}

#[test]
fn a_conic_edge_longer_than_one_turn_is_unsupported() {
    let mut value: Value =
        serde_json::from_str(&fixture_text("cylinder.brep.json").unwrap()).unwrap();
    let turn = std::f64::consts::TAU;
    let circles: Vec<u64> = value["geometry"]["curves"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .filter(|(_, curve)| curve["kind"] == "Circle")
        .map(|(index, _)| index as u64)
        .collect();
    for edge in value["topology"]["edges"].as_array_mut().unwrap() {
        if circles.contains(&edge["geometry"]["curve"].as_u64().unwrap_or(u64::MAX)) {
            edge["geometry"]["range"]["hi"] = Value::from(2.0 * turn);
        }
    }
    value["topology"]["faces"][0]["trim"]["uv_bounds"][0]["hi"] = Value::from(2.0 * turn);
    let body = BrepEnvelope::from_json(&value.to_string()).unwrap();
    assert_eq!(
        format!("{:?}", exported(&body).unwrap_err()),
        r#"UnsupportedGeometry("IFC conic edges cannot exceed one period")"#
    );
}

#[test]
fn more_than_100_000_estimated_entities_is_limit_exceeded() {
    let mut world = graph();
    let points = (0..2500)
        .map(|index| {
            let angle = index as f64 * std::f64::consts::TAU / 2500.0;
            [10.0 * angle.cos(), 0.0, 10.0 * angle.sin()]
        })
        .collect();
    world
        .create_primitive(
            Primitive::Polyline {
                points,
                closed: true,
            },
            named("outline"),
        )
        .unwrap();
    world
        .create_operation(
            CreatingOperation::Extrude {
                profile: "outline".into(),
                holes: Vec::new(),
                distance: 1.0,
            },
            named("prism"),
        )
        .unwrap();
    let error = world.export_ifc_body("prism").unwrap_err();
    assert_eq!(error.error_code(), ErrorCode::LimitExceeded);
    assert!(error
        .message()
        .contains("IFC prepared entity limit exceeded"));
}

fn scene() -> WorldGraph {
    let mut world = graph();
    world.create_system_assembly(named("group")).unwrap();
    world
        .create_primitive(
            Primitive::Cuboid {
                width: 2.0,
                height: 1.0,
                depth: 2.0,
            },
            CreateOptions {
                parent: Some("group".into()),
                ..named("box")
            },
        )
        .unwrap();
    world
        .transform(
            "group",
            Transform::Translate {
                offset: [10.0, 3.0, 0.0],
            },
        )
        .unwrap();
    world
        .create_primitive(
            Primitive::Rectangle {
                width: 1.0,
                breadth: 1.0,
            },
            named("profile"),
        )
        .unwrap();
    world
        .create_primitive(
            Primitive::Polyline {
                points: vec![[0.0; 3], [0.0, 2.0, 0.0]],
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
                caps: Some(false),
            },
            named("sheet"),
        )
        .unwrap();
    world
}

fn state(world: &WorldGraph, revision: u64) -> String {
    format!(
        "{} {:?} {} {:?} {}",
        world.revision(),
        world.changes_since(revision).unwrap(),
        world.brep("box").unwrap().to_json().unwrap(),
        world.mark_stats(),
        world
            .export_step(&["group".into()], &StepOptions::default())
            .unwrap()
            .0,
    )
}

#[test]
fn graph_ifc_body_is_world_placed_z_up_and_read_only() {
    let world = scene();
    let revision = world.revision();
    let before = state(&world, revision);
    let body = world.export_ifc_body("box").unwrap();
    let value = serde_json::to_value(&body).unwrap();
    assert_eq!(value["coordinate_space"], "world");
    assert_eq!(value["representation_type"], "AdvancedBrep");
    let points: Vec<Vec<f64>> = value["entities"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entity| entity["entity_type"] == "IfcCartesianPoint")
        .filter_map(|entity| {
            let values = entity["attributes"][0]["values"].as_array()?;
            (values.len() == 3).then(|| {
                values
                    .iter()
                    .map(|v| v["value"].as_f64().unwrap())
                    .collect()
            })
        })
        .collect();
    assert!(points.iter().any(|point| point[0] >= 10.5));
    assert!(points.iter().any(|point| point[2] >= 3.5));
    assert!(points.iter().all(|point| point[1].abs() <= 1.5));
    assert_eq!(state(&world, revision), before);
}

#[test]
fn graph_ifc_body_refuses_sheets_wires_assemblies_and_unknown_nodes() {
    let world = scene();
    for (og_id, code) in [
        ("sheet", ErrorCode::InvalidOperand),
        ("profile", ErrorCode::InvalidOperand),
        ("group", ErrorCode::InvalidOperand),
        ("missing", ErrorCode::UnknownNode),
    ] {
        assert_eq!(
            world.export_ifc_body(og_id).unwrap_err().error_code(),
            code,
            "{og_id}"
        );
    }
}
