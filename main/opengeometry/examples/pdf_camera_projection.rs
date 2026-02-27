use std::env;

use opengeometry::export::pdf::{export_brep_to_pdf_with_camera, PdfExportConfig};
use opengeometry::export::projection::{CameraParameters, HlrOptions, ProjectionMode};
use opengeometry::primitives::cuboid::OGCuboid;
use openmaths::Vector3;

fn parse_or_default(args: &[String], index: usize, default_value: f64) -> f64 {
    args.get(index)
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(default_value)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let output_path = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "pdf_camera_projection.pdf".to_string());

    let camera = CameraParameters {
        position: Vector3::new(
            parse_or_default(&args, 2, 4.0),
            parse_or_default(&args, 3, 3.0),
            parse_or_default(&args, 4, 4.0),
        ),
        target: Vector3::new(
            parse_or_default(&args, 5, 0.0),
            parse_or_default(&args, 6, 0.0),
            parse_or_default(&args, 7, 0.0),
        ),
        up: Vector3::new(
            parse_or_default(&args, 8, 0.0),
            parse_or_default(&args, 9, 1.0),
            parse_or_default(&args, 10, 0.0),
        ),
        near: parse_or_default(&args, 11, 0.1),
        projection_mode: ProjectionMode::Perspective,
    };

    let hlr = HlrOptions {
        hide_hidden_edges: true,
    };

    let mut cuboid = OGCuboid::new("perspective-cuboid".to_string());
    cuboid.set_config(Vector3::new(0.0, 0.0, 0.0), 2.2, 1.4, 1.8);

    let mut config = PdfExportConfig::a4_landscape();
    config.title = Some("Perspective Camera Projection".to_string());

    export_brep_to_pdf_with_camera(cuboid.brep(), &camera, &hlr, &output_path, &config)?;

    println!("Generated {}", output_path);
    println!(
    "Camera args: <output.pdf> <px py pz> <tx ty tz> <ux uy uz> <near> (projection is perspective)"
  );

    Ok(())
}
