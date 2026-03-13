use std::collections::HashMap;

use boolmesh::compute_boolean;
use boolmesh::prelude::{Manifold, OpType};

use crate::booleans::error::{BooleanError, BooleanErrorKind};
use crate::booleans::types::BooleanOperation;
use crate::brep::Brep;
use crate::operations::triangulate::triangulate_polygon_with_holes;
use openmaths::Vector3;

const EPSILON_FALLBACK: f64 = 1.0e-9;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Vec3f {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) z: f64,
}

impl Vec3f {
    pub(crate) fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub(crate) fn from_vector3(v: &Vector3) -> Self {
        Self::new(v.x, v.y, v.z)
    }

    pub(crate) fn to_vector3(self) -> Vector3 {
        Vector3::new(self.x, self.y, self.z)
    }

    pub(crate) fn add(self, other: Vec3f) -> Self {
        Self::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }

    pub(crate) fn sub(self, other: Vec3f) -> Self {
        Self::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }

    pub(crate) fn scale(self, factor: f64) -> Self {
        Self::new(self.x * factor, self.y * factor, self.z * factor)
    }

    pub(crate) fn dot(self, other: Vec3f) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub(crate) fn cross(self, other: Vec3f) -> Self {
        Self::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    pub(crate) fn norm_sq(self) -> f64 {
        self.dot(self)
    }

    pub(crate) fn norm(self) -> f64 {
        self.norm_sq().sqrt()
    }

    pub(crate) fn normalized(self, epsilon: f64) -> Option<Self> {
        let norm = self.norm();
        if norm <= epsilon.max(EPSILON_FALLBACK) {
            None
        } else {
            Some(self.scale(1.0 / norm))
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Vertex3 {
    pub(crate) position: Vec3f,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Plane3 {
    pub(crate) normal: Vec3f,
    pub(crate) w: f64,
}

impl Plane3 {
    fn from_points(a: Vec3f, b: Vec3f, c: Vec3f, epsilon: f64) -> Option<Self> {
        let normal = b.sub(a).cross(c.sub(a)).normalized(epsilon)?;
        Some(Self {
            normal,
            w: normal.dot(a),
        })
    }

    fn flip(&mut self) {
        self.normal = self.normal.scale(-1.0);
        self.w = -self.w;
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Polygon3 {
    pub(crate) vertices: Vec<Vertex3>,
    pub(crate) plane: Plane3,
}

impl Polygon3 {
    pub(crate) fn new(vertices: Vec<Vertex3>, epsilon: f64) -> Option<Self> {
        let cleaned = sanitize_polygon_vertices(vertices, epsilon);
        if cleaned.len() < 3 {
            return None;
        }

        let plane = plane_from_vertices(&cleaned, epsilon)?;
        Some(Self {
            vertices: cleaned,
            plane,
        })
    }

    pub(crate) fn from_positions(positions: Vec<Vec3f>, epsilon: f64) -> Option<Self> {
        let vertices = positions
            .into_iter()
            .map(|position| Vertex3 { position })
            .collect();
        Self::new(vertices, epsilon)
    }

    pub(crate) fn flipped(&self) -> Self {
        let mut plane = self.plane;
        plane.flip();
        let mut vertices = self.vertices.clone();
        vertices.reverse();
        Self { vertices, plane }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct TriangleMesh {
    pub(crate) positions: Vec<Vec3f>,
    pub(crate) triangles: Vec<[usize; 3]>,
}

#[derive(Clone, Copy, Debug)]
struct MeshBounds {
    min: Vec3f,
    max: Vec3f,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct QuantizedPoint(i64, i64, i64);

/// Converts each BRep face into oriented triangles so the solid kernel receives
/// a manifold triangle mesh rather than arbitrary n-gons.
pub(crate) fn brep_to_polygons(
    brep: &Brep,
    epsilon: f64,
) -> Result<(Vec<Polygon3>, usize), BooleanError> {
    let mut polygons = Vec::new();
    let mut triangle_count = 0;

    for face in &brep.faces {
        let (face_vertices, holes_vertices) = brep.get_vertices_and_holes_by_face_id(face.id);
        if face_vertices.len() < 3 {
            continue;
        }

        let triangles = triangulate_polygon_with_holes(&face_vertices, &holes_vertices);
        let all_vertices: Vec<Vector3> = face_vertices
            .iter()
            .copied()
            .chain(holes_vertices.iter().flatten().copied())
            .collect();

        for triangle in triangles {
            let Some(a) = all_vertices.get(triangle[0]).copied() else {
                continue;
            };
            let Some(b) = all_vertices.get(triangle[1]).copied() else {
                continue;
            };
            let Some(c) = all_vertices.get(triangle[2]).copied() else {
                continue;
            };

            let mut polygon = Polygon3::from_positions(
                vec![
                    Vec3f::from_vector3(&a),
                    Vec3f::from_vector3(&b),
                    Vec3f::from_vector3(&c),
                ],
                epsilon,
            )
            .ok_or_else(|| {
                BooleanError::new(
                    BooleanErrorKind::InvalidOperand,
                    format!(
                        "Face {} produced a degenerate triangle during boolean preparation",
                        face.id
                    ),
                )
            })?;

            let desired = Vec3f::from_vector3(&face.normal);
            if desired.norm_sq() > EPSILON_FALLBACK && polygon.plane.normal.dot(desired) < 0.0 {
                polygon = polygon.flipped();
            }

            polygons.push(polygon);
            triangle_count += 1;
        }
    }

    Ok((polygons, triangle_count))
}

/// Executes the solid boolean against already triangulated polygon soups and
/// returns the watertight triangle mesh emitted by boolmesh.
pub(crate) fn execute_solid_boolean(
    lhs: Vec<Polygon3>,
    rhs: Vec<Polygon3>,
    operation: BooleanOperation,
    epsilon: f64,
) -> Result<TriangleMesh, BooleanError> {
    let lhs_mesh = triangle_mesh_from_polygon_soup(&lhs, epsilon)?;
    let rhs_mesh = triangle_mesh_from_polygon_soup(&rhs, epsilon)?;
    execute_triangle_mesh_boolean(&lhs_mesh, &rhs_mesh, operation, epsilon)
}

/// Runs the solid kernel and converts the resulting manifold triangle mesh back
/// into polygon records for the planar-cap extraction path.
pub(crate) fn execute_polygon_boolean(
    lhs: Vec<Polygon3>,
    rhs: Vec<Polygon3>,
    operation: BooleanOperation,
    epsilon: f64,
) -> Result<Vec<Polygon3>, BooleanError> {
    let result_mesh = execute_solid_boolean(lhs, rhs, operation, epsilon)?;
    triangle_polygons_from_mesh(&result_mesh, epsilon)
}

/// Welds a polygon soup into indexed triangles so boolmesh receives a clean
/// manifold triangle buffer with shared vertices across adjacent faces.
fn triangle_mesh_from_polygon_soup(
    polygons: &[Polygon3],
    epsilon: f64,
) -> Result<TriangleMesh, BooleanError> {
    let mut mesh = TriangleMesh::default();
    let mut vertex_map: HashMap<QuantizedPoint, usize> = HashMap::new();
    let no_holes: Vec<Vec<Vector3>> = Vec::new();

    for polygon in polygons {
        let face_vertices: Vec<Vector3> = polygon
            .vertices
            .iter()
            .map(|vertex| vertex.position.to_vector3())
            .collect();
        if face_vertices.len() < 3 {
            continue;
        }

        let face_triangles = if face_vertices.len() == 3 {
            vec![[0, 1, 2]]
        } else {
            triangulate_polygon_with_holes(&face_vertices, &no_holes)
        };

        for triangle in face_triangles {
            let Some(a) = face_vertices.get(triangle[0]).copied() else {
                continue;
            };
            let Some(b) = face_vertices.get(triangle[1]).copied() else {
                continue;
            };
            let Some(c) = face_vertices.get(triangle[2]).copied() else {
                continue;
            };

            let mut positions = [
                Vec3f::from_vector3(&a),
                Vec3f::from_vector3(&b),
                Vec3f::from_vector3(&c),
            ];
            let triangle_normal = positions[1]
                .sub(positions[0])
                .cross(positions[2].sub(positions[0]));
            if triangle_normal.dot(polygon.plane.normal) < 0.0 {
                positions.swap(1, 2);
            }

            let indices = positions.map(|position| {
                weld_position(&mut mesh.positions, &mut vertex_map, position, epsilon)
            });
            if indices[0] == indices[1] || indices[1] == indices[2] || indices[2] == indices[0] {
                continue;
            }

            mesh.triangles.push(indices);
        }
    }

    Ok(mesh)
}

/// Invokes boolmesh on two closed triangle meshes and lifts the result back
/// into the kernel's simple indexed triangle representation.
fn execute_triangle_mesh_boolean(
    lhs: &TriangleMesh,
    rhs: &TriangleMesh,
    operation: BooleanOperation,
    epsilon: f64,
) -> Result<TriangleMesh, BooleanError> {
    let mut lhs_mesh = lhs.clone();
    let mut rhs_mesh = rhs.clone();
    orient_triangle_mesh_consistently(&mut lhs_mesh)?;
    orient_triangle_mesh_consistently(&mut rhs_mesh)?;
    validate_triangle_mesh_closed(&lhs_mesh)?;
    validate_triangle_mesh_closed(&rhs_mesh)?;

    if !meshes_overlap(&lhs_mesh, &rhs_mesh, epsilon) {
        return Ok(match operation {
            BooleanOperation::Union => concatenate_triangle_meshes(&lhs_mesh, &rhs_mesh),
            BooleanOperation::Intersection => TriangleMesh::default(),
            BooleanOperation::Subtraction => lhs_mesh,
        });
    }

    let mut lhs_manifold = manifold_from_triangle_mesh(&lhs_mesh, epsilon)?;
    let mut rhs_manifold = manifold_from_triangle_mesh(&rhs_mesh, epsilon)?;
    lhs_manifold.set_epsilon(epsilon, false);
    rhs_manifold.set_epsilon(epsilon, false);

    let result = compute_boolean(
        &lhs_manifold,
        &rhs_manifold,
        match operation {
            BooleanOperation::Union => OpType::Add,
            BooleanOperation::Intersection => OpType::Intersect,
            BooleanOperation::Subtraction => OpType::Subtract,
        },
    )
    .map_err(|error| {
        BooleanError::new(
            BooleanErrorKind::KernelFailure,
            format!("Robust solid boolean kernel failed: {}", error),
        )
    })?;

    let positions = result
        .ps
        .iter()
        .map(|point| Vec3f::new(point.x, point.y, point.z))
        .collect::<Vec<_>>();
    let triangles = result
        .hs
        .chunks(3)
        .filter_map(|halfedges| {
            if halfedges.len() != 3 {
                return None;
            }
            Some([halfedges[0].tail, halfedges[1].tail, halfedges[2].tail])
        })
        .filter(|triangle| {
            triangle[0] != triangle[1] && triangle[1] != triangle[2] && triangle[2] != triangle[0]
        })
        .collect();

    Ok(TriangleMesh {
        positions,
        triangles,
    })
}

/// Creates a boolmesh manifold from a welded triangle buffer while surfacing a
/// structured kernel error if the input mesh is not manifold.
fn manifold_from_triangle_mesh(
    mesh: &TriangleMesh,
    epsilon: f64,
) -> Result<Manifold, BooleanError> {
    let positions = mesh
        .positions
        .iter()
        .flat_map(|position| [position.x, position.y, position.z])
        .collect::<Vec<_>>();
    let indices = mesh
        .triangles
        .iter()
        .flat_map(|triangle| [triangle[0], triangle[1], triangle[2]])
        .collect::<Vec<_>>();

    let mut manifold = Manifold::new(&positions, &indices).map_err(|error| {
        BooleanError::new(
            BooleanErrorKind::InvalidOperand,
            format!(
                "Solid operand is not a valid manifold triangle mesh: {}",
                error
            ),
        )
    })?;
    manifold.set_epsilon(epsilon, false);
    Ok(manifold)
}

/// Converts a triangle mesh result back into polygons while preserving the
/// face winding produced by the robust solid kernel.
fn triangle_polygons_from_mesh(
    mesh: &TriangleMesh,
    epsilon: f64,
) -> Result<Vec<Polygon3>, BooleanError> {
    let mut polygons = Vec::with_capacity(mesh.triangles.len());

    for triangle in &mesh.triangles {
        let Some(a) = mesh.positions.get(triangle[0]).copied() else {
            return Err(BooleanError::new(
                BooleanErrorKind::KernelFailure,
                "Boolean kernel produced a triangle with an invalid first vertex index",
            ));
        };
        let Some(b) = mesh.positions.get(triangle[1]).copied() else {
            return Err(BooleanError::new(
                BooleanErrorKind::KernelFailure,
                "Boolean kernel produced a triangle with an invalid second vertex index",
            ));
        };
        let Some(c) = mesh.positions.get(triangle[2]).copied() else {
            return Err(BooleanError::new(
                BooleanErrorKind::KernelFailure,
                "Boolean kernel produced a triangle with an invalid third vertex index",
            ));
        };

        let polygon = Polygon3::from_positions(vec![a, b, c], epsilon).ok_or_else(|| {
            BooleanError::new(
                BooleanErrorKind::KernelFailure,
                "Boolean kernel produced a degenerate result triangle",
            )
        })?;
        polygons.push(polygon);
    }

    Ok(polygons)
}

/// Inserts a vertex into the indexed mesh using tolerance-based welding so the
/// rebuilt manifold preserves shared topology across adjacent faces.
fn weld_position(
    positions: &mut Vec<Vec3f>,
    vertex_map: &mut HashMap<QuantizedPoint, usize>,
    position: Vec3f,
    epsilon: f64,
) -> usize {
    let key = quantize_point(position, epsilon);
    if let Some(existing) = vertex_map.get(&key).copied() {
        existing
    } else {
        let index = positions.len();
        positions.push(position);
        vertex_map.insert(key, index);
        index
    }
}

/// Drops duplicate and collinear vertices so the solid kernel only receives
/// geometrically valid polygons.
fn sanitize_polygon_vertices(vertices: Vec<Vertex3>, epsilon: f64) -> Vec<Vertex3> {
    let mut cleaned = Vec::new();
    let epsilon_sq = epsilon.max(EPSILON_FALLBACK).powi(2);

    for vertex in vertices {
        let is_duplicate = cleaned.last().map_or(false, |previous: &Vertex3| {
            previous.position.sub(vertex.position).norm_sq() <= epsilon_sq
        });
        if !is_duplicate {
            cleaned.push(vertex);
        }
    }

    if cleaned.len() > 2 {
        let first = cleaned[0];
        let last = cleaned[cleaned.len() - 1];
        if first.position.sub(last.position).norm_sq() <= epsilon_sq {
            cleaned.pop();
        }
    }

    let mut compact = Vec::new();
    for index in 0..cleaned.len() {
        let prev = cleaned[(index + cleaned.len() - 1) % cleaned.len()];
        let current = cleaned[index];
        let next = cleaned[(index + 1) % cleaned.len()];
        let edge_a = current.position.sub(prev.position);
        let edge_b = next.position.sub(current.position);
        let keep = edge_a.cross(edge_b).norm_sq() > epsilon_sq;
        if keep {
            compact.push(current);
        }
    }

    compact
}

/// Finds the first non-degenerate support plane for a vertex loop.
fn plane_from_vertices(vertices: &[Vertex3], epsilon: f64) -> Option<Plane3> {
    if vertices.len() < 3 {
        return None;
    }

    for index in 1..(vertices.len() - 1) {
        let plane = Plane3::from_points(
            vertices[0].position,
            vertices[index].position,
            vertices[index + 1].position,
            epsilon,
        );
        if plane.is_some() {
            return plane;
        }
    }

    None
}

/// Quantizes a 3D point into tolerance-sized cells so nearby vertices weld to
/// a shared index before the mesh is handed to the solid kernel.
fn quantize_point(position: Vec3f, epsilon: f64) -> QuantizedPoint {
    let scale = epsilon.max(EPSILON_FALLBACK);
    QuantizedPoint(
        (position.x / scale).round() as i64,
        (position.y / scale).round() as i64,
        (position.z / scale).round() as i64,
    )
}

/// Reorients adjacent triangles so every shared edge is traversed in opposite
/// directions before the mesh is handed to the manifold kernel.
fn orient_triangle_mesh_consistently(mesh: &mut TriangleMesh) -> Result<(), BooleanError> {
    let mut edge_map: HashMap<(usize, usize), Vec<(usize, usize, usize)>> = HashMap::new();

    for (triangle_index, triangle) in mesh.triangles.iter().copied().enumerate() {
        for (start, end) in triangle_edges(triangle) {
            let key = if start <= end {
                (start, end)
            } else {
                (end, start)
            };
            edge_map
                .entry(key)
                .or_default()
                .push((triangle_index, start, end));
        }
    }

    let mut adjacency = vec![Vec::<(usize, bool)>::new(); mesh.triangles.len()];
    for uses in edge_map.values() {
        if uses.len() != 2 {
            continue;
        }

        let same_direction = uses[0].1 == uses[1].1 && uses[0].2 == uses[1].2;
        adjacency[uses[0].0].push((uses[1].0, same_direction));
        adjacency[uses[1].0].push((uses[0].0, same_direction));
    }

    let mut flip_state = vec![None; mesh.triangles.len()];
    for seed in 0..mesh.triangles.len() {
        if flip_state[seed].is_some() {
            continue;
        }

        flip_state[seed] = Some(false);
        let mut stack = vec![seed];
        while let Some(current) = stack.pop() {
            let current_flip = flip_state[current].unwrap_or(false);
            for &(neighbor, same_direction) in &adjacency[current] {
                let required_flip = current_flip ^ same_direction;
                match flip_state[neighbor] {
                    Some(existing) if existing != required_flip => {
                        return Err(BooleanError::new(
                            BooleanErrorKind::InvalidOperand,
                            "Triangle mesh has inconsistent winding across shared edges",
                        ));
                    }
                    Some(_) => {}
                    None => {
                        flip_state[neighbor] = Some(required_flip);
                        stack.push(neighbor);
                    }
                }
            }
        }
    }

    for (triangle, should_flip) in mesh.triangles.iter_mut().zip(flip_state.into_iter()) {
        if should_flip.unwrap_or(false) {
            triangle.swap(1, 2);
        }
    }

    Ok(())
}

/// Verifies that every undirected edge is used by exactly two triangles after
/// orientation normalization so the downstream solid kernel receives a closed mesh.
fn validate_triangle_mesh_closed(mesh: &TriangleMesh) -> Result<(), BooleanError> {
    let mut edge_counts: HashMap<(usize, usize), usize> = HashMap::new();

    for triangle in &mesh.triangles {
        for (start, end) in triangle_edges(*triangle) {
            let key = if start <= end {
                (start, end)
            } else {
                (end, start)
            };
            *edge_counts.entry(key).or_insert(0) += 1;
        }
    }

    if edge_counts.values().all(|count| *count == 2) {
        Ok(())
    } else {
        Err(BooleanError::new(
            BooleanErrorKind::InvalidOperand,
            "Triangle mesh is not closed or contains non-manifold edges",
        ))
    }
}

/// Combines disjoint triangle meshes into a single multi-shell result without
/// routing them through the solid kernel, which cannot emit an empty overlap.
fn concatenate_triangle_meshes(lhs: &TriangleMesh, rhs: &TriangleMesh) -> TriangleMesh {
    let offset = lhs.positions.len();
    let mut positions = lhs.positions.clone();
    positions.extend(rhs.positions.iter().copied());

    let mut triangles = lhs.triangles.clone();
    triangles.extend(rhs.triangles.iter().map(|triangle| {
        [
            triangle[0] + offset,
            triangle[1] + offset,
            triangle[2] + offset,
        ]
    }));

    TriangleMesh {
        positions,
        triangles,
    }
}

/// Determines whether two meshes have overlapping bounding boxes within the
/// boolean tolerance so disjoint cases can be short-circuited safely.
fn meshes_overlap(lhs: &TriangleMesh, rhs: &TriangleMesh, epsilon: f64) -> bool {
    let Some(lhs_bounds) = mesh_bounds(lhs) else {
        return false;
    };
    let Some(rhs_bounds) = mesh_bounds(rhs) else {
        return false;
    };

    !(lhs_bounds.max.x < rhs_bounds.min.x - epsilon
        || rhs_bounds.max.x < lhs_bounds.min.x - epsilon
        || lhs_bounds.max.y < rhs_bounds.min.y - epsilon
        || rhs_bounds.max.y < lhs_bounds.min.y - epsilon
        || lhs_bounds.max.z < rhs_bounds.min.z - epsilon
        || rhs_bounds.max.z < lhs_bounds.min.z - epsilon)
}

/// Computes a triangle mesh bounding box for the disjoint-operation fast path.
fn mesh_bounds(mesh: &TriangleMesh) -> Option<MeshBounds> {
    let first = *mesh.positions.first()?;
    let mut min = first;
    let mut max = first;

    for position in &mesh.positions[1..] {
        min.x = min.x.min(position.x);
        min.y = min.y.min(position.y);
        min.z = min.z.min(position.z);
        max.x = max.x.max(position.x);
        max.y = max.y.max(position.y);
        max.z = max.z.max(position.z);
    }

    Some(MeshBounds { min, max })
}

/// Returns the three directed edges of a triangle in winding order.
fn triangle_edges(triangle: [usize; 3]) -> [(usize, usize); 3] {
    [
        (triangle[0], triangle[1]),
        (triangle[1], triangle[2]),
        (triangle[2], triangle[0]),
    ]
}
