use opengeometry::world_graph::{ErrorCode, ErrorDetails};
use opengeometry_test_support::scenes::acceptance::{attempt_rail_minus_wall, recut_scene};
use opengeometry_test_support::scenes::coverage_gap::rail_minus_wall_clearance;

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
    assert_eq!(
        error.details(),
        ErrorDetails::Operate {
            handlers: Vec::new(),
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
