use std::collections::HashMap;

use openmaths::Vector3;

use crate::brep::Brep;

use super::super::{BrepDiagnostic, OGFreeformGeometry};

impl OGFreeformGeometry {
    pub(super) fn edge_lookup_by_endpoints(&self) -> HashMap<(u32, u32), u32> {
        let mut lookup = HashMap::new();
        for edge in &self.local_brep.edges {
            if let Some((start, end)) = self.local_brep.get_edge_endpoints(edge.id) {
                lookup.insert(undirected_edge_key(start, end), edge.id);
            }
        }
        lookup
    }

    pub(super) fn rebuild_faces_with_inserted_edge_vertex(
        &self,
        brep: &Brep,
        from_id: u32,
        to_id: u32,
        inserted_vertex_id: u32,
    ) -> (Vec<(Vec<u32>, Vec<Vec<u32>>)>, Vec<u32>) {
        let mut updated_faces = Vec::with_capacity(brep.faces.len());
        let mut affected_faces = Vec::new();

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

        (updated_faces, affected_faces)
    }

    pub(super) fn rebuild_wires_with_inserted_edge_vertex(
        &self,
        brep: &Brep,
        from_id: u32,
        to_id: u32,
        inserted_vertex_id: u32,
    ) -> (Vec<(Vec<u32>, bool)>, Vec<u32>) {
        let mut updated_wires = Vec::with_capacity(brep.wires.len());
        let mut affected_wires = Vec::new();

        for wire in &brep.wires {
            let wire_vertices = brep.get_wire_vertex_indices(wire.id);
            let (updated_vertices, changed) = if wire.is_closed {
                insert_vertex_into_loop(&wire_vertices, from_id, to_id, inserted_vertex_id)
            } else {
                insert_vertex_into_open_chain(&wire_vertices, from_id, to_id, inserted_vertex_id)
            };

            if changed {
                affected_wires.push(wire.id);
            }

            updated_wires.push((updated_vertices, wire.is_closed));
        }

        (updated_wires, affected_wires)
    }

    pub(super) fn single_face_loop_edges(&self, brep: &Brep) -> Result<Vec<u32>, BrepDiagnostic> {
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

    pub(super) fn incident_face_count(&self, edge_id: u32) -> usize {
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

pub(super) fn undirected_edge_key(a: u32, b: u32) -> (u32, u32) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

pub(super) fn interpolate_loop_edge_position(
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

pub(super) fn insert_vertex_into_loop(
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

pub(super) fn insert_vertex_into_open_chain(
    chain_vertices: &[u32],
    from_id: u32,
    to_id: u32,
    inserted_vertex_id: u32,
) -> (Vec<u32>, bool) {
    let mut updated = chain_vertices.to_vec();

    for index in 0..chain_vertices.len().saturating_sub(1) {
        let current = chain_vertices[index];
        let next = chain_vertices[index + 1];

        if (current == from_id && next == to_id) || (current == to_id && next == from_id) {
            updated.insert(index + 1, inserted_vertex_id);
            return (updated, true);
        }
    }

    (updated, false)
}

pub(super) fn face_outer_loop_edges(brep: &Brep, face_id: u32) -> Result<Vec<u32>, BrepDiagnostic> {
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

pub(super) fn split_loop_between_vertices(
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
