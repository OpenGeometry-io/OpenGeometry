use crate::brep::BrepBuilder;

use super::super::{BrepDiagnostic, EditEffect, OGFreeformGeometry, TopologyChangeJournal};
use super::topology::undirected_edge_key;

impl OGFreeformGeometry {
    pub(in crate::editor) fn remove_vertex_internal(
        &mut self,
        vertex_id: u32,
    ) -> Result<EditEffect, BrepDiagnostic> {
        if !self.supports_single_face_topology_edits() {
            return Err(BrepDiagnostic::error(
                "unsupported_topology",
                "removeVertex requires a single-face open surface",
            )
            .with_domain("vertex", Some(vertex_id)));
        }

        let old_brep = self.local_brep.clone();
        let face = old_brep
            .faces
            .first()
            .expect("single-face topology checked above");
        let mut loop_vertices = old_brep.get_loop_vertex_indices(face.outer_loop);
        if loop_vertices.len() <= 3 {
            return Err(BrepDiagnostic::error(
                "insufficient_vertices",
                "removeVertex requires a face with at least four vertices",
            )
            .with_domain("vertex", Some(vertex_id)));
        }

        let loop_edges = self.single_face_loop_edges(&old_brep)?;

        let Some(vertex_index) = loop_vertices
            .iter()
            .position(|candidate| *candidate == vertex_id)
        else {
            return Err(BrepDiagnostic::error(
                "vertex_not_on_boundary",
                format!("Vertex {} is not part of the editable boundary", vertex_id),
            )
            .with_domain("vertex", Some(vertex_id)));
        };

        let previous_index = if vertex_index == 0 {
            loop_vertices.len() - 1
        } else {
            vertex_index - 1
        };
        let previous_vertex = loop_vertices[previous_index];
        let next_vertex = loop_vertices[(vertex_index + 1) % loop_vertices.len()];

        let previous_edge = loop_edges[previous_index];
        let next_edge = loop_edges[vertex_index];

        loop_vertices.remove(vertex_index);

        let positions = old_brep
            .vertices
            .iter()
            .map(|vertex| vertex.position)
            .collect::<Vec<_>>();

        let mut builder = BrepBuilder::new(self.local_brep.id);
        builder.add_vertices(&positions);
        builder.add_face(&loop_vertices, &[]).map_err(|error| {
            BrepDiagnostic::error(
                "vertex_remove_failed",
                format!(
                    "Failed to rebuild face without vertex {}: {}",
                    vertex_id, error
                ),
            )
            .with_domain("vertex", Some(vertex_id))
        })?;

        self.local_brep = builder.build().map_err(|error| {
            BrepDiagnostic::error(
                "vertex_remove_failed",
                format!("Failed to finalize vertex removal: {}", error),
            )
            .with_domain("vertex", Some(vertex_id))
        })?;

        let mut journal = TopologyChangeJournal::default();
        journal.faces.map(face.id, vec![0]);
        journal.vertices.map(vertex_id, Vec::new());

        let edge_lookup = self.edge_lookup_by_endpoints();
        let merged_key = undirected_edge_key(previous_vertex, next_vertex);
        let merged_edge = edge_lookup.get(&merged_key).copied();

        for edge in &old_brep.edges {
            if edge.id == previous_edge || edge.id == next_edge {
                journal
                    .edges
                    .map(edge.id, merged_edge.map(|id| vec![id]).unwrap_or_default());
                continue;
            }

            if let Some((start, end)) = old_brep.get_edge_endpoints(edge.id) {
                let key = undirected_edge_key(start, end);
                if let Some(new_edge) = edge_lookup.get(&key).copied() {
                    journal.edges.map(edge.id, vec![new_edge]);
                }
            }
        }

        let changed_faces = vec![0];
        let changed_edges = self.local_brep.edges.iter().map(|edge| edge.id).collect();
        let changed_vertices = vec![previous_vertex, vertex_id, next_vertex];

        Ok(EditEffect {
            diagnostics: vec![BrepDiagnostic::info(
                "vertex_removed",
                format!("Removed vertex {} from boundary", vertex_id),
            )
            .with_domain("vertex", Some(vertex_id))],
            topology_journal: Some(journal),
            changed_faces,
            changed_edges,
            changed_vertices,
        })
    }
}
