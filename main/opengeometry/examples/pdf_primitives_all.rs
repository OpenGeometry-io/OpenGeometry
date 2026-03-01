use std::env;

use opengeometry::export::pdf::{export_scene_to_pdf_with_config, PdfExportConfig};
use opengeometry::export::projection::{CameraParameters, HlrOptions, ProjectionMode};
use opengeometry::primitives::arc::OGArc;
use opengeometry::primitives::cuboid::OGCuboid;
use opengeometry::primitives::cylinder::OGCylinder;
use opengeometry::primitives::line::OGLine;
use opengeometry::primitives::polygon::OGPolygon;
use opengeometry::primitives::polyline::OGPolyline;
use opengeometry::primitives::rectangle::OGRectangle;
use opengeometry::primitives::wedge::OGWedge;
use openmaths::Vector3;

fn export_named_scene(
    output_path: &str,
    title: &str,
    scene: &opengeometry::export::projection::Scene2D,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = PdfExportConfig::a4_landscape();
    config.title = Some(title.to_string());
    export_scene_to_pdf_with_config(scene, output_path, &config)?;
    println!("Generated {}", output_path);
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let output_prefix = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "pdf_primitives".to_string());

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

    let mut line = OGLine::new("line".to_string());
    line.set_config(Vector3::new(-1.2, 0.0, 0.0), Vector3::new(1.2, 0.0, 0.5));
    line.generate_geometry();

    let mut polyline = OGPolyline::new("polyline".to_string());
    polyline.set_config(vec![
        Vector3::new(-1.6, 0.0, -0.8),
        Vector3::new(-0.8, 0.2, 0.4),
        Vector3::new(0.0, 0.0, -0.2),
        Vector3::new(0.9, 0.4, 0.8),
    ]);

    let mut arc = OGArc::new("arc".to_string());
    arc.set_config(
        Vector3::new(0.0, 0.0, 0.0),
        1.2,
        0.0,
        1.25 * std::f64::consts::PI,
        40,
    );
    arc.generate_geometry();

    let mut rectangle = OGRectangle::new("rectangle".to_string());
    rectangle.set_config(Vector3::new(0.0, 0.0, 0.0), 2.4, 1.4);
    rectangle.generate_geometry();

    let mut polygon = OGPolygon::new("polygon".to_string());
    polygon.set_config(vec![
        Vector3::new(-1.1, 0.0, -0.6),
        Vector3::new(0.6, 0.0, -1.0),
        Vector3::new(1.3, 0.0, 0.0),
        Vector3::new(0.4, 0.0, 1.1),
        Vector3::new(-1.0, 0.0, 0.8),
    ]);

    let mut cuboid = OGCuboid::new("cuboid".to_string());
    cuboid.set_config(Vector3::new(0.0, 0.0, 0.0), 2.0, 1.5, 1.2);

    let mut cylinder = OGCylinder::new("cylinder".to_string());
    cylinder.set_config(
        Vector3::new(0.0, 0.0, 0.0),
        0.9,
        1.8,
        2.0 * std::f64::consts::PI,
        40,
    );

    let mut wedge = OGWedge::new("wedge".to_string());
    wedge.set_config(Vector3::new(0.0, 0.0, 0.0), 2.4, 1.6, 1.2);

    export_named_scene(
        &format!("{}_line.pdf", output_prefix),
        "OGLine Projection",
        &line.to_projected_scene2d(&camera, &hlr),
    )?;
    export_named_scene(
        &format!("{}_polyline.pdf", output_prefix),
        "OGPolyline Projection",
        &polyline.to_projected_scene2d(&camera, &hlr),
    )?;
    export_named_scene(
        &format!("{}_arc.pdf", output_prefix),
        "OGArc Projection",
        &arc.to_projected_scene2d(&camera, &hlr),
    )?;
    export_named_scene(
        &format!("{}_rectangle.pdf", output_prefix),
        "OGRectangle Projection",
        &rectangle.to_projected_scene2d(&camera, &hlr),
    )?;
    export_named_scene(
        &format!("{}_polygon.pdf", output_prefix),
        "OGPolygon Projection",
        &polygon.to_projected_scene2d(&camera, &hlr),
    )?;
    export_named_scene(
        &format!("{}_cuboid.pdf", output_prefix),
        "OGCuboid Projection",
        &cuboid.to_projected_scene2d(&camera, &hlr),
    )?;
    export_named_scene(
        &format!("{}_cylinder.pdf", output_prefix),
        "OGCylinder Projection",
        &cylinder.to_projected_scene2d(&camera, &hlr),
    )?;
    export_named_scene(
        &format!("{}_wedge.pdf", output_prefix),
        "OGWedge Projection",
        &wedge.to_projected_scene2d(&camera, &hlr),
    )?;

    Ok(())
}
