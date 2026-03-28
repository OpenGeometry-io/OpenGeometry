use std::collections::{HashMap, HashSet};

use openmaths::Vector3;

use crate::brep::{Brep, BrepBuilder};

use super::super::inspection::incident_faces_for_edge;
use super::super::validation::normalized;
use super::super::{
    BrepDiagnostic, EditEffect, OGFreeformGeometry, TopologyChangeJournal, GEOMETRY_EPSILON,
};
use super::topology::{
    face_outer_loop_edges, insert_vertex_into_loop, split_loop_between_vertices,
    undirected_edge_key,
};

#[derive(Clone)]
pub(in crate::editor) struct LoopCutFaceStep {
    pub face_id: u32,
    pub input_edge_id: u32,
    pub opposite_edge_id: u32,
}

#[derive(Clone)]
pub(in crate::editor) struct LoopCutRing {
    pub edge_ids: Vec<u32>,
    pub face_steps: Vec<LoopCutFaceStep>,
}

impl OGFreeformGeometry {
    pub(in crate::editor) fn loop_cut_edge_ring_internal(
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
}

pub(in crate::editor) fn collect_closed_quad_edge_ring(
    brep: &Brep,
    start_edge_id: u32,
) -> Result<LoopCutRing, BrepDiagnostic> {
    let incident_faces = incident_faces_for_edge(brep, start_edge_id);
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
        let incident_faces = incident_faces_for_edge(brep, opposite_edge_id);
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
