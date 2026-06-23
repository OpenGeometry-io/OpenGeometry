use std::cmp::Ordering;

use openmaths::Vector3;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

const BVH_LEAF_SIZE: usize = 8;
const RAY_EPSILON: f64 = 1.0e-12;

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Aabb3 {
    pub min: Vector3,
    pub max: Vector3,
}

impl Aabb3 {
    pub fn new(min: Vector3, max: Vector3) -> Result<Self, String> {
        validate_vector3(min, "AABB min")?;
        validate_vector3(max, "AABB max")?;

        if min.x > max.x || min.y > max.y || min.z > max.z {
            return Err(
                "Invalid AABB: min components must be less than or equal to max components."
                    .to_string(),
            );
        }

        Ok(Self { min, max })
    }

    pub fn from_coords(
        min_x: f64,
        min_y: f64,
        min_z: f64,
        max_x: f64,
        max_y: f64,
        max_z: f64,
    ) -> Result<Self, String> {
        Self::new(
            Vector3::new(min_x, min_y, min_z),
            Vector3::new(max_x, max_y, max_z),
        )
    }

    pub fn from_flat_vertices(vertices: &[f64]) -> Result<Option<Self>, String> {
        if vertices.is_empty() {
            return Ok(None);
        }

        if vertices.len() % 3 != 0 {
            return Err(
                "Invalid vertex buffer: flat vertex arrays must contain 3 values per vertex."
                    .to_string(),
            );
        }

        let mut min = Vector3::new(vertices[0], vertices[1], vertices[2]);
        let mut max = min;
        validate_vector3(min, "vertex")?;

        for vertex in vertices.chunks_exact(3).skip(1) {
            let point = Vector3::new(vertex[0], vertex[1], vertex[2]);
            validate_vector3(point, "vertex")?;
            min.x = min.x.min(point.x);
            min.y = min.y.min(point.y);
            min.z = min.z.min(point.z);
            max.x = max.x.max(point.x);
            max.y = max.y.max(point.y);
            max.z = max.z.max(point.z);
        }

        Ok(Some(Self { min, max }))
    }

    pub fn union(&self, other: &Self) -> Self {
        Self {
            min: Vector3::new(
                self.min.x.min(other.min.x),
                self.min.y.min(other.min.y),
                self.min.z.min(other.min.z),
            ),
            max: Vector3::new(
                self.max.x.max(other.max.x),
                self.max.y.max(other.max.y),
                self.max.z.max(other.max.z),
            ),
        }
    }

    pub fn center(&self) -> Vector3 {
        Vector3::new(
            (self.min.x + self.max.x) * 0.5,
            (self.min.y + self.max.y) * 0.5,
            (self.min.z + self.max.z) * 0.5,
        )
    }

    pub fn extent(&self) -> Vector3 {
        Vector3::new(
            self.max.x - self.min.x,
            self.max.y - self.min.y,
            self.max.z - self.min.z,
        )
    }

    pub fn intersects_aabb(&self, other: &Self) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
            && self.min.z <= other.max.z
            && self.max.z >= other.min.z
    }

    pub fn intersects_frustum(&self, frustum: &Frustum3) -> bool {
        frustum
            .planes
            .iter()
            .all(|plane| plane.distance_to_point(self.positive_vertex(plane.normal)) >= 0.0)
    }

    pub fn intersects_ray(&self, ray: &Ray3) -> Option<f64> {
        let mut t_min = 0.0;
        let mut t_max = ray.max_distance;

        update_ray_interval(
            ray.origin.x,
            ray.direction.x,
            self.min.x,
            self.max.x,
            &mut t_min,
            &mut t_max,
        )?;
        update_ray_interval(
            ray.origin.y,
            ray.direction.y,
            self.min.y,
            self.max.y,
            &mut t_min,
            &mut t_max,
        )?;
        update_ray_interval(
            ray.origin.z,
            ray.direction.z,
            self.min.z,
            self.max.z,
            &mut t_min,
            &mut t_max,
        )?;

        Some(t_min)
    }

    fn longest_axis(&self) -> Axis3 {
        let extent = self.extent();
        if extent.x >= extent.y && extent.x >= extent.z {
            Axis3::X
        } else if extent.y >= extent.z {
            Axis3::Y
        } else {
            Axis3::Z
        }
    }

    fn positive_vertex(&self, normal: Vector3) -> Vector3 {
        Vector3::new(
            if normal.x >= 0.0 {
                self.max.x
            } else {
                self.min.x
            },
            if normal.y >= 0.0 {
                self.max.y
            } else {
                self.min.y
            },
            if normal.z >= 0.0 {
                self.max.z
            } else {
                self.min.z
            },
        )
    }
}

#[derive(Clone, Copy)]
pub struct Plane3 {
    normal: Vector3,
    constant: f64,
}

impl Plane3 {
    pub fn new(normal: Vector3, constant: f64) -> Result<Self, String> {
        validate_vector3(normal, "frustum plane normal")?;
        if !constant.is_finite() {
            return Err("Invalid frustum plane: constant must be finite.".to_string());
        }

        let length_squared = normal.x * normal.x + normal.y * normal.y + normal.z * normal.z;
        if length_squared <= RAY_EPSILON {
            return Err("Invalid frustum plane: normal must be non-zero.".to_string());
        }

        Ok(Self { normal, constant })
    }

    fn distance_to_point(&self, point: Vector3) -> f64 {
        self.normal.x * point.x + self.normal.y * point.y + self.normal.z * point.z + self.constant
    }
}

pub struct Frustum3 {
    planes: Vec<Plane3>,
}

impl Frustum3 {
    pub fn new(planes: Vec<Plane3>) -> Result<Self, String> {
        if planes.is_empty() {
            return Err("Invalid frustum: at least one plane is required.".to_string());
        }

        Ok(Self { planes })
    }
}

#[derive(Clone, Copy)]
pub struct Ray3 {
    origin: Vector3,
    direction: Vector3,
    max_distance: f64,
}

impl Ray3 {
    pub fn new(origin: Vector3, direction: Vector3, max_distance: f64) -> Result<Self, String> {
        validate_vector3(origin, "ray origin")?;
        validate_vector3(direction, "ray direction")?;
        if !max_distance.is_finite() || max_distance < 0.0 {
            return Err(
                "Invalid ray: max distance must be a finite non-negative number.".to_string(),
            );
        }

        let max_component = direction
            .x
            .abs()
            .max(direction.y.abs())
            .max(direction.z.abs());
        if max_component == 0.0 {
            return Err("Invalid ray: direction must be non-zero.".to_string());
        }

        let scaled_direction = Vector3::new(
            direction.x / max_component,
            direction.y / max_component,
            direction.z / max_component,
        );
        let length_squared = scaled_direction.x * scaled_direction.x
            + scaled_direction.y * scaled_direction.y
            + scaled_direction.z * scaled_direction.z;
        let inv_length = 1.0 / length_squared.sqrt();
        Ok(Self {
            origin,
            direction: Vector3::new(
                scaled_direction.x * inv_length,
                scaled_direction.y * inv_length,
                scaled_direction.z * inv_length,
            ),
            max_distance,
        })
    }
}

#[derive(Clone, Copy)]
pub struct BvhPrimitive {
    pub id: u32,
    pub bounds: Aabb3,
}

impl BvhPrimitive {
    pub fn new(id: u32, bounds: Aabb3) -> Self {
        Self { id, bounds }
    }
}

#[derive(Clone, Copy, Serialize)]
pub struct RayHit {
    pub id: u32,
    pub distance: f64,
}

pub struct Bvh3 {
    primitives: Vec<BvhPrimitive>,
    indices: Vec<usize>,
    nodes: Vec<BvhNode>,
}

impl Bvh3 {
    pub fn build(primitives: Vec<BvhPrimitive>) -> Self {
        let mut indices = (0..primitives.len()).collect::<Vec<_>>();
        let mut nodes = Vec::new();

        if !indices.is_empty() {
            build_node(&mut indices, 0, &primitives, &mut nodes);
        }

        Self {
            primitives,
            indices,
            nodes,
        }
    }

    pub fn primitive_count(&self) -> usize {
        self.primitives.len()
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn query_aabb(&self, bounds: &Aabb3) -> Vec<u32> {
        self.query(|candidate| candidate.intersects_aabb(bounds))
    }

    pub fn query_frustum(&self, frustum: &Frustum3) -> Vec<u32> {
        self.query(|candidate| candidate.intersects_frustum(frustum))
    }

    pub fn raycast_first(&self, ray: &Ray3) -> Option<RayHit> {
        if self.nodes.is_empty() {
            return None;
        }

        let mut closest: Option<RayHit> = None;
        let mut stack = vec![0usize];

        while let Some(node_index) = stack.pop() {
            let node = &self.nodes[node_index];
            let Some(node_distance) = node.bounds.intersects_ray(ray) else {
                continue;
            };

            if closest.is_some_and(|hit| node_distance > hit.distance) {
                continue;
            }

            if node.is_leaf() {
                for primitive_index in &self.indices[node.start..node.end] {
                    let primitive = &self.primitives[*primitive_index];
                    let Some(distance) = primitive.bounds.intersects_ray(ray) else {
                        continue;
                    };

                    if closest.is_none_or(|hit| {
                        distance < hit.distance
                            || (distance == hit.distance && primitive.id < hit.id)
                    }) {
                        closest = Some(RayHit {
                            id: primitive.id,
                            distance,
                        });
                    }
                }
            } else {
                if let Some(left) = node.left {
                    stack.push(left);
                }
                if let Some(right) = node.right {
                    stack.push(right);
                }
            }
        }

        closest
    }

    fn query<F>(&self, intersects: F) -> Vec<u32>
    where
        F: Fn(&Aabb3) -> bool,
    {
        if self.nodes.is_empty() {
            return Vec::new();
        }

        let mut hits = Vec::new();
        let mut stack = vec![0usize];

        while let Some(node_index) = stack.pop() {
            let node = &self.nodes[node_index];
            if !intersects(&node.bounds) {
                continue;
            }

            if node.is_leaf() {
                for primitive_index in &self.indices[node.start..node.end] {
                    let primitive = &self.primitives[*primitive_index];
                    if intersects(&primitive.bounds) {
                        hits.push(primitive.id);
                    }
                }
            } else {
                if let Some(left) = node.left {
                    stack.push(left);
                }
                if let Some(right) = node.right {
                    stack.push(right);
                }
            }
        }

        hits
    }
}

#[wasm_bindgen]
pub struct OGSpatialIndex {
    bvh: Bvh3,
}

#[wasm_bindgen]
impl OGSpatialIndex {
    #[wasm_bindgen(constructor)]
    pub fn new(items_json: String) -> Result<OGSpatialIndex, JsValue> {
        let items: Vec<SpatialIndexItemPayload> =
            serde_json::from_str(&items_json).map_err(|err| {
                JsValue::from_str(&format!("Invalid spatial index items JSON: {}", err))
            })?;

        let primitives = items
            .into_iter()
            .map(|item| {
                Aabb3::from_coords(
                    item.min[0],
                    item.min[1],
                    item.min[2],
                    item.max[0],
                    item.max[1],
                    item.max[2],
                )
                .map(|bounds| BvhPrimitive::new(item.id, bounds))
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| JsValue::from_str(&err))?;

        Ok(Self {
            bvh: Bvh3::build(primitives),
        })
    }

    #[wasm_bindgen(js_name = fromFlatArrays)]
    pub fn from_flat_arrays(ids: Vec<u32>, bounds: Vec<f64>) -> Result<OGSpatialIndex, JsValue> {
        let primitives =
            primitives_from_flat_arrays(&ids, &bounds).map_err(|err| JsValue::from_str(&err))?;

        Ok(Self {
            bvh: Bvh3::build(primitives),
        })
    }

    #[wasm_bindgen(js_name = primitiveCount)]
    pub fn primitive_count(&self) -> usize {
        self.bvh.primitive_count()
    }

    #[wasm_bindgen(js_name = nodeCount)]
    pub fn node_count(&self) -> usize {
        self.bvh.node_count()
    }

    #[wasm_bindgen(js_name = queryAabb)]
    pub fn query_aabb(
        &self,
        min_x: f64,
        min_y: f64,
        min_z: f64,
        max_x: f64,
        max_y: f64,
        max_z: f64,
    ) -> Result<String, JsValue> {
        let bounds = Aabb3::from_coords(min_x, min_y, min_z, max_x, max_y, max_z)
            .map_err(|err| JsValue::from_str(&err))?;
        serialize_ids(self.bvh.query_aabb(&bounds))
    }

    #[wasm_bindgen(js_name = queryAabbIds)]
    pub fn query_aabb_ids(
        &self,
        min_x: f64,
        min_y: f64,
        min_z: f64,
        max_x: f64,
        max_y: f64,
        max_z: f64,
    ) -> Result<Vec<u32>, JsValue> {
        let bounds = Aabb3::from_coords(min_x, min_y, min_z, max_x, max_y, max_z)
            .map_err(|err| JsValue::from_str(&err))?;
        Ok(self.bvh.query_aabb(&bounds))
    }

    #[wasm_bindgen(js_name = queryFrustum)]
    pub fn query_frustum(&self, planes_json: String) -> Result<String, JsValue> {
        let frustum = parse_frustum_json(&planes_json).map_err(|err| JsValue::from_str(&err))?;
        serialize_ids(self.bvh.query_frustum(&frustum))
    }

    #[wasm_bindgen(js_name = queryFrustumIds)]
    pub fn query_frustum_ids(&self, planes_json: String) -> Result<Vec<u32>, JsValue> {
        let frustum = parse_frustum_json(&planes_json).map_err(|err| JsValue::from_str(&err))?;
        Ok(self.bvh.query_frustum(&frustum))
    }

    #[wasm_bindgen(js_name = raycastFirst)]
    pub fn raycast_first(
        &self,
        origin_x: f64,
        origin_y: f64,
        origin_z: f64,
        direction_x: f64,
        direction_y: f64,
        direction_z: f64,
        max_distance: f64,
    ) -> Result<String, JsValue> {
        let ray = Ray3::new(
            Vector3::new(origin_x, origin_y, origin_z),
            Vector3::new(direction_x, direction_y, direction_z),
            max_distance,
        )
        .map_err(|err| JsValue::from_str(&err))?;

        serde_json::to_string(&self.bvh.raycast_first(&ray))
            .map_err(|err| JsValue::from_str(&format!("Failed to serialize raycast hit: {}", err)))
    }
}

#[derive(Deserialize)]
struct SpatialIndexItemPayload {
    id: u32,
    min: [f64; 3],
    max: [f64; 3],
}

#[derive(Deserialize)]
struct PlanePayload {
    normal: [f64; 3],
    constant: f64,
}

#[derive(Clone, Copy)]
struct BvhNode {
    bounds: Aabb3,
    left: Option<usize>,
    right: Option<usize>,
    start: usize,
    end: usize,
}

impl BvhNode {
    fn is_leaf(&self) -> bool {
        self.left.is_none() && self.right.is_none()
    }
}

#[derive(Clone, Copy)]
enum Axis3 {
    X,
    Y,
    Z,
}

fn build_node(
    indices: &mut [usize],
    range_start: usize,
    primitives: &[BvhPrimitive],
    nodes: &mut Vec<BvhNode>,
) -> usize {
    let bounds = bounds_for_indices(indices, primitives);
    let node_index = nodes.len();
    nodes.push(BvhNode {
        bounds,
        left: None,
        right: None,
        start: range_start,
        end: range_start + indices.len(),
    });

    if indices.len() <= BVH_LEAF_SIZE {
        return node_index;
    }

    let axis = bounds.longest_axis();
    indices.sort_by(|lhs, rhs| {
        center_axis(primitives[*lhs].bounds.center(), axis)
            .partial_cmp(&center_axis(primitives[*rhs].bounds.center(), axis))
            .unwrap_or(Ordering::Equal)
    });

    let mid = indices.len() / 2;
    let (left_indices, right_indices) = indices.split_at_mut(mid);
    let left = build_node(left_indices, range_start, primitives, nodes);
    let right = build_node(right_indices, range_start + mid, primitives, nodes);

    nodes[node_index].left = Some(left);
    nodes[node_index].right = Some(right);
    node_index
}

fn bounds_for_indices(indices: &[usize], primitives: &[BvhPrimitive]) -> Aabb3 {
    let mut bounds = primitives[indices[0]].bounds;
    for index in &indices[1..] {
        bounds = bounds.union(&primitives[*index].bounds);
    }
    bounds
}

fn center_axis(center: Vector3, axis: Axis3) -> f64 {
    match axis {
        Axis3::X => center.x,
        Axis3::Y => center.y,
        Axis3::Z => center.z,
    }
}

fn parse_frustum_json(planes_json: &str) -> Result<Frustum3, String> {
    let planes: Vec<PlanePayload> = serde_json::from_str(planes_json)
        .map_err(|err| format!("Invalid frustum planes JSON: {}", err))?;

    let planes = planes
        .into_iter()
        .map(|plane| {
            Plane3::new(
                Vector3::new(plane.normal[0], plane.normal[1], plane.normal[2]),
                plane.constant,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    Frustum3::new(planes)
}

fn primitives_from_flat_arrays(ids: &[u32], bounds: &[f64]) -> Result<Vec<BvhPrimitive>, String> {
    if bounds.len() != ids.len() * 6 {
        return Err(format!(
            "Invalid spatial index arrays: expected {} bounds values for {} ids, got {}.",
            ids.len() * 6,
            ids.len(),
            bounds.len()
        ));
    }

    ids.iter()
        .enumerate()
        .map(|(index, id)| {
            let offset = index * 6;
            Aabb3::from_coords(
                bounds[offset],
                bounds[offset + 1],
                bounds[offset + 2],
                bounds[offset + 3],
                bounds[offset + 4],
                bounds[offset + 5],
            )
            .map(|bounds| BvhPrimitive::new(*id, bounds))
        })
        .collect()
}

fn serialize_ids(ids: Vec<u32>) -> Result<String, JsValue> {
    serde_json::to_string(&ids).map_err(|err| {
        JsValue::from_str(&format!("Failed to serialize spatial query ids: {}", err))
    })
}

fn update_ray_interval(
    origin: f64,
    direction: f64,
    min: f64,
    max: f64,
    t_min: &mut f64,
    t_max: &mut f64,
) -> Option<()> {
    if direction == 0.0 {
        return (origin >= min && origin <= max).then_some(());
    }

    let inv_direction = 1.0 / direction;
    let mut near = (min - origin) * inv_direction;
    let mut far = (max - origin) * inv_direction;

    if near > far {
        std::mem::swap(&mut near, &mut far);
    }

    *t_min = (*t_min).max(near);
    *t_max = (*t_max).min(far);

    (t_min <= t_max).then_some(())
}

fn validate_vector3(vector: Vector3, label: &str) -> Result<(), String> {
    if vector.x.is_finite() && vector.y.is_finite() && vector.z.is_finite() {
        Ok(())
    } else {
        Err(format!("Invalid {}: all components must be finite.", label))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn box_primitive(id: u32, min: (f64, f64, f64), max: (f64, f64, f64)) -> BvhPrimitive {
        BvhPrimitive::new(
            id,
            Aabb3::from_coords(min.0, min.1, min.2, max.0, max.1, max.2)
                .expect("valid test bounds"),
        )
    }

    fn sorted(mut ids: Vec<u32>) -> Vec<u32> {
        ids.sort_unstable();
        ids
    }

    fn test_frustum() -> Frustum3 {
        Frustum3::new(vec![
            Plane3::new(Vector3::new(1.0, 0.0, 0.0), 0.0).expect("left plane"),
            Plane3::new(Vector3::new(-1.0, 0.0, 0.0), 10.0).expect("right plane"),
            Plane3::new(Vector3::new(0.0, 1.0, 0.0), 0.0).expect("bottom plane"),
            Plane3::new(Vector3::new(0.0, -1.0, 0.0), 10.0).expect("top plane"),
            Plane3::new(Vector3::new(0.0, 0.0, 1.0), 0.0).expect("near plane"),
            Plane3::new(Vector3::new(0.0, 0.0, -1.0), 10.0).expect("far plane"),
        ])
        .expect("valid frustum")
    }

    #[test]
    fn flat_vertices_create_bounds_and_reject_invalid_buffers() {
        let bounds = Aabb3::from_flat_vertices(&[-1.0, 2.0, 3.0, 4.0, -5.0, 6.0, 0.5, 1.0, -7.0])
            .expect("valid vertex buffer")
            .expect("non-empty bounds");

        assert_eq!(bounds.min.x, -1.0);
        assert_eq!(bounds.min.y, -5.0);
        assert_eq!(bounds.min.z, -7.0);
        assert_eq!(bounds.max.x, 4.0);
        assert_eq!(bounds.max.y, 2.0);
        assert_eq!(bounds.max.z, 6.0);

        assert!(Aabb3::from_flat_vertices(&[])
            .expect("empty is valid")
            .is_none());
        assert!(Aabb3::from_flat_vertices(&[1.0, 2.0]).is_err());
    }

    #[test]
    fn bvh_queries_aabb_without_scanning_unrelated_bounds() {
        let bvh = Bvh3::build(vec![
            box_primitive(1, (-5.0, 0.0, 0.0), (-4.0, 1.0, 1.0)),
            box_primitive(2, (1.0, 1.0, 1.0), (2.0, 2.0, 2.0)),
            box_primitive(3, (3.0, 3.0, 3.0), (4.0, 4.0, 4.0)),
            box_primitive(4, (20.0, 0.0, 0.0), (21.0, 1.0, 1.0)),
        ]);

        assert_eq!(bvh.primitive_count(), 4);
        assert_eq!(bvh.node_count(), 1);

        let query = Aabb3::from_coords(0.0, 0.0, 0.0, 3.5, 3.5, 3.5).expect("valid query bounds");
        assert_eq!(sorted(bvh.query_aabb(&query)), vec![2, 3]);
    }

    #[test]
    fn bvh_builds_internal_nodes_for_large_sets_and_queries_frustum() {
        let primitives = (0..32)
            .map(|index| {
                let x = index as f64 * 2.0;
                box_primitive(index, (x, 0.0, 0.0), (x + 0.5, 0.5, 0.5))
            })
            .collect::<Vec<_>>();
        let bvh = Bvh3::build(primitives);

        assert_eq!(bvh.primitive_count(), 32);
        assert!(bvh.node_count() > 1);

        assert_eq!(
            sorted(bvh.query_frustum(&test_frustum())),
            vec![0, 1, 2, 3, 4, 5]
        );
    }

    #[test]
    fn raycast_first_returns_nearest_intersection() {
        let bvh = Bvh3::build(vec![
            box_primitive(10, (5.0, -1.0, -1.0), (6.0, 1.0, 1.0)),
            box_primitive(20, (2.0, -1.0, -1.0), (3.0, 1.0, 1.0)),
        ]);
        let ray = Ray3::new(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(10.0, 0.0, 0.0),
            100.0,
        )
        .expect("valid ray");

        let hit = bvh.raycast_first(&ray).expect("nearest hit");
        assert_eq!(hit.id, 20);
        assert_eq!(hit.distance, 2.0);
    }

    #[test]
    fn raycast_accepts_small_non_zero_direction_before_normalizing() {
        let bvh = Bvh3::build(vec![box_primitive(20, (2.0, -1.0, -1.0), (3.0, 1.0, 1.0))]);
        let ray = Ray3::new(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0e-7, 0.0, 0.0),
            100.0,
        )
        .expect("small non-zero direction should normalize");

        let hit = bvh.raycast_first(&ray).expect("hit");
        assert_eq!(hit.id, 20);
        assert_eq!(hit.distance, 2.0);
        assert!(Ray3::new(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 0.0),
            100.0,
        )
        .is_err());
    }

    #[test]
    fn raycast_does_not_treat_tiny_non_zero_axis_components_as_parallel() {
        let bvh = Bvh3::build(vec![box_primitive(
            20,
            (1.0, 0.0, -1.0),
            (2.0, 2.0e13, 1.0),
        )]);
        let ray = Ray3::new(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0e-13, 1.0, 0.0),
            2.0e13,
        )
        .expect("tiny non-zero axis component should remain directional");

        let hit = bvh
            .raycast_first(&ray)
            .expect("hit through tiny x component");
        assert_eq!(hit.id, 20);
        assert!(hit.distance > 9.0e12);
    }

    #[test]
    fn wasm_spatial_index_queries_aabb_frustum_and_ray() {
        let items = r#"[
            {"id": 1, "min": [-2, 0, 0], "max": [-1, 1, 1]},
            {"id": 2, "min": [2, 2, 2], "max": [3, 3, 3]},
            {"id": 3, "min": [6, 0, 0], "max": [7, 1, 1]}
        ]"#;
        let index = OGSpatialIndex::new(items.to_string()).expect("valid index");
        assert_eq!(index.primitive_count(), 3);

        let ids = index
            .query_aabb(0.0, 0.0, 0.0, 4.0, 4.0, 4.0)
            .expect("query ids");
        assert_eq!(ids, "[2]");

        let frustum_json = r#"[
            {"normal": [1, 0, 0], "constant": 0},
            {"normal": [-1, 0, 0], "constant": 10},
            {"normal": [0, 1, 0], "constant": 0},
            {"normal": [0, -1, 0], "constant": 10},
            {"normal": [0, 0, 1], "constant": 0},
            {"normal": [0, 0, -1], "constant": 10}
        ]"#;
        let mut frustum_ids = serde_json::from_str::<Vec<u32>>(
            &index
                .query_frustum(frustum_json.to_string())
                .expect("frustum ids"),
        )
        .expect("valid frustum ids");
        frustum_ids.sort_unstable();
        assert_eq!(frustum_ids, vec![2, 3]);

        let ray_hit = index
            .raycast_first(0.0, 2.5, 2.5, 1.0, 0.0, 0.0, 100.0)
            .expect("raycast hit");
        assert_eq!(ray_hit, r#"{"id":2,"distance":2.0}"#);
    }

    #[test]
    fn wasm_spatial_index_builds_from_flat_arrays_and_returns_ids() {
        let index = OGSpatialIndex::from_flat_arrays(
            vec![11, 22, 33],
            vec![
                -2.0, 0.0, 0.0, -1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 3.0, 3.0, 3.0, 6.0, 0.0, 0.0, 7.0,
                1.0, 1.0,
            ],
        )
        .expect("valid flat-array index");

        let ids = index
            .query_aabb_ids(0.0, 0.0, 0.0, 4.0, 4.0, 4.0)
            .expect("typed aabb query ids");
        assert_eq!(ids, vec![22]);

        assert!(primitives_from_flat_arrays(&[1], &[0.0, 0.0, 0.0]).is_err());
    }
}
