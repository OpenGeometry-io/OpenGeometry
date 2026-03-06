use opengeometry::operations::triangulate::triangulate_polygon_with_holes;
use opengeometry::primitives::cylinder::OGCylinder;
use opengeometry::primitives::line::OGLine;
use opengeometry::primitives::rectangle::OGRectangle;
use opengeometry::primitives::sphere::OGSphere;
use opengeometry::primitives::wedge::OGWedge;
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

#[test]
fn sphere_hlr_outline_suppresses_internal_tessellation_edges() {
    let mut sphere = OGSphere::new("sphere-hlr".to_string());
    sphere
        .set_config(Vector3::new(0.0, 0.0, 0.0), 1.0, 32, 20)
        .unwrap();

    let raw: Vec<f64> = serde_json::from_str(&sphere.get_outline_geometry_serialized()).unwrap();
    let hlr: Vec<f64> = serde_json::from_str(&sphere.get_outline_geometry_hlr_serialized(
        Vector3::new(3.0, 2.4, 4.0),
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(0.0, 1.0, 0.0),
        0.1,
        true,
    ))
    .unwrap();

    assert!(!raw.is_empty());
    assert!(!hlr.is_empty());
    assert!(hlr.len() < raw.len());
}

#[test]
fn cylinder_hlr_outline_suppresses_side_segment_edges() {
    let mut cylinder = OGCylinder::new("cylinder-hlr".to_string());
    cylinder
        .set_config(
            Vector3::new(0.0, 0.0, 0.0),
            1.0,
            2.0,
            std::f64::consts::TAU,
            36,
        )
        .unwrap();

    let raw: Vec<f64> = serde_json::from_str(&cylinder.get_outline_geometry_serialized()).unwrap();
    let hlr: Vec<f64> = serde_json::from_str(&cylinder.get_outline_geometry_hlr_serialized(
        Vector3::new(4.0, 2.8, 4.0),
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(0.0, 1.0, 0.0),
        0.1,
        true,
    ))
    .unwrap();

    assert!(!raw.is_empty());
    assert!(!hlr.is_empty());
    assert!(hlr.len() < raw.len());
}

#[test]
fn cylinder_hlr_from_below_hides_top_cap_rim() {
    let mut cylinder = OGCylinder::new("cylinder-cap-rim".to_string());
    cylinder
        .set_config(
            Vector3::new(0.0, 0.0, 0.0),
            1.0,
            2.0,
            std::f64::consts::TAU,
            36,
        )
        .unwrap();

    let y_min = -1.0;
    let y_max = 1.0;
    let epsilon = 1.0e-6;

    let hlr: Vec<f64> = serde_json::from_str(&cylinder.get_outline_geometry_hlr_serialized(
        Vector3::new(0.0, -6.0, 0.0),
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(0.0, 1.0, 0.0),
        0.1,
        true,
    ))
    .unwrap();

    let mut top_rim_segments = 0;
    let mut bottom_rim_segments = 0;
    for segment in hlr.chunks_exact(6) {
        let y1 = segment[1];
        let y2 = segment[4];

        if (y1 - y_max).abs() <= epsilon && (y2 - y_max).abs() <= epsilon {
            top_rim_segments += 1;
        }

        if (y1 - y_min).abs() <= epsilon && (y2 - y_min).abs() <= epsilon {
            bottom_rim_segments += 1;
        }
    }

    assert!(
        bottom_rim_segments > 0,
        "Expected bottom cap rim segments from below view"
    );
    assert_eq!(
        top_rim_segments, 0,
        "Top cap rim should be hidden when viewed from below"
    );
}

#[test]
fn wedge_face_triangulation_matches_face_winding() {
    let mut wedge = OGWedge::new("wedge-winding".to_string());
    wedge
        .set_config(Vector3::new(0.0, 0.0, 0.0), 2.0, 1.6, 1.4)
        .unwrap();

    for face in &wedge.brep().faces {
        let (face_vertices, hole_vertices) =
            wedge.brep().get_vertices_and_holes_by_face_id(face.id);
        if face_vertices.len() < 3 {
            continue;
        }

        let triangles = triangulate_polygon_with_holes(&face_vertices, &hole_vertices);
        let all_vertices: Vec<Vector3> = face_vertices
            .iter()
            .copied()
            .chain(hole_vertices.iter().flatten().copied())
            .collect();

        for tri in triangles {
            let p0 = all_vertices[tri[0]];
            let p1 = all_vertices[tri[1]];
            let p2 = all_vertices[tri[2]];

            let edge_a = [p1.x - p0.x, p1.y - p0.y, p1.z - p0.z];
            let edge_b = [p2.x - p0.x, p2.y - p0.y, p2.z - p0.z];
            let tri_normal = [
                edge_a[1] * edge_b[2] - edge_a[2] * edge_b[1],
                edge_a[2] * edge_b[0] - edge_a[0] * edge_b[2],
                edge_a[0] * edge_b[1] - edge_a[1] * edge_b[0],
            ];

            let dot = tri_normal[0] * face.normal.x
                + tri_normal[1] * face.normal.y
                + tri_normal[2] * face.normal.z;

            assert!(dot >= -1.0e-9);
        }
    }
}

#[test]
fn wedge_shell_faces_point_outward() {
    let mut wedge = OGWedge::new("wedge-outward".to_string());
    wedge
        .set_config(Vector3::new(0.0, 0.0, 0.0), 2.0, 1.6, 1.4)
        .unwrap();

    let vertices = &wedge.brep().vertices;
    let vertex_count = vertices.len() as f64;
    let shape_center = Vector3::new(
        vertices.iter().map(|v| v.position.x).sum::<f64>() / vertex_count,
        vertices.iter().map(|v| v.position.y).sum::<f64>() / vertex_count,
        vertices.iter().map(|v| v.position.z).sum::<f64>() / vertex_count,
    );

    for face in &wedge.brep().faces {
        let (face_vertices, _) = wedge.brep().get_vertices_and_holes_by_face_id(face.id);
        if face_vertices.is_empty() {
            continue;
        }

        let inv_len = 1.0 / face_vertices.len() as f64;
        let face_center = Vector3::new(
            face_vertices.iter().map(|v| v.x).sum::<f64>() * inv_len,
            face_vertices.iter().map(|v| v.y).sum::<f64>() * inv_len,
            face_vertices.iter().map(|v| v.z).sum::<f64>() * inv_len,
        );

        let outward = Vector3::new(
            face_center.x - shape_center.x,
            face_center.y - shape_center.y,
            face_center.z - shape_center.z,
        );

        let dot = face.normal.x * outward.x + face.normal.y * outward.y + face.normal.z * outward.z;
        assert!(
            dot > 1.0e-9,
            "face {} normal is not outward (dot={})",
            face.id,
            dot
        );
    }
}
