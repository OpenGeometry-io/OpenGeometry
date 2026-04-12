use std::error::Error;
use std::f64::consts::PI;
use std::fmt;

use crate::brep::{Brep, BrepBuilder, BrepError};
use openmaths::Vector3;
use uuid::Uuid;

const EPSILON: f64 = 1.0e-9;
const PLANAR_TOLERANCE_FACTOR: f64 = 1.0e-7;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SweepErrorKind {
    InvalidPath,
    InvalidProfile,
    NonPlanarProfile,
    NonPlanarClosedPath,
    PathReversal,
    ProjectionFailure,
    TopologyError,
}

#[derive(Debug, Clone)]
pub struct SweepError {
    kind: SweepErrorKind,
    message: String,
}

impl SweepError {
    fn new(kind: SweepErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn kind(&self) -> SweepErrorKind {
        self.kind
    }
}

impl fmt::Display for SweepError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for SweepError {}

impl From<BrepError> for SweepError {
    fn from(error: BrepError) -> Self {
        SweepError::new(
            SweepErrorKind::TopologyError,
            format!("BRep construction failed: {}", error),
        )
    }
}

#[derive(Clone, Copy, Debug)]
struct Vec2f {
    x: f64,
    y: f64,
}

impl Vec2f {
    fn new(x: f64, y: f64) -> Self {
        Self { x, y }
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

    #[cfg(test)]
    fn distance(self, other: Vec3f) -> f64 {
        self.sub(other).norm()
    }
}

#[derive(Clone, Copy, Debug)]
struct SectionPlane {
    origin: Vec3f,
    normal: Vec3f,
}

#[derive(Clone, Debug)]
struct ProfileData {
    local_points: Vec<Vec2f>,
    normal: Vec3f,
    u_axis: Vec3f,
    scale: f64,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Debug)]
struct PreparedSweep {
    path: Vec<Vec3f>,
    is_closed: bool,
    segment_dirs: Vec<Vec3f>,
    section_planes: Vec<SectionPlane>,
    sections: Vec<Vec<Vec3f>>,
}

pub fn sweep_profile_along_path(
    path_points: &[Vector3],
    profile_points: &[Vector3],
    options: SweepOptions,
) -> Result<Brep, SweepError> {
    let prepared = build_prepared_sweep(path_points, profile_points)?;

    let mut builder = BrepBuilder::new(Uuid::new_v4());
    let mut section_vertex_ids: Vec<Vec<u32>> = Vec::with_capacity(prepared.sections.len());

    for section in &prepared.sections {
        let section_vertices: Vec<Vector3> =
            section.iter().map(|point| point.to_vector3()).collect();
        section_vertex_ids.push(builder.add_vertices(&section_vertices));
    }

    let ring_size = prepared.sections[0].len();
    let side_segments = if prepared.is_closed {
        prepared.sections.len()
    } else {
        prepared.sections.len() - 1
    };

    for section_index in 0..side_segments {
        let next_section = (section_index + 1) % prepared.sections.len();

        for profile_index in 0..ring_size {
            let next_profile = (profile_index + 1) % ring_size;
            let face = orient_side_face_indices(
                &prepared.sections[section_index],
                &prepared.sections[next_section],
                &section_vertex_ids[section_index],
                &section_vertex_ids[next_section],
                prepared.path[section_index],
                prepared.path[next_section],
                profile_index,
                next_profile,
            );
            builder.add_face(&face, &[])?;
        }
    }

    if !prepared.is_closed {
        if options.cap_start {
            let start_indices = orient_cap_indices(
                &prepared.sections[0],
                section_vertex_ids[0].clone(),
                prepared.section_planes[0].normal.scale(-1.0),
            );
            builder.add_face(&start_indices, &[])?;
        }

        if options.cap_end {
            let last_index = prepared.sections.len() - 1;
            let end_indices = orient_cap_indices(
                &prepared.sections[last_index],
                section_vertex_ids[last_index].clone(),
                prepared.section_planes[last_index].normal,
            );
            builder.add_face(&end_indices, &[])?;
        }
    }

    let shell_closed = prepared.is_closed || (options.cap_start && options.cap_end);
    builder.add_shell_from_all_faces(shell_closed)?;
    builder.build().map_err(Into::into)
}

fn build_prepared_sweep(
    path_points: &[Vector3],
    profile_points: &[Vector3],
) -> Result<PreparedSweep, SweepError> {
    let (path, is_closed) = sanitize_path(path_points)?;
    let profile = sanitize_profile(profile_points)?;
    let segment_dirs = build_segment_dirs(&path, is_closed)?;

    if is_closed {
        ensure_closed_path_is_planar(&path)?;
    }

    let section_planes = build_section_planes(&path, &segment_dirs, is_closed)?;
    let sections = build_sections(&path, &segment_dirs, &section_planes, &profile, is_closed)?;

    Ok(PreparedSweep {
        path,
        is_closed,
        segment_dirs,
        section_planes,
        sections,
    })
}

fn sanitize_path(path: &[Vector3]) -> Result<(Vec<Vec3f>, bool), SweepError> {
    let mut cleaned = remove_consecutive_duplicates(path);

    if cleaned.len() < 2 {
        return Err(SweepError::new(
            SweepErrorKind::InvalidPath,
            "Sweep path requires at least 2 distinct points.",
        ));
    }

    let mut is_closed = false;
    if cleaned.len() >= 3 {
        let tolerance_sq = tolerance_for_vec3_points(&cleaned, PLANAR_TOLERANCE_FACTOR).powi(2);
        let first = cleaned[0];
        let last = cleaned[cleaned.len() - 1];
        if first.sub(last).norm_sq() <= tolerance_sq {
            cleaned.pop();
            is_closed = true;
        }
    }

    let minimum_points = if is_closed { 3 } else { 2 };
    if cleaned.len() < minimum_points {
        return Err(SweepError::new(
            SweepErrorKind::InvalidPath,
            "Sweep path degenerates after duplicate-point cleanup.",
        ));
    }

    Ok((cleaned, is_closed))
}

fn sanitize_profile(profile: &[Vector3]) -> Result<ProfileData, SweepError> {
    let mut cleaned = remove_consecutive_duplicates(profile);

    if cleaned.len() >= 3 {
        let tolerance_sq = tolerance_for_vec3_points(&cleaned, PLANAR_TOLERANCE_FACTOR).powi(2);
        let first = cleaned[0];
        let last = cleaned[cleaned.len() - 1];
        if first.sub(last).norm_sq() <= tolerance_sq {
            cleaned.pop();
        }
    }

    if cleaned.len() < 3 {
        return Err(SweepError::new(
            SweepErrorKind::InvalidProfile,
            "Sweep profile requires at least 3 distinct points.",
        ));
    }

    let centroid = cleaned
        .iter()
        .copied()
        .fold(Vec3f::new(0.0, 0.0, 0.0), |sum, point| sum.add(point))
        .scale(1.0 / cleaned.len() as f64);

    let Some(normal) = compute_polygon_normal(&cleaned) else {
        return Err(SweepError::new(
            SweepErrorKind::InvalidProfile,
            "Sweep profile is degenerate and does not define a plane.",
        ));
    };

    let tolerance = tolerance_for_vec3_points(&cleaned, PLANAR_TOLERANCE_FACTOR);
    for point in &cleaned {
        let distance = point.sub(centroid).dot(normal).abs();
        if distance > tolerance {
            return Err(SweepError::new(
                SweepErrorKind::NonPlanarProfile,
                format!(
                    "Sweep profile must be planar; point deviates from the profile plane by {:.3e}.",
                    distance
                ),
            ));
        }
    }

    let reference_axis = cleaned
        .iter()
        .enumerate()
        .find_map(|(index, point)| {
            let next = cleaned[(index + 1) % cleaned.len()];
            let edge = next.sub(*point);
            let projected = edge.sub(normal.scale(edge.dot(normal)));
            projected.normalized()
        })
        .or_else(|| {
            cleaned.iter().find_map(|point| {
                let delta = point.sub(centroid);
                let projected = delta.sub(normal.scale(delta.dot(normal)));
                projected.normalized()
            })
        });

    let Some(reference_axis) = reference_axis else {
        return Err(SweepError::new(
            SweepErrorKind::InvalidProfile,
            "Sweep profile collapses to its centroid after planar projection.",
        ));
    };

    let v_axis = normal
        .cross(reference_axis)
        .normalized()
        .unwrap_or_else(|| any_orthogonal(normal));
    let u_axis = v_axis.cross(normal).normalized().unwrap_or(reference_axis);

    let mut local_points: Vec<Vec2f> = cleaned
        .iter()
        .map(|point| {
            let delta = point.sub(centroid);
            Vec2f::new(delta.dot(u_axis), delta.dot(v_axis))
        })
        .collect();

    let signed_area = signed_area_2d(&local_points);
    if signed_area.abs() <= tolerance {
        return Err(SweepError::new(
            SweepErrorKind::InvalidProfile,
            "Sweep profile has near-zero signed area.",
        ));
    }

    if signed_area < 0.0 {
        local_points.reverse();
        local_points.rotate_right(1);
    }

    Ok(ProfileData {
        local_points,
        normal,
        u_axis,
        scale: point_set_scale_vec3(&cleaned),
    })
}

fn build_segment_dirs(path: &[Vec3f], is_closed: bool) -> Result<Vec<Vec3f>, SweepError> {
    let segment_count = if is_closed {
        path.len()
    } else {
        path.len() - 1
    };
    let mut segment_dirs = Vec::with_capacity(segment_count);

    for index in 0..segment_count {
        let next_index = (index + 1) % path.len();
        let delta = path[next_index].sub(path[index]);
        let Some(direction) = delta.normalized() else {
            return Err(SweepError::new(
                SweepErrorKind::InvalidPath,
                format!("Sweep path segment {} is degenerate.", index),
            ));
        };
        segment_dirs.push(direction);
    }

    Ok(segment_dirs)
}

fn ensure_closed_path_is_planar(path: &[Vec3f]) -> Result<(), SweepError> {
    let Some(normal) = compute_polygon_normal(path) else {
        return Err(SweepError::new(
            SweepErrorKind::NonPlanarClosedPath,
            "Closed sweep paths must define a planar loop with non-zero area.",
        ));
    };

    let tolerance = tolerance_for_vec3_points(path, PLANAR_TOLERANCE_FACTOR);
    let origin = path[0];

    for point in path {
        let distance = point.sub(origin).dot(normal).abs();
        if distance > tolerance {
            return Err(SweepError::new(
                SweepErrorKind::NonPlanarClosedPath,
                format!(
                    "Closed sweep paths must be planar; point deviates from the loop plane by {:.3e}.",
                    distance
                ),
            ));
        }
    }

    Ok(())
}

fn build_section_planes(
    path: &[Vec3f],
    segment_dirs: &[Vec3f],
    is_closed: bool,
) -> Result<Vec<SectionPlane>, SweepError> {
    let mut planes = Vec::with_capacity(path.len());

    if is_closed {
        for index in 0..path.len() {
            let prev_dir = segment_dirs[(index + path.len() - 1) % path.len()];
            let next_dir = segment_dirs[index];
            let normal = corner_plane_normal(prev_dir, next_dir)?;
            planes.push(SectionPlane {
                origin: path[index],
                normal,
            });
        }

        return Ok(planes);
    }

    planes.push(SectionPlane {
        origin: path[0],
        normal: segment_dirs[0],
    });

    for index in 1..(path.len() - 1) {
        let normal = corner_plane_normal(segment_dirs[index - 1], segment_dirs[index])?;
        planes.push(SectionPlane {
            origin: path[index],
            normal,
        });
    }

    planes.push(SectionPlane {
        origin: path[path.len() - 1],
        normal: segment_dirs[segment_dirs.len() - 1],
    });

    Ok(planes)
}

fn build_sections(
    path: &[Vec3f],
    segment_dirs: &[Vec3f],
    section_planes: &[SectionPlane],
    profile: &ProfileData,
    is_closed: bool,
) -> Result<Vec<Vec<Vec3f>>, SweepError> {
    if is_closed {
        return build_closed_planar_sections(path, segment_dirs, section_planes, profile);
    }

    let mut sections = vec![Vec::new(); path.len()];
    sections[0] = build_initial_section(profile, &section_planes[0]);

    let projection_tolerance = tolerance_for_scale(
        point_set_scale_vec3(path).max(profile.scale),
        PLANAR_TOLERANCE_FACTOR,
    );

    for index in 0..(path.len() - 1) {
        sections[index + 1] = project_section_to_plane(
            &sections[index],
            segment_dirs[index],
            &section_planes[index + 1],
            projection_tolerance,
        )?;
    }

    Ok(sections)
}

fn build_closed_planar_sections(
    path: &[Vec3f],
    segment_dirs: &[Vec3f],
    section_planes: &[SectionPlane],
    profile: &ProfileData,
) -> Result<Vec<Vec<Vec3f>>, SweepError> {
    let Some(path_normal) = compute_polygon_normal(path) else {
        return Err(SweepError::new(
            SweepErrorKind::NonPlanarClosedPath,
            "Closed sweep paths must define a planar loop with non-zero area.",
        ));
    };

    let rotated_hint = rotate_between_normals(
        profile.u_axis,
        profile.normal,
        section_planes[0].normal,
        profile.u_axis,
    );
    let (axis_u, axis_v) = build_plane_basis(section_planes[0].normal, rotated_hint);
    let lateral_axis =
        select_closed_path_lateral_axis(axis_u, axis_v, path_normal, section_planes[0].normal);

    let projection_tolerance = tolerance_for_scale(
        point_set_scale_vec3(path).max(profile.scale),
        PLANAR_TOLERANCE_FACTOR,
    );

    let first_corner_probe = offset_corner_point(
        path[0],
        segment_dirs[segment_dirs.len() - 1],
        segment_dirs[0],
        1.0,
        path_normal,
        1.0,
        projection_tolerance,
    )?;
    let lateral_normal_sign = if first_corner_probe.sub(path[0]).dot(lateral_axis) >= 0.0 {
        1.0
    } else {
        -1.0
    };

    let profile_offsets: Vec<(f64, f64)> = profile
        .local_points
        .iter()
        .map(|point| {
            let delta = axis_u.scale(point.x).add(axis_v.scale(point.y));
            (delta.dot(lateral_axis), delta.dot(path_normal))
        })
        .collect();

    let mut sections = Vec::with_capacity(path.len());
    for index in 0..path.len() {
        let prev_dir = segment_dirs[(index + path.len() - 1) % path.len()];
        let next_dir = segment_dirs[index];
        let mut section = Vec::with_capacity(profile_offsets.len());

        for (lateral_offset, depth_offset) in &profile_offsets {
            let offset_corner = offset_corner_point(
                path[index],
                prev_dir,
                next_dir,
                *lateral_offset,
                path_normal,
                lateral_normal_sign,
                projection_tolerance,
            )?;
            section.push(offset_corner.add(path_normal.scale(*depth_offset)));
        }

        sections.push(section);
    }

    Ok(sections)
}

fn build_initial_section(profile: &ProfileData, plane: &SectionPlane) -> Vec<Vec3f> {
    let rotated_hint =
        rotate_between_normals(profile.u_axis, profile.normal, plane.normal, profile.u_axis);
    let (u_axis, v_axis) = build_plane_basis(plane.normal, rotated_hint);

    profile
        .local_points
        .iter()
        .map(|point| {
            plane
                .origin
                .add(u_axis.scale(point.x))
                .add(v_axis.scale(point.y))
        })
        .collect()
}

fn select_closed_path_lateral_axis(
    axis_u: Vec3f,
    axis_v: Vec3f,
    path_normal: Vec3f,
    section_normal: Vec3f,
) -> Vec3f {
    let projected_u = axis_u.sub(path_normal.scale(axis_u.dot(path_normal)));
    let projected_v = axis_v.sub(path_normal.scale(axis_v.dot(path_normal)));

    if projected_u.norm_sq() >= projected_v.norm_sq() {
        if let Some(axis) = projected_u.normalized() {
            return axis;
        }
    } else if let Some(axis) = projected_v.normalized() {
        return axis;
    }

    path_normal
        .cross(section_normal)
        .normalized()
        .or_else(|| section_normal.cross(path_normal).normalized())
        .unwrap_or_else(|| any_orthogonal(path_normal))
}

fn offset_corner_point(
    origin: Vec3f,
    prev_dir: Vec3f,
    next_dir: Vec3f,
    offset: f64,
    path_normal: Vec3f,
    lateral_normal_sign: f64,
    tolerance: f64,
) -> Result<Vec3f, SweepError> {
    let prev_normal = path_normal.cross(prev_dir).scale(lateral_normal_sign);
    let next_normal = path_normal.cross(next_dir).scale(lateral_normal_sign);
    let prev_point = origin.add(prev_normal.scale(offset));
    let next_point = origin.add(next_normal.scale(offset));
    let denominator = prev_dir.cross(next_dir).dot(path_normal);

    if denominator.abs() <= tolerance {
        return Ok(prev_point.add(next_point).scale(0.5));
    }

    let delta = next_point.sub(prev_point);
    let t = delta.cross(next_dir).dot(path_normal) / denominator;
    Ok(prev_point.add(prev_dir.scale(t)))
}

fn project_section_to_plane(
    section: &[Vec3f],
    direction: Vec3f,
    target_plane: &SectionPlane,
    tolerance: f64,
) -> Result<Vec<Vec3f>, SweepError> {
    let denominator = target_plane.normal.dot(direction);
    if denominator.abs() <= tolerance {
        return Err(SweepError::new(
            SweepErrorKind::ProjectionFailure,
            "Sweep section projection became parallel to its target plane.",
        ));
    }

    Ok(section
        .iter()
        .map(|point| {
            let t = target_plane.normal.dot(target_plane.origin.sub(*point)) / denominator;
            point.add(direction.scale(t))
        })
        .collect())
}

fn corner_plane_normal(prev_dir: Vec3f, next_dir: Vec3f) -> Result<Vec3f, SweepError> {
    let combined = prev_dir.add(next_dir);
    combined.normalized().ok_or_else(|| {
        SweepError::new(
            SweepErrorKind::PathReversal,
            "Sweep path contains a 180-degree reversal, which cannot be mitred robustly.",
        )
    })
}

fn orient_cap_indices(section: &[Vec3f], indices: Vec<u32>, desired_normal: Vec3f) -> Vec<u32> {
    let actual_normal = compute_polygon_normal(section).unwrap_or(desired_normal);
    if actual_normal.dot(desired_normal) >= 0.0 {
        indices
    } else {
        let mut reversed = indices;
        reversed.reverse();
        reversed
    }
}

fn orient_side_face_indices(
    current_section: &[Vec3f],
    next_section: &[Vec3f],
    current_indices: &[u32],
    next_indices: &[u32],
    segment_start: Vec3f,
    segment_end: Vec3f,
    profile_index: usize,
    next_profile: usize,
) -> Vec<u32> {
    let face_points = [
        current_section[profile_index],
        current_section[next_profile],
        next_section[next_profile],
        next_section[profile_index],
    ];
    let mut face_indices = vec![
        current_indices[profile_index],
        current_indices[next_profile],
        next_indices[next_profile],
        next_indices[profile_index],
    ];

    let Some(face_normal) = compute_polygon_normal(&face_points) else {
        return face_indices;
    };

    let face_centroid = centroid_vec3f(&face_points);
    let segment_midpoint = segment_start.add(segment_end).scale(0.5);
    let radial_reference = face_centroid.sub(segment_midpoint);

    if radial_reference.norm_sq() > EPSILON * EPSILON && face_normal.dot(radial_reference) < 0.0 {
        face_indices.reverse();
    }

    face_indices
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

fn build_plane_basis(normal: Vec3f, hint: Vec3f) -> (Vec3f, Vec3f) {
    let projected_hint = hint.sub(normal.scale(hint.dot(normal)));
    let u_axis = projected_hint
        .normalized()
        .unwrap_or_else(|| any_orthogonal(normal));
    let v_axis = normal
        .cross(u_axis)
        .normalized()
        .unwrap_or_else(|| any_orthogonal(normal));
    let u_axis = v_axis.cross(normal).normalized().unwrap_or(u_axis);
    (u_axis, v_axis)
}

fn rotate_between_normals(
    vector: Vec3f,
    from_normal: Vec3f,
    to_normal: Vec3f,
    fallback_axis: Vec3f,
) -> Vec3f {
    let dot = from_normal.dot(to_normal).clamp(-1.0, 1.0);
    if dot >= 1.0 - EPSILON {
        return vector;
    }

    if dot <= -1.0 + EPSILON {
        return rotate_around_axis(vector, fallback_axis, PI);
    }

    let axis = from_normal
        .cross(to_normal)
        .normalized()
        .unwrap_or(fallback_axis);
    rotate_around_axis(vector, axis, dot.acos())
}

fn rotate_around_axis(vector: Vec3f, axis: Vec3f, angle: f64) -> Vec3f {
    let axis = axis.normalized().unwrap_or(axis);
    let cos_theta = angle.cos();
    let sin_theta = angle.sin();

    vector
        .scale(cos_theta)
        .add(axis.cross(vector).scale(sin_theta))
        .add(axis.scale(axis.dot(vector) * (1.0 - cos_theta)))
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

fn compute_polygon_normal(points: &[Vec3f]) -> Option<Vec3f> {
    if points.len() < 3 {
        return None;
    }

    let mut accumulated = Vec3f::new(0.0, 0.0, 0.0);
    for index in 0..points.len() {
        let current = points[index];
        let next = points[(index + 1) % points.len()];
        accumulated = accumulated.add(current.cross(next));
    }

    accumulated.normalized()
}

fn centroid_vec3f(points: &[Vec3f]) -> Vec3f {
    if points.is_empty() {
        return Vec3f::new(0.0, 0.0, 0.0);
    }

    points
        .iter()
        .copied()
        .fold(Vec3f::new(0.0, 0.0, 0.0), |sum, point| sum.add(point))
        .scale(1.0 / points.len() as f64)
}

fn signed_area_2d(points: &[Vec2f]) -> f64 {
    let mut area = 0.0;
    for index in 0..points.len() {
        let current = points[index];
        let next = points[(index + 1) % points.len()];
        area += current.x * next.y - next.x * current.y;
    }
    area * 0.5
}

fn point_set_scale_vec3(points: &[Vec3f]) -> f64 {
    if points.is_empty() {
        return 1.0;
    }

    let mut min = points[0];
    let mut max = points[0];

    for point in &points[1..] {
        min.x = min.x.min(point.x);
        min.y = min.y.min(point.y);
        min.z = min.z.min(point.z);
        max.x = max.x.max(point.x);
        max.y = max.y.max(point.y);
        max.z = max.z.max(point.z);
    }

    max.sub(min).norm().max(1.0)
}

fn tolerance_for_vec3_points(points: &[Vec3f], factor: f64) -> f64 {
    tolerance_for_scale(point_set_scale_vec3(points), factor)
}

fn tolerance_for_scale(scale: f64, factor: f64) -> f64 {
    scale.max(1.0) * factor
}

#[cfg(test)]
mod tests {
    use super::{
        build_prepared_sweep, centroid_vec3f, compute_polygon_normal, project_section_to_plane,
        sweep_profile_along_path, PreparedSweep, SectionPlane, SweepErrorKind, SweepOptions, Vec3f,
        EPSILON,
    };
    use crate::brep::Brep;
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

    fn concave_profile() -> Vec<Vector3> {
        vec![
            Vector3::new(-0.6, 0.0, -0.4),
            Vector3::new(0.1, 0.0, -0.4),
            Vector3::new(0.1, 0.0, -0.1),
            Vector3::new(0.45, 0.0, -0.1),
            Vector3::new(0.45, 0.0, 0.45),
            Vector3::new(-0.6, 0.0, 0.45),
        ]
    }

    fn assert_edge_lengths(section: &[Vec3f], expected: &[f64]) {
        assert_eq!(section.len(), expected.len());
        for index in 0..section.len() {
            let actual = section[index].distance(section[(index + 1) % section.len()]);
            let error = (actual - expected[index]).abs();
            assert!(
                error <= 1.0e-6,
                "edge {} length mismatch: actual={:.9} expected={:.9} error={:.3e}",
                index,
                actual,
                expected[index],
                error
            );
        }
    }

    fn assert_section_edges_axis_aligned(section: &[Vec3f]) {
        for index in 0..section.len() {
            let current = section[index];
            let next = section[(index + 1) % section.len()];
            let delta = next.sub(current);

            assert!(
                delta.z.abs() <= 1.0e-6,
                "section edge {} drifted off the cross-section plane: dz={:.3e}",
                index,
                delta.z
            );

            let aligned_x = delta.y.abs() <= 1.0e-6 && delta.x.abs() > 1.0e-6;
            let aligned_y = delta.x.abs() <= 1.0e-6 && delta.y.abs() > 1.0e-6;

            assert!(
                aligned_x || aligned_y,
                "section edge {} is rotated off-axis: dx={:.6}, dy={:.6}",
                index,
                delta.x,
                delta.y
            );
        }
    }

    fn assert_corner_projection_preserves_rectangle(
        prepared: &PreparedSweep,
        corner_index: usize,
        incoming_segment: usize,
        outgoing_segment: usize,
        width: f64,
        depth: f64,
    ) {
        let corner_origin = prepared.path[corner_index];
        let incoming_plane = SectionPlane {
            origin: corner_origin,
            normal: prepared.segment_dirs[incoming_segment],
        };
        let outgoing_plane = SectionPlane {
            origin: corner_origin,
            normal: prepared.segment_dirs[outgoing_segment],
        };

        let incoming_projection = project_section_to_plane(
            &prepared.sections[corner_index],
            prepared.segment_dirs[incoming_segment],
            &incoming_plane,
            1.0e-9,
        )
        .expect("incoming corner projection must succeed");
        let outgoing_projection = project_section_to_plane(
            &prepared.sections[corner_index],
            prepared.segment_dirs[outgoing_segment],
            &outgoing_plane,
            1.0e-9,
        )
        .expect("outgoing corner projection must succeed");

        assert_edge_lengths(&incoming_projection, &[width, depth, width, depth]);
        assert_edge_lengths(&outgoing_projection, &[width, depth, width, depth]);
    }

    fn assert_closed_side_faces_point_outward(path: &[Vector3], profile: &[Vector3]) {
        let prepared = build_prepared_sweep(path, profile).expect("prepared sweep should succeed");
        assert!(prepared.is_closed, "test path must be closed");

        let brep = sweep_profile_along_path(path, profile, SweepOptions::default())
            .expect("closed sweep should succeed");
        brep.validate_topology()
            .expect("closed sweep topology should validate");

        let ring_size = prepared.sections[0].len();
        let side_face_count = prepared.sections.len() * ring_size;
        assert_eq!(
            brep.faces.len(),
            side_face_count,
            "closed sweeps should only emit side faces"
        );

        for section_index in 0..prepared.sections.len() {
            let next_section = (section_index + 1) % prepared.sections.len();
            let segment_midpoint = prepared.path[section_index]
                .add(prepared.path[next_section])
                .scale(0.5);

            for profile_index in 0..ring_size {
                let face_id = (section_index * ring_size + profile_index) as u32;
                let vertices = brep.get_vertices_by_face_id(face_id);
                let face_points: Vec<Vec3f> = vertices.iter().map(Vec3f::from_vector3).collect();
                let face_normal =
                    compute_polygon_normal(&face_points).expect("side face should have a normal");
                let face_centroid = centroid_vec3f(&face_points);
                let radial_reference = face_centroid.sub(segment_midpoint);
                let alignment = face_normal.dot(radial_reference);

                assert!(
                    radial_reference.norm_sq() > EPSILON * EPSILON,
                    "side face {} centroid should not lie on the path midpoint",
                    face_id
                );
                assert!(
                    alignment > 1.0e-9,
                    "side face {} points inward: alignment={:.3e}",
                    face_id,
                    alignment
                );
            }
        }
    }

    fn unique_sorted_values(values: impl IntoIterator<Item = f64>, tolerance: f64) -> Vec<f64> {
        let mut ordered: Vec<f64> = values.into_iter().collect();
        ordered.sort_by(|lhs, rhs| lhs.partial_cmp(rhs).unwrap());

        let mut unique: Vec<f64> = Vec::new();
        for value in ordered {
            if unique
                .last()
                .map(|existing| (value - *existing).abs() <= tolerance)
                .unwrap_or(false)
            {
                continue;
            }
            unique.push(value);
        }

        unique
    }

    fn assert_window_frame_dimensions(
        brep: &Brep,
        expected_outer_width: f64,
        expected_inner_width: f64,
        expected_outer_height: f64,
        expected_inner_height: f64,
    ) {
        let tolerance = 1.0e-6;
        let xs = unique_sorted_values(
            brep.vertices.iter().map(|vertex| vertex.position.x),
            tolerance,
        );
        let ys = unique_sorted_values(
            brep.vertices.iter().map(|vertex| vertex.position.y),
            tolerance,
        );

        assert_eq!(
            xs.len(),
            4,
            "window frame should have four distinct x bands"
        );
        assert_eq!(
            ys.len(),
            4,
            "window frame should have four distinct y bands"
        );

        let observed_outer_width = xs[xs.len() - 1] - xs[0];
        let observed_inner_width = xs[xs.len() - 2] - xs[1];
        let observed_outer_height = ys[ys.len() - 1] - ys[0];
        let observed_inner_height = ys[ys.len() - 2] - ys[1];

        assert!(
            (observed_outer_width - expected_outer_width).abs() <= tolerance,
            "outer width mismatch: actual={:.9} expected={:.9}",
            observed_outer_width,
            expected_outer_width
        );
        assert!(
            (observed_inner_width - expected_inner_width).abs() <= tolerance,
            "inner width mismatch: actual={:.9} expected={:.9}",
            observed_inner_width,
            expected_inner_width
        );
        assert!(
            (observed_outer_height - expected_outer_height).abs() <= tolerance,
            "outer height mismatch: actual={:.9} expected={:.9}",
            observed_outer_height,
            expected_outer_height
        );
        assert!(
            (observed_inner_height - expected_inner_height).abs() <= tolerance,
            "inner height mismatch: actual={:.9} expected={:.9}",
            observed_inner_height,
            expected_inner_height
        );
    }

    #[test]
    fn open_sweep_with_caps_has_expected_topology() {
        let path = vec![Vector3::new(0.0, 0.0, 0.0), Vector3::new(0.0, 2.0, 0.0)];
        let profile = rectangle_profile(1.0, 1.0);

        let brep = sweep_profile_along_path(&path, &profile, SweepOptions::default())
            .expect("open sweep should succeed");

        assert_eq!(brep.vertices.len(), 8);
        assert_eq!(brep.faces.len(), 6);
        assert_eq!(brep.shells.len(), 1);
        assert!(brep.shells[0].is_closed);
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
        )
        .expect("open sweep without caps should succeed");

        assert_eq!(brep.vertices.len(), 12);
        assert_eq!(brep.faces.len(), 8);
        assert_eq!(brep.shells.len(), 1);
        assert!(!brep.shells[0].is_closed);
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

        let brep = sweep_profile_along_path(&path, &profile, SweepOptions::default())
            .expect("closed planar sweep should succeed");

        assert_eq!(brep.vertices.len(), 16);
        assert_eq!(brep.faces.len(), 16);
        assert_eq!(brep.shells.len(), 1);
        assert!(brep.shells[0].is_closed);
    }

    #[test]
    fn closed_rectangular_loop_side_faces_point_outward() {
        let path = vec![
            Vector3::new(-2.2, 0.0, -1.4),
            Vector3::new(2.4, 0.0, -1.4),
            Vector3::new(2.4, 0.0, 1.7),
            Vector3::new(-2.2, 0.0, 1.7),
            Vector3::new(-2.2, 0.0, -1.4),
        ];
        let profile = rectangle_profile(0.45, 0.28);

        assert_closed_side_faces_point_outward(&path, &profile);
    }

    #[test]
    fn closed_trapezoid_loop_side_faces_point_outward() {
        let path = vec![
            Vector3::new(-2.1, 0.0, -1.8),
            Vector3::new(1.8, 0.0, -1.5),
            Vector3::new(1.2, 0.0, 1.4),
            Vector3::new(-1.6, 0.0, 1.0),
            Vector3::new(-2.1, 0.0, -1.8),
        ];
        let profile = rectangle_profile(0.52, 0.24);

        assert_closed_side_faces_point_outward(&path, &profile);
    }

    #[test]
    fn right_angle_corner_projection_preserves_rectangle_profile() {
        let path = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(0.0, 2.0, 0.0),
            Vector3::new(2.0, 2.0, 0.0),
        ];
        let profile = rectangle_profile(1.0, 0.5);

        let prepared =
            build_prepared_sweep(&path, &profile).expect("prepared sweep should succeed");
        assert_corner_projection_preserves_rectangle(&prepared, 1, 0, 1, 1.0, 0.5);
    }

    #[test]
    fn right_angle_corner_projection_is_independent_of_segment_length() {
        let path = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(0.0, 4.0, 0.0),
            Vector3::new(8.0, 4.0, 0.0),
        ];
        let profile = rectangle_profile(1.2, 0.8);

        let prepared =
            build_prepared_sweep(&path, &profile).expect("prepared sweep should succeed");
        assert_corner_projection_preserves_rectangle(&prepared, 1, 0, 1, 1.2, 0.8);
    }

    #[test]
    fn straight_path_keeps_rectangle_edges_axis_aligned() {
        let path = vec![Vector3::new(0.0, 0.0, 0.0), Vector3::new(0.0, 0.0, 3.0)];
        let profile = rectangle_profile(1.0, 0.5);

        let prepared =
            build_prepared_sweep(&path, &profile).expect("prepared sweep should succeed");

        assert_section_edges_axis_aligned(&prepared.sections[0]);
        assert_section_edges_axis_aligned(&prepared.sections[1]);
    }

    #[test]
    fn closed_window_frame_loop_preserves_expected_dimensions() {
        let window_width = 1.2;
        let frame_width = 0.12;
        let frame_depth = 0.12;
        let window_height = 1.0;
        let sill_height = 1.05;
        let half_window_width = window_width * 0.5;
        let half_frame_width = frame_width * 0.5;

        let path = vec![
            Vector3::new(
                -(half_window_width + half_frame_width),
                sill_height - half_frame_width,
                0.0,
            ),
            Vector3::new(
                -(half_window_width + half_frame_width),
                sill_height + window_height + half_frame_width,
                0.0,
            ),
            Vector3::new(
                half_window_width + half_frame_width,
                sill_height + window_height + half_frame_width,
                0.0,
            ),
            Vector3::new(
                half_window_width + half_frame_width,
                sill_height - half_frame_width,
                0.0,
            ),
            Vector3::new(
                -(half_window_width + half_frame_width),
                sill_height - half_frame_width,
                0.0,
            ),
        ];
        let profile = rectangle_profile(frame_width, frame_depth);

        let brep = sweep_profile_along_path(&path, &profile, SweepOptions::default())
            .expect("closed window frame sweep should succeed");
        brep.validate_topology()
            .expect("closed window frame topology should validate");

        assert_window_frame_dimensions(
            &brep,
            window_width + frame_width * 2.0,
            window_width,
            window_height + frame_width * 2.0,
            window_height,
        );
    }

    #[test]
    fn larger_closed_window_frame_loop_preserves_expected_dimensions() {
        let window_width = 1.6;
        let frame_width = 0.14;
        let frame_depth = 0.2;
        let window_height = 1.2;
        let sill_height = 1.0;
        let half_window_width = window_width * 0.5;
        let half_frame_width = frame_width * 0.5;

        let path = vec![
            Vector3::new(
                -(half_window_width + half_frame_width),
                sill_height - half_frame_width,
                0.0,
            ),
            Vector3::new(
                -(half_window_width + half_frame_width),
                sill_height + window_height + half_frame_width,
                0.0,
            ),
            Vector3::new(
                half_window_width + half_frame_width,
                sill_height + window_height + half_frame_width,
                0.0,
            ),
            Vector3::new(
                half_window_width + half_frame_width,
                sill_height - half_frame_width,
                0.0,
            ),
            Vector3::new(
                -(half_window_width + half_frame_width),
                sill_height - half_frame_width,
                0.0,
            ),
        ];
        let profile = rectangle_profile(frame_width, frame_depth);

        let brep = sweep_profile_along_path(&path, &profile, SweepOptions::default())
            .expect("larger closed window frame sweep should succeed");
        brep.validate_topology()
            .expect("larger closed window frame topology should validate");

        assert_window_frame_dimensions(
            &brep,
            window_width + frame_width * 2.0,
            window_width,
            window_height + frame_width * 2.0,
            window_height,
        );
    }

    #[test]
    fn open_door_frame_path_keeps_requested_widths() {
        let panel_width = 1.0;
        let frame_width = 0.2;
        let frame_depth = 0.3;
        let door_height = 2.1;
        let half_panel_width = panel_width * 0.5;
        let half_frame_width = frame_width * 0.5;

        let path = vec![
            Vector3::new(-(half_panel_width + half_frame_width), 0.0, 0.0),
            Vector3::new(
                -(half_panel_width + half_frame_width),
                door_height + half_frame_width,
                0.0,
            ),
            Vector3::new(
                half_panel_width + half_frame_width,
                door_height + half_frame_width,
                0.0,
            ),
            Vector3::new(half_panel_width + half_frame_width, 0.0, 0.0),
        ];
        let profile = rectangle_profile(frame_width, frame_depth);

        let brep = sweep_profile_along_path(&path, &profile, SweepOptions::default())
            .expect("open door frame sweep should succeed");
        brep.validate_topology()
            .expect("open door frame topology should validate");

        let tolerance = 1.0e-6;
        let xs = unique_sorted_values(
            brep.vertices.iter().map(|vertex| vertex.position.x),
            tolerance,
        );
        let ys = unique_sorted_values(
            brep.vertices.iter().map(|vertex| vertex.position.y),
            tolerance,
        );

        assert_eq!(xs.len(), 4, "door frame should preserve four x bands");
        assert_eq!(ys.len(), 3, "door frame should preserve three y bands");
        assert!(
            ((xs[xs.len() - 1] - xs[0]) - (panel_width + frame_width * 2.0)).abs() <= tolerance,
            "door frame outer width changed unexpectedly"
        );
        assert!(
            ((xs[xs.len() - 2] - xs[1]) - panel_width).abs() <= tolerance,
            "door frame inner width changed unexpectedly"
        );
        assert!(
            ((ys[ys.len() - 1] - ys[0]) - (door_height + frame_width)).abs() <= tolerance,
            "door frame outer height changed unexpectedly"
        );
    }

    #[test]
    fn concave_profile_multibend_sweep_has_non_degenerate_faces() {
        let path = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(0.0, 1.2, 0.0),
            Vector3::new(1.3, 1.2, 0.0),
            Vector3::new(1.3, 2.0, 0.8),
            Vector3::new(2.1, 2.0, 0.8),
        ];

        let brep = sweep_profile_along_path(&path, &concave_profile(), SweepOptions::default())
            .expect("concave sweep should succeed");
        brep.validate_topology()
            .expect("concave sweep topology should validate");

        for face in &brep.faces {
            let vertices = brep.get_vertices_by_face_id(face.id);
            let face_points: Vec<Vec3f> = vertices.iter().map(Vec3f::from_vector3).collect();
            assert!(
                compute_polygon_normal(&face_points).is_some(),
                "face {} should not be degenerate",
                face.id
            );
        }
    }

    #[test]
    fn non_planar_closed_path_is_rejected() {
        let path = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
            Vector3::new(0.0, 1.0, 0.0),
            Vector3::new(0.0, 0.0, 0.0),
        ];

        let error = match sweep_profile_along_path(
            &path,
            &rectangle_profile(0.4, 0.4),
            SweepOptions::default(),
        ) {
            Ok(_) => panic!("non-planar closed path should fail"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), SweepErrorKind::NonPlanarClosedPath);
    }

    #[test]
    fn one_eighty_degree_reversal_is_rejected() {
        let path = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
            Vector3::new(0.0, -1.0, 0.0),
        ];

        let error = match sweep_profile_along_path(
            &path,
            &rectangle_profile(0.4, 0.4),
            SweepOptions::default(),
        ) {
            Ok(_) => panic!("180-degree reversal should fail"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), SweepErrorKind::PathReversal);
    }
}
