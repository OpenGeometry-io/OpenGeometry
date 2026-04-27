use std::collections::HashMap;

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

/// ISO 128 edge classification. Drives line weight and line type in export.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeClass {
    /// Silhouette: front-face edge adjacent to a back-facing face. Continuous 0.50 mm.
    VisibleOutline,
    /// Hard crease: two front-facing faces whose normals diverge (cos < 0.9995). Continuous 0.25 mm.
    VisibleCrease,
    /// Smooth interior edge between co-planar front faces. Hidden by default (opt-in).
    VisibleSmooth,
    /// Occluded by front-facing geometry. Dashed 0.18 mm when shown.
    Hidden,
    /// Intersects a section plane. Chain thick 0.70 mm.
    SectionCut,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Segment2D {
    Line {
        start: Vec2,
        end: Vec2,
    },
    Arc {
        center: Vec2,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
    },
    Ellipse {
        center: Vec2,
        rx: f64,
        ry: f64,
        rotation: f64,
        start_angle: f64,
        end_angle: f64,
    },
    CubicBezier {
        p0: Vec2,
        p1: Vec2,
        p2: Vec2,
        p3: Vec2,
    },
}

/// One classified output segment from the HLR projection.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClassifiedSegment {
    pub geometry: Segment2D,
    pub class: EdgeClass,
    /// AIA/NCS layer code (e.g. "A-WALL"). Populated by OGEntityRegistry in Phase 2.
    pub layer: Option<String>,
    /// BRep UUID of the originating entity.
    pub source_entity_id: Option<String>,
}

/// Backward-compat path type. Retained for callers that rely on Path2D structure.
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
    pub segments: Vec<ClassifiedSegment>,
}

/// Flat line representation used by the existing WASM `projectTo2DLines` API.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Line2D {
    pub start: Vec2,
    pub end: Vec2,
    pub stroke_width: Option<f64>,
    pub stroke_color: Option<(f64, f64, f64)>,
    /// ISO 128 edge class serialised as a string (e.g. "VisibleOutline").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
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
        self.segments.is_empty()
    }

    pub fn add_segment(&mut self, seg: ClassifiedSegment) {
        self.segments.push(seg);
    }

    /// Backward-compat: convert Path2D into classified segments (class = VisibleCrease).
    pub fn add_path(&mut self, path: Path2D) {
        for geom in path.segments {
            self.segments.push(ClassifiedSegment {
                geometry: geom,
                class: EdgeClass::VisibleCrease,
                layer: None,
                source_entity_id: None,
            });
        }
    }

    pub fn segments(&self) -> &[ClassifiedSegment] {
        &self.segments
    }

    /// Backward-compat: materialise a Vec<Path2D> from classified segments.
    pub fn paths(&self) -> Vec<Path2D> {
        self.segments
            .iter()
            .map(|seg| Path2D::with_segments(vec![seg.geometry.clone()]))
            .collect()
    }

    pub fn extend(&mut self, other: Scene2D) {
        self.segments.extend(other.segments);
    }

    pub fn bounding_box(&self) -> Option<(Vec2, Vec2)> {
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        let mut has_data = false;

        for seg in &self.segments {
            let Some((lo, hi)) = segment_bounds(&seg.geometry) else {
                continue;
            };
            min_x = min_x.min(lo.x);
            min_y = min_y.min(lo.y);
            max_x = max_x.max(hi.x);
            max_y = max_y.max(hi.y);
            has_data = true;
        }

        if has_data {
            Some((Vec2::new(min_x, min_y), Vec2::new(max_x, max_y)))
        } else {
            None
        }
    }

    /// Flatten to Line2D for the existing `projectTo2DLines` WASM path.
    pub fn to_lines(&self) -> Scene2DLines {
        let mut lines = Vec::new();
        for seg in &self.segments {
            if let Segment2D::Line { start, end } = seg.geometry {
                lines.push(Line2D {
                    start,
                    end,
                    stroke_width: None,
                    stroke_color: None,
                    class: Some(edge_class_str(seg.class).to_string()),
                });
            }
        }
        Scene2DLines {
            name: self.name.clone(),
            lines,
        }
    }
}

fn edge_class_str(class: EdgeClass) -> &'static str {
    match class {
        EdgeClass::VisibleOutline => "VisibleOutline",
        EdgeClass::VisibleCrease => "VisibleCrease",
        EdgeClass::VisibleSmooth => "VisibleSmooth",
        EdgeClass::Hidden => "Hidden",
        EdgeClass::SectionCut => "SectionCut",
    }
}

fn segment_bounds(seg: &Segment2D) -> Option<(Vec2, Vec2)> {
    match seg {
        Segment2D::Line { start, end } => Some((
            Vec2::new(start.x.min(end.x), start.y.min(end.y)),
            Vec2::new(start.x.max(end.x), start.y.max(end.y)),
        )),
        Segment2D::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            let pts = [
                Vec2::new(
                    center.x + radius * start_angle.cos(),
                    center.y + radius * start_angle.sin(),
                ),
                Vec2::new(
                    center.x + radius * end_angle.cos(),
                    center.y + radius * end_angle.sin(),
                ),
            ];
            points_bounds(&pts)
        }
        Segment2D::Ellipse {
            center,
            rx,
            ry,
            rotation,
            start_angle,
            end_angle,
        } => {
            let cos_r = rotation.cos();
            let sin_r = rotation.sin();
            let ellipse_pt = |a: f64| {
                let ex = rx * a.cos();
                let ey = ry * a.sin();
                Vec2::new(
                    center.x + ex * cos_r - ey * sin_r,
                    center.y + ex * sin_r + ey * cos_r,
                )
            };
            let pts = [ellipse_pt(*start_angle), ellipse_pt(*end_angle)];
            points_bounds(&pts)
        }
        Segment2D::CubicBezier { p0, p1, p2, p3 } => points_bounds(&[*p0, *p1, *p2, *p3]),
    }
}

fn points_bounds(pts: &[Vec2]) -> Option<(Vec2, Vec2)> {
    if pts.is_empty() {
        return None;
    }
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for p in pts {
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
    }
    Some((Vec2::new(min_x, min_y), Vec2::new(max_x, max_y)))
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
struct EdgeCandidate {
    id: u32,
    a: u32,
    b: u32,
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
    let candidates = collect_candidate_edges(brep);
    let source_id = brep.id.to_string();

    for edge in candidates {
        if !is_edge_vertex_index_valid(edge.a, edge.b, brep.vertices.len()) {
            continue;
        }

        let class = classify_edge(edge.id, &adjacency, &face_info);
        let should_emit = match class {
            EdgeClass::VisibleSmooth => false,
            EdgeClass::Hidden => !hlr.hide_hidden_edges,
            _ => true,
        };
        if !should_emit {
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

        scene.add_segment(ClassifiedSegment {
            geometry: Segment2D::Line {
                start: start_2d,
                end: end_2d,
            },
            class,
            layer: None,
            source_entity_id: Some(source_id.clone()),
        });
    }

    scene
}

fn classify_edge(
    edge_id: u32,
    adjacency: &HashMap<u32, Vec<usize>>,
    face_info: &[FaceInfo],
) -> EdgeClass {
    let adjacent_faces = adjacency.get(&edge_id).cloned().unwrap_or_default();

    if adjacent_faces.is_empty() {
        return EdgeClass::VisibleCrease;
    }

    let front_faces: Vec<FaceInfo> = adjacent_faces
        .iter()
        .filter_map(|&fi| face_info.get(fi).copied())
        .filter(|f| f.front_facing)
        .collect();

    if front_faces.is_empty() {
        return EdgeClass::Hidden;
    }

    if front_faces.len() < adjacent_faces.len() {
        return EdgeClass::VisibleOutline;
    }

    if has_crease(&front_faces) {
        EdgeClass::VisibleCrease
    } else {
        EdgeClass::VisibleSmooth
    }
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

fn build_edge_adjacency(brep: &Brep) -> HashMap<u32, Vec<usize>> {
    let face_index_by_id: HashMap<u32, usize> = brep
        .faces
        .iter()
        .enumerate()
        .map(|(index, face)| (face.id, index))
        .collect();

    let mut adjacency: HashMap<u32, Vec<usize>> = HashMap::new();

    for edge in &brep.edges {
        let mut adjacent_faces: Vec<usize> = Vec::new();

        let mut candidate_halfedges = vec![edge.halfedge];
        if let Some(twin_halfedge) = edge.twin_halfedge {
            candidate_halfedges.push(twin_halfedge);
        }

        for halfedge_id in candidate_halfedges {
            let Some(halfedge) = brep.halfedges.get(halfedge_id as usize) else {
                continue;
            };

            if let Some(face_id) = halfedge.face {
                if let Some(face_index) = face_index_by_id.get(&face_id) {
                    adjacent_faces.push(*face_index);
                }
            }

            if let Some(twin_id) = halfedge.twin {
                if let Some(twin) = brep.halfedges.get(twin_id as usize) {
                    if let Some(face_id) = twin.face {
                        if let Some(face_index) = face_index_by_id.get(&face_id) {
                            adjacent_faces.push(*face_index);
                        }
                    }
                }
            }
        }

        adjacent_faces.sort_unstable();
        adjacent_faces.dedup();
        adjacency.insert(edge.id, adjacent_faces);
    }

    adjacency
}

fn collect_candidate_edges(brep: &Brep) -> Vec<EdgeCandidate> {
    let mut candidates = Vec::new();

    for edge in &brep.edges {
        let Some((v1, v2)) = brep.get_edge_endpoints(edge.id) else {
            continue;
        };

        let Some(key) = EdgeKey::new(v1, v2) else {
            continue;
        };

        candidates.push(EdgeCandidate {
            id: edge.id,
            a: key.a,
            b: key.b,
        });
    }

    candidates.sort_by_key(|edge| (edge.a, edge.b, edge.id));
    candidates.dedup_by_key(|edge| edge.id);
    candidates
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
    let (face_vertices, _) = brep.get_vertices_and_holes_by_face_id(face.id);
    let points: Vec<[f64; 3]> = face_vertices.iter().map(vec3_to_arr).collect();

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

fn is_edge_vertex_index_valid(a: u32, b: u32, vertex_count: usize) -> bool {
    (a as usize) < vertex_count && (b as usize) < vertex_count
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
    use crate::brep::{Brep, BrepBuilder};
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
        let mut builder = BrepBuilder::new(Uuid::new_v4());
        builder.add_vertices(&[Vector3::new(-1.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0)]);
        builder.add_wire(&[0, 1], false).unwrap();
        let brep: Brep = builder.build().unwrap();

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

    #[test]
    fn classify_edge_returns_visible_crease_for_standalone_wire() {
        let mut builder = BrepBuilder::new(Uuid::new_v4());
        builder.add_vertices(&[Vector3::new(0.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0)]);
        builder.add_wire(&[0, 1], false).unwrap();
        let brep: Brep = builder.build().unwrap();

        let camera = CameraParameters::default();
        let frame = build_camera_frame(&camera).unwrap();
        let face_info = compute_face_info(&brep, &frame);
        let adjacency = build_edge_adjacency(&brep);

        for edge in &brep.edges {
            let class = classify_edge(edge.id, &adjacency, &face_info);
            assert_eq!(class, EdgeClass::VisibleCrease);
        }
    }

    #[test]
    fn classified_segments_carry_source_entity_id() {
        let mut builder = BrepBuilder::new(Uuid::new_v4());
        builder.add_vertices(&[Vector3::new(-1.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0)]);
        builder.add_wire(&[0, 1], false).unwrap();
        let brep: Brep = builder.build().unwrap();
        let expected_id = brep.id.to_string();

        let scene =
            project_brep_to_scene(&brep, &CameraParameters::default(), &HlrOptions::default());
        assert!(!scene.segments.is_empty());
        for seg in &scene.segments {
            assert_eq!(seg.source_entity_id.as_deref(), Some(expected_id.as_str()));
        }
    }

    #[test]
    fn to_lines_carries_edge_class_string() {
        let mut builder = BrepBuilder::new(Uuid::new_v4());
        builder.add_vertices(&[Vector3::new(-1.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0)]);
        builder.add_wire(&[0, 1], false).unwrap();
        let brep: Brep = builder.build().unwrap();

        let scene =
            project_brep_to_scene(&brep, &CameraParameters::default(), &HlrOptions::default());
        let lines = scene.to_lines();
        assert!(!lines.lines.is_empty());
        assert!(lines.lines[0].class.is_some());
    }
}
