use wasm_bindgen::prelude::JsValue;

use super::edits::collect_closed_quad_edge_ring;
use super::inspection::incident_faces_for_edge;
use super::validation::normalized;
use super::{EditCapabilities, FeatureEditCapabilities, OGFreeformGeometry};

impl OGFreeformGeometry {
    pub(super) fn build_entity_edit_capabilities(&self) -> EditCapabilities {
        let supports_edge_topology_edits = self.supports_any_edge_topology_edits();
        let supports_face_cut = self.supports_any_face_cut();
        let supports_loop_cut = self.supports_any_loop_cut();
        let supports_vertex_removal = self.supports_single_face_topology_edits();
        let can_remove_vertex = supports_vertex_removal && self.single_face_vertex_count() > 3;

        let mut reasons = Vec::new();
        if !supports_edge_topology_edits {
            reasons
                .push("insert/split require an edge that belongs to at least one face".to_string());
        }
        if !supports_vertex_removal {
            reasons.push(
                "removeVertex currently requires a single-face open surface topology".to_string(),
            );
        }
        if !supports_face_cut {
            reasons.push(
                "cutFace currently requires a face without holes and at least four boundary edges"
                    .to_string(),
            );
        }
        if !supports_loop_cut {
            reasons.push(
                "loopCut currently requires a closed quad edge ring on a face-backed solid edge"
                    .to_string(),
            );
        }

        EditCapabilities {
            can_push_pull_face: !self.local_brep.faces.is_empty(),
            can_move_face: !self.local_brep.faces.is_empty(),
            can_extrude_face: self
                .local_brep
                .faces
                .iter()
                .any(|face| face.inner_loops.is_empty() && normalized(face.normal).is_some()),
            can_cut_face: supports_face_cut,
            can_move_edge: !self.local_brep.edges.is_empty(),
            can_move_vertex: !self.local_brep.vertices.is_empty(),
            can_insert_vertex_on_edge: supports_edge_topology_edits,
            can_remove_vertex,
            can_split_edge: supports_edge_topology_edits,
            can_loop_cut: supports_loop_cut,
            reasons,
        }
    }

    pub(super) fn build_face_edit_capabilities(
        &self,
        face_id: u32,
    ) -> Result<FeatureEditCapabilities, JsValue> {
        let Some(face) = self
            .local_brep
            .faces
            .iter()
            .find(|candidate| candidate.id == face_id)
        else {
            return Err(JsValue::from_str(&format!(
                "Face {} does not exist",
                face_id
            )));
        };

        let can_extrude_face = face.inner_loops.is_empty() && normalized(face.normal).is_some();
        let can_cut_face = self.supports_face_cut_on_face(face_id);
        let mut reasons = Vec::new();
        if !can_extrude_face {
            reasons.push("extrudeFace currently supports planar faces without holes".to_string());
        }
        if !can_cut_face {
            reasons.push(
                "cutFace currently requires a face without holes and at least four boundary edges"
                    .to_string(),
            );
        }

        Ok(FeatureEditCapabilities {
            domain: "face".to_string(),
            topology_id: face_id,
            can_push_pull_face: true,
            can_move_face: true,
            can_extrude_face,
            can_cut_face,
            can_move_edge: false,
            can_move_vertex: false,
            can_insert_vertex_on_edge: false,
            can_remove_vertex: false,
            can_split_edge: false,
            can_loop_cut: false,
            reasons,
        })
    }

    pub(super) fn build_edge_edit_capabilities(
        &self,
        edge_id: u32,
    ) -> Result<FeatureEditCapabilities, JsValue> {
        if self.local_brep.edges.get(edge_id as usize).is_none() {
            return Err(JsValue::from_str(&format!(
                "Edge {} does not exist",
                edge_id
            )));
        }

        let supports_topology_edits = self.supports_face_backed_edge_topology_edits(edge_id);
        let supports_loop_cut = self.supports_loop_cut_from_edge(edge_id);
        let mut reasons = Vec::new();
        if !supports_topology_edits {
            reasons.push(
                "insertVertexOnEdge/splitEdge require an edge that belongs to at least one face"
                    .to_string(),
            );
        }
        if !supports_loop_cut {
            reasons.push(
                "loopCut currently requires an edge that belongs to a closed quad edge ring"
                    .to_string(),
            );
        }

        Ok(FeatureEditCapabilities {
            domain: "edge".to_string(),
            topology_id: edge_id,
            can_push_pull_face: false,
            can_move_face: false,
            can_extrude_face: false,
            can_cut_face: false,
            can_move_edge: true,
            can_move_vertex: false,
            can_insert_vertex_on_edge: supports_topology_edits,
            can_remove_vertex: false,
            can_split_edge: supports_topology_edits,
            can_loop_cut: supports_loop_cut,
            reasons,
        })
    }

    pub(super) fn build_vertex_edit_capabilities(
        &self,
        vertex_id: u32,
    ) -> Result<FeatureEditCapabilities, JsValue> {
        if self.local_brep.vertices.get(vertex_id as usize).is_none() {
            return Err(JsValue::from_str(&format!(
                "Vertex {} does not exist",
                vertex_id
            )));
        }

        let supports_topology_edits = self.supports_single_face_topology_edits();
        let can_remove_vertex = supports_topology_edits
            && self.single_face_vertex_count() > 3
            && self.single_face_contains_vertex(vertex_id);

        let mut reasons = Vec::new();
        if !supports_topology_edits {
            reasons.push("removeVertex requires a single-face open surface topology".to_string());
        } else if !can_remove_vertex {
            reasons.push(
                "removeVertex requires a boundary vertex on a face with at least four vertices"
                    .to_string(),
            );
        }

        Ok(FeatureEditCapabilities {
            domain: "vertex".to_string(),
            topology_id: vertex_id,
            can_push_pull_face: false,
            can_move_face: false,
            can_extrude_face: false,
            can_cut_face: false,
            can_move_edge: false,
            can_move_vertex: true,
            can_insert_vertex_on_edge: false,
            can_remove_vertex,
            can_split_edge: false,
            can_loop_cut: false,
            reasons,
        })
    }

    pub(super) fn supports_single_face_topology_edits(&self) -> bool {
        if self.local_brep.faces.len() != 1 {
            return false;
        }

        let Some(face) = self.local_brep.faces.first() else {
            return false;
        };

        if !face.inner_loops.is_empty() {
            return false;
        }

        let loop_vertices = self.local_brep.get_loop_vertex_indices(face.outer_loop);
        if loop_vertices.len() < 3 {
            return false;
        }

        for edge in &self.local_brep.edges {
            let incident = incident_faces_for_edge(&self.local_brep, edge.id);
            if incident.len() > 1 {
                return false;
            }
        }

        true
    }

    pub(super) fn supports_any_edge_topology_edits(&self) -> bool {
        self.local_brep
            .edges
            .iter()
            .any(|edge| self.supports_face_backed_edge_topology_edits(edge.id))
    }

    pub(super) fn supports_face_backed_edge_topology_edits(&self, edge_id: u32) -> bool {
        !incident_faces_for_edge(&self.local_brep, edge_id).is_empty()
    }

    pub(super) fn supports_any_face_cut(&self) -> bool {
        self.local_brep
            .faces
            .iter()
            .any(|face| self.supports_face_cut_on_face(face.id))
    }

    pub(super) fn supports_face_cut_on_face(&self, face_id: u32) -> bool {
        let Some(face) = self
            .local_brep
            .faces
            .iter()
            .find(|candidate| candidate.id == face_id)
        else {
            return false;
        };

        if !face.inner_loops.is_empty() {
            return false;
        }

        self.local_brep
            .get_loop_halfedges(face.outer_loop)
            .map(|halfedges| halfedges.len() >= 4)
            .unwrap_or(false)
    }

    pub(super) fn supports_any_loop_cut(&self) -> bool {
        self.local_brep
            .edges
            .iter()
            .any(|edge| self.supports_loop_cut_from_edge(edge.id))
    }

    pub(super) fn supports_loop_cut_from_edge(&self, edge_id: u32) -> bool {
        collect_closed_quad_edge_ring(&self.local_brep, edge_id).is_ok()
    }

    fn single_face_vertex_count(&self) -> usize {
        self.local_brep
            .faces
            .first()
            .map(|face| {
                self.local_brep
                    .get_loop_vertex_indices(face.outer_loop)
                    .len()
            })
            .unwrap_or(0)
    }

    fn single_face_contains_vertex(&self, vertex_id: u32) -> bool {
        let Some(face) = self.local_brep.faces.first() else {
            return false;
        };

        self.local_brep
            .get_loop_vertex_indices(face.outer_loop)
            .contains(&vertex_id)
    }
}
