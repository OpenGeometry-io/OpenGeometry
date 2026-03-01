use std::env;

use opengeometry::export::pdf::PdfExportConfig;
use opengeometry::export::projection::{CameraParameters, HlrOptions, ProjectionMode};
use opengeometry::primitives::polygon::OGPolygon;
use opengeometry::primitives::polyline::OGPolyline;
use opengeometry::scenegraph::OGSceneManager;
use openmaths::Vector3;

const EPSILON: f64 = 1.0e-9;

fn are_close_3d(a: Vector3, b: Vector3) -> bool {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    let dz = a.z - b.z;
    (dx * dx + dy * dy + dz * dz) <= EPSILON * EPSILON
}

fn build_wall_outline(left_offset: &[Vector3], right_offset: &[Vector3]) -> Vec<Vector3> {
    if left_offset.is_empty() || right_offset.is_empty() {
        return Vec::new();
    }

    let mut outline: Vec<Vector3> = left_offset.to_vec();

    let mut right_reversed = right_offset.to_vec();
    right_reversed.reverse();
    outline.extend(right_reversed);

    if outline.len() > 2 && are_close_3d(outline[0], outline[outline.len() - 1]) {
        outline.pop();
    }

    outline
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let output_path = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "./wall_from_polyline_offsets.pdf".to_string());

    let mut centerline = OGPolyline::new("wall-centerline".to_string());
    centerline.set_config(vec![
        Vector3::new(-2.6, 0.0, -1.9),
        Vector3::new(-1.2, 0.0, -1.0),
        Vector3::new(-0.2, 0.0, 0.1),
        Vector3::new(0.6, 0.0, 0.2),
        Vector3::new(0.1, 0.0, 1.0),
        Vector3::new(2.5, 0.0, 2.0),
    ]);

    let wall_thickness = 0.45;
    let half = wall_thickness * 0.5;
    let acute_threshold = 90.0;
    let bevel = true;

    let left_offset = centerline.get_offset_result(half, acute_threshold, bevel);
    let right_offset = centerline.get_offset_result(-half, acute_threshold, bevel);

    println!(
        "Left bevel vertices: {:?}",
        left_offset.beveled_vertex_indices
    );
    println!(
        "Right bevel vertices: {:?}",
        right_offset.beveled_vertex_indices
    );

    let mut left_polyline = OGPolyline::new("wall-left-offset".to_string());
    left_polyline.set_config(left_offset.points.clone());

    let mut right_polyline = OGPolyline::new("wall-right-offset".to_string());
    right_polyline.set_config(right_offset.points.clone());

    let wall_outline = build_wall_outline(&left_offset.points, &right_offset.points);
    if wall_outline.len() < 3 {
        return Err("Failed to create wall outline from offsets".into());
    }

    let mut wall_polygon = OGPolygon::new("wall-polygon".to_string());
    wall_polygon.set_config(wall_outline.clone());

    let mut manager = OGSceneManager::new();
    let scene_id = manager.create_scene_internal("wall-from-offsets");

    manager.add_polyline_to_scene_internal(&scene_id, "entity-centerline", &centerline)?;
    manager.add_polyline_to_scene_internal(&scene_id, "entity-left-offset", &left_polyline)?;
    manager.add_polyline_to_scene_internal(&scene_id, "entity-right-offset", &right_polyline)?;
    manager.add_polygon_to_scene_internal(&scene_id, "entity-wall-polygon", &wall_polygon)?;

    let camera = CameraParameters {
        position: Vector3::new(5.8, 4.2, 5.6),
        target: Vector3::new(0.0, 0.0, 0.0),
        up: Vector3::new(0.0, 1.0, 0.0),
        near: 0.1,
        projection_mode: ProjectionMode::Perspective,
    };

    let hlr = HlrOptions {
        hide_hidden_edges: false,
    };

    manager.project_scene_to_pdf_with_camera(
        &scene_id,
        &camera,
        &hlr,
        &output_path,
        &PdfExportConfig::a4_landscape(),
    )?;

    println!("Generated {}", output_path);
    Ok(())
}
