use openmaths::{Matrix4, Vector3};
use serde::{Deserialize, Serialize};

// Keep scale bounds aligned with downstream tolerance assumptions used by
// boolean and export paths.
const MIN_UNIFORM_SCALE: f64 = 1.0e-6;
const UNIFORM_SCALE_REL_TOLERANCE: f64 = 1.0e-9;

#[derive(Clone, Serialize, Deserialize)]
pub struct Placement3D {
    pub anchor: Vector3,
    translation: Vector3,
    rotation: Vector3,
    scale: Vector3,
}

impl Default for Placement3D {
    fn default() -> Self {
        Self::new()
    }
}

impl Placement3D {
    pub fn new() -> Self {
        Self {
            anchor: Vector3::new(0.0, 0.0, 0.0),
            translation: Vector3::new(0.0, 0.0, 0.0),
            rotation: Vector3::new(0.0, 0.0, 0.0),
            scale: Vector3::new(1.0, 1.0, 1.0),
        }
    }

    pub fn translation(&self) -> Vector3 {
        self.translation
    }

    pub fn rotation(&self) -> Vector3 {
        self.rotation
    }

    pub fn scale(&self) -> Vector3 {
        self.scale
    }

    pub fn set_anchor(&mut self, anchor: Vector3) {
        self.anchor = sanitize_vector(anchor, 0.0);
    }

    pub fn set_transform(
        &mut self,
        position: Vector3,
        rotation: Vector3,
        scale: Vector3,
    ) -> Result<(), String> {
        let validated_scale = validate_uniform_positive_scale(scale)?;
        self.translation = sanitize_vector(position, 0.0);
        self.rotation = sanitize_vector(rotation, 0.0);
        self.scale = validated_scale;
        Ok(())
    }

    pub fn set_translation(&mut self, translation: Vector3) {
        self.translation = sanitize_vector(translation, 0.0);
    }

    pub fn set_rotation(&mut self, rotation: Vector3) {
        self.rotation = sanitize_vector(rotation, 0.0);
    }

    pub fn set_scale(&mut self, scale: Vector3) -> Result<(), String> {
        self.scale = validate_uniform_positive_scale(scale)?;
        Ok(())
    }

    pub fn world_matrix(&self) -> Matrix4 {
        let rotation = self.rotation;
        let scale = self.scale;
        let translation = Vector3::new(
            self.anchor.x + self.translation.x,
            self.anchor.y + self.translation.y,
            self.anchor.z + self.translation.z,
        );

        let cos_x = rotation.x.cos();
        let sin_x = rotation.x.sin();
        let cos_y = rotation.y.cos();
        let sin_y = rotation.y.sin();
        let cos_z = rotation.z.cos();
        let sin_z = rotation.z.sin();

        let m11 = cos_y * cos_z;
        let m12 = -cos_y * sin_z;
        let m13 = sin_y;

        let m21 = sin_x * sin_y * cos_z + cos_x * sin_z;
        let m22 = cos_x * cos_z - sin_x * sin_y * sin_z;
        let m23 = -sin_x * cos_y;

        let m31 = sin_x * sin_z - cos_x * sin_y * cos_z;
        let m32 = sin_x * cos_z + cos_x * sin_y * sin_z;
        let m33 = cos_x * cos_y;

        Matrix4::set(
            m11 * scale.x,
            m12 * scale.y,
            m13 * scale.z,
            translation.x,
            m21 * scale.x,
            m22 * scale.y,
            m23 * scale.z,
            translation.y,
            m31 * scale.x,
            m32 * scale.y,
            m33 * scale.z,
            translation.z,
            0.0,
            0.0,
            0.0,
            1.0,
        )
    }
}

pub fn bounds_center_from_points(points: &[Vector3]) -> Option<Vector3> {
    let first = points.first()?;
    let mut min_x = first.x;
    let mut min_y = first.y;
    let mut min_z = first.z;
    let mut max_x = first.x;
    let mut max_y = first.y;
    let mut max_z = first.z;

    for point in &points[1..] {
        min_x = min_x.min(point.x);
        min_y = min_y.min(point.y);
        min_z = min_z.min(point.z);
        max_x = max_x.max(point.x);
        max_y = max_y.max(point.y);
        max_z = max_z.max(point.z);
    }

    Some(Vector3::new(
        (min_x + max_x) * 0.5,
        (min_y + max_y) * 0.5,
        (min_z + max_z) * 0.5,
    ))
}

pub fn bounds_center_from_point_sets(point_sets: &[&[Vector3]]) -> Option<Vector3> {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut min_z = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    let mut max_z = f64::NEG_INFINITY;
    let mut has_points = false;

    for point_set in point_sets {
        for point in *point_set {
            has_points = true;
            min_x = min_x.min(point.x);
            min_y = min_y.min(point.y);
            min_z = min_z.min(point.z);
            max_x = max_x.max(point.x);
            max_y = max_y.max(point.y);
            max_z = max_z.max(point.z);
        }
    }

    if !has_points {
        return None;
    }

    Some(Vector3::new(
        (min_x + max_x) * 0.5,
        (min_y + max_y) * 0.5,
        (min_z + max_z) * 0.5,
    ))
}

pub fn points_relative_to_anchor(points: &[Vector3], anchor: Vector3) -> Vec<Vector3> {
    points
        .iter()
        .map(|point| Vector3::new(point.x - anchor.x, point.y - anchor.y, point.z - anchor.z))
        .collect()
}

pub fn transform_points_with_placement(
    points: &[Vector3],
    placement: &Placement3D,
) -> Vec<Vector3> {
    let local_points = points_relative_to_anchor(points, placement.anchor);
    let matrix = placement.world_matrix();

    local_points
        .into_iter()
        .map(|mut point| {
            point.apply_matrix4(matrix.clone());
            point
        })
        .collect()
}

fn sanitize_vector(vector: Vector3, default: f64) -> Vector3 {
    Vector3::new(
        sanitize_component(vector.x, default),
        sanitize_component(vector.y, default),
        sanitize_component(vector.z, default),
    )
}

fn sanitize_component(value: f64, default: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        default
    }
}

pub fn validate_uniform_positive_scale(scale: Vector3) -> Result<Vector3, String> {
    let components = [scale.x, scale.y, scale.z];
    if components.iter().any(|value| !value.is_finite()) {
        return Err(
            "Invalid placement scale: all scale components must be finite numbers.".to_string(),
        );
    }

    if components.iter().any(|value| *value < MIN_UNIFORM_SCALE) {
        return Err(format!(
            "Invalid placement scale: all scale components must be >= {}.",
            MIN_UNIFORM_SCALE
        ));
    }

    let max_component = components.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let min_component = components.iter().copied().fold(f64::INFINITY, f64::min);
    let tolerance = UNIFORM_SCALE_REL_TOLERANCE * max_component.max(1.0);
    if (max_component - min_component) > tolerance {
        return Err(
            "Invalid placement scale: non-uniform scale is not allowed for placement.".to_string(),
        );
    }

    Ok(scale)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::{consts::PI, INFINITY, NAN};

    fn assert_close(actual: f64, expected: f64) {
        let delta = (actual - expected).abs();
        assert!(
            delta <= 1.0e-9,
            "expected {expected}, got {actual}, delta {delta}"
        );
    }

    fn assert_vec_close(actual: Vector3, expected: Vector3) {
        assert_close(actual.x, expected.x);
        assert_close(actual.y, expected.y);
        assert_close(actual.z, expected.z);
    }

    fn multiply_quaternions(
        lhs: (f64, f64, f64, f64),
        rhs: (f64, f64, f64, f64),
    ) -> (f64, f64, f64, f64) {
        (
            lhs.3 * rhs.0 + lhs.0 * rhs.3 + lhs.1 * rhs.2 - lhs.2 * rhs.1,
            lhs.3 * rhs.1 - lhs.0 * rhs.2 + lhs.1 * rhs.3 + lhs.2 * rhs.0,
            lhs.3 * rhs.2 + lhs.0 * rhs.1 - lhs.1 * rhs.0 + lhs.2 * rhs.3,
            lhs.3 * rhs.3 - lhs.0 * rhs.0 - lhs.1 * rhs.1 - lhs.2 * rhs.2,
        )
    }

    fn rotate_point_with_xyz_quaternion(point: Vector3, rotation: Vector3) -> Vector3 {
        let half_x = rotation.x * 0.5;
        let half_y = rotation.y * 0.5;
        let half_z = rotation.z * 0.5;

        let qx = (half_x.sin(), 0.0, 0.0, half_x.cos());
        let qy = (0.0, half_y.sin(), 0.0, half_y.cos());
        let qz = (0.0, 0.0, half_z.sin(), half_z.cos());
        let quaternion = multiply_quaternions(multiply_quaternions(qx, qy), qz);

        let u = Vector3::new(quaternion.0, quaternion.1, quaternion.2);
        let s = quaternion.3;
        let dot = u.x * point.x + u.y * point.y + u.z * point.z;
        let cross = Vector3::new(
            u.y * point.z - u.z * point.y,
            u.z * point.x - u.x * point.z,
            u.x * point.y - u.y * point.x,
        );
        let uu = u.x * u.x + u.y * u.y + u.z * u.z;

        Vector3::new(
            2.0 * dot * u.x + (s * s - uu) * point.x + 2.0 * s * cross.x,
            2.0 * dot * u.y + (s * s - uu) * point.y + 2.0 * s * cross.y,
            2.0 * dot * u.z + (s * s - uu) * point.z + 2.0 * s * cross.z,
        )
    }

    #[test]
    fn set_transform_rejects_non_finite_non_positive_and_non_uniform_scale() {
        let mut placement = Placement3D::new();
        placement.set_anchor(Vector3::new(NAN, INFINITY, -INFINITY));
        let non_finite = placement.set_transform(
            Vector3::new(NAN, 2.5, INFINITY),
            Vector3::new(-INFINITY, PI * 0.25, NAN),
            Vector3::new(1.0, f64::NAN, 1.0),
        );
        assert!(non_finite.is_err());

        let non_positive = placement.set_transform(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(-1.0, -1.0, -1.0),
        );
        assert!(non_positive.is_err());

        let non_uniform = placement.set_transform(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 1.25, 1.0),
        );
        assert!(non_uniform.is_err());
    }

    #[test]
    fn set_transform_sanitizes_non_finite_translation_and_rotation_for_valid_uniform_scale() {
        let mut placement = Placement3D::new();
        placement.set_anchor(Vector3::new(NAN, INFINITY, -INFINITY));
        placement
            .set_transform(
                Vector3::new(NAN, 2.5, INFINITY),
                Vector3::new(-INFINITY, PI * 0.25, NAN),
                Vector3::new(1.2, 1.2, 1.2),
            )
            .expect("uniform positive scale should be accepted");

        assert_vec_close(placement.anchor, Vector3::new(0.0, 0.0, 0.0));
        assert_vec_close(placement.translation(), Vector3::new(0.0, 2.5, 0.0));
        assert_vec_close(placement.rotation(), Vector3::new(0.0, PI * 0.25, 0.0));
        assert_vec_close(placement.scale(), Vector3::new(1.2, 1.2, 1.2));
    }

    #[test]
    fn world_matrix_applies_translation_rotation_and_scale_in_xyz_order() {
        let mut placement = Placement3D::new();
        placement.set_anchor(Vector3::new(10.0, -3.0, 4.0));
        placement
            .set_transform(
                Vector3::new(1.0, 2.0, -5.0),
                Vector3::new(0.4, -0.3, 0.25),
                Vector3::new(1.5, 1.5, 1.5),
            )
            .expect("uniform positive scale should be accepted");

        let local_point = Vector3::new(1.25, -2.0, 0.5);
        let mut transformed = local_point;
        transformed.apply_matrix4(placement.world_matrix());

        let scaled = Vector3::new(
            local_point.x * placement.scale().x,
            local_point.y * placement.scale().y,
            local_point.z * placement.scale().z,
        );
        let rotated = rotate_point_with_xyz_quaternion(scaled, placement.rotation());
        let expected = Vector3::new(
            placement.anchor.x + placement.translation().x + rotated.x,
            placement.anchor.y + placement.translation().y + rotated.y,
            placement.anchor.z + placement.translation().z + rotated.z,
        );

        assert_vec_close(transformed, expected);
    }
}
