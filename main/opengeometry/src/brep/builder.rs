use std::collections::HashMap;

use openmaths::Vector3;
use uuid::Uuid;

use super::error::{BrepError, BrepErrorKind};
use super::{Brep, Edge, Face, HalfEdge, Loop, Shell, Vertex, Wire};

const EPSILON: f64 = 1.0e-9;

#[derive(Clone)]
pub struct BrepBuilder {
    brep: Brep,
    undirected_edge_map: HashMap<(u32, u32), u32>,
    directed_halfedge_map: HashMap<(u32, u32), u32>,
    edge_halfedge_count: HashMap<u32, u32>,
}

impl BrepBuilder {
    pub fn new(id: Uuid) -> Self {
        Self {
            brep: Brep::new(id),
            undirected_edge_map: HashMap::new(),
            directed_halfedge_map: HashMap::new(),
            edge_halfedge_count: HashMap::new(),
        }
    }

    pub fn add_vertex(&mut self, position: Vector3) -> u32 {
        let vertex_id = self.brep.vertices.len() as u32;
        self.brep.vertices.push(Vertex::new(vertex_id, position));
        vertex_id
    }

    pub fn add_vertices(&mut self, positions: &[Vector3]) -> Vec<u32> {
        positions
            .iter()
            .map(|position| self.add_vertex(*position))
            .collect()
    }

    pub fn add_face(&mut self, outer: &[u32], holes: &[Vec<u32>]) -> Result<u32, BrepError> {
        let mut staging = self.clone();
        let face_id = staging.add_face_internal(outer, holes)?;
        *self = staging;
        Ok(face_id)
    }

    pub fn add_wire(&mut self, vertex_indices: &[u32], is_closed: bool) -> Result<u32, BrepError> {
        let mut staging = self.clone();
        let wire_id = staging.add_wire_internal(vertex_indices, is_closed)?;
        *self = staging;
        Ok(wire_id)
    }

    pub fn add_shell(&mut self, face_ids: &[u32], is_closed: bool) -> Result<u32, BrepError> {
        let mut staging = self.clone();
        let shell_id = staging.add_shell_internal(face_ids, is_closed)?;
        *self = staging;
        Ok(shell_id)
    }

    pub fn add_shell_from_all_faces(&mut self, is_closed: bool) -> Result<u32, BrepError> {
        if self.brep.faces.is_empty() {
            return Err(BrepError::new(
                BrepErrorKind::InvalidShell,
                "Cannot create shell from an empty face set",
            ));
        }

        let face_ids: Vec<u32> = (0..self.brep.faces.len() as u32).collect();
        self.add_shell(&face_ids, is_closed)
    }

    pub fn build(self) -> Result<Brep, BrepError> {
        self.brep.validate_topology()?;
        Ok(self.brep)
    }

    fn add_face_internal(&mut self, outer: &[u32], holes: &[Vec<u32>]) -> Result<u32, BrepError> {
        let outer = sanitize_indices(outer, true, 3, "outer loop")?;
        if !is_loop_non_degenerate(&outer) {
            return Err(BrepError::new(
                BrepErrorKind::DegenerateLoop,
                "Outer loop is degenerate",
            ));
        }

        let mut hole_loops = Vec::new();
        for hole in holes {
            let cleaned = sanitize_indices(hole, true, 3, "inner loop")?;
            if !is_loop_non_degenerate(&cleaned) {
                return Err(BrepError::new(
                    BrepErrorKind::DegenerateLoop,
                    "Inner loop is degenerate",
                ));
            }
            hole_loops.push(cleaned);
        }

        let face_id = self.brep.faces.len() as u32;
        self.brep.faces.push(Face::new(
            face_id,
            Vector3::new(0.0, 0.0, 0.0),
            0,
            Vec::new(),
            None,
        ));

        let outer_loop_id = self.add_face_loop(face_id, &outer, false)?;
        self.brep.faces[face_id as usize].outer_loop = outer_loop_id;

        for hole in &hole_loops {
            let loop_id = self.add_face_loop(face_id, hole, true)?;
            self.brep.faces[face_id as usize].inner_loops.push(loop_id);
        }

        let normal = self.compute_normal_from_loop(&outer)?;
        self.brep.faces[face_id as usize].set_normal(normal);

        Ok(face_id)
    }

    fn add_wire_internal(
        &mut self,
        vertex_indices: &[u32],
        is_closed: bool,
    ) -> Result<u32, BrepError> {
        let min_vertices = if is_closed { 3 } else { 2 };
        let vertices = sanitize_indices(vertex_indices, is_closed, min_vertices, "wire")?;

        let wire_id = self.brep.wires.len() as u32;
        self.brep
            .wires
            .push(Wire::new(wire_id, Vec::new(), is_closed));

        let halfedges = if is_closed {
            self.create_closed_halfedge_cycle(&vertices, None, None, Some(wire_id))?
        } else {
            self.create_open_halfedge_chain(&vertices, None, None, Some(wire_id))?
        };

        self.brep.wires[wire_id as usize].halfedges = halfedges;

        Ok(wire_id)
    }

    fn add_shell_internal(&mut self, face_ids: &[u32], is_closed: bool) -> Result<u32, BrepError> {
        if face_ids.is_empty() {
            return Err(BrepError::new(
                BrepErrorKind::InvalidShell,
                "Shell must reference at least one face",
            ));
        }

        for face_id in face_ids {
            let idx = *face_id as usize;
            let Some(face) = self.brep.faces.get(idx) else {
                return Err(BrepError::new(
                    BrepErrorKind::InvalidFace,
                    format!("Face {} does not exist", face_id),
                ));
            };

            if face.shell_ref.is_some() {
                return Err(BrepError::new(
                    BrepErrorKind::InvalidShell,
                    format!("Face {} already belongs to a shell", face_id),
                ));
            }
        }

        let shell_id = self.brep.shells.len() as u32;
        self.brep
            .shells
            .push(Shell::new(shell_id, face_ids.to_vec(), is_closed));

        for face_id in face_ids {
            self.brep.faces[*face_id as usize].shell_ref = Some(shell_id);
        }

        Ok(shell_id)
    }

    fn add_face_loop(
        &mut self,
        face_id: u32,
        vertex_indices: &[u32],
        is_hole: bool,
    ) -> Result<u32, BrepError> {
        let loop_id = self.brep.loops.len() as u32;
        self.brep
            .loops
            .push(Loop::new(loop_id, 0, face_id, is_hole));

        let halfedges =
            self.create_closed_halfedge_cycle(vertex_indices, Some(face_id), Some(loop_id), None)?;
        let Some(start_halfedge) = halfedges.first().copied() else {
            return Err(BrepError::new(
                BrepErrorKind::DegenerateLoop,
                "Loop generation produced no halfedges",
            ));
        };

        self.brep.loops[loop_id as usize].halfedge = start_halfedge;
        Ok(loop_id)
    }

    fn create_closed_halfedge_cycle(
        &mut self,
        vertex_indices: &[u32],
        face_ref: Option<u32>,
        loop_ref: Option<u32>,
        wire_ref: Option<u32>,
    ) -> Result<Vec<u32>, BrepError> {
        let mut halfedges = Vec::with_capacity(vertex_indices.len());

        for index in 0..vertex_indices.len() {
            let from = vertex_indices[index];
            let to = vertex_indices[(index + 1) % vertex_indices.len()];
            let halfedge_id = self.create_halfedge(from, to, face_ref, loop_ref, wire_ref)?;
            halfedges.push(halfedge_id);
        }

        for index in 0..halfedges.len() {
            let current = halfedges[index] as usize;
            let next = halfedges[(index + 1) % halfedges.len()];
            let prev = halfedges[(index + halfedges.len() - 1) % halfedges.len()];

            self.brep.halfedges[current].next = Some(next);
            self.brep.halfedges[current].prev = Some(prev);
        }

        Ok(halfedges)
    }

    fn create_open_halfedge_chain(
        &mut self,
        vertex_indices: &[u32],
        face_ref: Option<u32>,
        loop_ref: Option<u32>,
        wire_ref: Option<u32>,
    ) -> Result<Vec<u32>, BrepError> {
        let mut halfedges = Vec::with_capacity(vertex_indices.len().saturating_sub(1));

        for index in 0..(vertex_indices.len() - 1) {
            let from = vertex_indices[index];
            let to = vertex_indices[index + 1];
            let halfedge_id = self.create_halfedge(from, to, face_ref, loop_ref, wire_ref)?;
            halfedges.push(halfedge_id);
        }

        for index in 0..halfedges.len() {
            let current = halfedges[index] as usize;
            let next = if index + 1 < halfedges.len() {
                Some(halfedges[index + 1])
            } else {
                None
            };
            let prev = if index > 0 {
                Some(halfedges[index - 1])
            } else {
                None
            };

            self.brep.halfedges[current].next = next;
            self.brep.halfedges[current].prev = prev;
        }

        Ok(halfedges)
    }

    fn create_halfedge(
        &mut self,
        from: u32,
        to: u32,
        face_ref: Option<u32>,
        loop_ref: Option<u32>,
        wire_ref: Option<u32>,
    ) -> Result<u32, BrepError> {
        self.ensure_vertex_exists(from)?;
        self.ensure_vertex_exists(to)?;

        if from == to {
            return Err(BrepError::new(
                BrepErrorKind::DegenerateEdge,
                "Halfedge endpoints must differ",
            ));
        }

        if self.directed_halfedge_map.contains_key(&(from, to)) {
            return Err(BrepError::new(
                BrepErrorKind::InvalidHalfEdge,
                format!("Duplicate directed halfedge {} -> {}", from, to),
            ));
        }

        let undirected = undirected_key(from, to);
        let halfedge_id = self.brep.halfedges.len() as u32;

        let edge_id =
            if let Some(existing_edge_id) = self.undirected_edge_map.get(&undirected).copied() {
                let incidence = self
                    .edge_halfedge_count
                    .get(&existing_edge_id)
                    .copied()
                    .unwrap_or(0);

                if incidence >= 2 {
                    return Err(BrepError::new(
                        BrepErrorKind::NonManifoldEdge,
                        format!(
                            "Edge ({}, {}) would become non-manifold with more than two halfedges",
                            undirected.0, undirected.1
                        ),
                    ));
                }

                existing_edge_id
            } else {
                let new_edge_id = self.brep.edges.len() as u32;
                self.undirected_edge_map.insert(undirected, new_edge_id);
                self.edge_halfedge_count.insert(new_edge_id, 0);
                self.brep
                    .edges
                    .push(Edge::new(new_edge_id, halfedge_id, None));
                new_edge_id
            };

        let twin = self.directed_halfedge_map.get(&(to, from)).copied();

        self.brep.halfedges.push(HalfEdge::new(
            halfedge_id,
            from,
            to,
            edge_id,
            face_ref,
            loop_ref,
            wire_ref,
        ));

        if let Some(twin_id) = twin {
            self.brep.halfedges[halfedge_id as usize].twin = Some(twin_id);
            self.brep.halfedges[twin_id as usize].twin = Some(halfedge_id);
            self.brep.edges[edge_id as usize].twin_halfedge = Some(halfedge_id);
        }

        self.directed_halfedge_map.insert((from, to), halfedge_id);
        let incidence = self.edge_halfedge_count.entry(edge_id).or_insert(0);
        *incidence += 1;

        if self.brep.vertices[from as usize]
            .outgoing_halfedge
            .is_none()
        {
            self.brep.vertices[from as usize].outgoing_halfedge = Some(halfedge_id);
        }

        Ok(halfedge_id)
    }

    fn ensure_vertex_exists(&self, vertex_id: u32) -> Result<(), BrepError> {
        if self.brep.vertices.get(vertex_id as usize).is_none() {
            return Err(BrepError::new(
                BrepErrorKind::InvalidVertex,
                format!("Vertex {} does not exist", vertex_id),
            ));
        }

        Ok(())
    }

    fn compute_normal_from_loop(&self, loop_indices: &[u32]) -> Result<Vector3, BrepError> {
        if loop_indices.len() < 3 {
            return Err(BrepError::new(
                BrepErrorKind::DegenerateLoop,
                "Loop must have at least three vertices to compute a normal",
            ));
        }

        let p0 = self.brep.vertices[loop_indices[0] as usize].position;
        for index in 1..(loop_indices.len() - 1) {
            let p1 = self.brep.vertices[loop_indices[index] as usize].position;
            let p2 = self.brep.vertices[loop_indices[index + 1] as usize].position;

            let v1 = [p1.x - p0.x, p1.y - p0.y, p1.z - p0.z];
            let v2 = [p2.x - p0.x, p2.y - p0.y, p2.z - p0.z];

            let cross = [
                v1[1] * v2[2] - v1[2] * v2[1],
                v1[2] * v2[0] - v1[0] * v2[2],
                v1[0] * v2[1] - v1[1] * v2[0],
            ];
            let length_sq = cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2];

            if length_sq > EPSILON * EPSILON {
                let inv = length_sq.sqrt().recip();
                return Ok(Vector3::new(cross[0] * inv, cross[1] * inv, cross[2] * inv));
            }
        }

        Err(BrepError::new(
            BrepErrorKind::DegenerateLoop,
            "Failed to compute a stable face normal from loop vertices",
        ))
    }
}

fn sanitize_indices(
    indices: &[u32],
    is_closed: bool,
    min_vertices: usize,
    label: &str,
) -> Result<Vec<u32>, BrepError> {
    let mut cleaned: Vec<u32> = Vec::with_capacity(indices.len());
    for index in indices {
        if cleaned.last().copied() == Some(*index) {
            continue;
        }
        cleaned.push(*index);
    }

    if is_closed && cleaned.len() >= 2 && cleaned.first() == cleaned.last() {
        cleaned.pop();
    }

    if cleaned.len() < min_vertices {
        return Err(BrepError::new(
            BrepErrorKind::InvalidLoop,
            format!(
                "{} must have at least {} unique vertices",
                label, min_vertices
            ),
        ));
    }

    Ok(cleaned)
}

fn is_loop_non_degenerate(loop_indices: &[u32]) -> bool {
    if loop_indices.len() < 3 {
        return false;
    }

    let mut unique = HashMap::new();
    for index in loop_indices {
        unique.insert(*index, true);
    }

    unique.len() >= 3
}

fn undirected_key(a: u32, b: u32) -> (u32, u32) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

#[cfg(test)]
mod tests {
    use super::BrepBuilder;
    use openmaths::Vector3;
    use uuid::Uuid;

    #[test]
    fn builder_creates_closed_face_with_twin_halfedges() {
        let mut builder = BrepBuilder::new(Uuid::new_v4());
        let vertices = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 1.0),
            Vector3::new(0.0, 0.0, 1.0),
        ];
        builder.add_vertices(&vertices);

        builder.add_face(&[0, 1, 2, 3], &[]).unwrap();
        builder.add_face(&[0, 3, 2, 1], &[]).unwrap();

        let brep = builder.build().unwrap();

        assert_eq!(brep.faces.len(), 2);
        assert!(!brep.halfedges.is_empty());
        assert!(brep
            .halfedges
            .iter()
            .any(|halfedge| halfedge.twin.is_some()));
    }

    #[test]
    fn builder_rejects_non_manifold_edge() {
        let mut builder = BrepBuilder::new(Uuid::new_v4());
        let vertices = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 1.0),
            Vector3::new(0.0, 0.0, 1.0),
            Vector3::new(0.5, 1.0, 0.5),
        ];
        builder.add_vertices(&vertices);

        builder.add_face(&[0, 1, 2], &[]).unwrap();
        builder.add_face(&[1, 0, 3], &[]).unwrap();
        let third_face = builder.add_face(&[0, 1, 4], &[]);

        assert!(third_face.is_err());
    }
}
