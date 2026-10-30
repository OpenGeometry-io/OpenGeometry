use crate::body::record_body;
use crate::graph_case::{create_options, graph_case};
use crate::graph_record::{changes, node_state};
use crate::kernel::{
    classify_point, BodyType, BrepEnvelope, CopyOptions, EditScope, ModifyingOperation, Point3,
    Primitive, Transform, WorldGraph,
};
use crate::record::{Failure, Record};
use crate::runner::Case;
use opengeometry::brep::mirrored;
use opengeometry::tessellation::measure;
use opengeometry::world_graph::{ErrorCode, MirrorPlane};
use serde_json::Value;
use std::fs;
use std::path::Path;

const FIXTURES: [&str; 18] = [
    "annular-cylinder",
    "arc-edged-extrusion",
    "arc-edged-extrusion-with-holes",
    "box-cavity",
    "box-cut",
    "box-intersection",
    "box-union",
    "circle",
    "cone",
    "cuboid",
    "cylinder",
    "cylinder-with-hole",
    "frustum",
    "linear-extrusion",
    "revolved-bowl",
    "sphere-cut",
    "sphere",
    "torus",
];
const OBLIQUE: (Point3, Point3) = ([0.3, -0.2, 0.5], [1.0, 2.0, -0.5]);
const PLANES: [(Point3, Point3); 2] = [OBLIQUE, ([0.0; 3], [1.0, 0.0, 0.0])];
const GRID_STEPS: usize = 5;

fn fixture(name: &str) -> BrepEnvelope {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/cases")
        .join(format!("{name}.brep.json"));
    BrepEnvelope::from_json(&fs::read_to_string(path).unwrap()).unwrap()
}

fn reflected(point: Point3, (origin, normal): (Point3, Point3)) -> Point3 {
    let length = normal.iter().map(|value| value * value).sum::<f64>().sqrt();
    let unit = normal.map(|value| value / length);
    let distance = (0..3)
        .map(|axis| (point[axis] - origin[axis]) * unit[axis])
        .sum::<f64>();
    std::array::from_fn(|axis| point[axis] - 2.0 * distance * unit[axis])
}

fn grid(body: &BrepEnvelope) -> Vec<Point3> {
    let bounds = body.bounds().unwrap().unwrap();
    let at = |axis: usize, step: usize| {
        let span = bounds.axes[axis].hi - bounds.axes[axis].lo;
        bounds.axes[axis].lo - 0.1 * span + 1.2 * span * step as f64 / GRID_STEPS as f64
    };
    let steps = 0..=GRID_STEPS;
    steps
        .clone()
        .flat_map(|i| steps.clone().map(move |j| (i, j)))
        .flat_map(|(i, j)| (0..=GRID_STEPS).map(move |k| [at(0, i), at(1, j), at(2, k)]))
        .collect()
}

fn check_volume(name: &str, body: &BrepEnvelope, mirror: &BrepEnvelope) {
    let before = measure(body, 0.01).unwrap();
    let after = measure(mirror, 0.01).unwrap();
    assert!(after.volume > 0.0, "{name}: {}", after.volume);
    assert!(
        (after.volume - before.volume).abs() <= before.error_bound + after.error_bound,
        "{name}: {} against {}",
        after.volume,
        before.volume
    );
}

fn check_reflected_grid(
    name: &str,
    body: &BrepEnvelope,
    mirror: &BrepEnvelope,
    plane: (Point3, Point3),
) {
    for point in grid(body) {
        assert_eq!(
            classify_point(mirror, reflected(point, plane)).ok(),
            classify_point(body, point).ok(),
            "{name} at {point:?}"
        );
    }
}

fn check_vertices(name: &str, body: &BrepEnvelope, mirror: &BrepEnvelope, plane: (Point3, Point3)) {
    assert_eq!(mirror.topology.vertices.len(), body.topology.vertices.len());
    for (before, after) in body.topology.vertices.iter().zip(&mirror.topology.vertices) {
        let expected = reflected(before.position, plane);
        let distance = (0..3)
            .map(|axis| (after.position[axis] - expected[axis]).powi(2))
            .sum::<f64>()
            .sqrt();
        assert!(distance <= 1e-12, "{name}: {:?}", after.position);
    }
}

#[test]
fn every_fixture_mirrored_is_valid_keeps_its_volume_and_classifies_the_reflected_grid_alike() {
    for name in FIXTURES {
        let body = fixture(name);
        for plane in PLANES {
            let mirror = mirrored(&body, plane.0, plane.1)
                .unwrap_or_else(|error| panic!("{name}: {error:?}"));
            mirror.validate().unwrap();
            assert_eq!(mirror.revision, body.revision + 1, "{name}");
            assert_eq!(
                mirror.body_type().unwrap(),
                body.body_type().unwrap(),
                "{name}"
            );
            check_vertices(name, &body, &mirror, plane);
            if body.body_type().unwrap() == BodyType::Solid {
                check_volume(name, &body, &mirror);
                check_reflected_grid(name, &body, &mirror, plane);
            }
        }
    }
}

fn faces(body: &BrepEnvelope) -> Vec<Value> {
    let value: Value = serde_json::from_str(&body.to_json().unwrap()).unwrap();
    value["topology"]["faces"].as_array().unwrap().clone()
}

#[test]
fn a_mirrored_body_keeps_its_face_keys_and_sources_and_toggles_reversed() {
    for name in ["box-cut", "sphere-cut", "revolved-bowl"] {
        let body = fixture(name);
        let mirror = mirrored(&body, OBLIQUE.0, OBLIQUE.1).unwrap();
        let (before, after) = (faces(&body), faces(&mirror));
        assert_eq!(after.len(), before.len(), "{name}");
        for (old, new) in before.iter().zip(&after) {
            assert_eq!(new["key"], old["key"], "{name}");
            assert_eq!(
                new["provenance"]["sources"], old["provenance"]["sources"],
                "{name}"
            );
            assert_eq!(
                new["provenance"]["role"], old["provenance"]["role"],
                "{name}"
            );
            assert_eq!(
                new["provenance"]["reversed"].as_bool(),
                old["provenance"]["reversed"]
                    .as_bool()
                    .map(|reversed| !reversed),
                "{name}"
            );
        }
    }
}

#[test]
fn a_plane_without_a_finite_unit_normal_is_refused() {
    let body = fixture("cuboid");
    for (origin, normal) in [
        ([0.0; 3], [0.0; 3]),
        ([0.0; 3], [f64::NAN, 0.0, 1.0]),
        ([f64::INFINITY, 0.0, 0.0], [0.0, 0.0, 1.0]),
    ] {
        assert!(
            mirrored(&body, origin, normal).is_err(),
            "{origin:?} {normal:?}"
        );
    }
}

fn plane((origin, normal): (Point3, Point3)) -> MirrorPlane {
    MirrorPlane { origin, normal }
}

fn cone_graph() -> WorldGraph {
    let mut graph = opengeometry_test_support::world_graph::graph();
    let cone = Primitive::Cone {
        radius_bottom: 0.5,
        radius_top: 0.0,
        height: 1.5,
    };
    graph
        .create_primitive(cone, create_options("cone"))
        .unwrap();
    graph
        .transform(
            "cone",
            Transform::Translate {
                offset: [1.0, 2.0, 3.0],
            },
        )
        .unwrap();
    graph
}

#[test]
fn mirroring_in_the_graph_rebuilds_the_shape_in_its_own_coordinates_and_keeps_the_placement() {
    let mut graph = cone_graph();
    let placement = graph.placement("cone").unwrap();
    let before = graph.brep("cone").unwrap();
    graph
        .mirror("cone", plane(OBLIQUE), EditScope::Node)
        .unwrap();
    let after = graph.brep("cone").unwrap();
    assert_eq!(graph.placement("cone").unwrap(), placement);
    assert_eq!(after.revision, before.revision + 1);
    assert_eq!(graph.shape("cone").unwrap().revision, after.revision);
    let expected = mirrored(&before, OBLIQUE.0, OBLIQUE.1).unwrap();
    assert_eq!(after.topology, expected.topology);
    assert_eq!(after.geometry, expected.geometry);
    assert!(graph.report("cone").unwrap().is_none());
}

#[test]
fn mirroring_a_boolean_result_clears_its_report() {
    let mut graph = opengeometry_test_support::world_graph::graph();
    let cuboid = |size: f64| Primitive::Cuboid {
        width: size,
        height: size,
        depth: size,
    };
    graph
        .create_primitive(cuboid(2.0), create_options("block"))
        .unwrap();
    graph
        .create_primitive(cuboid(1.0), create_options("notch"))
        .unwrap();
    graph
        .operate(
            "block",
            ModifyingOperation::Subtract,
            &["notch".into()],
            EditScope::Node,
        )
        .unwrap();
    assert!(graph.report("block").unwrap().is_some());
    graph
        .mirror("block", plane(OBLIQUE), EditScope::Node)
        .unwrap();
    assert!(graph.report("block").unwrap().is_none());
}

#[test]
fn a_shared_shape_mirrors_only_with_all_instances_and_a_bad_plane_is_invalid_parameter() {
    let mut graph = cone_graph();
    let copy = CopyOptions {
        og_id: Some("cone-copy".into()),
        ..CopyOptions::default()
    };
    graph.instance("cone", copy).unwrap();
    let revision = graph.revision();
    let shared = graph
        .mirror("cone", plane(OBLIQUE), EditScope::Node)
        .unwrap_err();
    assert_eq!(shared.error_code(), ErrorCode::SharedShape);
    for bad in [
        ([0.0; 3], [0.0; 3]),
        ([f64::NAN, 0.0, 0.0], [0.0, 0.0, 1.0]),
    ] {
        let error = graph
            .mirror("cone", plane(bad), EditScope::AllInstances)
            .unwrap_err();
        assert_eq!(error.error_code(), ErrorCode::InvalidParameter);
    }
    assert_eq!(graph.revision(), revision);
    graph
        .mirror("cone", plane(OBLIQUE), EditScope::AllInstances)
        .unwrap();
    assert_eq!(
        graph.brep("cone").unwrap().to_json().unwrap(),
        graph.brep("cone-copy").unwrap().to_json().unwrap()
    );
    assert_eq!(
        graph.node("cone").unwrap().shape,
        graph.node("cone-copy").unwrap().shape
    );
}

fn fixture_case(name: &'static str) -> Case {
    Case::new(format!("mirror.{name}"), move |record| {
        match mirrored(&fixture(name), OBLIQUE.0, OBLIQUE.1) {
            Ok(mirror) => record_body(record, "mirrored", &mirror),
            Err(error) => record.error("mirrored", error),
        }
        Ok(())
    })
}

fn graph_mirror(graph: &mut WorldGraph, record: &mut Record) -> Result<(), Failure> {
    let cone = Primitive::Cone {
        radius_bottom: 0.5,
        radius_top: 0.0,
        height: 1.5,
    };
    graph.create_primitive(cone, create_options("cone"))?;
    changes(
        record,
        "mirror",
        graph.mirror("cone", plane(OBLIQUE), EditScope::Node),
    );
    let bad = plane(([0.0; 3], [0.0; 3]));
    changes(
        record,
        "zero-normal",
        graph.mirror("cone", bad, EditScope::Node),
    );
    node_state(record, graph, "cone");
    let cone = graph.brep("cone")?;
    record_body(record, "cone", &cone);
    Ok(())
}

pub(crate) fn cases() -> Vec<Case> {
    let mut cases = FIXTURES.into_iter().map(fixture_case).collect::<Vec<_>>();
    cases.push(graph_case("mirror.graph", graph_mirror));
    cases
}
