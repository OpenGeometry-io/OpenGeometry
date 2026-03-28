use std::collections::HashSet;

use crate::brep::BrepBuilder;

use super::super::{
    BrepDiagnostic, EditEffect, OGFreeformGeometry, TopologyChangeJournal, GEOMETRY_EPSILON,
};
use super::topology::{
    face_outer_loop_edges, insert_vertex_into_loop, interpolate_loop_edge_position,
    split_loop_between_vertices, undirected_edge_key,
};

impl OGFreeformGeometry {
    pub(in crate::editor) fn cut_face_internal(
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
}
