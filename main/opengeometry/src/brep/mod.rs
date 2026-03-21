pub mod builder;
pub mod edge;
pub mod error;
pub mod face;
pub mod halfedge;
pub mod r#loop;
pub mod shell;
pub mod vertex;
pub mod wire;

use std::cell::RefCell;
use std::collections::{hash_map::DefaultHasher, HashMap, HashSet};
use std::hash::{Hash, Hasher};

use openmaths::Vector3;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    operations::triangulate::triangulate_polygon_with_holes, spatial::placement::Placement3D,
};

pub use builder::BrepBuilder;
pub use edge::Edge;
pub use error::{BrepError, BrepErrorKind};
pub use face::Face;
pub use halfedge::HalfEdge;
pub use r#loop::Loop;
pub use shell::Shell;
pub use vertex::Vertex;
pub use wire::Wire;

const VALIDATION_GUARD_FACTOR: usize = 4;
const TRANSFORM_CACHE_LIMIT: usize = 128;

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
struct TransformCacheKey {
    brep_id: Uuid,
    local_signature: u64,
    anchor: [u64; 3],
    translation: [u64; 3],
    rotation: [u64; 3],
    scale: [u64; 3],
}

thread_local! {
    static WORLD_BREP_TRANSFORM_CACHE: RefCell<HashMap<TransformCacheKey, Brep>> =
        RefCell::new(HashMap::new());
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Brep {
    pub id: Uuid,
    pub vertices: Vec<Vertex>,
    pub halfedges: Vec<HalfEdge>,
    pub edges: Vec<Edge>,
    pub loops: Vec<Loop>,
    pub faces: Vec<Face>,
    pub wires: Vec<Wire>,
    pub shells: Vec<Shell>,
}

impl Brep {
    pub fn new(id: Uuid) -> Self {
        Self {
            id,
            vertices: Vec::new(),
            halfedges: Vec::new(),
            edges: Vec::new(),
            loops: Vec::new(),
            faces: Vec::new(),
            wires: Vec::new(),
            shells: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.vertices.clear();
        self.halfedges.clear();
        self.edges.clear();
        self.loops.clear();
        self.faces.clear();
        self.wires.clear();
        self.shells.clear();
    }

    pub fn get_vertex_count(&self) -> u32 {
        self.vertices.len() as u32
    }

    pub fn get_halfedge_count(&self) -> u32 {
        self.halfedges.len() as u32
    }

    pub fn get_edge_count(&self) -> u32 {
        self.edges.len() as u32
    }

    pub fn get_loop_count(&self) -> u32 {
        self.loops.len() as u32
    }

    pub fn get_face_count(&self) -> u32 {
        self.faces.len() as u32
    }

    pub fn get_wire_count(&self) -> u32 {
        self.wires.len() as u32
    }

    pub fn get_shell_count(&self) -> u32 {
        self.shells.len() as u32
    }

    pub fn get_flattened_vertices(&self) -> Vec<Vector3> {
        self.vertices.iter().map(|vertex| vertex.position).collect()
    }

    /**
     * Applies the given placement's world transformation to the BREP. This function transforms all vertices in the BREP from their local space to world space using the transformation defined by the placement. The transformation includes translation, rotation, and scaling as specified in the placement's world matrix. After applying this function, the vertices in the BREP will be updated to reflect their new positions in world space, which can then be used for rendering or further processing.
     */
    pub fn apply_transform(&mut self, placement: &Placement3D) {
        let placement_matrix = placement.world_matrix();
        for vertex in &mut self.vertices {
            vertex.position.apply_matrix4(placement_matrix.clone());
        }

        if !self.faces.is_empty() {
            self.recompute_face_normals();
        }
    }

    pub fn transformed(&self, placement: &Placement3D) -> Brep {
        let cache_key = self.transform_cache_key(placement);
        if let Some(cached) =
            WORLD_BREP_TRANSFORM_CACHE.with(|cache| cache.borrow().get(&cache_key).cloned())
        {
            return cached;
        }

        let mut transformed = self.clone();
        transformed.apply_transform(placement);

        WORLD_BREP_TRANSFORM_CACHE.with(|cache| {
            let mut cache_ref = cache.borrow_mut();
            if cache_ref.len() >= TRANSFORM_CACHE_LIMIT {
                cache_ref.clear();
            }
            cache_ref.insert(cache_key, transformed.clone());
        });

        transformed
    }

    pub fn bounds_center(&self) -> Option<Vector3> {
        let first_vertex = self.vertices.first()?;
        let mut min_x = first_vertex.position.x;
        let mut min_y = first_vertex.position.y;
        let mut min_z = first_vertex.position.z;
        let mut max_x = first_vertex.position.x;
        let mut max_y = first_vertex.position.y;
        let mut max_z = first_vertex.position.z;

        for vertex in &self.vertices[1..] {
            min_x = min_x.min(vertex.position.x);
            min_y = min_y.min(vertex.position.y);
            min_z = min_z.min(vertex.position.z);
            max_x = max_x.max(vertex.position.x);
            max_y = max_y.max(vertex.position.y);
            max_z = max_z.max(vertex.position.z);
        }

        Some(Vector3::new(
            (min_x + max_x) * 0.5,
            (min_y + max_y) * 0.5,
            (min_z + max_z) * 0.5,
        ))
    }

    fn transform_cache_key(&self, placement: &Placement3D) -> TransformCacheKey {
        TransformCacheKey {
            brep_id: self.id,
            local_signature: self.local_signature(),
            anchor: vector_bits(placement.anchor),
            translation: vector_bits(placement.translation()),
            rotation: vector_bits(placement.rotation()),
            scale: vector_bits(placement.scale()),
        }
    }

    fn local_signature(&self) -> u64 {
        let mut hasher = DefaultHasher::new();

        self.vertices.len().hash(&mut hasher);
        self.halfedges.len().hash(&mut hasher);
        self.edges.len().hash(&mut hasher);
        self.loops.len().hash(&mut hasher);
        self.faces.len().hash(&mut hasher);
        self.wires.len().hash(&mut hasher);
        self.shells.len().hash(&mut hasher);

        for vertex in &self.vertices {
            vertex.id.hash(&mut hasher);
            vertex.position.x.to_bits().hash(&mut hasher);
            vertex.position.y.to_bits().hash(&mut hasher);
            vertex.position.z.to_bits().hash(&mut hasher);
            vertex.outgoing_halfedge.hash(&mut hasher);
        }

        for halfedge in &self.halfedges {
            halfedge.id.hash(&mut hasher);
            halfedge.from.hash(&mut hasher);
            halfedge.to.hash(&mut hasher);
            halfedge.twin.hash(&mut hasher);
            halfedge.next.hash(&mut hasher);
            halfedge.prev.hash(&mut hasher);
            halfedge.edge.hash(&mut hasher);
            halfedge.face.hash(&mut hasher);
            halfedge.loop_ref.hash(&mut hasher);
            halfedge.wire_ref.hash(&mut hasher);
        }

        for edge in &self.edges {
            edge.id.hash(&mut hasher);
            edge.halfedge.hash(&mut hasher);
            edge.twin_halfedge.hash(&mut hasher);
        }

        for loop_ref in &self.loops {
            loop_ref.id.hash(&mut hasher);
            loop_ref.halfedge.hash(&mut hasher);
            loop_ref.face.hash(&mut hasher);
            loop_ref.is_hole.hash(&mut hasher);
        }

        for face in &self.faces {
            face.id.hash(&mut hasher);
            face.normal.x.to_bits().hash(&mut hasher);
            face.normal.y.to_bits().hash(&mut hasher);
            face.normal.z.to_bits().hash(&mut hasher);
            face.outer_loop.hash(&mut hasher);
            face.inner_loops.hash(&mut hasher);
            face.shell_ref.hash(&mut hasher);
        }

        for wire in &self.wires {
            wire.id.hash(&mut hasher);
            wire.halfedges.hash(&mut hasher);
            wire.is_closed.hash(&mut hasher);
        }

        for shell in &self.shells {
            shell.id.hash(&mut hasher);
            shell.faces.hash(&mut hasher);
            shell.is_closed.hash(&mut hasher);
        }

        hasher.finish()
    }

    pub fn recompute_face_normals(&mut self) {
        for face_index in 0..self.faces.len() {
            let outer_loop = self.faces[face_index].outer_loop;
            let loop_indices = self.get_loop_vertex_indices(outer_loop);
            if let Some(normal) = compute_loop_normal(self, &loop_indices) {
                self.faces[face_index].set_normal(normal);
            }
        }
    }

    pub fn get_edge_endpoints(&self, edge_id: u32) -> Option<(u32, u32)> {
        let edge = self.edges.get(edge_id as usize)?;
        let halfedge = self.halfedges.get(edge.halfedge as usize)?;
        Some((halfedge.from, halfedge.to))
    }

    pub fn collect_outline_segments(&self) -> Vec<(u32, u32)> {
        let mut segments = Vec::new();
        for edge in &self.edges {
            if let Some((from, to)) = self.get_edge_endpoints(edge.id) {
                segments.push((from, to));
            }
        }
        segments
    }

    /// Collects only feature edges whose adjacent faces form a visible crease
    /// or whose topology leaves them on a boundary.
    pub fn collect_feature_outline_segments(&self, crease_cos_threshold: f64) -> Vec<(u32, u32)> {
        let mut segments = Vec::new();

        for edge in &self.edges {
            let Some((from, to)) = self.get_edge_endpoints(edge.id) else {
                continue;
            };

            let Some(halfedge) = self.halfedges.get(edge.halfedge as usize) else {
                continue;
            };

            let primary_face = halfedge
                .face
                .and_then(|face_id| self.faces.get(face_id as usize));
            let twin_face = edge
                .twin_halfedge
                .and_then(|halfedge_id| self.halfedges.get(halfedge_id as usize))
                .and_then(|halfedge_ref| halfedge_ref.face)
                .and_then(|face_id| self.faces.get(face_id as usize));

            let include = match (primary_face, twin_face) {
                (Some(a), Some(b)) => {
                    let dot = a.normal.dot(&b.normal);
                    dot < crease_cos_threshold
                }
                _ => true,
            };

            if include {
                segments.push((from, to));
            }
        }

        segments
    }

    pub fn get_loop_halfedges(&self, loop_id: u32) -> Result<Vec<u32>, BrepError> {
        let loop_ref = self.loops.get(loop_id as usize).ok_or_else(|| {
            BrepError::new(
                BrepErrorKind::InvalidLoop,
                format!("Loop {} does not exist", loop_id),
            )
        })?;

        let start = loop_ref.halfedge;
        if self.halfedges.get(start as usize).is_none() {
            return Err(BrepError::new(
                BrepErrorKind::InvalidHalfEdge,
                format!("Loop {} points to invalid halfedge {}", loop_id, start),
            ));
        }

        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut current = start;
        let guard_limit = self.halfedges.len().saturating_mul(VALIDATION_GUARD_FACTOR);

        for _ in 0..guard_limit {
            if !visited.insert(current) {
                return Err(BrepError::new(
                    BrepErrorKind::BrokenTopology,
                    format!(
                        "Loop {} revisits halfedge {} before closure",
                        loop_id, current
                    ),
                ));
            }

            result.push(current);

            let halfedge = &self.halfedges[current as usize];
            let Some(next) = halfedge.next else {
                return Err(BrepError::new(
                    BrepErrorKind::BrokenTopology,
                    format!(
                        "Loop {} contains halfedge {} without next link",
                        loop_id, current
                    ),
                ));
            };

            if next == start {
                return Ok(result);
            }

            if self.halfedges.get(next as usize).is_none() {
                return Err(BrepError::new(
                    BrepErrorKind::InvalidHalfEdge,
                    format!("Loop {} references invalid next halfedge {}", loop_id, next),
                ));
            }

            current = next;
        }

        Err(BrepError::new(
            BrepErrorKind::BrokenTopology,
            format!("Loop {} exceeded traversal guard", loop_id),
        ))
    }

    pub fn get_loop_vertex_indices(&self, loop_id: u32) -> Vec<u32> {
        self.get_loop_halfedges(loop_id)
            .map(|halfedges| {
                halfedges
                    .iter()
                    .filter_map(|halfedge_id| self.halfedges.get(*halfedge_id as usize))
                    .map(|halfedge| halfedge.from)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn get_wire_vertex_indices(&self, wire_id: u32) -> Vec<u32> {
        let Some(wire) = self.wires.get(wire_id as usize) else {
            return Vec::new();
        };

        if wire.halfedges.is_empty() {
            return Vec::new();
        }

        let mut vertices = Vec::with_capacity(wire.halfedges.len() + 1);
        for halfedge_id in &wire.halfedges {
            if let Some(halfedge) = self.halfedges.get(*halfedge_id as usize) {
                vertices.push(halfedge.from);
            }
        }

        if !wire.is_closed {
            if let Some(last_halfedge_id) = wire.halfedges.last() {
                if let Some(last_halfedge) = self.halfedges.get(*last_halfedge_id as usize) {
                    vertices.push(last_halfedge.to);
                }
            }
        }

        vertices
    }

    pub fn get_wire_vertex_buffer(&self, wire_id: u32, repeat_first_when_closed: bool) -> Vec<f64> {
        let Some(wire) = self.wires.get(wire_id as usize) else {
            return Vec::new();
        };

        let mut vertex_ids = self.get_wire_vertex_indices(wire_id);
        if repeat_first_when_closed && wire.is_closed {
            if let Some(first_vertex_id) = vertex_ids.first().copied() {
                vertex_ids.push(first_vertex_id);
            }
        }

        let mut vertex_buffer = Vec::with_capacity(vertex_ids.len() * 3);
        for vertex_id in vertex_ids {
            let Some(vertex) = self.vertices.get(vertex_id as usize) else {
                continue;
            };

            vertex_buffer.push(vertex.position.x);
            vertex_buffer.push(vertex.position.y);
            vertex_buffer.push(vertex.position.z);
        }

        vertex_buffer
    }

    pub fn get_vertices_by_face_id(&self, face_id: u32) -> Vec<Vector3> {
        let Some(face) = self.faces.iter().find(|face| face.id == face_id) else {
            return Vec::new();
        };

        self.get_loop_vertex_indices(face.outer_loop)
            .into_iter()
            .filter_map(|vertex_id| self.vertices.get(vertex_id as usize))
            .map(|vertex| vertex.position)
            .collect()
    }

    pub fn get_vertices_and_holes_by_face_id(
        &self,
        face_id: u32,
    ) -> (Vec<Vector3>, Vec<Vec<Vector3>>) {
        let Some(face) = self.faces.iter().find(|face| face.id == face_id) else {
            return (Vec::new(), Vec::new());
        };

        let face_vertices = self
            .get_loop_vertex_indices(face.outer_loop)
            .into_iter()
            .filter_map(|vertex_id| self.vertices.get(vertex_id as usize))
            .map(|vertex| vertex.position)
            .collect();

        let mut holes_vertices = Vec::new();
        for loop_id in &face.inner_loops {
            let hole_vertices: Vec<Vector3> = self
                .get_loop_vertex_indices(*loop_id)
                .into_iter()
                .filter_map(|vertex_id| self.vertices.get(vertex_id as usize))
                .map(|vertex| vertex.position)
                .collect();

            holes_vertices.push(hole_vertices);
        }

        (face_vertices, holes_vertices)
    }

    /// Flattens every face into a triangle vertex buffer suitable for
    /// immediate-mode rendering consumers.
    pub fn get_triangle_vertex_buffer(&self) -> Vec<f64> {
        let mut vertex_buffer = Vec::new();

        for face in &self.faces {
            let (face_vertices, holes_vertices) = self.get_vertices_and_holes_by_face_id(face.id);
            if face_vertices.len() < 3 {
                continue;
            }

            let triangles = triangulate_polygon_with_holes(&face_vertices, &holes_vertices);
            let all_vertices: Vec<Vector3> = face_vertices
                .into_iter()
                .chain(holes_vertices.into_iter().flatten())
                .collect();

            for triangle in triangles {
                for vertex_index in triangle {
                    let Some(vertex) = all_vertices.get(vertex_index) else {
                        continue;
                    };
                    vertex_buffer.push(vertex.x);
                    vertex_buffer.push(vertex.y);
                    vertex_buffer.push(vertex.z);
                }
            }
        }

        vertex_buffer
    }

    /// Serializes every edge in the BRep as a flat line-segment buffer.
    pub fn get_outline_vertex_buffer(&self) -> Vec<f64> {
        self.outline_buffer_from_segments(self.collect_outline_segments())
    }

    /// Serializes only feature/boundary edges to keep outlines readable on
    /// heavily faceted boolean results.
    pub fn get_feature_outline_vertex_buffer(&self, crease_cos_threshold: f64) -> Vec<f64> {
        self.outline_buffer_from_segments(
            self.collect_feature_outline_segments(crease_cos_threshold),
        )
    }

    fn outline_buffer_from_segments(&self, segments: Vec<(u32, u32)>) -> Vec<f64> {
        let mut vertex_buffer = Vec::new();

        for (start_id, end_id) in segments {
            let Some(start_vertex) = self.vertices.get(start_id as usize) else {
                continue;
            };
            let Some(end_vertex) = self.vertices.get(end_id as usize) else {
                continue;
            };

            vertex_buffer.push(start_vertex.position.x);
            vertex_buffer.push(start_vertex.position.y);
            vertex_buffer.push(start_vertex.position.z);

            vertex_buffer.push(end_vertex.position.x);
            vertex_buffer.push(end_vertex.position.y);
            vertex_buffer.push(end_vertex.position.z);
        }

        vertex_buffer
    }

    pub fn validate_topology(&self) -> Result<(), BrepError> {
        for (index, vertex) in self.vertices.iter().enumerate() {
            if vertex.id as usize != index {
                return Err(BrepError::new(
                    BrepErrorKind::InvalidVertex,
                    format!("Vertex id mismatch at index {} (id={})", index, vertex.id),
                ));
            }

            if let Some(outgoing) = vertex.outgoing_halfedge {
                let Some(halfedge) = self.halfedges.get(outgoing as usize) else {
                    return Err(BrepError::new(
                        BrepErrorKind::InvalidHalfEdge,
                        format!(
                            "Vertex {} references missing outgoing halfedge {}",
                            vertex.id, outgoing
                        ),
                    ));
                };

                if halfedge.from != vertex.id {
                    return Err(BrepError::new(
                        BrepErrorKind::BrokenTopology,
                        format!(
                            "Vertex {} outgoing halfedge {} starts at {}",
                            vertex.id, outgoing, halfedge.from
                        ),
                    ));
                }
            }
        }

        let mut edge_to_halfedges: HashMap<u32, Vec<u32>> = HashMap::new();
        let mut edge_to_faces: HashMap<u32, HashSet<u32>> = HashMap::new();

        for (index, halfedge) in self.halfedges.iter().enumerate() {
            if halfedge.id as usize != index {
                return Err(BrepError::new(
                    BrepErrorKind::InvalidHalfEdge,
                    format!(
                        "Halfedge id mismatch at index {} (id={})",
                        index, halfedge.id
                    ),
                ));
            }

            if self.vertices.get(halfedge.from as usize).is_none()
                || self.vertices.get(halfedge.to as usize).is_none()
            {
                return Err(BrepError::new(
                    BrepErrorKind::InvalidVertex,
                    format!(
                        "Halfedge {} has invalid vertex references ({} -> {})",
                        halfedge.id, halfedge.from, halfedge.to
                    ),
                ));
            }

            let Some(edge) = self.edges.get(halfedge.edge as usize) else {
                return Err(BrepError::new(
                    BrepErrorKind::InvalidEdge,
                    format!(
                        "Halfedge {} references missing edge {}",
                        halfedge.id, halfedge.edge
                    ),
                ));
            };

            if edge.id != halfedge.edge {
                return Err(BrepError::new(
                    BrepErrorKind::InvalidEdge,
                    format!(
                        "Halfedge {} edge reference mismatch (edge id={}, ref={})",
                        halfedge.id, edge.id, halfedge.edge
                    ),
                ));
            }

            edge_to_halfedges
                .entry(halfedge.edge)
                .or_default()
                .push(halfedge.id);

            if let Some(face_id) = halfedge.face {
                if self.faces.get(face_id as usize).is_none() {
                    return Err(BrepError::new(
                        BrepErrorKind::InvalidFace,
                        format!(
                            "Halfedge {} references missing face {}",
                            halfedge.id, face_id
                        ),
                    ));
                }
                edge_to_faces
                    .entry(halfedge.edge)
                    .or_default()
                    .insert(face_id);
            }

            if let Some(twin_id) = halfedge.twin {
                let Some(twin) = self.halfedges.get(twin_id as usize) else {
                    return Err(BrepError::new(
                        BrepErrorKind::InvalidHalfEdge,
                        format!(
                            "Halfedge {} references missing twin {}",
                            halfedge.id, twin_id
                        ),
                    ));
                };

                if twin.twin != Some(halfedge.id) {
                    return Err(BrepError::new(
                        BrepErrorKind::BrokenTopology,
                        format!(
                            "Halfedge {} twin symmetry broken with {}",
                            halfedge.id, twin_id
                        ),
                    ));
                }

                if !(halfedge.from == twin.to && halfedge.to == twin.from) {
                    return Err(BrepError::new(
                        BrepErrorKind::BrokenTopology,
                        format!(
                            "Halfedge {} twin {} does not reverse endpoints",
                            halfedge.id, twin_id
                        ),
                    ));
                }
            }

            if let Some(next_id) = halfedge.next {
                let Some(next) = self.halfedges.get(next_id as usize) else {
                    return Err(BrepError::new(
                        BrepErrorKind::InvalidHalfEdge,
                        format!("Halfedge {} has missing next {}", halfedge.id, next_id),
                    ));
                };

                if next.prev != Some(halfedge.id) {
                    return Err(BrepError::new(
                        BrepErrorKind::BrokenTopology,
                        format!("Halfedge {} next/prev symmetry is broken", halfedge.id),
                    ));
                }
            }

            if let Some(prev_id) = halfedge.prev {
                let Some(prev) = self.halfedges.get(prev_id as usize) else {
                    return Err(BrepError::new(
                        BrepErrorKind::InvalidHalfEdge,
                        format!("Halfedge {} has missing prev {}", halfedge.id, prev_id),
                    ));
                };

                if prev.next != Some(halfedge.id) {
                    return Err(BrepError::new(
                        BrepErrorKind::BrokenTopology,
                        format!("Halfedge {} prev/next symmetry is broken", halfedge.id),
                    ));
                }
            }

            if let Some(loop_id) = halfedge.loop_ref {
                if self.loops.get(loop_id as usize).is_none() {
                    return Err(BrepError::new(
                        BrepErrorKind::InvalidLoop,
                        format!(
                            "Halfedge {} references missing loop {}",
                            halfedge.id, loop_id
                        ),
                    ));
                }
            }

            if let Some(wire_id) = halfedge.wire_ref {
                if self.wires.get(wire_id as usize).is_none() {
                    return Err(BrepError::new(
                        BrepErrorKind::InvalidWire,
                        format!(
                            "Halfedge {} references missing wire {}",
                            halfedge.id, wire_id
                        ),
                    ));
                }
            }
        }

        for (index, edge) in self.edges.iter().enumerate() {
            if edge.id as usize != index {
                return Err(BrepError::new(
                    BrepErrorKind::InvalidEdge,
                    format!("Edge id mismatch at index {} (id={})", index, edge.id),
                ));
            }

            let Some(halfedge_list) = edge_to_halfedges.get(&edge.id) else {
                return Err(BrepError::new(
                    BrepErrorKind::InvalidEdge,
                    format!("Edge {} has no halfedges", edge.id),
                ));
            };

            if halfedge_list.is_empty() || halfedge_list.len() > 2 {
                return Err(BrepError::new(
                    BrepErrorKind::NonManifoldEdge,
                    format!(
                        "Edge {} has invalid incidence count {}",
                        edge.id,
                        halfedge_list.len()
                    ),
                ));
            }

            if !halfedge_list.contains(&edge.halfedge) {
                return Err(BrepError::new(
                    BrepErrorKind::InvalidEdge,
                    format!(
                        "Edge {} primary halfedge {} is not linked to edge",
                        edge.id, edge.halfedge
                    ),
                ));
            }

            if let Some(twin_halfedge) = edge.twin_halfedge {
                if !halfedge_list.contains(&twin_halfedge) {
                    return Err(BrepError::new(
                        BrepErrorKind::InvalidEdge,
                        format!(
                            "Edge {} twin halfedge {} is not linked to edge",
                            edge.id, twin_halfedge
                        ),
                    ));
                }
            }

            if let Some(face_set) = edge_to_faces.get(&edge.id) {
                if face_set.len() > 2 {
                    return Err(BrepError::new(
                        BrepErrorKind::NonManifoldEdge,
                        format!("Edge {} touches more than two faces", edge.id),
                    ));
                }
            }
        }

        for loop_ref in &self.loops {
            if loop_ref.id as usize >= self.loops.len() {
                return Err(BrepError::new(
                    BrepErrorKind::InvalidLoop,
                    format!("Loop {} has invalid id", loop_ref.id),
                ));
            }

            let Some(face) = self.faces.get(loop_ref.face as usize) else {
                return Err(BrepError::new(
                    BrepErrorKind::InvalidFace,
                    format!(
                        "Loop {} references missing face {}",
                        loop_ref.id, loop_ref.face
                    ),
                ));
            };

            if loop_ref.is_hole {
                if !face.inner_loops.contains(&loop_ref.id) {
                    return Err(BrepError::new(
                        BrepErrorKind::BrokenTopology,
                        format!(
                            "Hole loop {} is not listed in face {}",
                            loop_ref.id, face.id
                        ),
                    ));
                }
            } else if face.outer_loop != loop_ref.id {
                return Err(BrepError::new(
                    BrepErrorKind::BrokenTopology,
                    format!("Face {} outer loop mismatch", face.id),
                ));
            }

            let loop_halfedges = self.get_loop_halfedges(loop_ref.id)?;
            if loop_halfedges.len() < 3 {
                return Err(BrepError::new(
                    BrepErrorKind::DegenerateLoop,
                    format!("Loop {} has fewer than three halfedges", loop_ref.id),
                ));
            }

            for halfedge_id in loop_halfedges {
                let halfedge = &self.halfedges[halfedge_id as usize];
                if halfedge.loop_ref != Some(loop_ref.id) {
                    return Err(BrepError::new(
                        BrepErrorKind::BrokenTopology,
                        format!("Halfedge {} loop reference mismatch", halfedge_id),
                    ));
                }

                if halfedge.face != Some(loop_ref.face) {
                    return Err(BrepError::new(
                        BrepErrorKind::BrokenTopology,
                        format!("Halfedge {} face reference mismatch", halfedge_id),
                    ));
                }
            }
        }

        for wire in &self.wires {
            if wire.id as usize >= self.wires.len() {
                return Err(BrepError::new(
                    BrepErrorKind::InvalidWire,
                    format!("Wire {} has invalid id", wire.id),
                ));
            }

            if wire.halfedges.is_empty() {
                return Err(BrepError::new(
                    BrepErrorKind::InvalidWire,
                    format!("Wire {} is empty", wire.id),
                ));
            }

            for (index, halfedge_id) in wire.halfedges.iter().enumerate() {
                let Some(halfedge) = self.halfedges.get(*halfedge_id as usize) else {
                    return Err(BrepError::new(
                        BrepErrorKind::InvalidHalfEdge,
                        format!(
                            "Wire {} references missing halfedge {}",
                            wire.id, halfedge_id
                        ),
                    ));
                };

                if halfedge.wire_ref != Some(wire.id) {
                    return Err(BrepError::new(
                        BrepErrorKind::BrokenTopology,
                        format!(
                            "Wire {} halfedge {} wire_ref mismatch",
                            wire.id, halfedge_id
                        ),
                    ));
                }

                if wire.is_closed {
                    let expected_next = wire.halfedges[(index + 1) % wire.halfedges.len()];
                    let expected_prev =
                        wire.halfedges[(index + wire.halfedges.len() - 1) % wire.halfedges.len()];

                    if halfedge.next != Some(expected_next) || halfedge.prev != Some(expected_prev)
                    {
                        return Err(BrepError::new(
                            BrepErrorKind::BrokenTopology,
                            format!("Closed wire {} has broken halfedge links", wire.id),
                        ));
                    }
                } else {
                    if index == 0 && halfedge.prev.is_some() {
                        return Err(BrepError::new(
                            BrepErrorKind::BrokenTopology,
                            format!("Open wire {} first halfedge must not have prev", wire.id),
                        ));
                    }

                    if index + 1 == wire.halfedges.len() && halfedge.next.is_some() {
                        return Err(BrepError::new(
                            BrepErrorKind::BrokenTopology,
                            format!("Open wire {} last halfedge must not have next", wire.id),
                        ));
                    }

                    if index + 1 < wire.halfedges.len() {
                        let expected_next = wire.halfedges[index + 1];
                        if halfedge.next != Some(expected_next) {
                            return Err(BrepError::new(
                                BrepErrorKind::BrokenTopology,
                                format!("Open wire {} has broken next link", wire.id),
                            ));
                        }
                    }
                }
            }
        }

        for (index, face) in self.faces.iter().enumerate() {
            if face.id as usize != index {
                return Err(BrepError::new(
                    BrepErrorKind::InvalidFace,
                    format!("Face id mismatch at index {} (id={})", index, face.id),
                ));
            }

            let Some(outer_loop) = self.loops.get(face.outer_loop as usize) else {
                return Err(BrepError::new(
                    BrepErrorKind::InvalidLoop,
                    format!(
                        "Face {} references missing outer loop {}",
                        face.id, face.outer_loop
                    ),
                ));
            };

            if outer_loop.is_hole {
                return Err(BrepError::new(
                    BrepErrorKind::InvalidLoop,
                    format!(
                        "Face {} outer loop {} marked as hole",
                        face.id, face.outer_loop
                    ),
                ));
            }

            for inner_loop_id in &face.inner_loops {
                let Some(inner_loop) = self.loops.get(*inner_loop_id as usize) else {
                    return Err(BrepError::new(
                        BrepErrorKind::InvalidLoop,
                        format!(
                            "Face {} references missing inner loop {}",
                            face.id, inner_loop_id
                        ),
                    ));
                };

                if !inner_loop.is_hole {
                    return Err(BrepError::new(
                        BrepErrorKind::InvalidLoop,
                        format!(
                            "Face {} inner loop {} is not marked as hole",
                            face.id, inner_loop_id
                        ),
                    ));
                }
            }

            if let Some(shell_id) = face.shell_ref {
                let Some(shell) = self.shells.get(shell_id as usize) else {
                    return Err(BrepError::new(
                        BrepErrorKind::InvalidShell,
                        format!("Face {} references missing shell {}", face.id, shell_id),
                    ));
                };

                if !shell.faces.contains(&face.id) {
                    return Err(BrepError::new(
                        BrepErrorKind::BrokenTopology,
                        format!("Face {} shell linkage is inconsistent", face.id),
                    ));
                }
            }
        }

        for shell in &self.shells {
            if shell.id as usize >= self.shells.len() {
                return Err(BrepError::new(
                    BrepErrorKind::InvalidShell,
                    format!("Shell {} has invalid id", shell.id),
                ));
            }

            if shell.faces.is_empty() {
                return Err(BrepError::new(
                    BrepErrorKind::InvalidShell,
                    format!("Shell {} is empty", shell.id),
                ));
            }

            let mut shell_edge_faces: HashMap<u32, HashSet<u32>> = HashMap::new();
            for face_id in &shell.faces {
                let Some(face) = self.faces.get(*face_id as usize) else {
                    return Err(BrepError::new(
                        BrepErrorKind::InvalidFace,
                        format!("Shell {} references missing face {}", shell.id, face_id),
                    ));
                };

                if face.shell_ref != Some(shell.id) {
                    return Err(BrepError::new(
                        BrepErrorKind::BrokenTopology,
                        format!(
                            "Shell {} face linkage mismatch for face {}",
                            shell.id, face.id
                        ),
                    ));
                }

                let mut loop_ids = vec![face.outer_loop];
                loop_ids.extend(face.inner_loops.iter().copied());

                for loop_id in loop_ids {
                    for halfedge_id in self.get_loop_halfedges(loop_id)? {
                        let halfedge = &self.halfedges[halfedge_id as usize];
                        shell_edge_faces
                            .entry(halfedge.edge)
                            .or_default()
                            .insert(face.id);
                    }
                }
            }

            if shell.is_closed {
                for (edge_id, faces) in shell_edge_faces {
                    if faces.len() != 2 {
                        return Err(BrepError::new(
                            BrepErrorKind::BrokenTopology,
                            format!(
                                "Closed shell {} has boundary edge {} with {} incident faces",
                                shell.id,
                                edge_id,
                                faces.len()
                            ),
                        ));
                    }
                }
            }
        }

        Ok(())
    }
}

fn vector_bits(vector: Vector3) -> [u64; 3] {
    [vector.x.to_bits(), vector.y.to_bits(), vector.z.to_bits()]
}

fn compute_loop_normal(brep: &Brep, loop_indices: &[u32]) -> Option<Vector3> {
    if loop_indices.len() < 3 {
        return None;
    }

    let p0 = brep.vertices.get(loop_indices[0] as usize)?.position;
    for index in 1..(loop_indices.len() - 1) {
        let p1 = brep.vertices.get(loop_indices[index] as usize)?.position;
        let p2 = brep
            .vertices
            .get(loop_indices[index + 1] as usize)?
            .position;

        let v1 = [p1.x - p0.x, p1.y - p0.y, p1.z - p0.z];
        let v2 = [p2.x - p0.x, p2.y - p0.y, p2.z - p0.z];

        let cross = [
            v1[1] * v2[2] - v1[2] * v2[1],
            v1[2] * v2[0] - v1[0] * v2[2],
            v1[0] * v2[1] - v1[1] * v2[0],
        ];
        let length_sq = cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2];

        if length_sq > 1.0e-18 {
            let inv = length_sq.sqrt().recip();
            return Some(Vector3::new(cross[0] * inv, cross[1] * inv, cross[2] * inv));
        }
    }

    None
}
