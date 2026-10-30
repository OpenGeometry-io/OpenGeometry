use crate::support::export_matched;
use opengeometry::brep::{mirrored, Accuracy, Frame3};
use opengeometry::exchange::ifc_body::export_ifc_body;
use opengeometry::primitives;
use opengeometry::primitives::{ProfileSegment, SegmentArc};
use opengeometry::world_graph::{
    CreatingOperation, EditScope, MirrorPlane, ModifyingOperation, Primitive, StepOptions,
    Transform, WorldGraph,
};
use opengeometry_test_support::world_graph::{graph, named};
use serde_json::Value;

fn oblique() -> MirrorPlane {
    MirrorPlane {
        origin: [0.3, -0.2, 0.5],
        normal: [1.0, 2.0, -0.5],
    }
}

fn check_every_edge_has_pcurves(world: &WorldGraph, og_id: &str) {
    let exported = export_matched(world, &[og_id], &StepOptions::default());
    assert_eq!(exported.report["solids"], 1);
    assert_eq!(exported.report["pcurvelessEdges"], 0);
    assert_eq!(exported.document.pcurveless_edges().unwrap(), 0);
}

#[test]
fn a_mirrored_arc_edged_extrusion_exports_with_a_pcurve_on_every_edge() {
    let mut world = graph();
    let edge = |to: [f64; 2]| ProfileSegment { to, arc: None };
    let outline = Primitive::Profile {
        start: [0.0, 0.0],
        segments: vec![
            edge([0.6, 0.0]),
            ProfileSegment {
                to: [0.6, 0.3],
                arc: Some(SegmentArc {
                    center: [0.6, 0.15],
                    counter_clockwise: true,
                }),
            },
            edge([0.0, 0.3]),
            edge([0.0, 0.0]),
        ],
        closed: true,
    };
    world.create_primitive(outline, named("outline")).unwrap();
    let extrude = CreatingOperation::Extrude {
        profile: "outline".into(),
        holes: Vec::new(),
        distance: 0.2,
    };
    world.create_operation(extrude, named("arm")).unwrap();
    world.mirror("arm", oblique(), EditScope::Node).unwrap();
    check_every_edge_has_pcurves(&world, "arm");
}

#[test]
fn a_mirrored_box_minus_cylinder_exports_with_a_pcurve_on_every_edge() {
    let mut world = graph();
    let plate = Primitive::Cuboid {
        width: 2.0,
        height: 1.0,
        depth: 2.0,
    };
    world.create_primitive(plate, named("plate")).unwrap();
    let bore = Primitive::Cylinder {
        radius: 0.4,
        height: 2.0,
    };
    world.create_primitive(bore, named("bore")).unwrap();
    let offset = [0.3, -0.5, 0.2];
    world
        .transform("bore", Transform::Translate { offset })
        .unwrap();
    world
        .operate(
            "plate",
            ModifyingOperation::Subtract,
            &["bore".into()],
            EditScope::Node,
        )
        .unwrap();
    world.mirror("plate", oblique(), EditScope::Node).unwrap();
    check_every_edge_has_pcurves(&world, "plate");
}

fn entity_counts(value: &Value) -> Vec<(String, usize)> {
    let mut counts = std::collections::BTreeMap::new();
    for entity in value["entities"].as_array().unwrap() {
        *counts
            .entry(entity["entity_type"].as_str().unwrap().to_string())
            .or_insert(0) += 1;
    }
    counts.into_iter().collect()
}

#[test]
fn a_mirrored_cone_exports_an_ifc_body_of_the_same_entities() {
    let accuracy = Accuracy {
        geometric: 1e-8,
        intersection: 1e-9,
        tessellation: 0.01,
        exchange: 1e-6,
    };
    let cone = primitives::cone("cone".into(), Frame3::IDENTITY, 1.0, 2.0, accuracy).unwrap();
    let plane = oblique();
    let mirror = mirrored(&cone, plane.origin, plane.normal).unwrap();
    let value =
        |body| serde_json::to_value(export_ifc_body(body, Frame3::IDENTITY).unwrap()).unwrap();
    let (before, after) = (value(&cone), value(&mirror));
    assert_eq!(entity_counts(&after), entity_counts(&before));
    assert_eq!(
        after["faces"].as_array().unwrap().len(),
        mirror.topology.faces.len()
    );
    assert_eq!(after["solids"].as_array().unwrap().len(), 1);
}
