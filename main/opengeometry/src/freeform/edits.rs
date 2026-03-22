use std::collections::{HashMap, HashSet};

use openmaths::Vector3;

use crate::brep::Brep;
use crate::brep::BrepBuilder;

use super::inspection::collect_face_vertex_ids;
use super::validation::{is_finite_vector, normalized};
use super::{
    BrepDiagnostic, ConstraintSettings, EditEffect, OGFreeformGeometry, TopologyChangeJournal,
    GEOMETRY_EPSILON,
};

#[derive(Clone)]
pub(super) struct LoopCutFaceStep {
    pub face_id: u32,
    pub input_edge_id: u32,
    pub opposite_edge_id: u32,
}

#[derive(Clone)]
pub(super) struct LoopCutRing {
    pub edge_ids: Vec<u32>,
    pub face_steps: Vec<LoopCutFaceStep>,
}

impl OGFreeformGeometry {
    pub(super) fn push_pull_face_internal(
        &mut self,
        face_id: u32,
        distance: f64,
        constraints: &ConstraintSettings,
    ) -> Result<EditEffect, BrepDiagnostic> {
        if !distance.is_finite() {
            return Err(BrepDiagnostic::error(
                "invalid_distance",
                "Face push/pull distance must be a finite number",
            )
            .with_domain("face", Some(face_id)));
        }

        let Some(face) = self
            .local_brep
            .faces
            .iter()
            .find(|candidate| candidate.id == face_id)
        else {
            return Err(BrepDiagnostic::error(
                "missing_face",
                format!("Face {} does not exist", face_id),
            )
            .with_domain("face", Some(face_id)));
        };

        let Some(normal) = normalized(face.normal) else {
            return Err(BrepDiagnostic::error(
                "invalid_face_normal",
                format!("Face {} does not have a valid normal", face_id),
            )
            .with_domain("face", Some(face_id)));
        };

        let base_translation = Vector3::new(
            normal.x * distance,
            normal.y * distance,
            normal.z * distance,
        );
        self.translate_face_by_vector_internal(face_id, base_translation, constraints)
    }

    pub(super) fn translate_face_by_vector_internal(
        &mut self,
        face_id: u32,
        translation: Vector3,
        constraints: &ConstraintSettings,
    ) -> Result<EditEffect, BrepDiagnostic> {
        if !is_finite_vector(translation) {
            return Err(BrepDiagnostic::error(
                "invalid_translation",
                "Face translation must contain finite numbers",
            )
            .with_domain("face", Some(face_id)));
        }

        let vertex_ids = {
            let Some(face) = self
                .local_brep
                .faces
                .iter()
                .find(|candidate| candidate.id == face_id)
            else {
                return Err(BrepDiagnostic::error(
                    "missing_face",
                    format!("Face {} does not exist", face_id),
                )
                .with_domain("face", Some(face_id)));
            };
            collect_face_vertex_ids(&self.local_brep, face).map_err(|message| {
                BrepDiagnostic::error("invalid_face_topology", message)
                    .with_domain("face", Some(face_id))
            })?
        };

        let (translation, mut diagnostics) =
            self.apply_constraints(translation, constraints, "face", face_id)?;

        for vertex_id in &vertex_ids {
            let Some(vertex) = self.local_brep.vertices.get_mut(*vertex_id as usize) else {
                return Err(BrepDiagnostic::error(
                    "missing_vertex_reference",
                    format!("Face {} references missing vertex {}", face_id, vertex_id),
                )
                .with_domain("vertex", Some(*vertex_id)));
            };

            vertex.position.x += translation.x;
            vertex.position.y += translation.y;
            vertex.position.z += translation.z;
        }

        self.local_brep.recompute_face_normals();

        let mut moved_vertices = HashSet::new();
        moved_vertices.extend(vertex_ids.iter().copied());
        let (changed_faces, changed_edges, changed_vertices) =
            self.collect_changed_domains_for_vertices(&moved_vertices);

        diagnostics.push(
            BrepDiagnostic::info(
                "face_translated",
                format!("Face {} translated successfully", face_id),
            )
            .with_domain("face", Some(face_id)),
        );

        Ok(EditEffect {
            diagnostics,
            topology_journal: None,
            changed_faces,
            changed_edges,
            changed_vertices,
        })
    }

    pub(super) fn move_edge_internal(
        &mut self,
        edge_id: u32,
        translation: Vector3,
        constraints: &ConstraintSettings,
    ) -> Result<EditEffect, BrepDiagnostic> {
        if !is_finite_vector(translation) {
            return Err(BrepDiagnostic::error(
                "invalid_translation",
                "Edge translation must contain finite numbers",
            )
            .with_domain("edge", Some(edge_id)));
        }

        let (start_id, end_id) = self.local_brep.get_edge_endpoints(edge_id).ok_or_else(|| {
            BrepDiagnostic::error("missing_edge", format!("Edge {} does not exist", edge_id))
                .with_domain("edge", Some(edge_id))
        })?;

        let (translation, diagnostics) =
            self.apply_constraints(translation, constraints, "edge", edge_id)?;

        let mut unique_vertices = HashSet::new();
        unique_vertices.insert(start_id);
        unique_vertices.insert(end_id);

        for vertex_id in &unique_vertices {
            let Some(vertex) = self.local_brep.vertices.get_mut(*vertex_id as usize) else {
                return Err(BrepDiagnostic::error(
                    "missing_vertex_reference",
                    format!("Edge {} references missing vertex {}", edge_id, vertex_id),
                )
                .with_domain("vertex", Some(*vertex_id)));
            };

            vertex.position.x += translation.x;
            vertex.position.y += translation.y;
            vertex.position.z += translation.z;
        }

        self.local_brep.recompute_face_normals();

        let (changed_faces, changed_edges, changed_vertices) =
            self.collect_changed_domains_for_vertices(&unique_vertices);

        Ok(EditEffect {
            diagnostics,
            topology_journal: None,
            changed_faces,
            changed_edges,
            changed_vertices,
        })
    }

    pub(super) fn move_vertex_internal(
        &mut self,
        vertex_id: u32,
        translation: Vector3,
        constraints: &ConstraintSettings,
    ) -> Result<EditEffect, BrepDiagnostic> {
        if !is_finite_vector(translation) {
            return Err(BrepDiagnostic::error(
                "invalid_translation",
                "Vertex translation must contain finite numbers",
            )
            .with_domain("vertex", Some(vertex_id)));
        }

        if self.local_brep.vertices.get(vertex_id as usize).is_none() {
            return Err(BrepDiagnostic::error(
                "missing_vertex",
                format!("Vertex {} does not exist", vertex_id),
            )
            .with_domain("vertex", Some(vertex_id)));
        }

        let (translation, diagnostics) =
            self.apply_constraints(translation, constraints, "vertex", vertex_id)?;

        let vertex = self
            .local_brep
            .vertices
            .get_mut(vertex_id as usize)
            .expect("vertex existence checked above");

        vertex.position.x += translation.x;
        vertex.position.y += translation.y;
        vertex.position.z += translation.z;

        self.local_brep.recompute_face_normals();

        let mut moved_vertices = HashSet::new();
        moved_vertices.insert(vertex_id);
        let (changed_faces, changed_edges, changed_vertices) =
            self.collect_changed_domains_for_vertices(&moved_vertices);

        Ok(EditEffect {
            diagnostics,
            topology_journal: None,
            changed_faces,
            changed_edges,
            changed_vertices,
        })
    }

    pub(super) fn extrude_face_internal(
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

    pub(super) fn insert_vertex_on_edge_internal(
        &mut self,
        edge_id: u32,
        t: f64,
    ) -> Result<EditEffect, BrepDiagnostic> {
        if !self.supports_face_backed_edge_topology_edits(edge_id) {
            return Err(BrepDiagnostic::error(
                "unsupported_topology",
                "insertVertexOnEdge requires an edge used by at least one face",
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

        let mut affected_faces = Vec::new();
        let updated_faces = self.rebuild_faces_with_inserted_edge_vertex(
            &old_brep,
            edge_id,
            from_id,
            to_id,
            inserted_vertex_id,
            &mut affected_faces,
        )?;

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

    pub(super) fn split_edge_internal(
        &mut self,
        edge_id: u32,
        t: f64,
    ) -> Result<EditEffect, BrepDiagnostic> {
        self.insert_vertex_on_edge_internal(edge_id, t)
    }

    pub(super) fn cut_face_internal(
        &mut self,
        face_id: u32,
        start_edge_id: u32,
        start_t: f64,
        end_edge_id: u32,
        end_t: f64,
    ) -> Result<EditEffect, BrepDiagnostic> {
        if start_edge_id == end_edge_id {
            return Err(BrepDiagnostic::error(
                "invalid_topology_selection",
                "cutFace requires two distinct edges on the target face",
            )
            .with_domain("face", Some(face_id)));
        }

        for (edge_id, t_value) in [(start_edge_id, start_t), (end_edge_id, end_t)] {
            if !t_value.is_finite()
                || t_value <= GEOMETRY_EPSILON
                || t_value >= 1.0 - GEOMETRY_EPSILON
            {
                return Err(BrepDiagnostic::error(
                    "invalid_parameter",
                    "cutFace expects t in (0, 1)",
                )
                .with_domain("edge", Some(edge_id)));
            }
        }

        if !self.supports_face_cut_on_face(face_id) {
            return Err(BrepDiagnostic::error(
                "unsupported_topology",
                "cutFace currently requires a face without holes and at least four boundary edges",
            )
            .with_domain("face", Some(face_id)));
        }

        let old_brep = self.local_brep.clone();
        let face = old_brep
            .faces
            .iter()
            .find(|candidate| candidate.id == face_id)
            .cloned()
            .ok_or_else(|| {
                BrepDiagnostic::error("missing_face", format!("Face {} does not exist", face_id))
                    .with_domain("face", Some(face_id))
            })?;

        let face_vertices = old_brep.get_loop_vertex_indices(face.outer_loop);
        let face_edges = face_outer_loop_edges(&old_brep, face_id)?;

        let start_edge_index = face_edges
            .iter()
            .position(|candidate| *candidate == start_edge_id)
            .ok_or_else(|| {
                BrepDiagnostic::error(
                    "edge_not_on_face",
                    format!("Edge {} is not part of face {}", start_edge_id, face_id),
                )
                .with_domain("edge", Some(start_edge_id))
            })?;
        let end_edge_index = face_edges
            .iter()
            .position(|candidate| *candidate == end_edge_id)
            .ok_or_else(|| {
                BrepDiagnostic::error(
                    "edge_not_on_face",
                    format!("Edge {} is not part of face {}", end_edge_id, face_id),
                )
                .with_domain("edge", Some(end_edge_id))
            })?;

        let start_from = face_vertices[start_edge_index];
        let start_to = face_vertices[(start_edge_index + 1) % face_vertices.len()];
        let end_from = face_vertices[end_edge_index];
        let end_to = face_vertices[(end_edge_index + 1) % face_vertices.len()];

        let start_inserted = interpolate_loop_edge_position(
            &old_brep,
            start_from,
            start_to,
            start_t,
            start_edge_id,
        )?;
        let end_inserted =
            interpolate_loop_edge_position(&old_brep, end_from, end_to, end_t, end_edge_id)?;

        let mut positions = old_brep
            .vertices
            .iter()
            .map(|vertex| vertex.position)
            .collect::<Vec<_>>();
        let start_inserted_vertex_id = positions.len() as u32;
        positions.push(start_inserted);
        let end_inserted_vertex_id = positions.len() as u32;
        positions.push(end_inserted);

        let mut primary_faces = Vec::with_capacity(old_brep.faces.len());
        let mut secondary_face_loop = None;
        let mut changed_faces = HashSet::new();

        for candidate_face in &old_brep.faces {
            let outer_loop = old_brep.get_loop_vertex_indices(candidate_face.outer_loop);
            let (outer_with_start, outer_start_changed) = insert_vertex_into_loop(
                &outer_loop,
                start_from,
                start_to,
                start_inserted_vertex_id,
            );
            let (outer_with_both, outer_end_changed) = insert_vertex_into_loop(
                &outer_with_start,
                end_from,
                end_to,
                end_inserted_vertex_id,
            );

            let mut holes = Vec::with_capacity(candidate_face.inner_loops.len());
            let mut hole_changed = false;
            for loop_id in &candidate_face.inner_loops {
                let hole_loop = old_brep.get_loop_vertex_indices(*loop_id);
                let (hole_with_start, hole_start_changed) = insert_vertex_into_loop(
                    &hole_loop,
                    start_from,
                    start_to,
                    start_inserted_vertex_id,
                );
                let (hole_with_both, hole_end_changed) = insert_vertex_into_loop(
                    &hole_with_start,
                    end_from,
                    end_to,
                    end_inserted_vertex_id,
                );
                hole_changed |= hole_start_changed || hole_end_changed;
                holes.push(hole_with_both);
            }

            if candidate_face.id == face_id {
                if !outer_start_changed || !outer_end_changed {
                    return Err(BrepDiagnostic::error(
                        "cut_face_failed",
                        format!(
                            "Failed to insert cut points into the boundary of face {}",
                            face_id
                        ),
                    )
                    .with_domain("face", Some(face_id)));
                }

                let (primary_loop, secondary_loop) = split_loop_between_vertices(
                    &outer_with_both,
                    start_inserted_vertex_id,
                    end_inserted_vertex_id,
                    face_id,
                )?;

                primary_faces.push((primary_loop, Vec::new()));
                secondary_face_loop = Some(secondary_loop);
                changed_faces.insert(face_id);
                continue;
            }

            if outer_start_changed || outer_end_changed || hole_changed {
                changed_faces.insert(candidate_face.id);
            }

            primary_faces.push((outer_with_both, holes));
        }

        let mut builder = BrepBuilder::new(self.local_brep.id);
        builder.add_vertices(&positions);

        for (outer, holes) in &primary_faces {
            builder.add_face(outer, holes).map_err(|error| {
                BrepDiagnostic::error(
                    "cut_face_failed",
                    format!("Failed to rebuild face during cutFace: {}", error),
                )
                .with_domain("face", Some(face_id))
            })?;
        }

        let secondary_face_loop = secondary_face_loop.ok_or_else(|| {
            BrepDiagnostic::error(
                "cut_face_failed",
                "Failed to create the second face produced by cutFace",
            )
            .with_domain("face", Some(face_id))
        })?;
        let new_face_id = builder
            .add_face(&secondary_face_loop, &[])
            .map_err(|error| {
                BrepDiagnostic::error(
                    "cut_face_failed",
                    format!("Failed to append the second cut face: {}", error),
                )
                .with_domain("face", Some(face_id))
            })?;

        for shell in &old_brep.shells {
            let mut shell_faces = Vec::new();
            for old_face_id in &shell.faces {
                shell_faces.push(*old_face_id);
                if *old_face_id == face_id {
                    shell_faces.push(new_face_id);
                }
            }

            builder
                .add_shell(&shell_faces, shell.is_closed)
                .map_err(|error| {
                    BrepDiagnostic::error(
                        "cut_face_failed",
                        format!("Failed to rebuild shell after cutFace: {}", error),
                    )
                    .with_domain("face", Some(face_id))
                })?;
        }

        self.local_brep = builder.build().map_err(|error| {
            BrepDiagnostic::error(
                "cut_face_failed",
                format!("Failed to finalize cutFace: {}", error),
            )
            .with_domain("face", Some(face_id))
        })?;

        let mut journal = TopologyChangeJournal::default();
        journal.faces.map(face_id, vec![face_id, new_face_id]);
        journal.faces.add_created(new_face_id);
        journal.vertices.add_created(start_inserted_vertex_id);
        journal.vertices.add_created(end_inserted_vertex_id);

        let edge_lookup = self.edge_lookup_by_endpoints();
        let mut changed_edges = HashSet::new();

        for edge in &old_brep.edges {
            let Some((from, to)) = old_brep.get_edge_endpoints(edge.id) else {
                continue;
            };

            let mapped = if edge.id == start_edge_id || edge.id == end_edge_id {
                let inserted_vertex_id = if edge.id == start_edge_id {
                    start_inserted_vertex_id
                } else {
                    end_inserted_vertex_id
                };

                let mut split_edges = Vec::new();
                let first_key = undirected_edge_key(from, inserted_vertex_id);
                let second_key = undirected_edge_key(inserted_vertex_id, to);

                if let Some(mapped_id) = edge_lookup.get(&first_key).copied() {
                    split_edges.push(mapped_id);
                }
                if let Some(mapped_id) = edge_lookup.get(&second_key).copied() {
                    split_edges.push(mapped_id);
                }

                changed_edges.extend(split_edges.iter().copied());
                split_edges
            } else {
                let key = undirected_edge_key(from, to);
                edge_lookup
                    .get(&key)
                    .copied()
                    .map(|mapped_id| vec![mapped_id])
                    .unwrap_or_default()
            };

            journal.edges.map(edge.id, mapped);
        }

        let connecting_edge_key =
            undirected_edge_key(start_inserted_vertex_id, end_inserted_vertex_id);
        let connecting_edge_id =
            edge_lookup
                .get(&connecting_edge_key)
                .copied()
                .ok_or_else(|| {
                    BrepDiagnostic::error(
                        "cut_face_failed",
                        "cutFace did not produce the new connecting edge",
                    )
                    .with_domain("face", Some(face_id))
                })?;
        journal.edges.add_created(connecting_edge_id);
        changed_edges.insert(connecting_edge_id);

        changed_faces.insert(new_face_id);

        let mut changed_faces = changed_faces.into_iter().collect::<Vec<_>>();
        changed_faces.sort_unstable();

        let mut changed_edges = changed_edges.into_iter().collect::<Vec<_>>();
        changed_edges.sort_unstable();

        let changed_vertices = vec![start_inserted_vertex_id, end_inserted_vertex_id];

        Ok(EditEffect {
            diagnostics: vec![BrepDiagnostic::info(
                "face_cut_applied",
                format!(
                    "Cut face {} between edges {} and {}",
                    face_id, start_edge_id, end_edge_id
                ),
            )
            .with_domain("face", Some(face_id))],
            topology_journal: Some(journal),
            changed_faces,
            changed_edges,
            changed_vertices,
        })
    }

    pub(super) fn loop_cut_edge_ring_internal(
        &mut self,
        edge_id: u32,
        t: f64,
    ) -> Result<EditEffect, BrepDiagnostic> {
        if !t.is_finite() || t <= GEOMETRY_EPSILON || t >= 1.0 - GEOMETRY_EPSILON {
            return Err(
                BrepDiagnostic::error("invalid_parameter", "loopCut expects t in (0, 1)")
                    .with_domain("edge", Some(edge_id)),
            );
        }

        let old_brep = self.local_brep.clone();
        let ring = collect_closed_quad_edge_ring(&old_brep, edge_id)?;

        let (reference_from, reference_to) =
            old_brep.get_edge_endpoints(edge_id).ok_or_else(|| {
                BrepDiagnostic::error("missing_edge", format!("Edge {} does not exist", edge_id))
                    .with_domain("edge", Some(edge_id))
            })?;

        let reference_direction = old_brep
            .vertices
            .get(reference_to as usize)
            .zip(old_brep.vertices.get(reference_from as usize))
            .and_then(|(to, from)| {
                normalized(Vector3::new(
                    to.position.x - from.position.x,
                    to.position.y - from.position.y,
                    to.position.z - from.position.z,
                ))
            });

        let mut positions = old_brep
            .vertices
            .iter()
            .map(|vertex| vertex.position)
            .collect::<Vec<_>>();
        let mut inserted_vertex_by_edge = HashMap::new();

        for ring_edge_id in &ring.edge_ids {
            let (from_id, to_id) = old_brep.get_edge_endpoints(*ring_edge_id).ok_or_else(|| {
                BrepDiagnostic::error(
                    "missing_edge",
                    format!("Edge {} does not exist", ring_edge_id),
                )
                .with_domain("edge", Some(*ring_edge_id))
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
                            from_id, ring_edge_id
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
                        format!(
                            "Vertex {} referenced by edge {} is missing",
                            to_id, ring_edge_id
                        ),
                    )
                    .with_domain("vertex", Some(to_id))
                })?;

            let edge_direction =
                normalized(Vector3::new(to.x - from.x, to.y - from.y, to.z - from.z));
            let local_t =
                if let (Some(reference), Some(direction)) = (reference_direction, edge_direction) {
                    if reference.dot(&direction) < 0.0 {
                        1.0 - t
                    } else {
                        t
                    }
                } else {
                    t
                };

            let inserted = Vector3::new(
                from.x + (to.x - from.x) * local_t,
                from.y + (to.y - from.y) * local_t,
                from.z + (to.z - from.z) * local_t,
            );

            let inserted_vertex_id = positions.len() as u32;
            positions.push(inserted);
            inserted_vertex_by_edge.insert(*ring_edge_id, inserted_vertex_id);
        }

        let face_step_by_id = ring
            .face_steps
            .iter()
            .map(|step| (step.face_id, step))
            .collect::<HashMap<_, _>>();

        let mut primary_faces = Vec::with_capacity(old_brep.faces.len());
        let mut extra_face_specs = Vec::<(u32, Vec<u32>)>::new();

        for face in &old_brep.faces {
            let outer_loop = old_brep.get_loop_vertex_indices(face.outer_loop);

            if let Some(step) = face_step_by_id.get(&face.id) {
                let inserted_a = *inserted_vertex_by_edge
                    .get(&step.input_edge_id)
                    .ok_or_else(|| {
                        BrepDiagnostic::error(
                            "loop_cut_failed",
                            format!("Missing inserted vertex for edge {}", step.input_edge_id),
                        )
                        .with_domain("edge", Some(step.input_edge_id))
                    })?;
                let inserted_b = *inserted_vertex_by_edge
                    .get(&step.opposite_edge_id)
                    .ok_or_else(|| {
                        BrepDiagnostic::error(
                            "loop_cut_failed",
                            format!("Missing inserted vertex for edge {}", step.opposite_edge_id),
                        )
                        .with_domain("edge", Some(step.opposite_edge_id))
                    })?;

                let (input_from, input_to) = old_brep
                    .get_edge_endpoints(step.input_edge_id)
                    .ok_or_else(|| {
                        BrepDiagnostic::error(
                            "missing_edge",
                            format!("Edge {} does not exist", step.input_edge_id),
                        )
                        .with_domain("edge", Some(step.input_edge_id))
                    })?;
                let (opposite_from, opposite_to) = old_brep
                    .get_edge_endpoints(step.opposite_edge_id)
                    .ok_or_else(|| {
                        BrepDiagnostic::error(
                            "missing_edge",
                            format!("Edge {} does not exist", step.opposite_edge_id),
                        )
                        .with_domain("edge", Some(step.opposite_edge_id))
                    })?;

                let (with_first, inserted_first) =
                    insert_vertex_into_loop(&outer_loop, input_from, input_to, inserted_a);
                let (with_both, inserted_second) =
                    insert_vertex_into_loop(&with_first, opposite_from, opposite_to, inserted_b);

                if !inserted_first || !inserted_second {
                    return Err(BrepDiagnostic::error(
                        "loop_cut_failed",
                        format!(
                            "Failed to insert loop-cut vertices into face {} boundary",
                            face.id
                        ),
                    )
                    .with_domain("face", Some(face.id)));
                }

                let (primary_loop, secondary_loop) =
                    split_loop_between_vertices(&with_both, inserted_a, inserted_b, face.id)?;

                primary_faces.push((primary_loop, Vec::new()));
                extra_face_specs.push((face.id, secondary_loop));
                continue;
            }

            let holes = face
                .inner_loops
                .iter()
                .map(|loop_id| old_brep.get_loop_vertex_indices(*loop_id))
                .collect::<Vec<_>>();
            primary_faces.push((outer_loop, holes));
        }

        let mut builder = BrepBuilder::new(self.local_brep.id);
        builder.add_vertices(&positions);

        for (outer, holes) in &primary_faces {
            builder.add_face(outer, holes).map_err(|error| {
                BrepDiagnostic::error(
                    "loop_cut_failed",
                    format!("Failed to rebuild primary face during loop cut: {}", error),
                )
                .with_domain("edge", Some(edge_id))
            })?;
        }

        let old_face_count = old_brep.faces.len() as u32;
        let mut extra_face_id_by_old = HashMap::new();

        for (index, (old_face_id, outer)) in extra_face_specs.iter().enumerate() {
            builder.add_face(outer, &[]).map_err(|error| {
                BrepDiagnostic::error(
                    "loop_cut_failed",
                    format!("Failed to rebuild split face during loop cut: {}", error),
                )
                .with_domain("face", Some(*old_face_id))
            })?;
            extra_face_id_by_old.insert(*old_face_id, old_face_count + index as u32);
        }

        for shell in &old_brep.shells {
            let mut shell_faces = Vec::new();
            for old_face_id in &shell.faces {
                shell_faces.push(*old_face_id);
                if let Some(extra_face_id) = extra_face_id_by_old.get(old_face_id) {
                    shell_faces.push(*extra_face_id);
                }
            }

            builder
                .add_shell(&shell_faces, shell.is_closed)
                .map_err(|error| {
                    BrepDiagnostic::error(
                        "loop_cut_failed",
                        format!("Failed to rebuild shell after loop cut: {}", error),
                    )
                    .with_domain("edge", Some(edge_id))
                })?;
        }

        self.local_brep = builder.build().map_err(|error| {
            BrepDiagnostic::error(
                "loop_cut_failed",
                format!("Failed to finalize loop cut: {}", error),
            )
            .with_domain("edge", Some(edge_id))
        })?;

        let mut journal = TopologyChangeJournal::default();
        for (old_face_id, extra_face_id) in &extra_face_id_by_old {
            journal
                .faces
                .map(*old_face_id, vec![*old_face_id, *extra_face_id]);
            journal.faces.add_created(*extra_face_id);
        }

        for inserted_vertex_id in inserted_vertex_by_edge.values() {
            journal.vertices.add_created(*inserted_vertex_id);
        }

        let edge_lookup = self.edge_lookup_by_endpoints();
        let mut changed_edges = HashSet::new();

        for edge in &old_brep.edges {
            let Some((from, to)) = old_brep.get_edge_endpoints(edge.id) else {
                continue;
            };

            let mapped = if let Some(inserted_vertex_id) = inserted_vertex_by_edge.get(&edge.id) {
                let mut split_edges = Vec::new();
                let first_key = undirected_edge_key(from, *inserted_vertex_id);
                let second_key = undirected_edge_key(*inserted_vertex_id, to);

                if let Some(id) = edge_lookup.get(&first_key).copied() {
                    split_edges.push(id);
                }
                if let Some(id) = edge_lookup.get(&second_key).copied() {
                    split_edges.push(id);
                }

                split_edges
            } else {
                let key = undirected_edge_key(from, to);
                edge_lookup
                    .get(&key)
                    .copied()
                    .map(|id| vec![id])
                    .unwrap_or_default()
            };

            changed_edges.extend(mapped.iter().copied());
            journal.edges.map(edge.id, mapped);
        }

        for step in &ring.face_steps {
            let inserted_a = inserted_vertex_by_edge[&step.input_edge_id];
            let inserted_b = inserted_vertex_by_edge[&step.opposite_edge_id];
            let key = undirected_edge_key(inserted_a, inserted_b);
            if let Some(new_edge_id) = edge_lookup.get(&key).copied() {
                journal.edges.add_created(new_edge_id);
                changed_edges.insert(new_edge_id);
            }
        }

        let mut changed_faces = ring
            .face_steps
            .iter()
            .map(|step| step.face_id)
            .collect::<Vec<_>>();
        changed_faces.extend(extra_face_id_by_old.values().copied());
        changed_faces.sort_unstable();
        changed_faces.dedup();

        let mut changed_edges = changed_edges.into_iter().collect::<Vec<_>>();
        changed_edges.sort_unstable();

        let mut changed_vertices = inserted_vertex_by_edge
            .values()
            .copied()
            .collect::<Vec<_>>();
        changed_vertices.sort_unstable();
        changed_vertices.dedup();

        Ok(EditEffect {
            diagnostics: vec![BrepDiagnostic::info(
                "loop_cut_applied",
                format!(
                    "Applied loop cut across {} quad faces starting from edge {}",
                    ring.face_steps.len(),
                    edge_id
                ),
            )
            .with_domain("edge", Some(edge_id))],
            topology_journal: Some(journal),
            changed_faces,
            changed_edges,
            changed_vertices,
        })
    }

    pub(super) fn remove_vertex_internal(
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

    fn apply_constraints(
        &self,
        incoming: Vector3,
        constraints: &ConstraintSettings,
        domain: &str,
        topology_id: u32,
    ) -> Result<(Vector3, Vec<BrepDiagnostic>), BrepDiagnostic> {
        let mut translation = incoming;
        let mut diagnostics = Vec::new();

        if !is_finite_vector(incoming) {
            return Err(BrepDiagnostic::error(
                "invalid_translation",
                "Translation contains non-finite numbers",
            )
            .with_domain(domain, Some(topology_id)));
        }

        if constraints.axis.is_some() && constraints.plane_normal.is_some() {
            return Err(BrepDiagnostic::error(
                "conflicting_constraints",
                "Specify either axis or plane constraint, not both",
            )
            .with_domain(domain, Some(topology_id)));
        }

        if constraints.constraint_frame == "world" {
            diagnostics.push(
                BrepDiagnostic::warning(
                    "world_frame_not_supported",
                    "World-frame constraints are currently interpreted in local space",
                )
                .with_domain(domain, Some(topology_id)),
            );
        }

        if let Some(axis) = constraints.axis {
            let Some(axis_normalized) = normalized(axis) else {
                return Err(BrepDiagnostic::error(
                    "invalid_axis_constraint",
                    "Axis constraint must have non-zero length",
                )
                .with_domain(domain, Some(topology_id)));
            };

            let projection = translation.dot(&axis_normalized);
            translation = Vector3::new(
                axis_normalized.x * projection,
                axis_normalized.y * projection,
                axis_normalized.z * projection,
            );
        }

        if let Some(plane_normal) = constraints.plane_normal {
            let Some(plane_normalized) = normalized(plane_normal) else {
                return Err(BrepDiagnostic::error(
                    "invalid_plane_constraint",
                    "Plane normal constraint must have non-zero length",
                )
                .with_domain(domain, Some(topology_id)));
            };

            let projection = translation.dot(&plane_normalized);
            translation = Vector3::new(
                translation.x - plane_normalized.x * projection,
                translation.y - plane_normalized.y * projection,
                translation.z - plane_normalized.z * projection,
            );
        }

        if constraints.preserve_coplanarity {
            diagnostics.push(
                BrepDiagnostic::info(
                    "preserve_coplanarity_requested",
                    "preserveCoplanarity hint recorded; direct vertex/edge edits may still alter adjacent coplanarity",
                )
                .with_domain(domain, Some(topology_id)),
            );
        }

        if !is_finite_vector(translation) {
            return Err(BrepDiagnostic::error(
                "invalid_translation",
                "Constraint resolution produced non-finite translation",
            )
            .with_domain(domain, Some(topology_id)));
        }

        Ok((translation, diagnostics))
    }

    fn collect_changed_domains_for_vertices(
        &self,
        vertex_ids: &HashSet<u32>,
    ) -> (Vec<u32>, Vec<u32>, Vec<u32>) {
        let mut changed_faces = HashSet::new();
        let mut changed_edges = HashSet::new();

        for halfedge in &self.local_brep.halfedges {
            if vertex_ids.contains(&halfedge.from) || vertex_ids.contains(&halfedge.to) {
                changed_edges.insert(halfedge.edge);
                if let Some(face_id) = halfedge.face {
                    changed_faces.insert(face_id);
                }
            }
        }

        let mut face_ids = changed_faces.into_iter().collect::<Vec<_>>();
        face_ids.sort_unstable();

        let mut edge_ids = changed_edges.into_iter().collect::<Vec<_>>();
        edge_ids.sort_unstable();

        let mut vertices = vertex_ids.iter().copied().collect::<Vec<_>>();
        vertices.sort_unstable();

        (face_ids, edge_ids, vertices)
    }

    fn edge_lookup_by_endpoints(&self) -> HashMap<(u32, u32), u32> {
        let mut lookup = HashMap::new();
        for edge in &self.local_brep.edges {
            if let Some((start, end)) = self.local_brep.get_edge_endpoints(edge.id) {
                lookup.insert(undirected_edge_key(start, end), edge.id);
            }
        }
        lookup
    }

    fn rebuild_faces_with_inserted_edge_vertex(
        &self,
        brep: &Brep,
        edge_id: u32,
        from_id: u32,
        to_id: u32,
        inserted_vertex_id: u32,
        affected_faces: &mut Vec<u32>,
    ) -> Result<Vec<(Vec<u32>, Vec<Vec<u32>>)>, BrepDiagnostic> {
        let mut updated_faces = Vec::with_capacity(brep.faces.len());

        for face in &brep.faces {
            let outer_loop = brep.get_loop_vertex_indices(face.outer_loop);
            let (outer, outer_changed) =
                insert_vertex_into_loop(&outer_loop, from_id, to_id, inserted_vertex_id);

            let mut holes = Vec::with_capacity(face.inner_loops.len());
            let mut hole_changed = false;
            for loop_id in &face.inner_loops {
                let hole_loop = brep.get_loop_vertex_indices(*loop_id);
                let (hole, changed) =
                    insert_vertex_into_loop(&hole_loop, from_id, to_id, inserted_vertex_id);
                holes.push(hole);
                hole_changed |= changed;
            }

            if outer_changed || hole_changed {
                affected_faces.push(face.id);
            }

            updated_faces.push((outer, holes));
        }

        if affected_faces.is_empty() {
            return Err(BrepDiagnostic::error(
                "edge_not_on_face",
                format!("Edge {} is not part of any editable face loop", edge_id),
            )
            .with_domain("edge", Some(edge_id)));
        }

        Ok(updated_faces)
    }

    fn single_face_loop_edges(&self, brep: &crate::brep::Brep) -> Result<Vec<u32>, BrepDiagnostic> {
        let face = brep.faces.first().ok_or_else(|| {
            BrepDiagnostic::error("missing_face", "Single-face topology has no face")
                .with_domain("face", Some(0))
        })?;

        let halfedges = brep.get_loop_halfedges(face.outer_loop).map_err(|error| {
            BrepDiagnostic::error(
                "invalid_face_topology",
                format!("Failed to inspect face loop: {}", error),
            )
            .with_domain("face", Some(face.id))
        })?;

        let mut edges = Vec::with_capacity(halfedges.len());
        for halfedge_id in halfedges {
            let Some(halfedge) = brep.halfedges.get(halfedge_id as usize) else {
                return Err(BrepDiagnostic::error(
                    "invalid_halfedge_reference",
                    format!("Missing halfedge {} in face loop", halfedge_id),
                )
                .with_domain("face", Some(face.id)));
            };
            edges.push(halfedge.edge);
        }

        Ok(edges)
    }

    fn incident_face_count(&self, edge_id: u32) -> usize {
        let Some(edge) = self.local_brep.edges.get(edge_id as usize) else {
            return 0;
        };

        let mut count = 0usize;
        let halfedges = [Some(edge.halfedge), edge.twin_halfedge];
        for halfedge_id in halfedges.into_iter().flatten() {
            if let Some(halfedge) = self.local_brep.halfedges.get(halfedge_id as usize) {
                if halfedge.face.is_some() {
                    count += 1;
                }
            }
        }

        count
    }
}

fn undirected_edge_key(a: u32, b: u32) -> (u32, u32) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

fn interpolate_loop_edge_position(
    brep: &Brep,
    from_id: u32,
    to_id: u32,
    t: f64,
    edge_id: u32,
) -> Result<Vector3, BrepDiagnostic> {
    let from = brep
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
    let to = brep
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

    Ok(Vector3::new(
        from.x + (to.x - from.x) * t,
        from.y + (to.y - from.y) * t,
        from.z + (to.z - from.z) * t,
    ))
}

fn insert_vertex_into_loop(
    loop_vertices: &[u32],
    from_id: u32,
    to_id: u32,
    inserted_vertex_id: u32,
) -> (Vec<u32>, bool) {
    let mut updated = loop_vertices.to_vec();

    for index in 0..loop_vertices.len() {
        let current = loop_vertices[index];
        let next = loop_vertices[(index + 1) % loop_vertices.len()];

        if (current == from_id && next == to_id) || (current == to_id && next == from_id) {
            updated.insert(index + 1, inserted_vertex_id);
            return (updated, true);
        }
    }

    (updated, false)
}

pub(super) fn collect_closed_quad_edge_ring(
    brep: &Brep,
    start_edge_id: u32,
) -> Result<LoopCutRing, BrepDiagnostic> {
    let incident_faces = super::inspection::incident_faces_for_edge(brep, start_edge_id);
    if incident_faces.len() != 2 {
        return Err(BrepDiagnostic::error(
            "unsupported_topology",
            "loopCut currently requires an edge with two incident faces in a closed quad ring",
        )
        .with_domain("edge", Some(start_edge_id)));
    }

    let mut last_error = None;
    for start_face_id in incident_faces {
        match traverse_closed_quad_edge_ring(brep, start_edge_id, start_face_id) {
            Ok(ring) => return Ok(ring),
            Err(error) => last_error = Some(error),
        }
    }

    Err(last_error.unwrap_or_else(|| {
        BrepDiagnostic::error(
            "unsupported_topology",
            "loopCut could not resolve a valid closed quad edge ring",
        )
        .with_domain("edge", Some(start_edge_id))
    }))
}

fn traverse_closed_quad_edge_ring(
    brep: &Brep,
    start_edge_id: u32,
    start_face_id: u32,
) -> Result<LoopCutRing, BrepDiagnostic> {
    let mut edge_ids = vec![start_edge_id];
    let mut face_steps = Vec::new();
    let mut visited_faces = HashSet::new();
    let mut current_edge_id = start_edge_id;
    let mut current_face_id = start_face_id;

    loop {
        if !visited_faces.insert(current_face_id) {
            return Err(BrepDiagnostic::error(
                "unsupported_topology",
                "loopCut edge ring revisited a face before closing cleanly",
            )
            .with_domain("face", Some(current_face_id)));
        }

        let face = brep
            .faces
            .iter()
            .find(|candidate| candidate.id == current_face_id)
            .ok_or_else(|| {
                BrepDiagnostic::error(
                    "missing_face",
                    format!("Face {} does not exist", current_face_id),
                )
                .with_domain("face", Some(current_face_id))
            })?;

        if !face.inner_loops.is_empty() {
            return Err(BrepDiagnostic::error(
                "unsupported_topology",
                "loopCut currently supports quad faces without holes",
            )
            .with_domain("face", Some(current_face_id)));
        }

        let loop_edges = face_outer_loop_edges(brep, face.id)?;
        if loop_edges.len() != 4 {
            return Err(BrepDiagnostic::error(
                "unsupported_topology",
                "loopCut currently supports quad edge rings only",
            )
            .with_domain("face", Some(current_face_id)));
        }

        let current_index = loop_edges
            .iter()
            .position(|candidate| *candidate == current_edge_id)
            .ok_or_else(|| {
                BrepDiagnostic::error(
                    "invalid_face_topology",
                    format!(
                        "Edge {} is not part of face {} while traversing loopCut ring",
                        current_edge_id, current_face_id
                    ),
                )
                .with_domain("edge", Some(current_edge_id))
            })?;
        let opposite_edge_id = loop_edges[(current_index + 2) % loop_edges.len()];

        face_steps.push(LoopCutFaceStep {
            face_id: current_face_id,
            input_edge_id: current_edge_id,
            opposite_edge_id,
        });

        if opposite_edge_id == start_edge_id {
            return Ok(LoopCutRing {
                edge_ids,
                face_steps,
            });
        }

        if edge_ids.contains(&opposite_edge_id) {
            return Err(BrepDiagnostic::error(
                "unsupported_topology",
                "loopCut encountered a repeated opposite edge before closing the ring",
            )
            .with_domain("edge", Some(opposite_edge_id)));
        }

        edge_ids.push(opposite_edge_id);
        let incident_faces = super::inspection::incident_faces_for_edge(brep, opposite_edge_id);
        if incident_faces.len() != 2 {
            return Err(BrepDiagnostic::error(
                "unsupported_topology",
                "loopCut currently requires a closed quad ring without boundary edges",
            )
            .with_domain("edge", Some(opposite_edge_id)));
        }

        current_face_id = incident_faces
            .into_iter()
            .find(|face_id| *face_id != current_face_id)
            .ok_or_else(|| {
                BrepDiagnostic::error(
                    "unsupported_topology",
                    format!(
                        "Edge {} did not expose a traversable neighboring face for loopCut",
                        opposite_edge_id
                    ),
                )
                .with_domain("edge", Some(opposite_edge_id))
            })?;
        current_edge_id = opposite_edge_id;
    }
}

fn face_outer_loop_edges(brep: &Brep, face_id: u32) -> Result<Vec<u32>, BrepDiagnostic> {
    let face = brep
        .faces
        .iter()
        .find(|candidate| candidate.id == face_id)
        .ok_or_else(|| {
            BrepDiagnostic::error("missing_face", format!("Face {} does not exist", face_id))
                .with_domain("face", Some(face_id))
        })?;

    let loop_halfedges = brep.get_loop_halfedges(face.outer_loop).map_err(|error| {
        BrepDiagnostic::error(
            "invalid_face_topology",
            format!("Failed to inspect face {} loop: {}", face_id, error),
        )
        .with_domain("face", Some(face_id))
    })?;

    let mut edges = Vec::with_capacity(loop_halfedges.len());
    for halfedge_id in loop_halfedges {
        let halfedge = brep.halfedges.get(halfedge_id as usize).ok_or_else(|| {
            BrepDiagnostic::error(
                "invalid_halfedge_reference",
                format!(
                    "Face {} references missing halfedge {}",
                    face_id, halfedge_id
                ),
            )
            .with_domain("face", Some(face_id))
        })?;
        edges.push(halfedge.edge);
    }

    Ok(edges)
}

fn split_loop_between_vertices(
    loop_vertices: &[u32],
    first_inserted_vertex_id: u32,
    second_inserted_vertex_id: u32,
    face_id: u32,
) -> Result<(Vec<u32>, Vec<u32>), BrepDiagnostic> {
    let first_index = loop_vertices
        .iter()
        .position(|vertex_id| *vertex_id == first_inserted_vertex_id)
        .ok_or_else(|| {
            BrepDiagnostic::error(
                "loop_cut_failed",
                format!(
                    "Inserted loop-cut vertex {} is missing from face {} boundary",
                    first_inserted_vertex_id, face_id
                ),
            )
            .with_domain("face", Some(face_id))
        })?;
    let second_index = loop_vertices
        .iter()
        .position(|vertex_id| *vertex_id == second_inserted_vertex_id)
        .ok_or_else(|| {
            BrepDiagnostic::error(
                "loop_cut_failed",
                format!(
                    "Inserted loop-cut vertex {} is missing from face {} boundary",
                    second_inserted_vertex_id, face_id
                ),
            )
            .with_domain("face", Some(face_id))
        })?;

    if first_index == second_index {
        return Err(BrepDiagnostic::error(
            "loop_cut_failed",
            format!(
                "Face {} loopCut vertices collapsed to the same boundary slot",
                face_id
            ),
        )
        .with_domain("face", Some(face_id)));
    }

    let primary = slice_cyclic_loop(loop_vertices, first_index, second_index);
    let secondary = slice_cyclic_loop(loop_vertices, second_index, first_index);

    if primary.len() < 3 || secondary.len() < 3 {
        return Err(BrepDiagnostic::error(
            "loop_cut_failed",
            format!("Face {} loopCut produced an invalid face boundary", face_id),
        )
        .with_domain("face", Some(face_id)));
    }

    Ok((primary, secondary))
}

fn slice_cyclic_loop(loop_vertices: &[u32], start_index: usize, end_index: usize) -> Vec<u32> {
    let mut output = vec![loop_vertices[start_index]];
    let mut current_index = start_index;

    while current_index != end_index {
        current_index = (current_index + 1) % loop_vertices.len();
        output.push(loop_vertices[current_index]);
    }

    output
}
