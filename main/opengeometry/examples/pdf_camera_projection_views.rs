use std::env;

use opengeometry::export::pdf::{export_scene_to_pdf_with_config, PdfExportConfig};
use opengeometry::export::projection::{CameraParameters, HlrOptions, ProjectionMode};
use opengeometry::primitives::cuboid::OGCuboid;
use opengeometry::primitives::cylinder::OGCylinder;
use openmaths::Vector3;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let output_prefix = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "pdf_camera_projection_views".to_string());

    let camera = CameraParameters {
        position: Vector3::new(6.0, 5.0, 6.0),
        target: Vector3::new(0.0, 0.0, 0.0),
        up: Vector3::new(0.0, 1.0, 0.0),
        near: 0.1,
        projection_mode: ProjectionMode::Orthographic,
    };

    let mut cuboid = OGCuboid::new("ortho-cuboid".to_string());
    cuboid.set_config(Vector3::new(-1.8, 0.0, 0.0), 2.0, 1.5, 1.5);

    let mut cylinder = OGCylinder::new("ortho-cylinder".to_string());
    cylinder.set_config(
        Vector3::new(1.8, 0.0, 0.0),
        0.9,
        1.8,
        2.0 * std::f64::consts::PI,
        32,
    );

    let mut scene_hlr_on = cuboid.to_projected_scene2d(
        &camera,
        &HlrOptions {
            hide_hidden_edges: true,
        },
    );
    scene_hlr_on.extend(cylinder.to_projected_scene2d(
        &camera,
        &HlrOptions {
            hide_hidden_edges: true,
        },
    ));

    let mut scene_hlr_off = cuboid.to_projected_scene2d(
        &camera,
        &HlrOptions {
            hide_hidden_edges: false,
        },
    );
    scene_hlr_off.extend(cylinder.to_projected_scene2d(
        &camera,
        &HlrOptions {
            hide_hidden_edges: false,
        },
    ));

    let mut config = PdfExportConfig::a4_landscape();
    config.title = Some("Orthographic Projection HLR Comparison".to_string());

    let output_hlr_on = format!("{}_hlr_on.pdf", output_prefix);
    let output_hlr_off = format!("{}_hlr_off.pdf", output_prefix);

    export_scene_to_pdf_with_config(&scene_hlr_on, &output_hlr_on, &config)?;
    export_scene_to_pdf_with_config(&scene_hlr_off, &output_hlr_off, &config)?;

    println!("Generated {}", output_hlr_on);
    println!("Generated {}", output_hlr_off);

    Ok(())
}
