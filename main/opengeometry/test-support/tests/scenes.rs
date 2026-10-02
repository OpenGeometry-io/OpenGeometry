use opengeometry::world_graph::StepOptions;
use opengeometry_test_support::part21::Document;
use opengeometry_test_support::scenes::acceptance::{acceptance_scene, level_export};
use opengeometry_test_support::scenes::storey::{build_storey, openings, StoreySize};
use std::collections::BTreeSet;

#[test]
fn acceptance_scene_exports_twelve_products_and_skips_three_wires_byte_identically() {
    let scene = acceptance_scene();
    let (text, report) = level_export(&scene, "millimetre", "Z");
    let (again, _) = level_export(&scene, "millimetre", "Z");
    assert!(text == again, "two level exports differ");
    assert_eq!(report.products, 12);
    let bodies: Vec<&str> = report
        .bodies
        .iter()
        .map(|body| body.og_id.as_str())
        .collect();
    let mut expected = vec!["wall-1".to_string()];
    expected.extend((1..=11).map(|index| format!("rail-{index}")));
    assert_eq!(bodies, expected);
    let skipped: Vec<&str> = report
        .skipped
        .iter()
        .map(|item| item.og_id.as_str())
        .collect();
    assert_eq!(skipped, ["wall-profile", "rail-path", "rail-disc"]);
    Document::parse(&text).unwrap();
}

#[test]
fn acceptance_scene_rails_each_export_pcurveless_edges() {
    let scene = acceptance_scene();
    let (_, report) = level_export(&scene, "millimetre", "Z");
    let rails: Vec<usize> = report
        .bodies
        .iter()
        .filter(|body| body.og_id.starts_with("rail-"))
        .map(|body| body.pcurveless_edges)
        .collect();
    assert_eq!(rails.len(), 11);
    assert!(rails.iter().all(|edges| *edges > 0), "{rails:?}");
}

#[test]
fn storey_walls_are_distinct_shapes_with_four_measured_openings() {
    let mut scene = acceptance_scene();
    let size = StoreySize {
        walls: 3,
        rail_instances: 4,
    };
    let storey = build_storey(&mut scene, size);
    let shapes: BTreeSet<String> = storey
        .walls
        .iter()
        .map(|wall| scene.graph.brep(wall).unwrap().id.clone())
        .collect();
    assert_eq!(shapes.len(), 3);
    for wall in &storey.walls {
        assert_eq!(openings(&scene.graph.brep(wall).unwrap()), 4, "{wall}");
    }
    let nodes: Vec<String> = storey.walls.iter().chain(&storey.rails).cloned().collect();
    let (text, report) = scene
        .graph
        .export_step(&nodes, &StepOptions::default())
        .unwrap();
    assert_eq!(report.products, 7);
    Document::parse(&text).unwrap();
}
