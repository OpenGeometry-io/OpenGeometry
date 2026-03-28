use crate::brep::BrepBuilder;
use openmaths::Vector3;

use super::super::{
    BrepDiagnostic, EditEffect, OGFreeformGeometry, TopologyChangeJournal, GEOMETRY_EPSILON,
};
use super::topology::undirected_edge_key;

impl OGFreeformGeometry {
    pub(in crate::editor) fn insert_vertex_on_edge_internal(
        &mut self,
        edge_id: u32,
        t: f64,
    ) -> Result<EditEffect, BrepDiagnostic> {
        if !self.supports_rebuildable_edge_topology_edits(edge_id) {
            return Err(BrepDiagnostic::error(
                "unsupported_topology",
                "insertVertexOnEdge requires an edge used by a rebuildable face loop or wire",
            )
            .with_domain("edge", Some(edge_id)));
        }

        if !t.is_finite() || t <= GEOMETRY_EPSILON || t >= 1.0 - GEOMETRY_EPSILON {
            return Err(BrepDiagnostic::error(
                "invalid_parameter",
                "insertVertexOnEdge expects t in (0, 1)",
            )
            .with_domain("edge", Some(edge_id)));
        }

        let old_brep = self.local_brep.clone();
        let (from_id, to_id) = old_brep.get_edge_endpoints(edge_id).ok_or_else(|| {
            BrepDiagnostic::error("missing_edge", format!("Edge {} does not exist", edge_id))
                .with_domain("edge", Some(edge_id))
        })?;

        let from = old_brep
            .vertices
            .get(from_id as usize)
            .map(|vertex| vertex.position)
            .ok_or_else(|| {
                BrepDiagnostic::error(
                    "missing_vertex_reference",
                    format!(
                        "Vertex {} referenced by edge {} is missing",
                        from_id, edge_id
                    ),
                )
                .with_domain("vertex", Some(from_id))
            })?;

        let to = old_brep
            .vertices
            .get(to_id as usize)
            .map(|vertex| vertex.position)
            .ok_or_else(|| {
                BrepDiagnostic::error(
                    "missing_vertex_reference",
                    format!("Vertex {} referenced by edge {} is missing", to_id, edge_id),
                )
                .with_domain("vertex", Some(to_id))
            })?;

        let inserted = Vector3::new(
            from.x + (to.x - from.x) * t,
            from.y + (to.y - from.y) * t,
            from.z + (to.z - from.z) * t,
        );

        let mut positions = old_brep
            .vertices
            .iter()
            .map(|vertex| vertex.position)
            .collect::<Vec<_>>();
        let inserted_vertex_id = positions.len() as u32;
        positions.push(inserted);

        let (updated_faces, affected_faces) = self.rebuild_faces_with_inserted_edge_vertex(
            &old_brep,
            from_id,
            to_id,
            inserted_vertex_id,
        );
        let (updated_wires, affected_wires) = self.rebuild_wires_with_inserted_edge_vertex(
            &old_brep,
            from_id,
            to_id,
            inserted_vertex_id,
        );

        if affected_faces.is_empty() && affected_wires.is_empty() {
            return Err(BrepDiagnostic::error(
                "unsupported_topology",
                "insertVertexOnEdge requires an edge used by a rebuildable face loop or wire",
            )
            .with_domain("edge", Some(edge_id)));
        }

        let mut builder = BrepBuilder::new(self.local_brep.id);
        builder.add_vertices(&positions);

        for (outer, holes) in &updated_faces {
            builder.add_face(outer, holes).map_err(|error| {
                BrepDiagnostic::error(
                    "edge_split_failed",
                    format!("Failed to rebuild face with inserted vertex: {}", error),
                )
                .with_domain("edge", Some(edge_id))
            })?;
        }

        for (wire_vertices, is_closed) in &updated_wires {
            builder
                .add_wire(wire_vertices, *is_closed)
                .map_err(|error| {
                    BrepDiagnostic::error(
                        "edge_split_failed",
                        format!("Failed to rebuild wire with inserted vertex: {}", error),
                    )
                    .with_domain("edge", Some(edge_id))
                })?;
        }

        for shell in &old_brep.shells {
            builder
                .add_shell(&shell.faces, shell.is_closed)
                .map_err(|error| {
                    BrepDiagnostic::error(
                        "edge_split_failed",
                        format!("Failed to rebuild shell after edge split: {}", error),
                    )
                    .with_domain("edge", Some(edge_id))
                })?;
        }

        self.local_brep = builder.build().map_err(|error| {
            BrepDiagnostic::error(
                "edge_split_failed",
                format!("Failed to finalize edge split: {}", error),
            )
            .with_domain("edge", Some(edge_id))
        })?;

        let mut journal = TopologyChangeJournal::default();
        for face_id in &affected_faces {
            journal.faces.map(*face_id, vec![*face_id]);
        }
        journal.vertices.add_created(inserted_vertex_id);

        let edge_lookup = self.edge_lookup_by_endpoints();
        for edge in &old_brep.edges {
            if edge.id == edge_id {
                let first_key = undirected_edge_key(from_id, inserted_vertex_id);
                let second_key = undirected_edge_key(inserted_vertex_id, to_id);

                let mut mapped = Vec::new();
                if let Some(id) = edge_lookup.get(&first_key).copied() {
                    mapped.push(id);
                }
                if let Some(id) = edge_lookup.get(&second_key).copied() {
                    mapped.push(id);
                }
                journal.edges.map(edge.id, mapped);
                continue;
            }

            if let Some((start, end)) = old_brep.get_edge_endpoints(edge.id) {
                let key = undirected_edge_key(start, end);
                if let Some(new_edge) = edge_lookup.get(&key).copied() {
                    journal.edges.map(edge.id, vec![new_edge]);
                }
            }
        }

        let mut changed_faces = affected_faces;
        changed_faces.sort_unstable();
        changed_faces.dedup();
        let changed_edges = self.local_brep.edges.iter().map(|edge| edge.id).collect();
        let changed_vertices = vec![from_id, to_id, inserted_vertex_id];

        Ok(EditEffect {
            diagnostics: vec![BrepDiagnostic::info(
                "vertex_inserted_on_edge",
                format!("Inserted vertex on edge {}", edge_id),
            )
            .with_domain("edge", Some(edge_id))],
            topology_journal: Some(journal),
            changed_faces,
            changed_edges,
            changed_vertices,
        })
    }

    pub(in crate::editor) fn split_edge_internal(
        &mut self,
        edge_id: u32,
        t: f64,
    ) -> Result<EditEffect, BrepDiagnostic> {
        self.insert_vertex_on_edge_internal(edge_id, t)
    }
}
