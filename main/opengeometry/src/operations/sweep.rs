use crate::brep::{Brep, Edge, Face, Vertex};
use openmaths::Vector3;
use uuid::Uuid;

const EPSILON: f64 = 1.0e-9;

#[derive(Clone, Copy)]
pub struct SweepOptions {
    pub cap_start: bool,
    pub cap_end: bool,
}

impl Default for SweepOptions {
    fn default() -> Self {
        SweepOptions {
            cap_start: true,
            cap_end: true,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Vec3f {
    x: f64,
    y: f64,
    z: f64,
}

impl Vec3f {
    fn new(x: f64, y: f64, z: f64) -> Self {
        Vec3f { x, y, z }
    }

    fn from_vector3(v: &Vector3) -> Self {
        Vec3f::new(v.x, v.y, v.z)
    }

    fn to_vector3(self) -> Vector3 {
        Vector3::new(self.x, self.y, self.z)
    }

    fn add(self, other: Vec3f) -> Vec3f {
        Vec3f::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }

    fn sub(self, other: Vec3f) -> Vec3f {
        Vec3f::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }

    fn scale(self, s: f64) -> Vec3f {
        Vec3f::new(self.x * s, self.y * s, self.z * s)
    }

    fn dot(self, other: Vec3f) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    fn cross(self, other: Vec3f) -> Vec3f {
        Vec3f::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    fn norm_sq(self) -> f64 {
        self.dot(self)
    }

    fn norm(self) -> f64 {
        self.norm_sq().sqrt()
    }

    fn normalized(self) -> Option<Vec3f> {
        let n = self.norm();
        if n <= EPSILON {
            None
        } else {
            Some(self.scale(1.0 / n))
        }
    }
}

#[derive(Clone, Copy)]
struct SweepFrame {
    tangent: Vec3f,
    normal: Vec3f,
    binormal: Vec3f,
}

#[derive(Clone, Copy)]
struct LocalProfilePoint {
    u: f64,
    v: f64,
    w: f64,
}

pub fn sweep_profile_along_path(
    path_points: &[Vector3],
    profile_points: &[Vector3],
    options: SweepOptions,
) -> Brep {
    let (clean_path, path_closed) = sanitize_path(path_points);
    let clean_profile = sanitize_profile(profile_points);

    let mut brep = Brep::new(Uuid::new_v4());

    if clean_path.len() < 2 || clean_profile.len() < 3 {
        return brep;
    }

    let frames = build_path_frames(&clean_path, path_closed);
    if frames.len() != clean_path.len() {
        return brep;
    }

    let local_profile = build_local_profile(&clean_profile);
    if local_profile.len() != clean_profile.len() {
        return brep;
    }

    let section_count = clean_path.len();
    let ring_size = local_profile.len();

    for section_index in 0..section_count {
        let section_origin = clean_path[section_index];
        let frame = frames[section_index];

        for local in &local_profile {
            let world_point = section_origin
                .add(frame.normal.scale(local.u))
                .add(frame.binormal.scale(local.v))
                .add(frame.tangent.scale(local.w));

            brep.vertices.push(Vertex::new(
                brep.get_vertex_count(),
                world_point.to_vector3(),
            ));
        }
    }

    let side_segments = if path_closed {
        section_count
    } else {
        section_count - 1
    };

    for section_index in 0..side_segments {
        let next_section = (section_index + 1) % section_count;

        for profile_index in 0..ring_size {
            let next_profile = (profile_index + 1) % ring_size;

            let a = (section_index * ring_size + profile_index) as u32;
            let b = (section_index * ring_size + next_profile) as u32;
            let c = (next_section * ring_size + next_profile) as u32;
            let d = (next_section * ring_size + profile_index) as u32;

            add_face_with_edges(&mut brep, vec![a, b, c, d]);
        }
    }

    if !path_closed {
        if options.cap_start {
            let mut start_face: Vec<u32> = (0..ring_size as u32).collect();
            start_face.reverse();
            add_face_with_edges(&mut brep, start_face);
        }

        if options.cap_end {
            let end_start = ((section_count - 1) * ring_size) as u32;
            let end_face: Vec<u32> = (0..ring_size as u32)
                .map(|index| end_start + index)
                .collect();
            add_face_with_edges(&mut brep, end_face);
        }
    }

    brep
}

fn sanitize_path(path: &[Vector3]) -> (Vec<Vec3f>, bool) {
    let mut cleaned = remove_consecutive_duplicates(path);

    let mut is_closed = false;
    if cleaned.len() >= 3 {
        let first = cleaned[0];
        let last = cleaned[cleaned.len() - 1];
        if first.sub(last).norm_sq() <= EPSILON * EPSILON {
            cleaned.pop();
            is_closed = true;
        }
    }

    (cleaned, is_closed)
}

fn sanitize_profile(profile: &[Vector3]) -> Vec<Vec3f> {
    let mut cleaned = remove_consecutive_duplicates(profile);
    if cleaned.len() >= 3 {
        let first = cleaned[0];
        let last = cleaned[cleaned.len() - 1];
        if first.sub(last).norm_sq() <= EPSILON * EPSILON {
            cleaned.pop();
        }
    }
    cleaned
}

fn remove_consecutive_duplicates(points: &[Vector3]) -> Vec<Vec3f> {
    let mut cleaned: Vec<Vec3f> = Vec::new();
    for point in points {
        let candidate = Vec3f::from_vector3(point);
        if cleaned
            .last()
            .map(|last| candidate.sub(*last).norm_sq() <= EPSILON * EPSILON)
            .unwrap_or(false)
        {
            continue;
        }
        cleaned.push(candidate);
    }
    cleaned
}

fn build_path_frames(path: &[Vec3f], is_closed: bool) -> Vec<SweepFrame> {
    if path.len() < 2 {
        return Vec::new();
    }

    let mut tangents: Vec<Vec3f> = Vec::with_capacity(path.len());
    for index in 0..path.len() {
        let tangent = compute_path_tangent(path, is_closed, index);
        if let Some(normalized) = tangent.normalized() {
            tangents.push(normalized);
        } else if let Some(previous) = tangents.last().copied() {
            tangents.push(previous);
        } else {
            tangents.push(Vec3f::new(0.0, 1.0, 0.0));
        }
    }

    let mut frames: Vec<SweepFrame> = Vec::with_capacity(path.len());

    let first_tangent = tangents[0];
    let mut first_normal = any_orthogonal(first_tangent);
    let mut first_binormal = first_tangent.cross(first_normal);

    if first_binormal.norm_sq() <= EPSILON * EPSILON {
        first_normal = Vec3f::new(1.0, 0.0, 0.0);
        first_binormal = first_tangent.cross(first_normal);
    }

    first_binormal = first_binormal
        .normalized()
        .unwrap_or(Vec3f::new(0.0, 0.0, 1.0));
    first_normal = first_binormal
        .cross(first_tangent)
        .normalized()
        .unwrap_or(Vec3f::new(1.0, 0.0, 0.0));

    frames.push(SweepFrame {
        tangent: first_tangent,
        normal: first_normal,
        binormal: first_binormal,
    });

    for index in 1..path.len() {
        let prev = frames[index - 1];
        let tangent = tangents[index];

        let axis = prev.tangent.cross(tangent);
        let axis_norm = axis.norm();

        let mut normal = if axis_norm <= EPSILON {
            if prev.tangent.dot(tangent) < 0.0 {
                any_orthogonal(tangent)
            } else {
                prev.normal
            }
        } else {
            let axis_normalized = axis.scale(1.0 / axis_norm);
            let cos_theta = prev.tangent.dot(tangent).clamp(-1.0, 1.0);
            let theta = cos_theta.acos();
            rotate_around_axis(prev.normal, axis_normalized, theta)
        };

        let mut binormal = tangent.cross(normal);
        if binormal.norm_sq() <= EPSILON * EPSILON {
            normal = any_orthogonal(tangent);
            binormal = tangent.cross(normal);
        }

        binormal = binormal.normalized().unwrap_or(Vec3f::new(0.0, 0.0, 1.0));
        normal = binormal
            .cross(tangent)
            .normalized()
            .unwrap_or(Vec3f::new(1.0, 0.0, 0.0));

        frames.push(SweepFrame {
            tangent,
            normal,
            binormal,
        });
    }

    frames
}

fn compute_path_tangent(path: &[Vec3f], is_closed: bool, index: usize) -> Vec3f {
    let count = path.len();

    if is_closed {
        let prev = path[(index + count - 1) % count];
        let next = path[(index + 1) % count];
        return next.sub(prev);
    }

    if index == 0 {
        path[1].sub(path[0])
    } else if index == count - 1 {
        path[count - 1].sub(path[count - 2])
    } else {
        path[index + 1].sub(path[index - 1])
    }
}

fn build_local_profile(profile: &[Vec3f]) -> Vec<LocalProfilePoint> {
    if profile.len() < 3 {
        return Vec::new();
    }

    let mut centroid = Vec3f::new(0.0, 0.0, 0.0);
    for point in profile {
        centroid = centroid.add(*point);
    }
    centroid = centroid.scale(1.0 / profile.len() as f64);

    let normal = compute_profile_normal(profile);

    let mut u = profile[0].sub(centroid);
    if u.norm_sq() <= EPSILON * EPSILON {
        u = profile[1].sub(centroid);
    }

    if u.norm_sq() <= EPSILON * EPSILON {
        u = any_orthogonal(normal);
    }

    let mut u = u.normalized().unwrap_or(any_orthogonal(normal));
    let mut v = normal.cross(u);
    if v.norm_sq() <= EPSILON * EPSILON {
        u = any_orthogonal(normal);
        v = normal.cross(u);
    }
    v = v.normalized().unwrap_or(any_orthogonal(u));
    u = v
        .cross(normal)
        .normalized()
        .unwrap_or(any_orthogonal(normal));

    profile
        .iter()
        .map(|point| {
            let delta = point.sub(centroid);
            LocalProfilePoint {
                u: delta.dot(u),
                v: delta.dot(v),
                w: delta.dot(normal),
            }
        })
        .collect()
}

fn compute_profile_normal(profile: &[Vec3f]) -> Vec3f {
    for i in 0..profile.len() {
        let a = profile[i];
        let b = profile[(i + 1) % profile.len()];
        let c = profile[(i + 2) % profile.len()];

        let ab = b.sub(a);
        let bc = c.sub(b);
        let normal = ab.cross(bc);

        if let Some(n) = normal.normalized() {
            return n;
        }
    }

    Vec3f::new(0.0, 1.0, 0.0)
}

fn any_orthogonal(direction: Vec3f) -> Vec3f {
    let mut reference = Vec3f::new(0.0, 1.0, 0.0);
    if direction.dot(reference).abs() > 0.95 {
        reference = Vec3f::new(1.0, 0.0, 0.0);
    }

    let mut orthogonal = reference.cross(direction);
    if orthogonal.norm_sq() <= EPSILON * EPSILON {
        orthogonal = Vec3f::new(0.0, 0.0, 1.0).cross(direction);
    }

    orthogonal.normalized().unwrap_or(Vec3f::new(1.0, 0.0, 0.0))
}

fn rotate_around_axis(vector: Vec3f, axis: Vec3f, angle: f64) -> Vec3f {
    let cos_theta = angle.cos();
    let sin_theta = angle.sin();

    vector
        .scale(cos_theta)
        .add(axis.cross(vector).scale(sin_theta))
        .add(axis.scale(axis.dot(vector) * (1.0 - cos_theta)))
}

fn add_face_with_edges(brep: &mut Brep, face_indices: Vec<u32>) {
    if face_indices.len() < 3 {
        return;
    }

    let face_id = brep.get_face_count();
    brep.faces.push(Face::new(face_id, face_indices.clone()));

    for index in 0..face_indices.len() {
        let v1 = face_indices[index];
        let v2 = face_indices[(index + 1) % face_indices.len()];
        let edge_id = brep.get_edge_count();
        brep.edges.push(Edge::new(edge_id, v1, v2));
    }
}

#[cfg(test)]
mod tests {
    use super::{sweep_profile_along_path, SweepOptions};
    use openmaths::Vector3;

    fn rectangle_profile(width: f64, depth: f64) -> Vec<Vector3> {
        let hw = width * 0.5;
        let hd = depth * 0.5;
        vec![
            Vector3::new(-hw, 0.0, -hd),
            Vector3::new(hw, 0.0, -hd),
            Vector3::new(hw, 0.0, hd),
            Vector3::new(-hw, 0.0, hd),
        ]
    }

    #[test]
    fn open_sweep_with_caps_has_expected_topology() {
        let path = vec![Vector3::new(0.0, 0.0, 0.0), Vector3::new(0.0, 2.0, 0.0)];
        let profile = rectangle_profile(1.0, 1.0);

        let brep = sweep_profile_along_path(&path, &profile, SweepOptions::default());

        assert_eq!(brep.vertices.len(), 8);
        assert_eq!(brep.faces.len(), 6);
        assert!(!brep.edges.is_empty());
    }

    #[test]
    fn open_sweep_without_caps_only_generates_side_faces() {
        let path = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
            Vector3::new(1.0, 2.0, 0.0),
        ];
        let profile = rectangle_profile(1.0, 0.5);

        let brep = sweep_profile_along_path(
            &path,
            &profile,
            SweepOptions {
                cap_start: false,
                cap_end: false,
            },
        );

        assert_eq!(brep.vertices.len(), 12);
        assert_eq!(brep.faces.len(), 8);
    }

    #[test]
    fn closed_path_sweep_does_not_generate_end_caps() {
        let path = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 1.0),
            Vector3::new(0.0, 0.0, 1.0),
            Vector3::new(0.0, 0.0, 0.0),
        ];
        let profile = rectangle_profile(0.4, 0.4);

        let brep = sweep_profile_along_path(&path, &profile, SweepOptions::default());

        assert_eq!(brep.vertices.len(), 16);
        assert_eq!(brep.faces.len(), 16);
    }
}
