use openmaths::Vector3;

use crate::brep::Brep;
use crate::operations::triangulate::triangulate_polygon_with_holes;

use super::{BrepDiagnostic, FACE_AREA_EPSILON, GEOMETRY_EPSILON};

pub(super) fn normalized(vector: Vector3) -> Option<Vector3> {
    let length_sq = vector.x * vector.x + vector.y * vector.y + vector.z * vector.z;
    if !length_sq.is_finite() || length_sq <= GEOMETRY_EPSILON * GEOMETRY_EPSILON {
        return None;
    }

    let inverse_length = length_sq.sqrt().recip();
    Some(Vector3::new(
        vector.x * inverse_length,
        vector.y * inverse_length,
        vector.z * inverse_length,
    ))
}

pub(super) fn is_finite_vector(vector: Vector3) -> bool {
    vector.x.is_finite() && vector.y.is_finite() && vector.z.is_finite()
}

fn face_area(brep: &Brep, face_id: u32) -> f64 {
    let (face_vertices, hole_vertices) = brep.get_vertices_and_holes_by_face_id(face_id);
    if face_vertices.len() < 3 {
        return 0.0;
    }

    let triangles = triangulate_polygon_with_holes(&face_vertices, &hole_vertices);
    let all_vertices: Vec<Vector3> = face_vertices
        .iter()
        .copied()
        .chain(hole_vertices.iter().flatten().copied())
        .collect();

    let mut area = 0.0;
    for triangle in triangles {
        let Some(a) = all_vertices.get(triangle[0]) else {
            continue;
        };
        let Some(b) = all_vertices.get(triangle[1]) else {
            continue;
        };
        let Some(c) = all_vertices.get(triangle[2]) else {
            continue;
        };

        let ab = Vector3::new(b.x - a.x, b.y - a.y, b.z - a.z);
        let ac = Vector3::new(c.x - a.x, c.y - a.y, c.z - a.z);
        let cross = ab.cross(&ac);
        let triangle_area =
            (cross.x * cross.x + cross.y * cross.y + cross.z * cross.z).sqrt() * 0.5;
        area += triangle_area;
    }

    area
}

pub(super) fn validate_geometry(brep: &Brep) -> Vec<BrepDiagnostic> {
    let mut diagnostics = Vec::new();

    for vertex in &brep.vertices {
        if !is_finite_vector(vertex.position) {
            diagnostics.push(
                BrepDiagnostic::error(
                    "non_finite_vertex",
                    format!("Vertex {} has non-finite coordinates", vertex.id),
                )
                .with_domain("vertex", Some(vertex.id)),
            );
        }
    }

    for edge in &brep.edges {
        let Some((start_id, end_id)) = brep.get_edge_endpoints(edge.id) else {
            diagnostics.push(
                BrepDiagnostic::error(
                    "invalid_edge_endpoints",
                    format!("Edge {} has invalid endpoints", edge.id),
                )
                .with_domain("edge", Some(edge.id)),
            );
            continue;
        };

        let Some(start) = brep
            .vertices
            .get(start_id as usize)
            .map(|vertex| vertex.position)
        else {
            diagnostics.push(
                BrepDiagnostic::error(
                    "missing_edge_vertex",
                    format!("Edge {} start vertex {} is missing", edge.id, start_id),
                )
                .with_domain("edge", Some(edge.id)),
            );
            continue;
        };
        let Some(end) = brep
            .vertices
            .get(end_id as usize)
            .map(|vertex| vertex.position)
        else {
            diagnostics.push(
                BrepDiagnostic::error(
                    "missing_edge_vertex",
                    format!("Edge {} end vertex {} is missing", edge.id, end_id),
                )
                .with_domain("edge", Some(edge.id)),
            );
            continue;
        };

        let dx = start.x - end.x;
        let dy = start.y - end.y;
        let dz = start.z - end.z;
        let length_sq = dx * dx + dy * dy + dz * dz;
        if !length_sq.is_finite() || length_sq <= GEOMETRY_EPSILON * GEOMETRY_EPSILON {
            diagnostics.push(
                BrepDiagnostic::error(
                    "collapsed_edge",
                    format!("Edge {} collapsed to near-zero length", edge.id),
                )
                .with_domain("edge", Some(edge.id)),
            );
        }
    }

    for face in &brep.faces {
        let area = face_area(brep, face.id);
        if !area.is_finite() || area <= FACE_AREA_EPSILON {
            diagnostics.push(
                BrepDiagnostic::error(
                    "degenerate_face",
                    format!("Face {} has near-zero area", face.id),
                )
                .with_domain("face", Some(face.id)),
            );
        }
    }

    diagnostics
}
