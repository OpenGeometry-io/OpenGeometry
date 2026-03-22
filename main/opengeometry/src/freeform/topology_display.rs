use openmaths::Vector3;

use crate::operations::triangulate::triangulate_polygon_with_holes;

use super::{
    OGFreeformGeometry, TopologyEdgeRenderData, TopologyFaceRenderData, TopologyRenderData,
    TopologyVertexRenderData,
};

impl OGFreeformGeometry {
    pub(super) fn build_topology_display_data(&self) -> TopologyRenderData {
        let brep = self.world_brep();
        let mut faces_payload = Vec::with_capacity(brep.faces.len());

        for face in &brep.faces {
            let (face_vertices, hole_vertices) = brep.get_vertices_and_holes_by_face_id(face.id);
            if face_vertices.len() < 3 {
                continue;
            }

            let triangles = triangulate_polygon_with_holes(&face_vertices, &hole_vertices);
            let all_vertices: Vec<Vector3> = face_vertices
                .iter()
                .copied()
                .chain(hole_vertices.iter().flatten().copied())
                .collect();

            let mut positions = Vec::with_capacity(all_vertices.len() * 3);
            for vertex in &all_vertices {
                positions.push(vertex.x);
                positions.push(vertex.y);
                positions.push(vertex.z);
            }

            let mut indices = Vec::with_capacity(triangles.len() * 3);
            for triangle in triangles {
                indices.push(triangle[0] as u32);
                indices.push(triangle[1] as u32);
                indices.push(triangle[2] as u32);
            }

            faces_payload.push(TopologyFaceRenderData {
                face_id: face.id,
                positions,
                indices,
            });
        }

        let mut edges_payload = Vec::with_capacity(brep.edges.len());
        for edge in &brep.edges {
            let Some((start, end)) = brep.get_edge_endpoints(edge.id) else {
                continue;
            };

            let Some(start_vertex) = brep.vertices.get(start as usize) else {
                continue;
            };
            let Some(end_vertex) = brep.vertices.get(end as usize) else {
                continue;
            };

            edges_payload.push(TopologyEdgeRenderData {
                edge_id: edge.id,
                positions: vec![
                    start_vertex.position.x,
                    start_vertex.position.y,
                    start_vertex.position.z,
                    end_vertex.position.x,
                    end_vertex.position.y,
                    end_vertex.position.z,
                ],
            });
        }

        let vertices_payload = brep
            .vertices
            .iter()
            .map(|vertex| TopologyVertexRenderData {
                vertex_id: vertex.id,
                position: vertex.position,
            })
            .collect();

        TopologyRenderData {
            faces: faces_payload,
            edges: edges_payload,
            vertices: vertices_payload,
        }
    }
}
