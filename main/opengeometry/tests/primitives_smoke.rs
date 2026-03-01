use opengeometry::primitives::line::OGLine;
use opengeometry::primitives::sphere::OGSphere;
use openmaths::Vector3;

#[test]
fn sphere_geometry_and_outline_are_non_empty() {
    let mut sphere = OGSphere::new("sphere-smoke".to_string());
    sphere.set_config(Vector3::new(0.0, 0.0, 0.0), 1.0, 16, 10);

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
    sphere.set_config(Vector3::new(0.0, 0.0, 0.0), 1.0, 1, 1);

    assert!(!sphere.brep().vertices.is_empty());
    assert!(!sphere.brep().faces.is_empty());
}

#[test]
fn line_offset_smoke() {
    let mut line = OGLine::new("line-smoke".to_string());
    line.set_config(Vector3::new(-1.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0));
    line.generate_geometry();

    let result = line.get_offset_result(0.25, 35.0, true);
    assert_eq!(result.points.len(), 2);
}
