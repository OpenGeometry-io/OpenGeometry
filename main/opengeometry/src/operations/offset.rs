use openmaths::Vector3;
use serde::{Deserialize, Serialize};

const EPSILON: f64 = 1.0e-9;

#[derive(Clone, Copy)]
pub struct OffsetOptions {
    pub bevel: bool,
    pub acute_threshold_degrees: f64,
}

impl Default for OffsetOptions {
    fn default() -> Self {
        OffsetOptions {
            bevel: true,
            acute_threshold_degrees: 35.0,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct OffsetResult {
    pub points: Vec<Vector3>,
    pub beveled_vertex_indices: Vec<u32>,
    pub is_closed: bool,
}

impl OffsetResult {
    pub fn empty(is_closed: bool) -> Self {
        OffsetResult {
            points: Vec::new(),
            beveled_vertex_indices: Vec::new(),
            is_closed,
        }
    }
}

#[derive(Clone, Copy)]
struct Point2 {
    x: f64,
    z: f64,
}

impl Point2 {
    fn new(x: f64, z: f64) -> Self {
        Point2 { x, z }
    }

    fn from_vec3(v: Vector3) -> Self {
        Point2::new(v.x, v.z)
    }

    fn add(self, other: Point2) -> Point2 {
        Point2::new(self.x + other.x, self.z + other.z)
    }

    fn sub(self, other: Point2) -> Point2 {
        Point2::new(self.x - other.x, self.z - other.z)
    }

    fn scale(self, scalar: f64) -> Point2 {
        Point2::new(self.x * scalar, self.z * scalar)
    }

    fn dot(self, other: Point2) -> f64 {
        self.x * other.x + self.z * other.z
    }

    fn cross(self, other: Point2) -> f64 {
        self.x * other.z - self.z * other.x
    }

    fn length(self) -> f64 {
        (self.x * self.x + self.z * self.z).sqrt()
    }

    fn normalize(self) -> Option<Point2> {
        let len = self.length();
        if len <= EPSILON {
            None
        } else {
            Some(self.scale(1.0 / len))
        }
    }
}

pub fn offset_path(
    points: &[Vector3],
    distance: f64,
    force_closed: Option<bool>,
    options: OffsetOptions,
) -> OffsetResult {
    let (clean_points, is_closed) = sanitize_path(points, force_closed);

    if clean_points.len() < 2 {
        return OffsetResult::empty(is_closed);
    }

    if is_closed && clean_points.len() < 3 {
        return OffsetResult::empty(is_closed);
    }

    if distance.abs() <= EPSILON {
        let mut result = OffsetResult {
            points: clean_points,
            beveled_vertex_indices: Vec::new(),
            is_closed,
        };
        close_result_if_needed(&mut result);
        return result;
    }

    let segment_count = if is_closed {
        clean_points.len()
    } else {
        clean_points.len() - 1
    };

    let mut segment_dirs: Vec<Point2> = Vec::with_capacity(segment_count);
    let mut segment_normals: Vec<Point2> = Vec::with_capacity(segment_count);

    for segment_index in 0..segment_count {
        let i0 = segment_index;
        let i1 = if is_closed {
            (segment_index + 1) % clean_points.len()
        } else {
            segment_index + 1
        };

        let start = Point2::from_vec3(clean_points[i0]);
        let end = Point2::from_vec3(clean_points[i1]);
        let dir = end.sub(start);

        let Some(unit_dir) = dir.normalize() else {
            return OffsetResult::empty(is_closed);
        };

        let left_normal = Point2::new(-unit_dir.z, unit_dir.x);
        segment_dirs.push(unit_dir);
        segment_normals.push(left_normal);
    }

    let mut result = OffsetResult::empty(is_closed);

    if !is_closed {
        let first = clean_points[0];
        let first_offset = Point2::from_vec3(first).add(segment_normals[0].scale(distance));
        push_unique(
            &mut result.points,
            Vector3::new(first_offset.x, first.y, first_offset.z),
        );

        for vertex_index in 1..(clean_points.len() - 1) {
            append_offset_corner(
                &clean_points,
                &segment_dirs,
                &segment_normals,
                false,
                vertex_index,
                distance,
                options,
                &mut result,
            );
        }

        let last = clean_points[clean_points.len() - 1];
        let last_offset =
            Point2::from_vec3(last).add(segment_normals[segment_normals.len() - 1].scale(distance));
        push_unique(
            &mut result.points,
            Vector3::new(last_offset.x, last.y, last_offset.z),
        );

        close_result_if_needed(&mut result);
        return result;
    }

    for vertex_index in 0..clean_points.len() {
        append_offset_corner(
            &clean_points,
            &segment_dirs,
            &segment_normals,
            true,
            vertex_index,
            distance,
            options,
            &mut result,
        );
    }

    close_result_if_needed(&mut result);
    result
}

fn sanitize_path(points: &[Vector3], force_closed: Option<bool>) -> (Vec<Vector3>, bool) {
    let mut clean: Vec<Vector3> = Vec::new();

    for point in points {
        if clean
            .last()
            .map(|last| are_close_3d(*last, *point))
            .unwrap_or(false)
        {
            continue;
        }
        clean.push(*point);
    }

    if clean.is_empty() {
        return (clean, force_closed.unwrap_or(false));
    }

    let mut is_closed = force_closed.unwrap_or(false);
    if force_closed.is_none() && clean.len() >= 3 {
        is_closed = are_close_3d(clean[0], clean[clean.len() - 1]);
    }

    if is_closed && clean.len() >= 2 && are_close_3d(clean[0], clean[clean.len() - 1]) {
        clean.pop();
    }

    (clean, is_closed)
}

fn append_offset_corner(
    clean_points: &[Vector3],
    segment_dirs: &[Point2],
    segment_normals: &[Point2],
    is_closed_path: bool,
    vertex_index: usize,
    distance: f64,
    options: OffsetOptions,
    result: &mut OffsetResult,
) {
    let point = clean_points[vertex_index];
    let corner = Point2::from_vec3(point);

    let prev_segment = if vertex_index == 0 {
        segment_dirs.len() - 1
    } else {
        vertex_index - 1
    };
    let next_segment = vertex_index % segment_dirs.len();

    let prev_dir = segment_dirs[prev_segment];
    let next_dir = segment_dirs[next_segment];

    let prev_normal = segment_normals[prev_segment];
    let next_normal = segment_normals[next_segment];

    let prev_offset_anchor = corner.add(prev_normal.scale(distance));
    let next_offset_anchor = corner.add(next_normal.scale(distance));

    let dot = prev_dir.dot(next_dir).clamp(-1.0, 1.0);
    let turn_angle = dot.acos();
    let interior_angle = std::f64::consts::PI - turn_angle;
    let threshold = options
        .acute_threshold_degrees
        .clamp(1.0, 179.0)
        .to_radians();

    let turn_cross = prev_dir.cross(next_dir);
    let nearly_collinear = turn_cross.abs() <= 1.0e-7;

    if nearly_collinear && dot > 0.9999 {
        push_unique(
            &mut result.points,
            Vector3::new(prev_offset_anchor.x, point.y, prev_offset_anchor.z),
        );
        return;
    }

    let turn_sign = turn_cross * distance;
    let is_outer_corner = turn_sign > EPSILON;
    let is_inner_corner = turn_sign < -EPSILON;

    // For open paths, the inner side of a turn should be clipped between the
    // two segment offsets; mitering this case creates long spikes/triangles.
    if !is_closed_path && is_inner_corner {
        push_unique(
            &mut result.points,
            Vector3::new(prev_offset_anchor.x, point.y, prev_offset_anchor.z),
        );
        push_unique(
            &mut result.points,
            Vector3::new(next_offset_anchor.x, point.y, next_offset_anchor.z),
        );
        return;
    }
    let bevel_due_to_angle = options.bevel && is_outer_corner && interior_angle <= threshold;

    if bevel_due_to_angle {
        push_unique(
            &mut result.points,
            Vector3::new(prev_offset_anchor.x, point.y, prev_offset_anchor.z),
        );
        push_unique(
            &mut result.points,
            Vector3::new(next_offset_anchor.x, point.y, next_offset_anchor.z),
        );
        result.beveled_vertex_indices.push(vertex_index as u32);
        return;
    }

    if let Some(intersection) =
        line_intersection_2d(prev_offset_anchor, prev_dir, next_offset_anchor, next_dir)
    {
        push_unique(
            &mut result.points,
            Vector3::new(intersection.x, point.y, intersection.z),
        );
    } else if options.bevel && is_outer_corner {
        push_unique(
            &mut result.points,
            Vector3::new(prev_offset_anchor.x, point.y, prev_offset_anchor.z),
        );
        push_unique(
            &mut result.points,
            Vector3::new(next_offset_anchor.x, point.y, next_offset_anchor.z),
        );
        result.beveled_vertex_indices.push(vertex_index as u32);
    } else {
        let midpoint = Point2::new(
            (prev_offset_anchor.x + next_offset_anchor.x) * 0.5,
            (prev_offset_anchor.z + next_offset_anchor.z) * 0.5,
        );
        push_unique(
            &mut result.points,
            Vector3::new(midpoint.x, point.y, midpoint.z),
        );
    }
}

fn close_result_if_needed(result: &mut OffsetResult) {
    if !result.is_closed || result.points.len() < 2 {
        return;
    }

    let first = result.points[0];
    let last = result.points[result.points.len() - 1];
    if !are_close_3d(first, last) {
        result.points.push(first);
    }
}

fn line_intersection_2d(p1: Point2, d1: Point2, p2: Point2, d2: Point2) -> Option<Point2> {
    let denom = d1.cross(d2);
    if denom.abs() <= EPSILON {
        return None;
    }

    let delta = p2.sub(p1);
    let t = delta.cross(d2) / denom;
    Some(p1.add(d1.scale(t)))
}

fn are_close_3d(a: Vector3, b: Vector3) -> bool {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    let dz = a.z - b.z;
    (dx * dx + dy * dy + dz * dz) <= EPSILON * EPSILON
}

fn push_unique(points: &mut Vec<Vector3>, candidate: Vector3) {
    if points
        .last()
        .map(|last| are_close_3d(*last, candidate))
        .unwrap_or(false)
    {
        return;
    }
    points.push(candidate);
}

#[cfg(test)]
mod tests {
    use super::{offset_path, OffsetOptions};
    use openmaths::Vector3;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1.0e-6
    }

    #[test]
    fn line_offset_generates_parallel_points() {
        let input = vec![Vector3::new(0.0, 0.0, 0.0), Vector3::new(5.0, 0.0, 0.0)];

        let result = offset_path(&input, 1.0, Some(false), OffsetOptions::default());

        assert_eq!(result.points.len(), 2);
        assert!((result.points[0].z - 1.0).abs() < 1.0e-6);
        assert!((result.points[1].z - 1.0).abs() < 1.0e-6);
    }

    #[test]
    fn acute_corner_can_beveled() {
        let input = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(2.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.3),
        ];

        let result = offset_path(
            &input,
            0.3,
            Some(false),
            OffsetOptions {
                bevel: true,
                acute_threshold_degrees: 45.0,
            },
        );

        assert!(result.points.len() >= 4);
        assert!(!result.beveled_vertex_indices.is_empty());
    }

    #[test]
    fn closed_offset_keeps_closed_flag() {
        let input = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(2.0, 0.0, 0.0),
            Vector3::new(2.0, 0.0, 2.0),
            Vector3::new(0.0, 0.0, 2.0),
            Vector3::new(0.0, 0.0, 0.0),
        ];

        let result = offset_path(&input, 0.2, None, OffsetOptions::default());

        assert!(result.is_closed);
        assert!(result.points.len() >= 5);
        assert!((result.points[0].x - result.points[result.points.len() - 1].x).abs() < 1.0e-6);
        assert!((result.points[0].z - result.points[result.points.len() - 1].z).abs() < 1.0e-6);
    }

    #[test]
    fn inner_acute_corner_is_not_beveled() {
        let input = vec![
            Vector3::new(-1.2, 0.0, -2.4),
            Vector3::new(0.2, 0.0, -1.7),
            Vector3::new(0.8, 0.0, -0.6),
            Vector3::new(0.0, 0.0, -0.3),
            Vector3::new(2.4, 0.0, 1.2),
        ];

        let result = offset_path(
            &input,
            0.45,
            Some(false),
            OffsetOptions {
                bevel: true,
                acute_threshold_degrees: 90.0,
            },
        );

        assert!(result.beveled_vertex_indices.contains(&2));
        assert!(!result.beveled_vertex_indices.contains(&3));
    }

    #[test]
    fn open_path_inner_corner_is_clipped() {
        let input = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(2.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 1.0),
            Vector3::new(3.0, 0.0, 1.0),
        ];

        let result = offset_path(
            &input,
            0.5,
            Some(false),
            OffsetOptions {
                bevel: false,
                acute_threshold_degrees: 1.0,
            },
        );

        // Start + outer corner + two clipped inner-corner anchors + end.
        assert_eq!(result.points.len(), 5);

        let sqrt2 = 2.0_f64.sqrt();
        let expected_prev_anchor_x = 1.0 - 0.5 / sqrt2;
        let expected_prev_anchor_z = 1.0 - 0.5 / sqrt2;

        assert!(approx_eq(result.points[2].x, expected_prev_anchor_x));
        assert!(approx_eq(result.points[2].z, expected_prev_anchor_z));
        assert!(approx_eq(result.points[3].x, 1.0));
        assert!(approx_eq(result.points[3].z, 1.5));
    }
}
