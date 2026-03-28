use std::collections::{HashMap, HashSet};

use openmaths::Vector3;

use crate::brep::BrepBuilder;

use super::super::validation::normalized;
use super::super::{
    BrepDiagnostic, ConstraintSettings, EditEffect, OGFreeformGeometry, TopologyChangeJournal,
    GEOMETRY_EPSILON,
};
use super::topology::undirected_edge_key;

impl OGFreeformGeometry {
    pub(in crate::editor) fn extrude_face_internal(
        &mut self,
        face_id: u32,
        distance: f64,
        open_surface_mode: bool,
    ) -> Result<EditEffect, BrepDiagnostic> {
        if !distance.is_finite() || distance.abs() <= GEOMETRY_EPSILON {
            return Err(BrepDiagnostic::error(
                "invalid_distance",
                "Extrude distance must be a finite non-zero number",
            )
            .with_domain("face", Some(face_id)));
        }

        let Some(face) = self
            .local_brep
            .faces
            .iter()
            .find(|candidate| candidate.id == face_id)
            .cloned()
        else {
            return Err(BrepDiagnostic::error(
                "missing_face",
                format!("Face {} does not exist", face_id),
            )
            .with_domain("face", Some(face_id)));
        };

        if !face.inner_loops.is_empty() {
            return Err(BrepDiagnostic::error(
                "unsupported_face",
                "extrudeFace currently supports faces without holes",
            )
            .with_domain("face", Some(face_id)));
        }

        if open_surface_mode {
            let mut effect =
                self.push_pull_face_internal(face_id, distance, &ConstraintSettings::default())?;
            effect.diagnostics.push(
                BrepDiagnostic::info(
                    "open_surface_mode",
                    "openSurfaceMode uses topology-preserving face translation",
                )
                .with_domain("face", Some(face_id)),
            );
            return Ok(effect);
        }

        let loop_vertices = self.local_brep.get_loop_vertex_indices(face.outer_loop);
        if loop_vertices.len() < 3 {
            return Err(BrepDiagnostic::error(
                "invalid_face_topology",
                "Face has insufficient vertices for extrusion",
            )
            .with_domain("face", Some(face_id)));
        }

        let mut boundary_edge_count = 0usize;
        for edge in &self.local_brep.edges {
            let mut incident_count = 0usize;
            if let Some((a, b)) = self.local_brep.get_edge_endpoints(edge.id) {
                for index in 0..loop_vertices.len() {
                    let from = loop_vertices[index];
                    let to = loop_vertices[(index + 1) % loop_vertices.len()];
                    if undirected_edge_key(a, b) == undirected_edge_key(from, to) {
                        incident_count = self.incident_face_count(edge.id);
                        break;
                    }
                }
            }
            if incident_count == 1 {
                boundary_edge_count += 1;
            }
        }

        if boundary_edge_count == 0 {
            let mut effect =
                self.push_pull_face_internal(face_id, distance, &ConstraintSettings::default())?;
            effect.diagnostics.push(
                BrepDiagnostic::info(
                    "solid_face_extrude",
                    "Closed-solid face extrusion resolved to push/pull translation",
                )
                .with_domain("face", Some(face_id)),
            );
            return Ok(effect);
        }

        if !self.supports_single_face_topology_edits() {
            return Err(BrepDiagnostic::error(
                "unsupported_topology",
                "extrudeFace currently supports open-surface extrusion on single-face entities",
            )
            .with_domain("face", Some(face_id)));
        }

        let Some(normal) = normalized(face.normal) else {
            return Err(BrepDiagnostic::error(
                "invalid_face_normal",
                "Face normal is invalid for extrusion",
            )
            .with_domain("face", Some(face_id)));
        };

        let old_brep = self.local_brep.clone();
        let old_positions: HashMap<u32, Vector3> = old_brep
            .vertices
            .iter()
            .map(|vertex| (vertex.id, vertex.position))
            .collect();

        let mut positions = old_brep
            .vertices
            .iter()
            .map(|vertex| vertex.position)
            .collect::<Vec<_>>();

        for vertex_id in &loop_vertices {
            let Some(vertex) = positions.get_mut(*vertex_id as usize) else {
                return Err(BrepDiagnostic::error(
                    "missing_vertex_reference",
                    format!("Face {} references missing vertex {}", face_id, vertex_id),
                )
                .with_domain("vertex", Some(*vertex_id)));
            };
            vertex.x += normal.x * distance;
            vertex.y += normal.y * distance;
            vertex.z += normal.z * distance;
        }

        let mut bottom_vertex_map: HashMap<u32, u32> = HashMap::new();
        for vertex_id in &loop_vertices {
            let bottom_id = positions.len() as u32;
            let Some(original_position) = old_positions.get(vertex_id) else {
                return Err(BrepDiagnostic::error(
                    "missing_vertex_reference",
                    format!("Vertex {} not found in source face", vertex_id),
                )
                .with_domain("vertex", Some(*vertex_id)));
            };
            positions.push(*original_position);
            bottom_vertex_map.insert(*vertex_id, bottom_id);
        }

        let mut builder = BrepBuilder::new(self.local_brep.id);
        builder.add_vertices(&positions);

        builder.add_face(&loop_vertices, &[]).map_err(|error| {
            BrepDiagnostic::error(
                "extrude_failed",
                format!("Failed to build extruded top face: {}", error),
            )
            .with_domain("face", Some(face_id))
        })?;

        let bottom_loop: Vec<u32> = loop_vertices
            .iter()
            .rev()
            .map(|vertex_id| bottom_vertex_map[vertex_id])
            .collect();

        builder.add_face(&bottom_loop, &[]).map_err(|error| {
            BrepDiagnostic::error(
                "extrude_failed",
                format!("Failed to build extruded bottom face: {}", error),
            )
            .with_domain("face", Some(face_id))
        })?;

        for index in 0..loop_vertices.len() {
            let top_current = loop_vertices[index];
            let top_next = loop_vertices[(index + 1) % loop_vertices.len()];
            let bottom_current = bottom_vertex_map[&top_current];
            let bottom_next = bottom_vertex_map[&top_next];

            let side = vec![top_current, bottom_current, bottom_next, top_next];
            builder.add_face(&side, &[]).map_err(|error| {
                BrepDiagnostic::error(
                    "extrude_failed",
                    format!("Failed to build extruded side face: {}", error),
                )
                .with_domain("face", Some(face_id))
            })?;
        }

        builder.add_shell_from_all_faces(true).map_err(|error| {
            BrepDiagnostic::error(
                "extrude_failed",
                format!("Failed to build closed shell: {}", error),
            )
            .with_domain("face", Some(face_id))
        })?;

        self.local_brep = builder.build().map_err(|error| {
            BrepDiagnostic::error(
                "extrude_failed",
                format!("Failed to finalize extruded topology: {}", error),
            )
            .with_domain("face", Some(face_id))
        })?;

        let mut journal = TopologyChangeJournal::default();
        journal.faces.map(face_id, vec![0]);

        let after_edge_lookup = self.edge_lookup_by_endpoints();
        for edge in &old_brep.edges {
            if let Some((from, to)) = old_brep.get_edge_endpoints(edge.id) {
                let key = undirected_edge_key(from, to);
                if let Some(new_edge_id) = after_edge_lookup.get(&key).copied() {
                    journal.edges.map(edge.id, vec![new_edge_id]);
                }
            }
        }

        let new_face_ids: HashSet<u32> = self.local_brep.faces.iter().map(|face| face.id).collect();
        for face in new_face_ids {
            if face != 0 {
                journal.faces.add_created(face);
            }
        }

        let mapped_edges: HashSet<u32> =
            journal.edges.mapping.values().flatten().copied().collect();
        for edge in &self.local_brep.edges {
            if !mapped_edges.contains(&edge.id) {
                journal.edges.add_created(edge.id);
            }
        }

        let old_vertex_count = old_brep.vertices.len() as u32;
        for vertex_id in old_vertex_count..self.local_brep.vertices.len() as u32 {
            journal.vertices.add_created(vertex_id);
        }

        let changed_faces = self
            .local_brep
            .faces
            .iter()
            .map(|face| face.id)
            .collect::<Vec<_>>();
        let changed_edges = self
            .local_brep
            .edges
            .iter()
            .map(|edge| edge.id)
            .collect::<Vec<_>>();
        let changed_vertices = self
            .local_brep
            .vertices
            .iter()
            .map(|vertex| vertex.id)
            .collect::<Vec<_>>();

        Ok(EditEffect {
            diagnostics: vec![BrepDiagnostic::info(
                "face_extruded",
                format!("Face {} extruded successfully", face_id),
            )
            .with_domain("face", Some(face_id))],
            topology_journal: Some(journal),
            changed_faces,
            changed_edges,
            changed_vertices,
        })
    }
}
