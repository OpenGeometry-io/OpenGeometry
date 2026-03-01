use std::env;

use opengeometry::export::pdf::PdfExportConfig;
use opengeometry::export::projection::{CameraParameters, HlrOptions, ProjectionMode};
use opengeometry::primitives::curve::OGCurve;
use opengeometry::primitives::line::OGLine;
use opengeometry::primitives::polyline::OGPolyline;
use opengeometry::primitives::rectangle::OGRectangle;
use opengeometry::scenegraph::OGSceneManager;
use openmaths::Vector3;

fn are_close(a: Vector3, b: Vector3) -> bool {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    let dz = a.z - b.z;
    (dx * dx + dy * dy + dz * dz) <= 1.0e-12
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let output_path = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "./offset_primitives.pdf".to_string());

    let mut base_line = OGLine::new("base-line".to_string());
    base_line.set_config(Vector3::new(-3.2, 0.0, -2.2), Vector3::new(-0.8, 0.0, -0.9));
    base_line.generate_geometry();

    let line_offset_points = base_line.get_offset_points(0.35, 35.0, true);
    let mut offset_line = OGLine::new("offset-line".to_string());
    if line_offset_points.len() == 2 {
        offset_line.set_config(line_offset_points[0], line_offset_points[1]);
        offset_line.generate_geometry();
    }

    let mut base_polyline = OGPolyline::new("base-polyline".to_string());
    base_polyline.set_config(vec![
        Vector3::new(-1.2, 0.0, -2.4),
        Vector3::new(0.2, 0.0, -1.7),
        Vector3::new(0.8, 0.0, -0.6),
        Vector3::new(0.0, 0.0, -0.3),
        Vector3::new(2.4, 0.0, 1.2),
    ]);

    let polyline_offset = base_polyline.get_offset_result(0.45, 90.0, true);
    println!(
        "Polyline beveled vertices: {:?}",
        polyline_offset.beveled_vertex_indices
    );

    let mut offset_polyline = OGPolyline::new("offset-polyline".to_string());
    offset_polyline.set_config(polyline_offset.points.clone());

    let mut base_rectangle = OGRectangle::new("base-rectangle".to_string());
    base_rectangle.set_config(Vector3::new(1.4, 0.0, -2.0), 1.6, 1.0);
    base_rectangle.generate_geometry();

    let rectangle_offset = base_rectangle.get_offset_result(0.25, 40.0, true);
    let mut rectangle_offset_outline = rectangle_offset.points.clone();
    if rectangle_offset.is_closed
        && !rectangle_offset_outline.is_empty()
        && !are_close(
            rectangle_offset_outline[0],
            rectangle_offset_outline[rectangle_offset_outline.len() - 1],
        )
    {
        rectangle_offset_outline.push(rectangle_offset_outline[0]);
    }

    let mut offset_rectangle_polyline = OGPolyline::new("offset-rectangle".to_string());
    offset_rectangle_polyline.set_config(rectangle_offset_outline);

    let mut base_curve = OGCurve::new("base-curve".to_string());
    base_curve.set_config(vec![
        Vector3::new(-3.0, 0.0, 1.2),
        Vector3::new(-2.0, 0.0, 1.7),
        Vector3::new(-1.1, 0.0, 1.6),
        Vector3::new(-0.4, 0.0, 2.0),
    ]);

    let curve_offset = base_curve.get_offset_result(0.3, 45.0, true);
    let mut curve_offset_polyline = OGPolyline::new("offset-curve".to_string());
    curve_offset_polyline.set_config(curve_offset.points.clone());

    let mut manager = OGSceneManager::new();
    let scene_id = manager.create_scene_internal("offset-primitives");

    manager.add_line_to_scene_internal(&scene_id, "entity-base-line", &base_line)?;
    manager.add_line_to_scene_internal(&scene_id, "entity-offset-line", &offset_line)?;
    manager.add_polyline_to_scene_internal(&scene_id, "entity-base-polyline", &base_polyline)?;
    manager.add_polyline_to_scene_internal(
        &scene_id,
        "entity-offset-polyline",
        &offset_polyline,
    )?;
    manager.add_rectangle_to_scene_internal(&scene_id, "entity-base-rectangle", &base_rectangle)?;
    manager.add_polyline_to_scene_internal(
        &scene_id,
        "entity-offset-rectangle-outline",
        &offset_rectangle_polyline,
    )?;
    manager.add_brep_entity_to_scene_internal(
        &scene_id,
        "entity-base-curve",
        "OGCurve",
        base_curve.brep(),
    )?;
    manager.add_polyline_to_scene_internal(
        &scene_id,
        "entity-offset-curve",
        &curve_offset_polyline,
    )?;

    let camera = CameraParameters {
        position: Vector3::new(5.8, 4.6, 5.0),
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
