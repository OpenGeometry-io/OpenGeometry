use super::bounds::PatchBounds;
use super::error::GeometryError;
use crate::math::{add, cross, dot, finite, norm, scale, sub, Interval, Point3};
use serde::{Deserialize, Serialize};

pub(crate) type UV = [f64; 2];
pub(crate) type UVBox = [Interval; 2];

pub(crate) fn unit(a: Point3) -> Result<Point3, GeometryError> {
    let n = finite(norm(a))?;
    if n == 0.0 {
        return Err(GeometryError::SingularParameterization);
    }
    checked(a.map(|value| value / n))
}
pub(crate) fn checked(p: Point3) -> Result<Point3, GeometryError> {
    for v in p {
        finite(v)?;
    }
    Ok(p)
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame3 {
    pub origin: Point3,
    pub x: Point3,
    pub y: Point3,
    pub z: Point3,
}

impl Frame3 {
    pub const IDENTITY: Self = Self {
        origin: [0.0; 3],
        x: [1.0, 0.0, 0.0],
        y: [0.0, 1.0, 0.0],
        z: [0.0, 0.0, 1.0],
    };

    pub fn from_axis(
        origin: Point3,
        axis: Point3,
        reference: Point3,
    ) -> Result<Self, GeometryError> {
        checked(origin)?;
        let z = unit(axis)?;
        let x = unit(sub(reference, scale(z, dot(z, reference))))?;
        let y = unit(cross(z, x))?;
        let frame = Self { origin, x, y, z };
        frame.validate()?;
        Ok(frame)
    }

    pub(crate) fn validate(&self) -> Result<(), GeometryError> {
        for v in [self.origin, self.x, self.y, self.z] {
            checked(v)?;
        }
        if [self.x, self.y, self.z]
            .iter()
            .any(|&v| (norm(v) - 1.0).abs() > 1e-12)
            || dot(self.x, self.y).abs() > 1e-12
            || dot(self.x, self.z).abs() > 1e-12
            || dot(self.y, self.z).abs() > 1e-12
            || norm(sub(cross(self.x, self.y), self.z)) > 1e-12
        {
            return Err(GeometryError::InvalidGeometry(
                "frame must be right-handed and orthonormal".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn vector(&self, p: Point3) -> Point3 {
        add(
            add(scale(self.x, p[0]), scale(self.y, p[1])),
            scale(self.z, p[2]),
        )
    }
    pub fn point(&self, p: Point3) -> Point3 {
        add(self.origin, self.vector(p))
    }
    pub(crate) fn local(&self, p: Point3) -> Point3 {
        let d = sub(p, self.origin);
        [dot(d, self.x), dot(d, self.y), dot(d, self.z)]
    }

    pub(super) fn bounds(&self, local: [Interval; 3]) -> Result<PatchBounds, GeometryError> {
        let mut axes = [Interval::point(0.0)?; 3];
        for i in 0..3 {
            axes[i] = Interval::point(self.origin[i])?
                .add_interval(local[0].mul_interval(Interval::point(self.x[i])?)?)?
                .add_interval(local[1].mul_interval(Interval::point(self.y[i])?)?)?
                .add_interval(local[2].mul_interval(Interval::point(self.z[i])?)?)?;
        }
        Ok(PatchBounds { axes })
    }
}

pub(crate) const GROUND: Frame3 = Frame3 {
    origin: [0.0; 3],
    x: [1.0, 0.0, 0.0],
    y: [0.0, 0.0, -1.0],
    z: [0.0, 1.0, 0.0],
};
