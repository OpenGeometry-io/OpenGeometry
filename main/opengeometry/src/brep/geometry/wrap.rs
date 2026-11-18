use super::turned::lift;
use crate::brep::bounds::PatchBounds;
use crate::brep::error::GeometryError;
use crate::brep::frame::{checked, Frame3};
use crate::math::{add, cross, scale, Interval, Point3};
use serde::{Deserialize, Serialize};

pub(crate) const WRAP_SPLITS: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Wrap {
    pub frame: Frame3,
    pub radius: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub semi_angle: f64,
}

fn is_zero(x: &f64) -> bool {
    *x == 0.0
}

struct OnCone {
    e: f64,
    sigma: f64,
    gamma: f64,
    theta: f64,
    w: f64,
    lambda: f64,
}

impl Wrap {
    pub(crate) fn validate(&self) -> Result<(), GeometryError> {
        self.frame.validate()?;
        if !(self.radius.is_finite() && self.radius > 0.0) {
            return Err(GeometryError::InvalidGeometry(
                "wrap radius must be positive".into(),
            ));
        }
        (self.semi_angle.is_finite() && self.semi_angle.abs() < std::f64::consts::FRAC_PI_2)
            .then_some(())
            .ok_or_else(|| {
                GeometryError::InvalidGeometry(
                    "wrap semi-angle must lie within a right angle".into(),
                )
            })
    }

    fn cone(&self) -> bool {
        self.semi_angle != 0.0
    }

    fn axes(&self, p: Point3) -> [Point3; 3] {
        let (s, c) = (p[0] / self.radius).sin_cos();
        [[-s, c, 0.0], [0.0, 0.0, 1.0], [c, s, 0.0]]
    }

    fn on_cone(&self, p: Point3) -> OnCone {
        let s = self.semi_angle.sin();
        let k = s / self.radius;
        let (a, b) = (1.0 + k * p[1], k * p[0]);
        let e = a.hypot(b);
        let w = p[2] - self.radius;
        OnCone {
            e,
            sigma: b / e,
            gamma: a / e,
            theta: b.atan2(a) / s,
            w,
            lambda: 1.0 + self.semi_angle.cos() * w / (self.radius * e),
        }
    }

    #[cfg(test)]
    pub(crate) fn on_cone_theta(&self, p: Point3) -> f64 {
        self.on_cone(p).theta
    }

    fn turned(theta: f64) -> [Point3; 3] {
        let (s, c) = theta.sin_cos();
        [[c, s, 0.0], [-s, c, 0.0], [0.0, 0.0, 1.0]]
    }

    fn cone_jacobian(&self, p: Point3) -> [Point3; 3] {
        let (s, c) = self.semi_angle.sin_cos();
        let q = self.on_cone(p);
        let [r, t, a] = Self::turned(q.theta);
        let g = add(scale(r, s), scale(a, c));
        let n = add(scale(r, c), scale(a, -s));
        [
            add(scale(g, q.sigma), scale(t, q.lambda * q.gamma)),
            add(scale(g, q.gamma), scale(t, -q.lambda * q.sigma)),
            n,
        ]
    }

    pub(crate) fn regular(&self, p: Point3) -> bool {
        if !self.cone() {
            return p[2] > 0.0;
        }
        let q = self.on_cone(p);
        q.e > 0.0 && q.lambda > 0.0
    }

    pub fn point(&self, p: Point3) -> Point3 {
        if self.cone() {
            let (s, c) = self.semi_angle.sin_cos();
            let q = self.on_cone(p);
            let k = s / self.radius;
            let radial = q.e * self.radius + c * q.w;
            let rise = c * (k * (p[0] * p[0] + p[1] * p[1]) + 2.0 * p[1]) / (q.e + 1.0);
            let (st, ct) = q.theta.sin_cos();
            return self.frame.point([radial * ct, radial * st, rise - s * q.w]);
        }
        let (s, c) = (p[0] / self.radius).sin_cos();
        self.frame.point([p[2] * c, p[2] * s, p[1]])
    }

    pub(crate) fn push(&self, p: Point3, v: Point3) -> Point3 {
        if self.cone() {
            let [jx, jy, jz] = self.cone_jacobian(p);
            return self
                .frame
                .vector(add(add(scale(jx, v[0]), scale(jy, v[1])), scale(jz, v[2])));
        }
        let [t, a, r] = self.axes(p);
        let local = add(
            add(scale(t, v[0] * p[2] / self.radius), scale(a, v[1])),
            scale(r, v[2]),
        );
        self.frame.vector(local)
    }

    pub(crate) fn second(&self, p: Point3, a: Point3, b: Point3) -> Point3 {
        if self.cone() {
            return self.frame.vector(self.cone_second(p, a, b));
        }
        let [t, _, r] = self.axes(p);
        let xx = scale(r, -p[2] / (self.radius * self.radius));
        let xz = scale(t, 1.0 / self.radius);
        self.frame.vector(add(
            scale(xx, a[0] * b[0]),
            scale(xz, a[0] * b[2] + a[2] * b[0]),
        ))
    }

    fn cone_second(&self, p: Point3, a: Point3, b: Point3) -> Point3 {
        let (s, c) = self.semi_angle.sin_cos();
        let big_r = self.radius;
        let k = s / big_r;
        let q = self.on_cone(p);
        let [r, t, ax] = Self::turned(q.theta);
        let g = add(scale(r, s), scale(ax, c));
        let theta_b = (q.gamma * b[0] - q.sigma * b[1]) / (big_r * q.e);
        let lambda_b = -c * q.w * k / (big_r * q.e * q.e) * (q.sigma * b[0] + q.gamma * b[1])
            + c * b[2] / (big_r * q.e);
        let sigma_b = q.gamma * s * theta_b;
        let gamma_b = -q.sigma * s * theta_b;
        let (l, sg, gm) = (q.lambda, q.sigma, q.gamma);
        let djx = add(
            add(scale(g, sigma_b), scale(r, -l * gm * theta_b)),
            scale(t, sg * s * theta_b + lambda_b * gm + l * gamma_b),
        );
        let djy = add(
            add(scale(g, gamma_b), scale(r, l * sg * theta_b)),
            scale(t, gm * s * theta_b - lambda_b * sg - l * sigma_b),
        );
        let djz = scale(t, c * theta_b);
        add(add(scale(djx, a[0]), scale(djy, a[1])), scale(djz, a[2]))
    }

    pub(crate) fn push_normal(&self, p: Point3, n: Point3) -> Point3 {
        if self.cone() {
            let [jx, jy, jz] = self.cone_jacobian(p);
            return self.frame.vector(add(
                add(scale(cross(jy, jz), n[0]), scale(cross(jz, jx), n[1])),
                scale(cross(jx, jy), n[2]),
            ));
        }
        let [t, a, r] = self.axes(p);
        let k = p[2] / self.radius;
        self.frame.vector(add(
            add(scale(t, n[0]), scale(a, k * n[1])),
            scale(r, k * n[2]),
        ))
    }

    pub fn flatten(&self, q: Point3, hint: Option<Point3>) -> Result<Point3, GeometryError> {
        let l = self.frame.local(q);
        let z = l[0].hypot(l[1]);
        if z == 0.0 {
            return Err(GeometryError::SingularParameterization);
        }
        if self.cone() {
            let (s, c) = self.semi_angle.sin_cos();
            let big_r = self.radius;
            let theta = lift(l[1].atan2(l[0]), hint.map(|h| self.on_cone(h).theta));
            let w = c * (z - big_r) - s * l[2];
            let e = (z - c * w) / big_r;
            if e <= 0.0 {
                return Err(GeometryError::SingularParameterization);
            }
            let (sp, cp) = (theta * s).sin_cos();
            return checked([e * big_r * sp / s, big_r * (e * cp - 1.0) / s, big_r + w]);
        }
        let angle = lift(l[1].atan2(l[0]), hint.map(|h| h[0] / self.radius));
        checked([angle * self.radius, l[2], z])
    }

    fn cone_box(
        &self,
        x: Interval,
        y: Interval,
    ) -> Result<Option<(Interval, Interval)>, GeometryError> {
        let s = self.semi_angle.sin();
        let k = Interval::point(s / self.radius)?;
        let a = Interval::point(1.0)?.add_interval(y.mul_interval(k)?)?;
        let b = x.mul_interval(k)?;
        if a.lo <= 0.0 && b.contains(0.0) {
            return Ok(None);
        }
        let e = a.square()?.add_interval(b.square()?)?.sqrt()?;
        let corners =
            [(b.lo, a.lo), (b.lo, a.hi), (b.hi, a.lo), (b.hi, a.hi)].map(|(b, a)| b.atan2(a));
        let lo = corners.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = corners.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let pad = |v: f64| 4.0 * f64::EPSILON * (1.0 + v.abs());
        let (lo, hi) = if s > 0.0 {
            (lo / s, hi / s)
        } else {
            (hi / s, lo / s)
        };
        Ok(Some((e, Interval::new(lo - pad(lo), hi + pad(hi))?)))
    }

    pub(crate) fn enclose_parts<D: Copy>(
        &self,
        flat: &dyn Fn(D) -> Result<PatchBounds, GeometryError>,
        halves: &dyn Fn(D) -> [D; 2],
        domain: D,
        depth: usize,
    ) -> Result<PatchBounds, GeometryError> {
        match self.enclose(flat(domain)?) {
            Err(GeometryError::SingularParameterization) if depth > 0 => {
                let [a, b] =
                    halves(domain).map(|part| self.enclose_parts(flat, halves, part, depth - 1));
                let (a, b) = (a?, b?);
                Ok(PatchBounds {
                    axes: std::array::from_fn(|i| a.axes[i].hull(b.axes[i])),
                })
            }
            other => other,
        }
    }

    pub(crate) fn derivative_bounds_parts<D: Copy>(
        &self,
        flat: &dyn Fn(D) -> Result<PatchBounds, GeometryError>,
        halves: &dyn Fn(D) -> [D; 2],
        domain: D,
        depth: usize,
    ) -> Result<(f64, f64), GeometryError> {
        match self.derivative_bounds(flat(domain)?) {
            Err(GeometryError::SingularParameterization) if depth > 0 => {
                let [a, b] = halves(domain)
                    .map(|part| self.derivative_bounds_parts(flat, halves, part, depth - 1));
                let (a, b) = (a?, b?);
                Ok((a.0.max(b.0), a.1.max(b.1)))
            }
            other => other,
        }
    }

    pub(crate) fn enclose(&self, flat: PatchBounds) -> Result<PatchBounds, GeometryError> {
        let [x, y, z] = flat.axes;
        if self.cone() {
            let (s, c) = self.semi_angle.sin_cos();
            let big_r = self.radius;
            let point = Interval::point;
            let w = z.sub_interval(point(big_r)?)?;
            let Some((e, theta)) = self.cone_box(x, y)? else {
                return Err(GeometryError::SingularParameterization);
            };
            let radial = e
                .mul_interval(point(big_r)?)?
                .add_interval(w.mul_interval(point(c)?)?)?;
            let rise = e
                .sub_interval(point(1.0)?)?
                .mul_interval(point(c * big_r / s)?)?
                .sub_interval(w.mul_interval(point(s)?)?)?;
            let (ct, st) = (theta.cos()?, theta.sin()?);
            return self
                .frame
                .bounds([ct.mul_interval(radial)?, st.mul_interval(radial)?, rise]);
        }
        let angle = x.mul_interval(Interval::point(1.0 / self.radius)?)?;
        let (c, s) = (angle.cos()?, angle.sin()?);
        self.frame
            .bounds([c.mul_interval(z)?, s.mul_interval(z)?, y])
    }

    pub(crate) fn derivative_bounds(&self, flat: PatchBounds) -> Result<(f64, f64), GeometryError> {
        let [x, y, z] = flat.axes;
        if self.cone() {
            let (s, c) = (self.semi_angle.sin().abs(), self.semi_angle.cos());
            let big_r = self.radius;
            let Some((e, _)) = self.cone_box(x, y)? else {
                return Err(GeometryError::SingularParameterization);
            };
            if e.lo <= 0.0 {
                return Err(GeometryError::SingularParameterization);
            }
            let w = (z.lo - big_r).abs().max((z.hi - big_r).abs());
            let lambda = 1.0 + c * w / (big_r * e.lo);
            let theta = 1.0 / (big_r * e.lo);
            let dlambda = c * w * s / (big_r * big_r * e.lo * e.lo) + c / (big_r * e.lo);
            let second = theta * (2.0 * s + lambda * (s + 1.0) + c) + dlambda;
            return Ok((lambda, 3f64.sqrt() * second));
        }
        let z = z.lo.abs().max(z.hi.abs());
        Ok((
            (z / self.radius).max(1.0),
            z / (self.radius * self.radius) + 1.0 / self.radius,
        ))
    }
}
