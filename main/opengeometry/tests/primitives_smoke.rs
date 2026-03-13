use opengeometry::primitives::line::OGLine;
use opengeometry::primitives::rectangle::OGRectangle;
use opengeometry::primitives::sphere::OGSphere;
use openmaths::Vector3;

#[test]
fn sphere_geometry_and_outline_are_non_empty() {
    let mut sphere = OGSphere::new("sphere-smoke".to_string());
    sphere
        .set_config(Vector3::new(0.0, 0.0, 0.0), 1.0, 16, 10)
        .unwrap();

    assert!(!sphere.brep().vertices.is_empty());
    assert!(!sphere.brep().edges.is_empty());
    assert!(!sphere.brep().faces.is_empty());

    let geometry: Vec<f64> = serde_json::from_str(&sphere.get_geometry_serialized()).unwrap();
    let outline: Vec<f64> =
        serde_json::from_str(&sphere.get_outline_geometry_serialized()).unwrap();

    assert!(!geometry.is_empty());
    assert!(!outline.is_empty());
    assert_eq!(geometry.len() % 9, 0);
    assert_eq!(outline.len() % 6, 0);
}

#[test]
fn sphere_segment_inputs_are_clamped() {
    let mut sphere = OGSphere::new("sphere-clamp".to_string());
    sphere
        .set_config(Vector3::new(0.0, 0.0, 0.0), 1.0, 1, 1)
        .unwrap();

    assert!(!sphere.brep().vertices.is_empty());
    assert!(!sphere.brep().faces.is_empty());
}

#[test]
fn line_offset_smoke() {
    let mut line = OGLine::new("line-smoke".to_string());
    line.set_config(Vector3::new(-1.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0))
        .unwrap();
    line.generate_geometry().unwrap();

    let result = line.get_offset_result(0.25, 35.0, true);
    assert_eq!(result.points.len(), 2);
}

#[test]
fn rectangle_generates_face_loop_without_duplicate_halfedges() {
    let mut rectangle = OGRectangle::new("rectangle-smoke".to_string());
    rectangle
        .set_config(Vector3::new(0.0, 0.0, 0.0), 2.0, 1.0)
        .unwrap();
    rectangle.generate_geometry().unwrap();

    assert_eq!(rectangle.brep().faces.len(), 1);
    assert!(rectangle.brep().wires.is_empty());

    let geometry: Vec<f64> = serde_json::from_str(&rectangle.get_geometry_serialized()).unwrap();
    assert_eq!(geometry.len(), 15);
}
