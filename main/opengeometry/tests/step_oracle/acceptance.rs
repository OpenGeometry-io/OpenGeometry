use crate::support::export_matched;
use opengeometry::world_graph::StepOptions;
use opengeometry_test_support::part21::Document;
use opengeometry_test_support::scenes::acceptance::{acceptance_scene, level_export};
use opengeometry_test_support::tolerance::ordered_bits;
use serde_json::Value;

type Extents = [f64; 6];

#[test]
fn acceptance_level_passes_the_oracle_in_metre_and_millimetre_with_y_and_z_up() {
    let scene = acceptance_scene();
    let mut products = vec!["wall-1".to_string()];
    products.extend((1..=11).map(|index| format!("rail-{index}")));
    let quoted: Vec<String> = products.iter().map(|og_id| format!("'{og_id}'")).collect();
    for unit in ["metre", "millimetre"] {
        for up_axis in ["Y", "Z"] {
            let exported = export_matched(&scene.graph, &[&scene.level], &options(unit, up_axis));
            assert_eq!(
                og_ids(&exported.report["bodies"]),
                products,
                "{unit} {up_axis}"
            );
            assert_eq!(
                og_ids(&exported.report["skipped"]),
                ["wall-profile", "rail-path", "rail-disc"],
                "{unit} {up_axis}"
            );
            assert_eq!(
                exported.document.product_names().unwrap(),
                quoted,
                "{unit} {up_axis}"
            );
        }
    }
}

#[test]
fn acceptance_rails_each_parse_pcurveless_edges() {
    let scene = acceptance_scene();
    let (text, _) = level_export(&scene, "millimetre", "Z");
    let document = Document::parse(&text).unwrap();
    let counts = document.product_counts().unwrap();
    let names = document.product_names().unwrap();
    let rails: Vec<usize> = counts
        .iter()
        .zip(&names)
        .filter(|(_, name)| name.starts_with("'rail-"))
        .map(|(product, _)| product.pcurveless_edges)
        .collect();
    assert_eq!(rails.len(), 11, "{names:?}");
    assert!(rails.iter().all(|edges| *edges > 0), "{rails:?}");
    let total: usize = counts.iter().map(|product| product.pcurveless_edges).sum();
    assert_eq!(total, document.pcurveless_edges().unwrap());
}

#[test]
fn acceptance_z_up_millimetre_extents_are_the_y_up_metre_extents_mapped() {
    let scene = acceptance_scene();
    let (z_up, report) = level_export(&scene, "millimetre", "Z");
    let (y_up, _) = level_export(&scene, "metre", "Y");
    let z_up = vertex_extents(&Document::parse(&z_up).unwrap());
    let y_up = vertex_extents(&Document::parse(&y_up).unwrap());
    assert_within_one_ulp(z_up, mapped(y_up));
    let hull = report
        .bodies
        .iter()
        .map(|body| mapped(scene.graph.bounds(&body.og_id).unwrap().unwrap()))
        .reduce(union)
        .unwrap();
    assert_inside(z_up, hull);
    let wall = export_matched(&scene.graph, &[&scene.wall], &StepOptions::default());
    assert_within_one_ulp(
        vertex_extents(&wall.document),
        [-3000.0, -100.0, 3000.0, 3000.0, 100.0, 7000.0],
    );
}

fn options(unit: &str, up_axis: &str) -> StepOptions {
    StepOptions {
        unit: unit.into(),
        up_axis: up_axis.into(),
        ..StepOptions::default()
    }
}

fn og_ids(items: &Value) -> Vec<String> {
    items
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["ogId"].as_str().unwrap().to_string())
        .collect()
}

fn vertex_extents(document: &Document) -> Extents {
    let points = document.vertex_positions().unwrap();
    assert!(!points.is_empty());
    points
        .iter()
        .map(|point| [point[0], point[1], point[2], point[0], point[1], point[2]])
        .reduce(union)
        .unwrap()
}

fn union(left: Extents, right: Extents) -> Extents {
    [
        left[0].min(right[0]),
        left[1].min(right[1]),
        left[2].min(right[2]),
        left[3].max(right[3]),
        left[4].max(right[4]),
        left[5].max(right[5]),
    ]
}

fn mapped(metres_y_up: Extents) -> Extents {
    let [xmin, ymin, zmin, xmax, ymax, zmax] = metres_y_up;
    [xmin, -zmax, ymin, xmax, -zmin, ymax].map(|value| value * 1000.0)
}

fn assert_within_one_ulp(actual: Extents, expected: Extents) {
    let close = actual
        .iter()
        .zip(&expected)
        .all(|(a, b)| ordered_bits(*a).abs_diff(ordered_bits(*b)) <= 1);
    assert!(
        close,
        "extents {actual:?} differ from {expected:?} by more than 1 ULP"
    );
}

fn assert_inside(extents: Extents, hull: Extents) {
    let inside = (0..3).all(|axis| {
        ordered_bits(extents[axis]) + 1 >= ordered_bits(hull[axis])
            && ordered_bits(extents[axis + 3]) <= ordered_bits(hull[axis + 3]) + 1
    });
    assert!(
        inside,
        "extents {extents:?} leave the exported bodies' hull {hull:?}"
    );
}
