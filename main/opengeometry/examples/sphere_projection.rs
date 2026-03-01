use std::env;

use opengeometry::export::pdf::{export_scene_to_pdf_with_config, PdfExportConfig};
use opengeometry::export::projection::{CameraParameters, HlrOptions, ProjectionMode};
use opengeometry::primitives::sphere::OGSphere;
use openmaths::Vector3;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let output_path = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "sphere_projection.pdf".to_string());

    let mut sphere = OGSphere::new("sphere-projection".to_string());
    sphere.set_config(Vector3::new(0.0, 0.0, 0.0), 1.4, 36, 20);

    println!("Sphere vertices: {}", sphere.brep().vertices.len());
    println!("Sphere edges: {}", sphere.brep().edges.len());
    println!("Sphere faces: {}", sphere.brep().faces.len());

    let camera = CameraParameters {
        position: Vector3::new(5.0, 4.0, 5.0),
        target: Vector3::new(0.0, 0.0, 0.0),
        up: Vector3::new(0.0, 1.0, 0.0),
        near: 0.1,
        projection_mode: ProjectionMode::Perspective,
    };

    let hlr = HlrOptions {
        hide_hidden_edges: false,
    };

    let scene = sphere.to_projected_scene2d(&camera, &hlr);
    let mut config = PdfExportConfig::a4_landscape();
    config.title = Some("Sphere Projection".to_string());

    export_scene_to_pdf_with_config(&scene, &output_path, &config)?;
    println!("Generated {}", output_path);

    Ok(())
}
