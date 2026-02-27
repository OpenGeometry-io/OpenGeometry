use std::collections::{HashMap, HashSet};

use openmaths::Vector3;
use serde::{Deserialize, Serialize};

use crate::brep::Brep;

const EPSILON: f64 = 1.0e-9;
const CREASE_COS_THRESHOLD: f64 = 0.9995;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum ProjectionMode {
    Orthographic,
    Perspective,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CameraParameters {
    pub position: Vector3,
    pub target: Vector3,
    pub up: Vector3,
    pub near: f64,
    pub projection_mode: ProjectionMode,
}

impl Default for CameraParameters {
    fn default() -> Self {
        Self {
            position: Vector3::new(3.0, 3.0, 3.0),
            target: Vector3::new(0.0, 0.0, 0.0),
            up: Vector3::new(0.0, 1.0, 0.0),
            near: 0.01,
            projection_mode: ProjectionMode::Orthographic,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HlrOptions {
    pub hide_hidden_edges: bool,
}

impl Default for HlrOptions {
    fn default() -> Self {
        Self {
            hide_hidden_edges: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

impl Vec2 {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Segment2D {
    Line { start: Vec2, end: Vec2 },
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Path2D {
    pub segments: Vec<Segment2D>,
    pub stroke_width: Option<f64>,
    pub stroke_color: Option<(f64, f64, f64)>,
}

impl Path2D {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_segments(segments: Vec<Segment2D>) -> Self {
        Self {
            segments,
            ..Self::default()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    pub fn push_segment(&mut self, segment: Segment2D) {
        self.segments.push(segment);
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Scene2D {
    pub name: Option<String>,
    pub paths: Vec<Path2D>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Line2D {
    pub start: Vec2,
    pub end: Vec2,
    pub stroke_width: Option<f64>,
    pub stroke_color: Option<(f64, f64, f64)>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Scene2DLines {
    pub name: Option<String>,
    pub lines: Vec<Line2D>,
}

impl Scene2D {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_name(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            ..Self::default()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.paths.iter().all(Path2D::is_empty)
    }

    pub fn add_path(&mut self, path: Path2D) {
        if !path.is_empty() {
            self.paths.push(path);
        }
    }

    pub fn paths(&self) -> &[Path2D] {
        &self.paths
    }

    pub fn extend(&mut self, other: Scene2D) {
        for path in other.paths {
            self.add_path(path);
        }
    }

    pub fn bounding_box(&self) -> Option<(Vec2, Vec2)> {
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        let mut has_data = false;

        for path in &self.paths {
            for segment in &path.segments {
                match segment {
                    Segment2D::Line { start, end } => {
                        min_x = min_x.min(start.x).min(end.x);
                        min_y = min_y.min(start.y).min(end.y);
                        max_x = max_x.max(start.x).max(end.x);
                        max_y = max_y.max(start.y).max(end.y);
                        has_data = true;
                    }
                }
            }
        }

        if has_data {
            Some((Vec2::new(min_x, min_y), Vec2::new(max_x, max_y)))
        } else {
            None
        }
    }

    pub fn to_lines(&self) -> Scene2DLines {
        let mut lines = Vec::new();

        for path in &self.paths {
            for segment in &path.segments {
                match segment {
                    Segment2D::Line { start, end } => lines.push(Line2D {
                        start: *start,
                        end: *end,
                        stroke_width: path.stroke_width,
                        stroke_color: path.stroke_color,
                    }),
                }
            }
        }

        Scene2DLines {
            name: self.name.clone(),
            lines,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct CameraFrame {
    position: [f64; 3],
    right: [f64; 3],
    up: [f64; 3],
    forward: [f64; 3],
    near: f64,
    mode: ProjectionMode,
}

#[derive(Clone, Copy, Debug)]
struct ViewPoint {
    x: f64,
    y: f64,
    z: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
struct EdgeKey {
    a: u32,
    b: u32,
}

impl EdgeKey {
    fn new(v1: u32, v2: u32) -> Option<Self> {
        if v1 == v2 {
            return None;
        }

        let (a, b) = if v1 < v2 { (v1, v2) } else { (v2, v1) };
        Some(Self { a, b })
    }
}

#[derive(Clone, Copy, Debug)]
struct FaceInfo {
    front_facing: bool,
    normal: [f64; 3],
}

pub fn project_brep_to_scene(brep: &Brep, camera: &CameraParameters, hlr: &HlrOptions) -> Scene2D {
    let mut scene = Scene2D::with_name(format!("BRep {}", brep.id));
    if brep.vertices.is_empty() {
        return scene;
    }

    let Some(frame) = build_camera_frame(camera) else {
        return scene;
    };

    let face_info = compute_face_info(brep, &frame);
    let adjacency = build_edge_adjacency(brep);
    let candidates = collect_candidate_edges(brep, &adjacency);

    let mut path = Path2D::new();
    for edge in candidates {
        if !is_edge_vertex_index_valid(edge, brep.vertices.len()) {
            continue;
        }

        if hlr.hide_hidden_edges && !is_edge_visible(edge, &adjacency, &face_info) {
            continue;
        }

        let start_world = &brep.vertices[edge.a as usize].position;
        let end_world = &brep.vertices[edge.b as usize].position;

        let start_view = world_to_view(start_world, &frame);
        let end_view = world_to_view(end_world, &frame);
        let Some((start_clipped, end_clipped)) =
            clip_segment_to_near_plane(start_view, end_view, frame.near)
        else {
            continue;
        };

        let Some(start_2d) = project_view_point(start_clipped, frame.mode) else {
            continue;
        };
        let Some(end_2d) = project_view_point(end_clipped, frame.mode) else {
            continue;
        };

        if is_zero_length_2d(start_2d, end_2d) {
            continue;
        }

        path.push_segment(Segment2D::Line {
            start: start_2d,
            end: end_2d,
        });
    }

    scene.add_path(path);
    scene
}

fn build_camera_frame(camera: &CameraParameters) -> Option<CameraFrame> {
    let position = vec3_to_arr(&camera.position);
    let target = vec3_to_arr(&camera.target);
    let up_hint = vec3_to_arr(&camera.up);
    let near = camera.near.max(EPSILON);

    let forward = normalize(sub(target, position))?;
    let right = normalize(cross(forward, up_hint))
        .or_else(|| normalize(cross(forward, [0.0, 1.0, 0.0])))
        .or_else(|| normalize(cross(forward, [1.0, 0.0, 0.0])))?;
    let up = normalize(cross(right, forward))?;

    Some(CameraFrame {
        position,
        right,
        up,
        forward,
        near,
        mode: camera.projection_mode,
    })
}

fn world_to_view(point: &Vector3, frame: &CameraFrame) -> ViewPoint {
    let relative = sub(vec3_to_arr(point), frame.position);
    ViewPoint {
        x: dot(relative, frame.right),
        y: dot(relative, frame.up),
        z: dot(relative, frame.forward),
    }
}

fn clip_segment_to_near_plane(
    mut start: ViewPoint,
    mut end: ViewPoint,
    near: f64,
) -> Option<(ViewPoint, ViewPoint)> {
    if start.z < near && end.z < near {
        return None;
    }

    if start.z < near {
        let denominator = end.z - start.z;
        if denominator.abs() < EPSILON {
            return None;
        }
        let t = (near - start.z) / denominator;
        start = interpolate(start, end, t);
        start.z = near;
    } else if end.z < near {
        let denominator = end.z - start.z;
        if denominator.abs() < EPSILON {
            return None;
        }
        let t = (near - start.z) / denominator;
        end = interpolate(start, end, t);
        end.z = near;
    }

    Some((start, end))
}

fn project_view_point(point: ViewPoint, mode: ProjectionMode) -> Option<Vec2> {
    match mode {
        ProjectionMode::Orthographic => Some(Vec2::new(point.x, point.y)),
        ProjectionMode::Perspective => {
            if point.z <= EPSILON {
                return None;
            }
            Some(Vec2::new(point.x / point.z, point.y / point.z))
        }
    }
}

fn build_edge_adjacency(brep: &Brep) -> HashMap<EdgeKey, Vec<usize>> {
    let mut adjacency: HashMap<EdgeKey, Vec<usize>> = HashMap::new();

    for (face_index, face) in brep.faces.iter().enumerate() {
        if face.face_indices.len() < 2 {
            continue;
        }

        let count = face.face_indices.len();
        for i in 0..count {
            let v1 = face.face_indices[i];
            let v2 = face.face_indices[(i + 1) % count];

            let Some(edge) = EdgeKey::new(v1, v2) else {
                continue;
            };

            let faces = adjacency.entry(edge).or_default();
            if !faces.contains(&face_index) {
                faces.push(face_index);
            }
        }
    }

    adjacency
}

fn collect_candidate_edges(brep: &Brep, adjacency: &HashMap<EdgeKey, Vec<usize>>) -> Vec<EdgeKey> {
    let mut keys: HashSet<EdgeKey> = HashSet::new();
    for key in adjacency.keys() {
        keys.insert(*key);
    }

    for edge in &brep.edges {
        if let Some(key) = EdgeKey::new(edge.v1, edge.v2) {
            keys.insert(key);
        }
    }

    for edge in &brep.hole_edges {
        if let Some(key) = EdgeKey::new(edge.v1, edge.v2) {
            keys.insert(key);
        }
    }

    let mut edges: Vec<EdgeKey> = keys.into_iter().collect();
    edges.sort_by_key(|key| (key.a, key.b));
    edges
}

fn compute_face_info(brep: &Brep, frame: &CameraFrame) -> Vec<FaceInfo> {
    let mut info = Vec::with_capacity(brep.faces.len());
    for face in &brep.faces {
        let Some((normal, center)) = compute_face_normal_and_center(brep, face) else {
            info.push(FaceInfo {
                front_facing: true,
                normal: [0.0, 0.0, 0.0],
            });
            continue;
        };

        let to_camera = sub(frame.position, center);
        let front_facing = dot(normal, to_camera) > 0.0;
        info.push(FaceInfo {
            front_facing,
            normal,
        });
    }
    info
}

fn compute_face_normal_and_center(
    brep: &Brep,
    face: &crate::brep::Face,
) -> Option<([f64; 3], [f64; 3])> {
    let mut points: Vec<[f64; 3]> = Vec::new();
    for vertex_id in &face.face_indices {
        let idx = *vertex_id as usize;
        if let Some(vertex) = brep.vertices.get(idx) {
            points.push(vec3_to_arr(&vertex.position));
        }
    }

    if points.len() < 3 {
        return None;
    }

    let mut center = [0.0, 0.0, 0.0];
    for point in &points {
        center = add(center, *point);
    }
    center = mul_scalar(center, 1.0 / points.len() as f64);

    let default_normal = normalize(vec3_to_arr(&face.normal));
    if let Some(normal) = default_normal {
        return Some((normal, center));
    }

    let p0 = points[0];
    for i in 1..(points.len() - 1) {
        let edge_a = sub(points[i], p0);
        let edge_b = sub(points[i + 1], p0);
        let normal_candidate = cross(edge_a, edge_b);
        if let Some(normal) = normalize(normal_candidate) {
            return Some((normal, center));
        }
    }

    None
}

fn is_edge_visible(
    edge: EdgeKey,
    adjacency: &HashMap<EdgeKey, Vec<usize>>,
    face_info: &[FaceInfo],
) -> bool {
    let Some(adjacent_faces) = adjacency.get(&edge) else {
        return true;
    };

    if adjacent_faces.is_empty() {
        return true;
    }

    let mut front_faces = Vec::new();
    for face_index in adjacent_faces {
        if let Some(face) = face_info.get(*face_index) {
            if face.front_facing {
                front_faces.push(*face);
            }
        }
    }

    if front_faces.is_empty() {
        return false;
    }

    if front_faces.len() < adjacent_faces.len() {
        return true;
    }

    if adjacent_faces.len() == 1 {
        return true;
    }

    has_crease(&front_faces)
}

fn has_crease(front_faces: &[FaceInfo]) -> bool {
    for i in 0..front_faces.len() {
        for j in (i + 1)..front_faces.len() {
            let dot_product = dot(front_faces[i].normal, front_faces[j].normal);
            if dot_product < CREASE_COS_THRESHOLD {
                return true;
            }
        }
    }

    false
}

fn is_edge_vertex_index_valid(edge: EdgeKey, vertex_count: usize) -> bool {
    (edge.a as usize) < vertex_count && (edge.b as usize) < vertex_count
}

fn is_zero_length_2d(start: Vec2, end: Vec2) -> bool {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    dx * dx + dy * dy <= EPSILON * EPSILON
}

fn interpolate(start: ViewPoint, end: ViewPoint, t: f64) -> ViewPoint {
    ViewPoint {
        x: start.x + (end.x - start.x) * t,
        y: start.y + (end.y - start.y) * t,
        z: start.z + (end.z - start.z) * t,
    }
}

fn vec3_to_arr(vec: &Vector3) -> [f64; 3] {
    [vec.x, vec.y, vec.z]
}

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn mul_scalar(v: [f64; 3], s: f64) -> [f64; 3] {
    [v[0] * s, v[1] * s, v[2] * s]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize(v: [f64; 3]) -> Option<[f64; 3]> {
    let len_sq = dot(v, v);
    if len_sq <= EPSILON * EPSILON {
        return None;
    }
    let inv = len_sq.sqrt().recip();
    Some([v[0] * inv, v[1] * inv, v[2] * inv])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brep::{Brep, Edge, Vertex};
    use uuid::Uuid;

    #[test]
    fn test_clip_segment_against_near_plane() {
        let start = ViewPoint {
            x: 0.0,
            y: 0.0,
            z: 0.5,
        };
        let end = ViewPoint {
            x: 1.0,
            y: 0.0,
            z: 2.0,
        };
        let clipped = clip_segment_to_near_plane(start, end, 1.0).unwrap();

        assert!((clipped.0.z - 1.0).abs() < 1.0e-9);
        assert!((clipped.1.z - 2.0).abs() < 1.0e-9);
    }

    #[test]
    fn test_project_perspective() {
        let point = ViewPoint {
            x: 2.0,
            y: 1.0,
            z: 4.0,
        };
        let projected = project_view_point(point, ProjectionMode::Perspective).unwrap();
        assert!((projected.x - 0.5).abs() < 1.0e-9);
        assert!((projected.y - 0.25).abs() < 1.0e-9);
    }

    #[test]
    fn test_project_edge_only_brep() {
        let mut brep = Brep::new(Uuid::new_v4());
        brep.vertices
            .push(Vertex::new(0, Vector3::new(-1.0, 0.0, 0.0)));
        brep.vertices
            .push(Vertex::new(1, Vector3::new(1.0, 0.0, 0.0)));
        brep.edges.push(Edge::new(0, 0, 1));

        let camera = CameraParameters {
            position: Vector3::new(0.0, 0.0, 5.0),
            target: Vector3::new(0.0, 0.0, 0.0),
            up: Vector3::new(0.0, 1.0, 0.0),
            near: 0.01,
            projection_mode: ProjectionMode::Orthographic,
        };

        let scene = project_brep_to_scene(&brep, &camera, &HlrOptions::default());
        assert!(!scene.is_empty());
        assert_eq!(scene.paths().len(), 1);
        assert_eq!(scene.paths()[0].segments.len(), 1);

        let line_scene = scene.to_lines();
        assert_eq!(line_scene.lines.len(), 1);
    }
}
