use opengeometry::export::projection::{CameraParameters, HlrOptions, ProjectionMode};
use opengeometry::primitives::cuboid::OGCuboid;
use opengeometry::primitives::line::OGLine;
use opengeometry::scenegraph::OGSceneManager;
use openmaths::Vector3;
use std::env;
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let output_prefix = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "scenegraph_projection".to_string());

    let mut manager = OGSceneManager::new();
    let scene_id = manager.create_scene_internal("JSON Inspect Scene");

    let mut line = OGLine::new("line-json".to_string());
    line.set_config(Vector3::new(-1.0, 0.0, 0.0), Vector3::new(1.0, 0.4, 0.6));
    line.generate_geometry();

    let mut cuboid = OGCuboid::new("box-json".to_string());
    cuboid.set_config(Vector3::new(0.0, 0.0, 0.0), 1.8, 1.2, 1.2);

    manager.add_line_to_scene_internal(&scene_id, "line-entity", &line)?;
    manager.add_cuboid_to_scene_internal(&scene_id, "box-entity", &cuboid)?;

    let camera = CameraParameters {
        position: Vector3::new(4.0, 3.0, 4.0),
        target: Vector3::new(0.0, 0.0, 0.0),
        up: Vector3::new(0.0, 1.0, 0.0),
        near: 0.1,
        projection_mode: ProjectionMode::Perspective,
    };

    let hlr = HlrOptions {
        hide_hidden_edges: true,
    };

    let scene_json = manager.project_scene_to_2d_json_pretty(&scene_id, &camera, &hlr)?;
    let lines_json = manager.project_scene_to_2d_lines_json_pretty(&scene_id, &camera, &hlr)?;

    let scene_path = format!("{}_scene2d.json", output_prefix);
    let lines_path = format!("{}_lines2d.json", output_prefix);

    fs::write(&scene_path, scene_json)?;
    fs::write(&lines_path, lines_json)?;

    println!("Generated {}", scene_path);
    println!("Generated {}", lines_path);

    Ok(())
}
