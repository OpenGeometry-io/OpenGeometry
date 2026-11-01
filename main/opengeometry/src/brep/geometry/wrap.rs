use super::turned::lift;
use crate::brep::bounds::PatchBounds;
use crate::brep::error::GeometryError;
use crate::brep::frame::{checked, Frame3};
use crate::math::{add, scale, Interval, Point3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Wrap {
    pub frame: Frame3,
    pub radius: f64,
}

impl Wrap {
    pub(crate) fn validate(&self) -> Result<(), GeometryError> {
        self.frame.validate()?;
        (self.radius.is_finite() && self.radius > 0.0)
            .then_some(())
            .ok_or_else(|| GeometryError::InvalidGeometry("wrap radius must be positive".into()))
    }

    fn axes(&self, p: Point3) -> [Point3; 3] {
        let (s, c) = (p[0] / self.radius).sin_cos();
        [[-s, c, 0.0], [0.0, 0.0, 1.0], [c, s, 0.0]]
    }

    pub fn point(&self, p: Point3) -> Point3 {
        let (s, c) = (p[0] / self.radius).sin_cos();
        self.frame.point([p[2] * c, p[2] * s, p[1]])
    }

    pub(crate) fn push(&self, p: Point3, v: Point3) -> Point3 {
        let [t, a, r] = self.axes(p);
        let local = add(
            add(scale(t, v[0] * p[2] / self.radius), scale(a, v[1])),
            scale(r, v[2]),
        );
        self.frame.vector(local)
    }

    pub(crate) fn second(&self, p: Point3, a: Point3, b: Point3) -> Point3 {
        let [t, _, r] = self.axes(p);
        let xx = scale(r, -p[2] / (self.radius * self.radius));
        let xz = scale(t, 1.0 / self.radius);
        self.frame.vector(add(
            scale(xx, a[0] * b[0]),
            scale(xz, a[0] * b[2] + a[2] * b[0]),
        ))
    }

    pub(crate) fn push_normal(&self, p: Point3, n: Point3) -> Point3 {
        let [t, a, r] = self.axes(p);
        let k = p[2] / self.radius;
        self.frame.vector(add(
            add(scale(t, n[0]), scale(a, k * n[1])),
            scale(r, k * n[2]),
        ))
    }

    pub fn flatten(&self, q: Point3, hint_x: Option<f64>) -> Result<Point3, GeometryError> {
        let l = self.frame.local(q);
        let z = l[0].hypot(l[1]);
        if z == 0.0 {
            return Err(GeometryError::SingularParameterization);
        }
        let angle = lift(l[1].atan2(l[0]), hint_x.map(|x| x / self.radius));
        checked([angle * self.radius, l[2], z])
    }

    pub(crate) fn enclose(&self, flat: PatchBounds) -> Result<PatchBounds, GeometryError> {
        let [x, y, z] = flat.axes;
        let angle = x.mul_interval(Interval::point(1.0 / self.radius)?)?;
        let (c, s) = (angle.cos()?, angle.sin()?);
        self.frame
            .bounds([c.mul_interval(z)?, s.mul_interval(z)?, y])
    }

    pub(crate) fn derivative_bounds(&self, z_max: f64) -> (f64, f64) {
        let z = z_max.abs();
        (
            (z / self.radius).max(1.0),
            z / (self.radius * self.radius) + 1.0 / self.radius,
        )
    }
}
