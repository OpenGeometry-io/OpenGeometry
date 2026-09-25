use crate::analytic::geometry::{add, dot, scale};
use crate::analytic::{Frame3, GeometryError, Point3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Similarity3 {
    pub frame: Frame3,
    pub scale: f64,
}

impl Default for Similarity3 {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Similarity3 {
    pub const IDENTITY: Self = Self {
        frame: Frame3::IDENTITY,
        scale: 1.0,
    };

    pub fn validate(&self) -> Result<(), GeometryError> {
        self.frame.validate()?;
        if !self.scale.is_finite() || self.scale <= 0.0 {
            return Err(GeometryError::InvalidGeometry(
                "similarity scale must be finite and positive".into(),
            ));
        }
        Ok(())
    }

    pub fn apply_point(&self, point: Point3) -> Point3 {
        add(self.frame.origin, self.apply_vector(point))
    }

    pub fn apply_vector(&self, vector: Point3) -> Point3 {
        scale(self.frame.vector(vector), self.scale)
    }

    pub fn compose(&self, child: &Self) -> Self {
        Self {
            frame: Frame3 {
                origin: self.apply_point(child.frame.origin),
                x: self.frame.vector(child.frame.x),
                y: self.frame.vector(child.frame.y),
                z: self.frame.vector(child.frame.z),
            },
            scale: self.scale * child.scale,
        }
    }

    pub fn inverse(&self) -> Self {
        let frame = &self.frame;
        let inverse_scale = 1.0 / self.scale;
        let rotated_origin = [
            dot(frame.origin, frame.x),
            dot(frame.origin, frame.y),
            dot(frame.origin, frame.z),
        ];
        Self {
            frame: Frame3 {
                origin: scale(rotated_origin, -inverse_scale),
                x: [frame.x[0], frame.y[0], frame.z[0]],
                y: [frame.x[1], frame.y[1], frame.z[1]],
                z: [frame.x[2], frame.y[2], frame.z[2]],
            },
            scale: inverse_scale,
        }
    }

    pub fn to_column_major(&self) -> [f64; 16] {
        let x = scale(self.frame.x, self.scale);
        let y = scale(self.frame.y, self.scale);
        let z = scale(self.frame.z, self.scale);
        let o = self.frame.origin;
        [
            x[0], x[1], x[2], 0.0, y[0], y[1], y[2], 0.0, z[0], z[1], z[2], 0.0, o[0], o[1], o[2],
            1.0,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turn_about_y(origin: Point3, scale: f64) -> Similarity3 {
        Similarity3 {
            frame: Frame3 {
                origin,
                x: [0.0, 0.0, -1.0],
                y: [0.0, 1.0, 0.0],
                z: [1.0, 0.0, 0.0],
            },
            scale,
        }
    }

    fn assert_point_close(actual: Point3, expected: Point3) {
        for axis in 0..3 {
            assert!(
                (actual[axis] - expected[axis]).abs() < 1e-12,
                "{actual:?} != {expected:?}"
            );
        }
    }

    #[test]
    fn inverse_undoes_the_transform() {
        let transform = turn_about_y([2.0, -1.0, 3.0], 2.5);
        let point = [0.3, 0.7, -1.9];
        assert_point_close(
            transform
                .inverse()
                .apply_point(transform.apply_point(point)),
            point,
        );
        assert_point_close(
            transform.compose(&transform.inverse()).apply_point(point),
            point,
        );
    }

    #[test]
    fn compose_applies_the_child_first() {
        let parent = turn_about_y([10.0, 0.0, 0.0], 1.0);
        let child = Similarity3 {
            frame: Frame3 {
                origin: [1.0, 0.0, 0.0],
                ..Frame3::IDENTITY
            },
            scale: 2.0,
        };
        let point = [0.5, 1.0, 0.0];
        assert_point_close(
            parent.compose(&child).apply_point(point),
            parent.apply_point(child.apply_point(point)),
        );
        assert_point_close(parent.compose(&child).apply_point(point), [10.0, 2.0, -2.0]);
    }

    #[test]
    fn composition_is_associative() {
        let a = turn_about_y([1.0, 2.0, 3.0], 1.5);
        let b = turn_about_y([-4.0, 0.5, 2.0], 0.5);
        let c = turn_about_y([0.0, -3.0, 1.0], 3.0);
        let point = [0.25, -0.5, 0.75];
        assert_point_close(
            a.compose(&b).compose(&c).apply_point(point),
            a.compose(&b.compose(&c)).apply_point(point),
        );
    }

    #[test]
    fn validate_rejects_non_positive_scale_and_skewed_frames() {
        assert!(turn_about_y([0.0; 3], 0.0).validate().is_err());
        assert!(turn_about_y([0.0; 3], f64::NAN).validate().is_err());
        let skewed = Similarity3 {
            frame: Frame3 {
                x: [1.0, 0.1, 0.0],
                ..Frame3::IDENTITY
            },
            scale: 1.0,
        };
        assert!(skewed.validate().is_err());
        assert!(turn_about_y([0.0; 3], 2.0).validate().is_ok());
    }

    #[test]
    fn column_major_matrix_matches_apply_point() {
        let transform = turn_about_y([2.0, -1.0, 3.0], 2.0);
        let m = transform.to_column_major();
        let p = [0.3, 0.7, -1.9];
        let by_matrix = [
            m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12],
            m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13],
            m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14],
        ];
        assert_point_close(by_matrix, transform.apply_point(p));
    }
}
