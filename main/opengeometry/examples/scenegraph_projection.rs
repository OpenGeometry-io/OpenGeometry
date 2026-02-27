use opengeometry::export::pdf::PdfExportConfig;
use opengeometry::export::projection::{CameraParameters, HlrOptions, ProjectionMode};
use opengeometry::primitives::cuboid::OGCuboid;
use opengeometry::primitives::line::OGLine;
use opengeometry::primitives::polyline::OGPolyline;
use opengeometry::scenegraph::OGSceneManager;
use openmaths::Vector3;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let output_path = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "scenegraph_projection.pdf".to_string());

    let mut manager = OGSceneManager::new();
    let scene_id = manager.create_scene_internal("Main Scene");

    let mut line = OGLine::new("line-01".to_string());
    line.set_config(Vector3::new(-1.4, 0.0, -0.2), Vector3::new(1.4, 0.4, 0.5));
    line.generate_geometry();

    let mut polyline = OGPolyline::new("polyline-01".to_string());
    polyline.set_config(vec![
        Vector3::new(-1.8, 0.0, -1.2),
        Vector3::new(-0.8, 0.1, -0.2),
        Vector3::new(0.1, 0.0, -0.8),
        Vector3::new(1.1, 0.3, -0.1),
    ]);

    let mut cuboid = OGCuboid::new("cuboid-01".to_string());
    cuboid.set_config(Vector3::new(0.0, 0.0, 0.5), 1.8, 1.2, 1.2);

    manager.add_line_to_scene_internal(&scene_id, "entity-line", &line)?;
    manager.add_polyline_to_scene_internal(&scene_id, "entity-polyline", &polyline)?;
    manager.add_cuboid_to_scene_internal(&scene_id, "entity-cuboid", &cuboid)?;

    let camera = CameraParameters {
        position: Vector3::new(4.5, 3.5, 4.0),
        target: Vector3::new(0.0, 0.0, 0.0),
        up: Vector3::new(0.0, 1.0, 0.0),
        near: 0.1,
        projection_mode: ProjectionMode::Perspective,
    };

    let hlr = HlrOptions {
        hide_hidden_edges: true,
    };

    manager.project_scene_to_pdf_with_camera(
        &scene_id,
        &camera,
        &hlr,
        &output_path,
        &PdfExportConfig::a4_landscape(),
    )?;

    let projected_json = manager.project_scene_to_2d_json(&scene_id, &camera, &hlr)?;
    println!("Generated {}", output_path);
    println!("projectTo2DCamera JSON size: {}", projected_json.len());

    Ok(())
}
