use std::collections::{BTreeMap, HashMap, HashSet};

use uuid::Uuid;

use crate::booleans::error::{BooleanError, BooleanErrorKind};
use crate::booleans::solid::{Polygon3, TriangleMesh, Vec3f};
use crate::brep::{Brep, BrepBuilder, Shell};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct QuantizedPoint(i64, i64, i64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct QuantizedPlane(i64, i64, i64, i64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct QuantizedPoint2(i64, i64);

#[derive(Clone, Copy, Debug)]
struct Vec2f {
    x: f64,
    y: f64,
}

impl Vec2f {
    fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    fn sub(self, other: Vec2f) -> Self {
        Self::new(self.x - other.x, self.y - other.y)
    }

    fn dot(self, other: Vec2f) -> f64 {
        self.x * other.x + self.y * other.y
    }

    fn cross(self, other: Vec2f) -> f64 {
        self.x * other.y - self.y * other.x
    }

    fn norm_sq(self) -> f64 {
        self.dot(self)
    }
}

#[derive(Clone, Copy, Debug)]
struct PlaneBasis {
    origin: Vec3f,
    u_axis: Vec3f,
    v_axis: Vec3f,
}

impl PlaneBasis {
    fn from_polygon(polygon: &Polygon3, tolerance: f64) -> Option<Self> {
        let origin = polygon.vertices.first()?.position;
        let normal = polygon.plane.normal.normalized(tolerance)?;
        let reference = if normal.z.abs() < 0.9 {
            Vec3f::new(0.0, 0.0, 1.0)
        } else {
            Vec3f::new(0.0, 1.0, 0.0)
        };
        let u_axis = reference.cross(normal).normalized(tolerance)?;
        let v_axis = normal.cross(u_axis).normalized(tolerance)?;
        Some(Self {
            origin,
            u_axis,
            v_axis,
        })
    }

    fn project(&self, point: Vec3f) -> Vec2f {
        let relative = point.sub(self.origin);
        Vec2f::new(relative.dot(self.u_axis), relative.dot(self.v_axis))
    }

    fn lift(&self, point: Vec2f) -> Vec3f {
        self.origin
            .add(self.u_axis.scale(point.x))
            .add(self.v_axis.scale(point.y))
    }
}

#[derive(Clone, Copy, Debug)]
struct BoundaryEdge {
    start: QuantizedPoint2,
    end: QuantizedPoint2,
}

#[derive(Clone, Debug)]
struct RebuiltFace {
    outer: Vec<Vec3f>,
    holes: Vec<Vec<Vec3f>>,
}

/// Rebuilds a face-based BRep from the polygon soup emitted by the boolean engine.
pub(crate) fn build_brep_from_polygons(
    polygons: &[Polygon3],
    tolerance: f64,
    create_shells: bool,
) -> Result<Brep, BooleanError> {
    let polygons = deduplicate_polygons(split_t_junctions(polygons, tolerance), tolerance);
    let rebuilt_faces = merge_coplanar_faces(&polygons, tolerance)?;
    let mut builder = BrepBuilder::new(Uuid::new_v4());
    let mut vertex_map: HashMap<QuantizedPoint, u32> = HashMap::new();

    for rebuilt_face in rebuilt_faces {
        let outer_indices = collect_face_indices(
            &mut builder,
            &mut vertex_map,
            &rebuilt_face.outer,
            tolerance,
        );
        let outer_indices = sanitize_indices(outer_indices);
        if outer_indices.len() < 3 {
            continue;
        }

        let hole_indices: Vec<Vec<u32>> = rebuilt_face
            .holes
            .iter()
            .map(|hole| {
                sanitize_indices(collect_face_indices(
                    &mut builder,
                    &mut vertex_map,
                    hole,
                    tolerance,
                ))
            })
            .filter(|hole| hole.len() >= 3)
            .collect();

        builder
            .add_face(&outer_indices, &hole_indices)
            .map_err(|error| {
                BooleanError::new(
                    BooleanErrorKind::TopologyError,
                    format!("Failed to rebuild boolean face: {}", error),
                )
            })?;
    }

    let mut brep = builder.build().map_err(|error| {
        BooleanError::new(
            BooleanErrorKind::TopologyError,
            format!("Failed to finalize boolean BRep: {}", error),
        )
    })?;

    if create_shells && !brep.faces.is_empty() {
        assign_shells_by_connectivity(&mut brep);
        brep.validate_topology().map_err(|error| {
            BooleanError::new(
                BooleanErrorKind::TopologyError,
                format!("Rebuilt boolean shell topology is invalid: {}", error),
            )
        })?;
    }

    Ok(brep)
}

/// Rebuilds a BRep directly from a manifold triangle mesh so watertight solid
/// results do not depend on the coplanar face merge path.
pub(crate) fn build_brep_from_triangle_mesh(
    mesh: &TriangleMesh,
    tolerance: f64,
    create_shells: bool,
) -> Result<Brep, BooleanError> {
    let mut builder = BrepBuilder::new(Uuid::new_v4());
    let mut vertex_map: HashMap<QuantizedPoint, u32> = HashMap::new();
    let mut welded_indices = vec![0; mesh.positions.len()];

    for (index, position) in mesh.positions.iter().copied().enumerate() {
        let key = quantize(position, tolerance);
        let vertex_id = if let Some(existing) = vertex_map.get(&key).copied() {
            existing
        } else {
            let id = builder.add_vertex(position.to_vector3());
            vertex_map.insert(key, id);
            id
        };
        welded_indices[index] = vertex_id;
    }

    for triangle in &mesh.triangles {
        let Some(&a) = welded_indices.get(triangle[0]) else {
            continue;
        };
        let Some(&b) = welded_indices.get(triangle[1]) else {
            continue;
        };
        let Some(&c) = welded_indices.get(triangle[2]) else {
            continue;
        };

        let face = sanitize_indices(vec![a, b, c]);
        if face.len() < 3 {
            continue;
        }

        builder.add_face(&face, &[]).map_err(|error| {
            BooleanError::new(
                BooleanErrorKind::TopologyError,
                format!("Failed to rebuild boolean triangle face: {}", error),
            )
        })?;
    }

    let mut brep = builder.build().map_err(|error| {
        BooleanError::new(
            BooleanErrorKind::TopologyError,
            format!("Failed to finalize boolean triangle BRep: {}", error),
        )
    })?;

    if create_shells && !brep.faces.is_empty() {
        assign_shells_by_connectivity(&mut brep);
        brep.validate_topology().map_err(|error| {
            BooleanError::new(
                BooleanErrorKind::TopologyError,
                format!("Rebuilt boolean shell topology is invalid: {}", error),
            )
        })?;
    }

    Ok(brep)
}

/// Collapses exact duplicate polygons before the coplanar merge step starts.
fn deduplicate_polygons(polygons: Vec<Polygon3>, tolerance: f64) -> Vec<Polygon3> {
    let mut seen_faces: HashSet<Vec<QuantizedPoint>> = HashSet::new();
    let mut unique = Vec::new();

    for polygon in polygons {
        let positions = sanitize_positions(
            polygon
                .vertices
                .iter()
                .map(|vertex| vertex.position)
                .collect::<Vec<_>>(),
            tolerance,
        );
        if positions.len() < 3 {
            continue;
        }

        let signature = polygon_signature(&positions, tolerance);
        if seen_faces.insert(signature) {
            unique.push(polygon);
        }
    }

    unique
}

/// Merges coplanar polygon fragments by cancelling shared edges and tracing the
/// surviving boundary loops back into face outer/inner rings.
fn merge_coplanar_faces(
    polygons: &[Polygon3],
    tolerance: f64,
) -> Result<Vec<RebuiltFace>, BooleanError> {
    let mut grouped: BTreeMap<QuantizedPlane, Vec<Polygon3>> = BTreeMap::new();

    for polygon in polygons {
        grouped
            .entry(quantize_plane(polygon, tolerance))
            .or_default()
            .push(polygon.clone());
    }

    let mut rebuilt_faces = Vec::new();
    for group in grouped.into_values() {
        rebuilt_faces.extend(merge_coplanar_group(&group, tolerance)?);
    }

    Ok(rebuilt_faces)
}

/// Merges one coplanar polygon bucket into stitched faces with optional holes.
fn merge_coplanar_group(
    polygons: &[Polygon3],
    tolerance: f64,
) -> Result<Vec<RebuiltFace>, BooleanError> {
    let Some(basis) = PlaneBasis::from_polygon(&polygons[0], tolerance) else {
        return Err(BooleanError::new(
            BooleanErrorKind::TopologyError,
            "Failed to derive a stable plane basis while rebuilding boolean faces",
        ));
    };

    let mut point_map: HashMap<QuantizedPoint2, Vec2f> = HashMap::new();
    let mut edge_map: HashMap<(QuantizedPoint2, QuantizedPoint2), Vec<BoundaryEdge>> =
        HashMap::new();

    for polygon in polygons {
        let mut positions = sanitize_positions(
            polygon
                .vertices
                .iter()
                .map(|vertex| vertex.position)
                .collect::<Vec<_>>(),
            tolerance,
        );
        if positions.len() < 3 {
            continue;
        }

        let mut projected = positions
            .iter()
            .copied()
            .map(|position| basis.project(position))
            .collect::<Vec<_>>();

        if signed_area_2d(&projected) < 0.0 {
            positions.reverse();
            projected.reverse();
        }

        for index in 0..positions.len() {
            let next = (index + 1) % positions.len();
            let start_key = quantize_2d(projected[index], tolerance);
            let end_key = quantize_2d(projected[next], tolerance);
            if start_key == end_key {
                continue;
            }

            point_map.entry(start_key).or_insert(projected[index]);
            point_map.entry(end_key).or_insert(projected[next]);

            edge_map
                .entry(ordered_segment_key(start_key, end_key))
                .or_default()
                .push(BoundaryEdge {
                    start: start_key,
                    end: end_key,
                });
        }
    }

    let boundary_edges: Vec<BoundaryEdge> = edge_map
        .into_values()
        .filter_map(|occurrences| {
            if occurrences.len() % 2 == 1 {
                occurrences.last().copied()
            } else {
                None
            }
        })
        .collect();

    if boundary_edges.is_empty() {
        return Ok(Vec::new());
    }

    let loops = trace_boundary_loops(&boundary_edges, &point_map, tolerance);
    Ok(classify_planar_loops(loops, &basis, &point_map, tolerance))
}

/// Splits polygon edges anywhere another polygon contributes an on-edge vertex
/// so the rebuilt BRep does not inherit T-junction cracks.
fn split_t_junctions(polygons: &[Polygon3], tolerance: f64) -> Vec<Polygon3> {
    let all_points: Vec<Vec3f> = polygons
        .iter()
        .flat_map(|polygon| polygon.vertices.iter().map(|vertex| vertex.position))
        .collect();
    let tolerance_sq = tolerance * tolerance;

    polygons
        .iter()
        .map(|polygon| {
            let mut positions = Vec::new();

            for index in 0..polygon.vertices.len() {
                let start = polygon.vertices[index].position;
                let end = polygon.vertices[(index + 1) % polygon.vertices.len()].position;
                positions.push(start);

                let mut inserts = all_points
                    .iter()
                    .copied()
                    .filter_map(|candidate| {
                        let start_distance = candidate.sub(start).norm_sq();
                        let end_distance = candidate.sub(end).norm_sq();
                        if start_distance <= tolerance_sq || end_distance <= tolerance_sq {
                            return None;
                        }

                        point_on_segment(candidate, start, end, tolerance)
                    })
                    .collect::<Vec<_>>();

                inserts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

                let mut last_t = None;
                for (t, point) in inserts {
                    if last_t
                        .map(|prev: f64| (prev - t).abs() <= tolerance)
                        .unwrap_or(false)
                    {
                        continue;
                    }
                    positions.push(point);
                    last_t = Some(t);
                }
            }

            let Some(mut rebuilt) = Polygon3::from_positions(positions, tolerance) else {
                return polygon.clone();
            };

            if rebuilt.plane.normal.dot(polygon.plane.normal) < 0.0 {
                rebuilt = rebuilt.flipped();
            }
            rebuilt
        })
        .collect()
}

/// Converts merged face rings into builder vertex ids while welding points by tolerance.
fn collect_face_indices(
    builder: &mut BrepBuilder,
    vertex_map: &mut HashMap<QuantizedPoint, u32>,
    positions: &[Vec3f],
    tolerance: f64,
) -> Vec<u32> {
    let positions = sanitize_positions(positions.to_vec(), tolerance);
    let mut indices = Vec::with_capacity(positions.len());

    for position in positions {
        let key = quantize(position, tolerance);
        let vertex_id = if let Some(existing) = vertex_map.get(&key).copied() {
            existing
        } else {
            let id = builder.add_vertex(position.to_vector3());
            vertex_map.insert(key, id);
            id
        };
        indices.push(vertex_id);
    }

    indices
}

/// Groups rebuilt faces into shells by shared-edge connectivity and marks
/// whether each connected component is closed.
fn assign_shells_by_connectivity(brep: &mut Brep) {
    brep.shells.clear();
    for face in &mut brep.faces {
        face.shell_ref = None;
    }

    let mut edge_to_faces: HashMap<u32, Vec<u32>> = HashMap::new();
    for halfedge in &brep.halfedges {
        if let Some(face_id) = halfedge.face {
            edge_to_faces
                .entry(halfedge.edge)
                .or_default()
                .push(face_id);
        }
    }

    let mut face_adjacency: HashMap<u32, HashSet<u32>> = HashMap::new();
    for faces in edge_to_faces.values() {
        if faces.len() < 2 {
            continue;
        }

        for &face_id in faces {
            let entry = face_adjacency.entry(face_id).or_default();
            for &other_face in faces {
                if other_face != face_id {
                    entry.insert(other_face);
                }
            }
        }
    }

    let mut visited = HashSet::new();
    for face_id in 0..brep.faces.len() as u32 {
        if !visited.insert(face_id) {
            continue;
        }

        let mut stack = vec![face_id];
        let mut component = Vec::new();
        while let Some(current) = stack.pop() {
            component.push(current);
            for adjacent in face_adjacency.get(&current).cloned().unwrap_or_default() {
                if visited.insert(adjacent) {
                    stack.push(adjacent);
                }
            }
        }

        let shell_id = brep.shells.len() as u32;
        let is_closed = component_is_closed(&component, &edge_to_faces);
        brep.shells
            .push(Shell::new(shell_id, component.clone(), is_closed));
        for face_id in component {
            if let Some(face) = brep.faces.get_mut(face_id as usize) {
                face.shell_ref = Some(shell_id);
            }
        }
    }
}

/// Treats a component as closed only when none of its edges are exposed exactly once.
fn component_is_closed(component: &[u32], edge_to_faces: &HashMap<u32, Vec<u32>>) -> bool {
    let component_faces: HashSet<u32> = component.iter().copied().collect();
    for faces in edge_to_faces.values() {
        let incident = faces
            .iter()
            .copied()
            .filter(|face_id| component_faces.contains(face_id))
            .count();
        if incident == 1 {
            return false;
        }
    }
    true
}

/// Removes consecutive duplicates and a repeated closing vertex from a polygon loop.
fn sanitize_positions(points: Vec<Vec3f>, tolerance: f64) -> Vec<Vec3f> {
    let tolerance_sq = tolerance * tolerance;
    let mut sanitized = Vec::new();

    for point in points {
        let is_duplicate = sanitized
            .last()
            .map(|previous: &Vec3f| previous.sub(point).norm_sq() <= tolerance_sq)
            .unwrap_or(false);
        if !is_duplicate {
            sanitized.push(point);
        }
    }

    if sanitized.len() > 2 {
        let first = sanitized[0];
        let last = sanitized[sanitized.len() - 1];
        if first.sub(last).norm_sq() <= tolerance_sq {
            sanitized.pop();
        }
    }

    sanitized
}

/// Removes duplicate/collinear planar boundary vertices after the merge trace.
fn sanitize_loop_keys(
    mut keys: Vec<QuantizedPoint2>,
    point_map: &HashMap<QuantizedPoint2, Vec2f>,
    tolerance: f64,
) -> Vec<QuantizedPoint2> {
    keys.dedup();
    if keys.len() > 2 && keys.first() == keys.last() {
        keys.pop();
    }

    if keys.len() < 3 {
        return Vec::new();
    }

    let tolerance_sq = tolerance * tolerance;
    let mut sanitized = Vec::new();

    for index in 0..keys.len() {
        let prev = keys[(index + keys.len() - 1) % keys.len()];
        let current = keys[index];
        let next = keys[(index + 1) % keys.len()];

        let Some(prev_point) = point_map.get(&prev).copied() else {
            continue;
        };
        let Some(current_point) = point_map.get(&current).copied() else {
            continue;
        };
        let Some(next_point) = point_map.get(&next).copied() else {
            continue;
        };

        let edge_a = current_point.sub(prev_point);
        let edge_b = next_point.sub(current_point);
        if edge_a.norm_sq() <= tolerance_sq || edge_b.norm_sq() <= tolerance_sq {
            continue;
        }

        if edge_a.cross(edge_b).abs() <= tolerance_sq && edge_a.dot(edge_b) >= 0.0 {
            continue;
        }

        sanitized.push(current);
    }

    sanitized
}

/// Removes consecutive duplicate vertex ids before the face is handed to the builder.
fn sanitize_indices(indices: Vec<u32>) -> Vec<u32> {
    let mut sanitized = Vec::new();
    for index in indices {
        let is_duplicate = sanitized.last().copied() == Some(index);
        if !is_duplicate {
            sanitized.push(index);
        }
    }

    if sanitized.len() > 2 && sanitized.first() == sanitized.last() {
        sanitized.pop();
    }

    sanitized
}

/// Builds an order-independent face signature so duplicate polygons can be discarded.
fn polygon_signature(points: &[Vec3f], tolerance: f64) -> Vec<QuantizedPoint> {
    let mut signature: Vec<QuantizedPoint> = points
        .iter()
        .copied()
        .map(|point| quantize(point, tolerance))
        .collect();
    signature.sort_unstable_by_key(|point| (point.0, point.1, point.2));
    signature
}

/// Quantizes a point into tolerance-sized grid cells for robust matching.
fn quantize(point: Vec3f, tolerance: f64) -> QuantizedPoint {
    let scale = tolerance.max(1.0e-9);
    QuantizedPoint(
        (point.x / scale).round() as i64,
        (point.y / scale).round() as i64,
        (point.z / scale).round() as i64,
    )
}

/// Quantizes a planar point so coplanar edge stitching can use hash-map lookups.
fn quantize_2d(point: Vec2f, tolerance: f64) -> QuantizedPoint2 {
    let scale = tolerance.max(1.0e-9);
    QuantizedPoint2(
        (point.x / scale).round() as i64,
        (point.y / scale).round() as i64,
    )
}

/// Buckets polygons that lie on the same support plane.
fn quantize_plane(polygon: &Polygon3, tolerance: f64) -> QuantizedPlane {
    let normal_scale = 1.0e4;
    let offset_scale = (tolerance * 8.0).max(1.0e-6);
    QuantizedPlane(
        (polygon.plane.normal.x * normal_scale).round() as i64,
        (polygon.plane.normal.y * normal_scale).round() as i64,
        (polygon.plane.normal.z * normal_scale).round() as i64,
        (polygon.plane.w / offset_scale).round() as i64,
    )
}

/// Uses the undirected segment key so shared interior edges can cancel cleanly.
fn ordered_segment_key(
    start: QuantizedPoint2,
    end: QuantizedPoint2,
) -> (QuantizedPoint2, QuantizedPoint2) {
    if start <= end {
        (start, end)
    } else {
        (end, start)
    }
}

/// Traces closed loops from the surviving boundary edges of a coplanar polygon group.
fn trace_boundary_loops(
    boundary_edges: &[BoundaryEdge],
    point_map: &HashMap<QuantizedPoint2, Vec2f>,
    tolerance: f64,
) -> Vec<Vec<QuantizedPoint2>> {
    let mut adjacency: HashMap<QuantizedPoint2, Vec<usize>> = HashMap::new();
    for (index, edge) in boundary_edges.iter().enumerate() {
        adjacency.entry(edge.start).or_default().push(index);
    }

    let mut used = HashSet::new();
    let mut loops = Vec::new();

    for start_index in 0..boundary_edges.len() {
        if used.contains(&start_index) {
            continue;
        }

        let start_edge = boundary_edges[start_index];
        let mut loop_keys = vec![start_edge.start];
        let mut current_index = start_index;
        let mut valid = true;

        for _ in 0..=boundary_edges.len() {
            let edge = boundary_edges[current_index];
            used.insert(current_index);

            if edge.end == loop_keys[0] {
                break;
            }

            loop_keys.push(edge.end);

            let Some(next_index) = choose_next_boundary_edge(
                edge.end,
                edge.start,
                boundary_edges,
                &adjacency,
                &used,
                point_map,
            ) else {
                valid = false;
                break;
            };

            current_index = next_index;
        }

        if !valid {
            continue;
        }

        let loop_keys = sanitize_loop_keys(loop_keys, point_map, tolerance);
        if loop_keys.len() >= 3 {
            loops.push(loop_keys);
        }
    }

    loops
}

/// Chooses the next edge in a traced boundary loop, preferring the straightest
/// continuation so split edges collapse back into one ring.
fn choose_next_boundary_edge(
    current: QuantizedPoint2,
    previous: QuantizedPoint2,
    boundary_edges: &[BoundaryEdge],
    adjacency: &HashMap<QuantizedPoint2, Vec<usize>>,
    used: &HashSet<usize>,
    point_map: &HashMap<QuantizedPoint2, Vec2f>,
) -> Option<usize> {
    let point = point_map.get(&current).copied()?;
    let previous_point = point_map.get(&previous).copied()?;
    let incoming = point.sub(previous_point);
    let incoming_norm_sq = incoming.norm_sq();

    let candidates: Vec<usize> = adjacency
        .get(&current)?
        .iter()
        .copied()
        .filter(|index| !used.contains(index))
        .collect();

    if candidates.is_empty() {
        return None;
    }

    let non_backtracking: Vec<usize> = candidates
        .iter()
        .copied()
        .filter(|index| boundary_edges[*index].end != previous)
        .collect();

    let candidates = if non_backtracking.is_empty() {
        candidates
    } else {
        non_backtracking
    };

    if candidates.len() == 1 {
        return candidates.first().copied();
    }

    let mut best = None;
    for candidate in candidates {
        let Some(candidate_point) = point_map.get(&boundary_edges[candidate].end).copied() else {
            continue;
        };

        let outgoing = candidate_point.sub(point);
        let outgoing_norm_sq = outgoing.norm_sq();
        if incoming_norm_sq <= 0.0 || outgoing_norm_sq <= 0.0 {
            continue;
        }

        let alignment =
            incoming.dot(outgoing) / (incoming_norm_sq.sqrt() * outgoing_norm_sq.sqrt());
        let turn = incoming.cross(outgoing).abs();

        let score = (alignment, -turn);
        if best
            .map(|(_, best_score)| score > best_score)
            .unwrap_or(true)
        {
            best = Some((candidate, score));
        }
    }

    best.map(|(candidate, _)| candidate)
}

/// Separates traced loops into outer rings and holes and lifts them back onto
/// the original plane.
fn classify_planar_loops(
    loops: Vec<Vec<QuantizedPoint2>>,
    basis: &PlaneBasis,
    point_map: &HashMap<QuantizedPoint2, Vec2f>,
    tolerance: f64,
) -> Vec<RebuiltFace> {
    let mut outers = Vec::new();
    let mut holes = Vec::new();

    for loop_keys in loops {
        let points = loop_keys
            .iter()
            .filter_map(|key| point_map.get(key).copied())
            .collect::<Vec<_>>();
        if points.len() < 3 {
            continue;
        }

        let area = signed_area_2d(&points);
        if area.abs() <= tolerance * tolerance {
            continue;
        }

        if area > 0.0 {
            outers.push(points);
        } else {
            holes.push(points);
        }
    }

    if outers.is_empty() {
        for hole in holes.drain(..) {
            let mut reversed = hole;
            reversed.reverse();
            outers.push(reversed);
        }
    }

    let mut rebuilt_faces: Vec<RebuiltFace> = outers
        .iter()
        .map(|outer| RebuiltFace {
            outer: lift_loop(outer, basis),
            holes: Vec::new(),
        })
        .collect();

    let outer_areas: Vec<f64> = outers
        .iter()
        .map(|outer| signed_area_2d(outer).abs())
        .collect();

    for hole in holes {
        let probe = hole[0];
        let mut container = None;
        let mut smallest_area = f64::INFINITY;

        for (index, outer) in outers.iter().enumerate() {
            if point_in_polygon(probe, outer) && outer_areas[index] < smallest_area {
                container = Some(index);
                smallest_area = outer_areas[index];
            }
        }

        if let Some(index) = container {
            rebuilt_faces[index].holes.push(lift_loop(&hole, basis));
        }
    }

    rebuilt_faces
}

/// Converts a planar loop back into 3D while preserving the support plane.
fn lift_loop(loop_points: &[Vec2f], basis: &PlaneBasis) -> Vec<Vec3f> {
    loop_points
        .iter()
        .copied()
        .map(|point| basis.lift(point))
        .collect()
}

/// Computes the signed area so merged loops can be classified as outer rings or holes.
fn signed_area_2d(points: &[Vec2f]) -> f64 {
    if points.len() < 3 {
        return 0.0;
    }

    let mut area = 0.0;
    for index in 0..points.len() {
        let current = points[index];
        let next = points[(index + 1) % points.len()];
        area += current.cross(next);
    }

    area * 0.5
}

/// Uses ray casting to assign traced holes to the smallest enclosing outer loop.
fn point_in_polygon(point: Vec2f, polygon: &[Vec2f]) -> bool {
    let mut inside = false;

    for index in 0..polygon.len() {
        let current = polygon[index];
        let next = polygon[(index + 1) % polygon.len()];
        let denominator = next.y - current.y;
        if denominator.abs() <= 1.0e-12 {
            continue;
        }

        let intersects = ((current.y > point.y) != (next.y > point.y))
            && (point.x < (next.x - current.x) * (point.y - current.y) / denominator + current.x);
        if intersects {
            inside = !inside;
        }
    }

    inside
}

/// Returns the parametric position of a point that lies strictly inside the segment.
fn point_on_segment(
    point: Vec3f,
    start: Vec3f,
    end: Vec3f,
    tolerance: f64,
) -> Option<(f64, Vec3f)> {
    let segment = end.sub(start);
    let segment_length_sq = segment.norm_sq();
    if segment_length_sq <= tolerance * tolerance {
        return None;
    }

    let t = point.sub(start).dot(segment) / segment_length_sq;
    if t <= tolerance || t >= 1.0 - tolerance {
        return None;
    }

    let closest = start.add(segment.scale(t));
    if point.sub(closest).norm_sq() > tolerance * tolerance {
        return None;
    }

    Some((t, point))
}
