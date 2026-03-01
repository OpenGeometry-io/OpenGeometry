use std::env;

use opengeometry::export::pdf::{export_scene_to_pdf_with_config, PdfExportConfig};
use opengeometry::export::projection::{CameraParameters, HlrOptions, ProjectionMode};
use opengeometry::primitives::polyline::OGPolyline;
use opengeometry::primitives::rectangle::OGRectangle;
use opengeometry::primitives::sweep::OGSweep;
use openmaths::Vector3;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let output_path = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "./sweep_path_profile.pdf".to_string());

    let mut path = OGPolyline::new("sweep-path".to_string());
    path.set_config(vec![
        Vector3::new(-2.0, 0.0, -1.0),
        Vector3::new(-1.0, 0.6, 0.4),
        Vector3::new(0.2, 1.2, 0.9),
        Vector3::new(1.2, 1.8, 0.2),
        Vector3::new(2.0, 2.2, -0.8),
    ]);

    let mut rectangle_profile = OGRectangle::new("sweep-profile".to_string());
    rectangle_profile.set_config(Vector3::new(0.0, 0.0, 0.0), 0.7, 0.4);
    rectangle_profile.generate_geometry();

    let profile_points: Vec<Vector3> = rectangle_profile
        .brep()
        .vertices
        .iter()
        .map(|vertex| vertex.position)
        .collect();

    let mut sweep = OGSweep::new("sweep".to_string());
    sweep.set_config_with_caps(path.get_raw_points(), profile_points, true, true);

    println!("Sweep vertices: {}", sweep.brep().vertices.len());
    println!("Sweep edges: {}", sweep.brep().edges.len());
    println!("Sweep faces: {}", sweep.brep().faces.len());

    let camera = CameraParameters {
        position: Vector3::new(5.5, 4.0, 5.0),
        target: Vector3::new(0.0, 1.0, 0.0),
        up: Vector3::new(0.0, 1.0, 0.0),
        near: 0.1,
        projection_mode: ProjectionMode::Perspective,
    };

    let hlr = HlrOptions {
        hide_hidden_edges: false,
    };

    let scene = sweep.to_projected_scene2d(&camera, &hlr);

    let mut config = PdfExportConfig::a4_landscape();
    config.title = Some("Sweep Path Profile".to_string());

    export_scene_to_pdf_with_config(&scene, &output_path, &config)?;
    println!("Generated {}", output_path);

    Ok(())
}
