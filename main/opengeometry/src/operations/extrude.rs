use super::windingsort;
use crate::{
    brep::{Brep, BrepBuilder},
    geometry::basegeometry::BaseGeometry,
};
use openmaths::Vector3;

#[derive(Clone)]
pub struct Geometry {
    pub vertices: Vec<Vector3>,
    pub edges: Vec<Vec<u32>>,
    pub faces: Vec<Vec<u32>>,
}

pub fn extrude_polygon_by_buffer_geometry(geom_buf: BaseGeometry, height: f64) -> Geometry {
    let base = windingsort::ccw_test(geom_buf.get_vertices());
    if base.len() < 3 {
        return Geometry {
            vertices: Vec::new(),
            edges: Vec::new(),
            faces: Vec::new(),
        };
    }

    let mut vertices = base.clone();
    for point in &base {
        vertices.push(Vector3::new(point.x, point.y + height, point.z));
    }

    let mut edges = Vec::new();
    let n = base.len() as u32;

    for i in 0..n {
        edges.push(vec![i, (i + 1) % n]);
        edges.push(vec![i, i + n]);
        edges.push(vec![i + n, ((i + 1) % n) + n]);
    }

    let mut faces = Vec::new();
    faces.push((0..n).collect());

    for i in 0..n {
        let next = (i + 1) % n;
        faces.push(vec![i, next, next + n, i + n]);
    }

    let mut top_face: Vec<u32> = (0..n).map(|i| i + n).collect();
    top_face.reverse();
    faces.push(top_face);

    Geometry {
        vertices,
        edges,
        faces,
    }
}

pub fn extrude_brep_face(brep_face: Brep, height: f64) -> Brep {
    let base_points = if let Some(face) = brep_face.faces.first() {
        brep_face
            .get_vertices_by_face_id(face.id)
            .into_iter()
            .collect::<Vec<_>>()
    } else if let Some(wire) = brep_face.wires.first() {
        brep_face
            .get_wire_vertex_indices(wire.id)
            .into_iter()
            .filter_map(|vertex_id| brep_face.vertices.get(vertex_id as usize))
            .map(|vertex| vertex.position)
            .collect::<Vec<_>>()
    } else {
        brep_face.get_flattened_vertices()
    };

    if base_points.len() < 3 {
        return Brep::new(brep_face.id);
    }

    let mut base = windingsort::ccw_test(base_points);
    if base.len() < 3 {
        return Brep::new(brep_face.id);
    }

    if let (Some(first), Some(last)) = (base.first().copied(), base.last().copied()) {
        let dx = first.x - last.x;
        let dy = first.y - last.y;
        let dz = first.z - last.z;
        if dx * dx + dy * dy + dz * dz <= 1.0e-12 {
            base.pop();
        }
    }

    if base.len() < 3 {
        return Brep::new(brep_face.id);
    }

    let mut builder = BrepBuilder::new(brep_face.id);

    let mut all_vertices = base.clone();
    all_vertices.extend(
        base.iter()
            .map(|point| Vector3::new(point.x, point.y + height, point.z)),
    );

    builder.add_vertices(&all_vertices);

    let n = base.len() as u32;

    // Bottom face should be flipped for outward shell orientation.
    let mut bottom: Vec<u32> = (0..n).collect();
    bottom.reverse();
    if builder.add_face(&bottom, &[]).is_err() {
        return Brep::new(brep_face.id);
    }

    let top: Vec<u32> = (0..n).map(|i| i + n).collect();
    if builder.add_face(&top, &[]).is_err() {
        return Brep::new(brep_face.id);
    }

    for i in 0..n {
        let next = (i + 1) % n;
        let side = vec![i, next, next + n, i + n];
        if builder.add_face(&side, &[]).is_err() {
            return Brep::new(brep_face.id);
        }
    }

    if builder.add_shell_from_all_faces(true).is_err() {
        return Brep::new(brep_face.id);
    }

    builder.build().unwrap_or_else(|_| Brep::new(brep_face.id))
}
