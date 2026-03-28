use std::collections::HashSet;

use openmaths::Vector3;

use super::super::inspection::collect_face_vertex_ids;
use super::super::validation::{is_finite_vector, normalized};
use super::super::{BrepDiagnostic, ConstraintSettings, EditEffect, OGFreeformGeometry};

impl OGFreeformGeometry {
    pub(in crate::editor) fn push_pull_face_internal(
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

    pub(in crate::editor) fn translate_face_by_vector_internal(
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

    pub(in crate::editor) fn move_edge_internal(
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

    pub(in crate::editor) fn move_vertex_internal(
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
}
