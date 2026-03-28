use std::collections::HashSet;

use openmaths::Vector3;
use wasm_bindgen::prelude::JsValue;

use crate::brep::{Brep, Face};

use super::{EdgeInfo, FaceInfo, OGFreeformGeometry, VertexInfo};

impl OGFreeformGeometry {
    pub(super) fn build_face_info(&self, face_id: u32) -> Result<FaceInfo, JsValue> {
        let brep = self.world_brep();
        let face = brep
            .faces
            .iter()
            .find(|face| face.id == face_id)
            .ok_or_else(|| JsValue::from_str(&format!("Face {} does not exist", face_id)))?;

        let loop_ids = face_loop_ids(face);
        let edge_ids = collect_face_edge_ids(&brep, face)?;
        let vertex_ids = collect_face_vertex_ids(&brep, face)?;
        let adjacent_face_ids = collect_adjacent_faces(&brep, &edge_ids, face.id);

        Ok(FaceInfo {
            face_id,
            centroid: compute_face_centroid(&brep, face).unwrap_or(Vector3::new(0.0, 0.0, 0.0)),
            normal: face.normal,
            surface_type: "planar".to_string(),
            loop_ids,
            edge_ids,
            vertex_ids,
            adjacent_face_ids,
        })
    }

    pub(super) fn build_edge_info(&self, edge_id: u32) -> Result<EdgeInfo, JsValue> {
        let brep = self.world_brep();
        let (start_id, end_id) = brep
            .get_edge_endpoints(edge_id)
            .ok_or_else(|| JsValue::from_str(&format!("Edge {} does not exist", edge_id)))?;

        let start = brep
            .vertices
            .get(start_id as usize)
            .map(|vertex| vertex.position)
            .ok_or_else(|| {
                JsValue::from_str(&format!("Edge {} start vertex is missing", edge_id))
            })?;

        let end = brep
            .vertices
            .get(end_id as usize)
            .map(|vertex| vertex.position)
            .ok_or_else(|| JsValue::from_str(&format!("Edge {} end vertex is missing", edge_id)))?;

        Ok(EdgeInfo {
            edge_id,
            curve_type: "line".to_string(),
            start_vertex_id: start_id,
            end_vertex_id: end_id,
            start,
            end,
            incident_face_ids: incident_faces_for_edge(&brep, edge_id),
        })
    }

    pub(super) fn build_vertex_info(&self, vertex_id: u32) -> Result<VertexInfo, JsValue> {
        let brep = self.world_brep();
        let vertex = brep
            .vertices
            .get(vertex_id as usize)
            .ok_or_else(|| JsValue::from_str(&format!("Vertex {} does not exist", vertex_id)))?;

        let (edge_ids, face_ids) = connected_edges_and_faces(&brep, vertex_id);

        Ok(VertexInfo {
            vertex_id,
            position: vertex.position,
            edge_ids,
            face_ids,
        })
    }
}

pub(super) fn face_loop_ids(face: &Face) -> Vec<u32> {
    let mut loop_ids = vec![face.outer_loop];
    loop_ids.extend(face.inner_loops.iter().copied());
    loop_ids
}

pub(super) fn collect_face_vertex_ids(brep: &Brep, face: &Face) -> Result<Vec<u32>, String> {
    let mut vertex_ids = Vec::new();
    let mut seen = HashSet::new();

    for loop_id in face_loop_ids(face) {
        let loop_vertices = brep
            .get_loop_vertex_indices(loop_id)
            .into_iter()
            .collect::<Vec<_>>();

        if loop_vertices.is_empty() {
            return Err(format!(
                "Loop {} for face {} does not contain vertices",
                loop_id, face.id
            ));
        }

        for vertex_id in loop_vertices {
            if seen.insert(vertex_id) {
                vertex_ids.push(vertex_id);
            }
        }
    }

    Ok(vertex_ids)
}

fn collect_face_edge_ids(brep: &Brep, face: &Face) -> Result<Vec<u32>, JsValue> {
    let mut edge_ids = Vec::new();
    let mut seen = HashSet::new();

    for loop_id in face_loop_ids(face) {
        let loop_halfedges = brep.get_loop_halfedges(loop_id).map_err(|error| {
            JsValue::from_str(&format!("Failed to read loop {}: {}", loop_id, error))
        })?;

        for halfedge_id in loop_halfedges {
            let Some(halfedge) = brep.halfedges.get(halfedge_id as usize) else {
                continue;
            };
            if seen.insert(halfedge.edge) {
                edge_ids.push(halfedge.edge);
            }
        }
    }

    Ok(edge_ids)
}

fn collect_adjacent_faces(brep: &Brep, edge_ids: &[u32], current_face_id: u32) -> Vec<u32> {
    let mut adjacent = Vec::new();
    let mut seen = HashSet::new();

    for edge_id in edge_ids {
        for face_id in incident_faces_for_edge(brep, *edge_id) {
            if face_id == current_face_id {
                continue;
            }
            if seen.insert(face_id) {
                adjacent.push(face_id);
            }
        }
    }

    adjacent
}

pub(super) fn incident_faces_for_edge(brep: &Brep, edge_id: u32) -> Vec<u32> {
    let mut faces = Vec::new();
    let mut seen = HashSet::new();

    let Some(edge) = brep.edges.get(edge_id as usize) else {
        return faces;
    };

    let mut halfedges = vec![edge.halfedge];
    if let Some(twin_halfedge) = edge.twin_halfedge {
        halfedges.push(twin_halfedge);
    }

    for halfedge_id in halfedges {
        let Some(halfedge) = brep.halfedges.get(halfedge_id as usize) else {
            continue;
        };

        if let Some(face_id) = halfedge.face {
            if seen.insert(face_id) {
                faces.push(face_id);
            }
        }
    }

    faces
}

fn connected_edges_and_faces(brep: &Brep, vertex_id: u32) -> (Vec<u32>, Vec<u32>) {
    let mut edge_ids = Vec::new();
    let mut face_ids = Vec::new();
    let mut seen_edges = HashSet::new();
    let mut seen_faces = HashSet::new();

    for halfedge in &brep.halfedges {
        if halfedge.from != vertex_id && halfedge.to != vertex_id {
            continue;
        }

        if seen_edges.insert(halfedge.edge) {
            edge_ids.push(halfedge.edge);
        }

        if let Some(face_id) = halfedge.face {
            if seen_faces.insert(face_id) {
                face_ids.push(face_id);
            }
        }
    }

    (edge_ids, face_ids)
}

fn compute_face_centroid(brep: &Brep, face: &Face) -> Option<Vector3> {
    let vertices = brep.get_vertices_by_face_id(face.id);
    if vertices.is_empty() {
        return None;
    }

    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    let mut sum_z = 0.0;

    for vertex in &vertices {
        sum_x += vertex.x;
        sum_y += vertex.y;
        sum_z += vertex.z;
    }

    let count = vertices.len() as f64;
    Some(Vector3::new(sum_x / count, sum_y / count, sum_z / count))
}
