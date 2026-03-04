use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BooleanOperation {
    Union,
    Intersection,
    Difference,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BooleanConstraints {
    #[serde(default = "default_epsilon")]
    pub epsilon: f64,
    #[serde(default = "default_snap")]
    pub snap: f64,
}

fn default_epsilon() -> f64 {
    1e-6
}

fn default_snap() -> f64 {
    1e-6
}

impl Default for BooleanConstraints {
    fn default() -> Self {
        Self {
            epsilon: default_epsilon(),
            snap: default_snap(),
        }
    }
}

#[wasm_bindgen]
pub struct OGBoolean {
    last_result: Vec<Polygon>,
    last_constraints: BooleanConstraints,
}

#[wasm_bindgen]
impl OGBoolean {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            last_result: Vec::new(),
            last_constraints: BooleanConstraints::default(),
        }
    }

    #[wasm_bindgen]
    pub fn compute(
        &mut self,
        mesh_a_serialized: String,
        mesh_b_serialized: String,
        operation: String,
        constraints_serialized: Option<String>,
    ) -> Result<String, JsValue> {
        let vertices_a: Vec<f64> = serde_json::from_str(&mesh_a_serialized)
            .map_err(|err| JsValue::from_str(&err.to_string()))?;
        let vertices_b: Vec<f64> = serde_json::from_str(&mesh_b_serialized)
            .map_err(|err| JsValue::from_str(&err.to_string()))?;

        let constraints = constraints_serialized
            .as_deref()
            .map(serde_json::from_str)
            .transpose()
            .map_err(|err| JsValue::from_str(&err.to_string()))?
            .unwrap_or_default();

        let op: BooleanOperation =
            serde_json::from_str(&format!("\"{}\"", operation.to_lowercase())).map_err(|_| {
                JsValue::from_str("operation must be union, intersection, or difference")
            })?;

        let polygons_a = triangles_to_polygons(&vertices_a, &constraints)?;
        let polygons_b = triangles_to_polygons(&vertices_b, &constraints)?;

        let mut result = match op {
            BooleanOperation::Union => csg_union(polygons_a, polygons_b, constraints.epsilon),
            BooleanOperation::Intersection => {
                csg_intersection(polygons_a, polygons_b, constraints.epsilon)
            }
            BooleanOperation::Difference => {
                csg_subtract(polygons_a, polygons_b, constraints.epsilon)
            }
        };

        weld_vertices(&mut result, constraints.epsilon);
        self.last_constraints = constraints;
        self.last_result = result;

        let flattened = polygons_to_triangle_buffer(&self.last_result);
        serde_json::to_string(&flattened).map_err(|err| JsValue::from_str(&err.to_string()))
    }

    #[wasm_bindgen]
    pub fn get_outline_geometry_serialized(&self) -> Result<String, JsValue> {
        let outline = polygons_to_outline_buffer(&self.last_result, &self.last_constraints);
        serde_json::to_string(&outline).map_err(|err| JsValue::from_str(&err.to_string()))
    }
}

#[derive(Debug, Clone, Copy)]
struct Vec3 {
    x: f64,
    y: f64,
    z: f64,
}

impl Vec3 {
    fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    fn plus(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }

    fn minus(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }

    fn times(self, t: f64) -> Self {
        Self::new(self.x * t, self.y * t, self.z * t)
    }

    fn dot(self, rhs: Self) -> f64 {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z
    }

    fn cross(self, rhs: Self) -> Self {
        Self::new(
            self.y * rhs.z - self.z * rhs.y,
            self.z * rhs.x - self.x * rhs.z,
            self.x * rhs.y - self.y * rhs.x,
        )
    }

    fn length(self) -> f64 {
        self.dot(self).sqrt()
    }

    fn normalize(self) -> Self {
        let len = self.length();
        if len == 0.0 {
            self
        } else {
            self.times(1.0 / len)
        }
    }
}

#[derive(Debug, Clone)]
struct Vertex {
    pos: Vec3,
}

impl Vertex {
    fn interpolate(&self, other: &Vertex, t: f64) -> Vertex {
        Vertex {
            pos: self.pos.plus(other.pos.minus(self.pos).times(t)),
        }
    }

    fn flip(&mut self) {}
}

#[derive(Debug, Clone)]
struct Plane {
    normal: Vec3,
    w: f64,
    epsilon: f64,
}

impl Plane {
    fn from_points(a: Vec3, b: Vec3, c: Vec3, epsilon: f64) -> Self {
        let normal = b.minus(a).cross(c.minus(a)).normalize();
        Self {
            normal,
            w: normal.dot(a),
            epsilon,
        }
    }

    fn flip(&mut self) {
        self.normal = self.normal.times(-1.0);
        self.w = -self.w;
    }

    fn split_polygon(
        &self,
        polygon: &Polygon,
        coplanar_front: &mut Vec<Polygon>,
        coplanar_back: &mut Vec<Polygon>,
        front: &mut Vec<Polygon>,
        back: &mut Vec<Polygon>,
    ) {
        const COPLANAR: i32 = 0;
        const FRONT: i32 = 1;
        const BACK: i32 = 2;
        const SPANNING: i32 = 3;

        let mut polygon_type = 0;
        let mut types = Vec::with_capacity(polygon.vertices.len());
        for vertex in &polygon.vertices {
            let t = self.normal.dot(vertex.pos) - self.w;
            let vertex_type = if t < -self.epsilon {
                BACK
            } else if t > self.epsilon {
                FRONT
            } else {
                COPLANAR
            };
            polygon_type |= vertex_type;
            types.push(vertex_type);
        }

        match polygon_type {
            COPLANAR => {
                if self.normal.dot(polygon.plane.normal) > 0.0 {
                    coplanar_front.push(polygon.clone());
                } else {
                    coplanar_back.push(polygon.clone());
                }
            }
            FRONT => front.push(polygon.clone()),
            BACK => back.push(polygon.clone()),
            SPANNING => {
                let mut f: Vec<Vertex> = Vec::new();
                let mut b: Vec<Vertex> = Vec::new();
                for i in 0..polygon.vertices.len() {
                    let j = (i + 1) % polygon.vertices.len();
                    let ti = types[i];
                    let tj = types[j];
                    let vi = &polygon.vertices[i];
                    let vj = &polygon.vertices[j];

                    if ti != BACK {
                        f.push(vi.clone());
                    }
                    if ti != FRONT {
                        b.push(vi.clone());
                    }
                    if (ti | tj) == SPANNING {
                        let t = (self.w - self.normal.dot(vi.pos))
                            / self.normal.dot(vj.pos.minus(vi.pos));
                        let v = vi.interpolate(vj, t);
                        f.push(v.clone());
                        b.push(v);
                    }
                }

                if f.len() >= 3 {
                    front.push(Polygon::new(f, self.epsilon));
                }
                if b.len() >= 3 {
                    back.push(Polygon::new(b, self.epsilon));
                }
            }
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone)]
struct Polygon {
    vertices: Vec<Vertex>,
    plane: Plane,
}

impl Polygon {
    fn new(vertices: Vec<Vertex>, epsilon: f64) -> Self {
        let plane = Plane::from_points(vertices[0].pos, vertices[1].pos, vertices[2].pos, epsilon);
        Self { vertices, plane }
    }

    fn flip(&mut self) {
        self.vertices.reverse();
        for v in &mut self.vertices {
            v.flip();
        }
        self.plane.flip();
    }
}

#[derive(Debug, Clone)]
struct Node {
    plane: Option<Plane>,
    front: Option<Box<Node>>,
    back: Option<Box<Node>>,
    polygons: Vec<Polygon>,
}

impl Node {
    fn new(polygons: Vec<Polygon>) -> Self {
        let mut node = Self {
            plane: None,
            front: None,
            back: None,
            polygons: Vec::new(),
        };
        node.build(polygons);
        node
    }

    fn all_polygons(&self) -> Vec<Polygon> {
        let mut polygons = self.polygons.clone();
        if let Some(front) = &self.front {
            polygons.extend(front.all_polygons());
        }
        if let Some(back) = &self.back {
            polygons.extend(back.all_polygons());
        }
        polygons
    }

    fn invert(&mut self) {
        for polygon in &mut self.polygons {
            polygon.flip();
        }
        if let Some(plane) = &mut self.plane {
            plane.flip();
        }
        if let Some(front) = &mut self.front {
            front.invert();
        }
        if let Some(back) = &mut self.back {
            back.invert();
        }
        std::mem::swap(&mut self.front, &mut self.back);
    }

    fn clip_polygons(&self, polygons: Vec<Polygon>) -> Vec<Polygon> {
        let Some(plane) = &self.plane else {
            return polygons;
        };

        let mut front = Vec::new();
        let mut back = Vec::new();
        let mut coplanar_front = Vec::new();
        let mut coplanar_back = Vec::new();

        for polygon in polygons {
            plane.split_polygon(
                &polygon,
                &mut coplanar_front,
                &mut coplanar_back,
                &mut front,
                &mut back,
            );
        }

        front.extend(coplanar_front);
        back.extend(coplanar_back);

        if let Some(node) = &self.front {
            front = node.clip_polygons(front);
        }
        if let Some(node) = &self.back {
            back = node.clip_polygons(back);
        } else {
            back.clear();
        }

        front.extend(back);
        front
    }

    fn clip_to(&mut self, bsp: &Node) {
        self.polygons = bsp.clip_polygons(self.polygons.clone());
        if let Some(front) = &mut self.front {
            front.clip_to(bsp);
        }
        if let Some(back) = &mut self.back {
            back.clip_to(bsp);
        }
    }

    fn build(&mut self, polygons: Vec<Polygon>) {
        if polygons.is_empty() {
            return;
        }

        if self.plane.is_none() {
            self.plane = Some(polygons[0].plane.clone());
        }

        let mut front = Vec::new();
        let mut back = Vec::new();
        let mut coplanar_front = Vec::new();
        let mut coplanar_back = Vec::new();

        if let Some(plane) = &self.plane {
            for polygon in polygons {
                plane.split_polygon(
                    &polygon,
                    &mut coplanar_front,
                    &mut coplanar_back,
                    &mut front,
                    &mut back,
                );
            }
        }

        self.polygons.extend(coplanar_front);
        self.polygons.extend(coplanar_back);

        if !front.is_empty() {
            if self.front.is_none() {
                self.front = Some(Box::new(Node::new(Vec::new())));
            }
            if let Some(node) = &mut self.front {
                node.build(front);
            }
        }

        if !back.is_empty() {
            if self.back.is_none() {
                self.back = Some(Box::new(Node::new(Vec::new())));
            }
            if let Some(node) = &mut self.back {
                node.build(back);
            }
        }
    }
}

fn csg_union(a: Vec<Polygon>, b: Vec<Polygon>, _epsilon: f64) -> Vec<Polygon> {
    let mut a_node = Node::new(a);
    let mut b_node = Node::new(b);

    a_node.clip_to(&b_node);
    b_node.clip_to(&a_node);
    b_node.invert();
    b_node.clip_to(&a_node);
    b_node.invert();

    a_node.build(b_node.all_polygons());
    a_node.all_polygons()
}

fn csg_subtract(a: Vec<Polygon>, b: Vec<Polygon>, _epsilon: f64) -> Vec<Polygon> {
    let mut a_node = Node::new(a);
    let mut b_node = Node::new(b);

    a_node.invert();
    a_node.clip_to(&b_node);
    b_node.clip_to(&a_node);
    b_node.invert();
    b_node.clip_to(&a_node);
    b_node.invert();
    a_node.build(b_node.all_polygons());
    a_node.invert();

    a_node.all_polygons()
}

fn csg_intersection(a: Vec<Polygon>, b: Vec<Polygon>, _epsilon: f64) -> Vec<Polygon> {
    let mut a_node = Node::new(a);
    let mut b_node = Node::new(b);

    a_node.invert();
    b_node.clip_to(&a_node);
    b_node.invert();
    a_node.clip_to(&b_node);
    b_node.clip_to(&a_node);
    a_node.build(b_node.all_polygons());
    a_node.invert();

    a_node.all_polygons()
}

fn triangles_to_polygons(
    vertices: &[f64],
    constraints: &BooleanConstraints,
) -> Result<Vec<Polygon>, JsValue> {
    if vertices.len() % 9 != 0 {
        return Err(JsValue::from_str(
            "triangle buffer must be flat xyz triples whose length is divisible by 9",
        ));
    }

    let polygons = vertices
        .chunks_exact(9)
        .map(|chunk| {
            let points = [
                snap_vec3(Vec3::new(chunk[0], chunk[1], chunk[2]), constraints.snap),
                snap_vec3(Vec3::new(chunk[3], chunk[4], chunk[5]), constraints.snap),
                snap_vec3(Vec3::new(chunk[6], chunk[7], chunk[8]), constraints.snap),
            ];
            Polygon::new(
                points.into_iter().map(|pos| Vertex { pos }).collect(),
                constraints.epsilon,
            )
        })
        .collect();

    Ok(polygons)
}

fn snap_vec3(v: Vec3, snap: f64) -> Vec3 {
    if snap <= 0.0 {
        return v;
    }

    Vec3::new(
        (v.x / snap).round() * snap,
        (v.y / snap).round() * snap,
        (v.z / snap).round() * snap,
    )
}

fn weld_vertices(polygons: &mut [Polygon], epsilon: f64) {
    if epsilon <= 0.0 {
        return;
    }

    for polygon in polygons {
        for vertex in &mut polygon.vertices {
            vertex.pos = snap_vec3(vertex.pos, epsilon);
        }
    }
}

fn polygons_to_triangle_buffer(polygons: &[Polygon]) -> Vec<f64> {
    let mut out = Vec::new();
    for polygon in polygons {
        if polygon.vertices.len() < 3 {
            continue;
        }

        let base = polygon.vertices[0].pos;
        for i in 1..(polygon.vertices.len() - 1) {
            let b = polygon.vertices[i].pos;
            let c = polygon.vertices[i + 1].pos;
            out.extend_from_slice(&[base.x, base.y, base.z, b.x, b.y, b.z, c.x, c.y, c.z]);
        }
    }

    out
}

fn polygons_to_outline_buffer(polygons: &[Polygon], constraints: &BooleanConstraints) -> Vec<f64> {
    let mut edges: HashMap<EdgeKey, Vec<EdgeSample>> = HashMap::new();

    for polygon in polygons {
        if polygon.vertices.len() < 2 {
            continue;
        }

        for i in 0..polygon.vertices.len() {
            let start = polygon.vertices[i].pos;
            let end = polygon.vertices[(i + 1) % polygon.vertices.len()].pos;
            let key = EdgeKey::from_points(start, end, constraints);
            let entry = edges.entry(key).or_default();
            entry.push(EdgeSample {
                start,
                end,
                normal: polygon.plane.normal,
            });
        }
    }

    let mut out = Vec::new();
    for samples in edges.values() {
        if should_emit_edge(samples, constraints) {
            let representative = samples[0];
            out.extend_from_slice(&[
                representative.start.x,
                representative.start.y,
                representative.start.z,
                representative.end.x,
                representative.end.y,
                representative.end.z,
            ]);
        }
    }

    out
}

fn should_emit_edge(samples: &[EdgeSample], constraints: &BooleanConstraints) -> bool {
    if samples.len() <= 1 {
        return true;
    }

    let feature_angle_degrees = 28.0_f64;
    let feature_dot = (feature_angle_degrees.to_radians()).cos();

    for i in 0..samples.len() {
        for j in (i + 1)..samples.len() {
            let dot = samples[i].normal.dot(samples[j].normal).abs();
            if dot < feature_dot {
                return true;
            }
        }
    }

    // If all normals are close, it is a smooth/coplanar tessellation edge and should be hidden.
    let _ = constraints;
    false
}

#[derive(Debug, Clone, Copy)]
struct EdgeSample {
    start: Vec3,
    end: Vec3,
    normal: Vec3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct EdgeKey {
    a: QuantizedPoint,
    b: QuantizedPoint,
}

impl EdgeKey {
    fn from_points(a: Vec3, b: Vec3, constraints: &BooleanConstraints) -> Self {
        let qa = QuantizedPoint::from_vec3(a, constraints);
        let qb = QuantizedPoint::from_vec3(b, constraints);

        if qa <= qb {
            Self { a: qa, b: qb }
        } else {
            Self { a: qb, b: qa }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct QuantizedPoint {
    x: i64,
    y: i64,
    z: i64,
}

impl QuantizedPoint {
    fn from_vec3(v: Vec3, constraints: &BooleanConstraints) -> Self {
        let tol = constraints.snap.max(constraints.epsilon).max(1e-5);
        let scale = 1.0 / tol;
        Self {
            x: (v.x * scale).round() as i64,
            y: (v.y * scale).round() as i64,
            z: (v.z * scale).round() as i64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn union_of_overlapping_cubes_returns_triangles() {
        let cube_a = vec![
            -1.0, -1.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, -1.0, -1.0, 1.0, 1.0, -1.0,
            -1.0, 1.0, -1.0,
        ];
        let cube_b = vec![
            0.0, -1.0, -1.0, 2.0, -1.0, -1.0, 2.0, 1.0, -1.0, 0.0, -1.0, -1.0, 2.0, 1.0, -1.0, 0.0,
            1.0, -1.0,
        ];

        let constraints = BooleanConstraints::default();
        let a = triangles_to_polygons(&cube_a, &constraints).unwrap();
        let b = triangles_to_polygons(&cube_b, &constraints).unwrap();

        let triangles = polygons_to_triangle_buffer(&csg_union(a, b, constraints.epsilon));
        assert!(!triangles.is_empty());
        assert_eq!(triangles.len() % 9, 0);
    }

    #[test]
    fn coplanar_shared_triangle_edge_is_removed_from_outline() {
        let poly_a = Polygon::new(
            vec![
                Vertex {
                    pos: Vec3::new(0.0, 0.0, 0.0),
                },
                Vertex {
                    pos: Vec3::new(1.0, 0.0, 0.0),
                },
                Vertex {
                    pos: Vec3::new(1.0, 0.0, 1.0),
                },
            ],
            1e-6,
        );
        let poly_b = Polygon::new(
            vec![
                Vertex {
                    pos: Vec3::new(0.0, 0.0, 0.0),
                },
                Vertex {
                    pos: Vec3::new(1.0, 0.0, 1.0),
                },
                Vertex {
                    pos: Vec3::new(0.0, 0.0, 1.0),
                },
            ],
            1e-6,
        );

        let outline = polygons_to_outline_buffer(&[poly_a, poly_b], &BooleanConstraints::default());
        // rectangle perimeter only: 4 line segments * 6 coordinates
        assert_eq!(outline.len(), 24);
    }
    #[test]
    fn outline_is_generated_for_polygon_result() {
        let polygon = Polygon::new(
            vec![
                Vertex {
                    pos: Vec3::new(0.0, 0.0, 0.0),
                },
                Vertex {
                    pos: Vec3::new(1.0, 0.0, 0.0),
                },
                Vertex {
                    pos: Vec3::new(1.0, 0.0, 1.0),
                },
                Vertex {
                    pos: Vec3::new(0.0, 0.0, 1.0),
                },
            ],
            1e-6,
        );

        let outline = polygons_to_outline_buffer(&[polygon], &BooleanConstraints::default());
        assert_eq!(outline.len(), 24);
    }
}
