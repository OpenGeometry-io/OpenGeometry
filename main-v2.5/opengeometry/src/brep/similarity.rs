use super::error::GeometryError;
use super::frame::{checked, unit, Frame3};
use crate::math::{add, cross, dot, norm, scale, Point3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Similarity3 {
    pub frame: Frame3,
    pub(crate) scale: f64,
}

impl Frame3 {
    fn reorthonormalized(self) -> Self {
        let x = scale(self.x, 1.0 / norm(self.x));
        let z = cross(x, self.y);
        let z = scale(z, 1.0 / norm(z));
        let y = cross(z, x);
        let snap = |v: f64| {
            if v.abs() <= 4.0 * f64::EPSILON {
                0.0
            } else if (v.abs() - 1.0).abs() <= 4.0 * f64::EPSILON {
                v.signum()
            } else {
                v
            }
        };
        Self {
            origin: self.origin,
            x: x.map(snap),
            y: y.map(snap),
            z: z.map(snap),
        }
    }
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

    pub(crate) fn from_axis_angle(
        origin: Point3,
        axis: Point3,
        angle: f64,
        scale_factor: f64,
    ) -> Result<Self, GeometryError> {
        if !angle.is_finite() {
            return Err(GeometryError::InvalidGeometry(
                "rotation angle must be finite".into(),
            ));
        }
        Self::from_axis_cos_sin(origin, axis, angle.cos(), angle.sin(), scale_factor)
    }

    pub(crate) fn from_axis_cos_sin(
        origin: Point3,
        axis: Point3,
        cos: f64,
        sin: f64,
        scale_factor: f64,
    ) -> Result<Self, GeometryError> {
        checked(origin)?;
        let axis = unit(axis)?;
        let rotate = |vector: Point3| {
            add(
                add(scale(vector, cos), scale(cross(axis, vector), sin)),
                scale(axis, dot(axis, vector) * (1.0 - cos)),
            )
        };
        let result = Self {
            frame: Frame3 {
                origin,
                x: rotate([1.0, 0.0, 0.0]),
                y: rotate([0.0, 1.0, 0.0]),
                z: rotate([0.0, 0.0, 1.0]),
            }
            .reorthonormalized(),
            scale: scale_factor,
        };
        result.validate()?;
        Ok(result)
    }

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

    pub(crate) fn apply_vector(&self, vector: Point3) -> Point3 {
        scale(self.frame.vector(vector), self.scale)
    }

    pub(crate) fn compose(&self, child: &Self) -> Self {
        Self {
            frame: Frame3 {
                origin: self.apply_point(child.frame.origin),
                x: self.frame.vector(child.frame.x),
                y: self.frame.vector(child.frame.y),
                z: self.frame.vector(child.frame.z),
            }
            .reorthonormalized(),
            scale: self.scale * child.scale,
        }
    }

    pub(crate) fn inverse(&self) -> Self {
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
