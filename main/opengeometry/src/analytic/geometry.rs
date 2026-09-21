use super::GeometryError;
use crate::math::{finite, interval::Interval};
use serde::{Deserialize, Serialize};

pub type Point3 = [f64; 3];
pub type UV = [f64; 2];
pub type UVBox = [Interval; 2];

pub(crate) fn add(a: Point3, b: Point3) -> Point3 {
    std::array::from_fn(|i| a[i] + b[i])
}
pub(crate) fn sub(a: Point3, b: Point3) -> Point3 {
    std::array::from_fn(|i| a[i] - b[i])
}
pub(crate) fn scale(a: Point3, s: f64) -> Point3 {
    a.map(|v| v * s)
}
pub(crate) fn dot(a: Point3, b: Point3) -> f64 {
    a[0].mul_add(b[0], a[1].mul_add(b[1], a[2] * b[2]))
}
pub(crate) fn cross(a: Point3, b: Point3) -> Point3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
pub(crate) fn norm(a: Point3) -> f64 {
    a[0].hypot(a[1]).hypot(a[2])
}
pub(crate) fn unit(a: Point3) -> Result<Point3, GeometryError> {
    let n = finite(norm(a))?;
    if n == 0.0 {
        return Err(GeometryError::SingularParameterization);
    }
    checked(a.map(|value| value / n))
}
fn checked(p: Point3) -> Result<Point3, GeometryError> {
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

    pub fn validate(&self) -> Result<(), GeometryError> {
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

    pub fn vector(&self, p: Point3) -> Point3 {
        add(
            add(scale(self.x, p[0]), scale(self.y, p[1])),
            scale(self.z, p[2]),
        )
    }
    pub fn point(&self, p: Point3) -> Point3 {
        add(self.origin, self.vector(p))
    }
    pub fn local(&self, p: Point3) -> Point3 {
        let d = sub(p, self.origin);
        [dot(d, self.x), dot(d, self.y), dot(d, self.z)]
    }

    fn bounds(&self, local: [Interval; 3]) -> Result<PatchBounds, GeometryError> {
        let mut axes = [Interval::point(0.0)?; 3];
        for i in 0..3 {
            axes[i] = Interval::point(self.origin[i])?
                .add(local[0].mul(Interval::point(self.x[i])?)?)?
                .add(local[1].mul(Interval::point(self.y[i])?)?)?
                .add(local[2].mul(Interval::point(self.z[i])?)?)?;
        }
        Ok(PatchBounds { axes })
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct PatchBounds {
    pub axes: [Interval; 3],
}
impl PatchBounds {
    pub fn contains(&self, point: Point3) -> bool {
        (0..3).all(|i| self.axes[i].contains(point[i]))
    }
    pub fn overlaps(&self, other: &Self) -> bool {
        (0..3).all(|i| self.axes[i].overlaps(other.axes[i]))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SurfaceChart {
    pub domain: [[f64; 2]; 2],
    pub periods: [Option<f64>; 2],
    pub singular_v: &'static [f64],
}
const PLANE_CHART: [SurfaceChart; 1] = [SurfaceChart {
    domain: [[-f64::MAX, f64::MAX]; 2],
    periods: [None, None],
    singular_v: &[],
}];
const SPHERE_CHART: [SurfaceChart; 1] = [SurfaceChart {
    domain: [
        [0.0, std::f64::consts::TAU],
        [-std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2],
    ],
    periods: [Some(std::f64::consts::TAU), None],
    singular_v: &[-std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2],
}];
const CYLINDER_CHART: [SurfaceChart; 1] = [SurfaceChart {
    domain: [[0.0, std::f64::consts::TAU], [-f64::MAX, f64::MAX]],
    periods: [Some(std::f64::consts::TAU), None],
    singular_v: &[],
}];
const CONE_CHART: [SurfaceChart; 1] = [SurfaceChart {
    domain: [[0.0, std::f64::consts::TAU], [0.0, f64::MAX]],
    periods: [Some(std::f64::consts::TAU), None],
    singular_v: &[0.0],
}];
const TORUS_CHART: [SurfaceChart; 1] = [SurfaceChart {
    domain: [[0.0, std::f64::consts::TAU]; 2],
    periods: [Some(std::f64::consts::TAU); 2],
    singular_v: &[],
}];

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum SurfaceGeometry {
    Plane {
        frame: Frame3,
    },
    Sphere {
        frame: Frame3,
        radius: f64,
    },
    Cylinder {
        frame: Frame3,
        radius: f64,
    },
    Cone {
        frame: Frame3,
        semi_angle: f64,
    },
    Torus {
        frame: Frame3,
        major_radius: f64,
        minor_radius: f64,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct SurfaceJet2 {
    pub point: Point3,
    pub du: Point3,
    pub dv: Point3,
    pub duu: Point3,
    pub duv: Point3,
    pub dvv: Point3,
}

pub trait Surface {
    fn point_at(&self, uv: UV) -> Result<Point3, GeometryError>;
    fn normal_at(&self, uv: UV) -> Result<Point3, GeometryError>;
    fn derivatives(&self, uv: UV) -> Result<SurfaceJet2, GeometryError>;
    fn project(&self, p: Point3, hint: Option<UV>) -> Result<UV, GeometryError>;
    fn charts(&self) -> &[SurfaceChart];
    fn enclose(&self, domain: UVBox) -> Result<PatchBounds, GeometryError>;
}

impl SurfaceGeometry {
    pub fn frame(&self) -> &Frame3 {
        match self {
            Self::Plane { frame }
            | Self::Sphere { frame, .. }
            | Self::Cylinder { frame, .. }
            | Self::Cone { frame, .. }
            | Self::Torus { frame, .. } => frame,
        }
    }
    pub fn validate(&self) -> Result<(), GeometryError> {
        self.frame().validate()?;
        let valid = match self {
            Self::Plane { .. } => true,
            Self::Sphere { radius, .. } | Self::Cylinder { radius, .. } => {
                radius.is_finite() && *radius > 0.0
            }
            Self::Cone { semi_angle, .. } => {
                semi_angle.is_finite()
                    && *semi_angle > 0.0
                    && *semi_angle < std::f64::consts::FRAC_PI_2
            }
            Self::Torus {
                major_radius,
                minor_radius,
                ..
            } => {
                major_radius.is_finite()
                    && minor_radius.is_finite()
                    && *minor_radius > 0.0
                    && major_radius > minor_radius
            }
        };
        if !valid {
            return Err(GeometryError::InvalidGeometry(
                "invalid radius, cone angle, or ring torus".into(),
            ));
        }
        Ok(())
    }

    fn check_uv(&self, uv: UV) -> Result<(), GeometryError> {
        self.validate()?;
        for x in uv {
            finite(x)?;
        }
        if matches!(self, Self::Sphere { .. }) && uv[1].abs() > std::f64::consts::FRAC_PI_2
            || matches!(self, Self::Cone { .. }) && uv[1] < 0.0
        {
            return Err(GeometryError::InvalidGeometry(
                "UV outside surface domain".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn parameter_in_domain(&self, uv: UV) -> bool {
        uv.into_iter().all(f64::is_finite)
            && (!matches!(self, Self::Sphere { .. }) || uv[1].abs() <= std::f64::consts::FRAC_PI_2)
            && (!matches!(self, Self::Cone { .. }) || uv[1] >= 0.0)
    }
}

fn lift(angle: f64, hint: Option<f64>) -> f64 {
    hint.map_or(angle, |h| {
        angle + ((h - angle) / std::f64::consts::TAU).round() * std::f64::consts::TAU
    })
}

impl Surface for SurfaceGeometry {
    fn point_at(&self, uv: UV) -> Result<Point3, GeometryError> {
        Ok(self.derivatives(uv)?.point)
    }

    fn derivatives(&self, uv: UV) -> Result<SurfaceJet2, GeometryError> {
        self.check_uv(uv)?;
        let (su, cu) = uv[0].sin_cos();
        let (sv, cv) = uv[1].sin_cos();
        let radial = [cu, su, 0.0];
        let tangent = [-su, cu, 0.0];
        let z = [0.0, 0.0, 1.0];
        let zero = [0.0; 3];
        let (p, du, dv, duu, duv, dvv) = match self {
            Self::Plane { .. } => (
                [uv[0], uv[1], 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                zero,
                zero,
                zero,
            ),
            Self::Sphere { radius: r, .. } => {
                let p = add(scale(radial, r * cv), scale(z, r * sv));
                (
                    p,
                    scale(tangent, r * cv),
                    add(scale(radial, -r * sv), scale(z, r * cv)),
                    scale(radial, -r * cv),
                    scale(tangent, -r * sv),
                    scale(p, -1.0),
                )
            }
            Self::Cylinder { radius: r, .. } => (
                add(scale(radial, *r), scale(z, uv[1])),
                scale(tangent, *r),
                z,
                scale(radial, -r),
                zero,
                zero,
            ),
            Self::Cone { semi_angle, .. } => {
                let k = semi_angle.tan();
                let r = uv[1] * k;
                (
                    add(scale(radial, r), scale(z, uv[1])),
                    scale(tangent, r),
                    add(scale(radial, k), z),
                    scale(radial, -r),
                    scale(tangent, k),
                    zero,
                )
            }
            Self::Torus {
                major_radius: r,
                minor_radius: a,
                ..
            } => {
                let b = r + a * cv;
                (
                    add(scale(radial, b), scale(z, a * sv)),
                    scale(tangent, b),
                    add(scale(radial, -a * sv), scale(z, a * cv)),
                    scale(radial, -b),
                    scale(tangent, -a * sv),
                    add(scale(radial, -a * cv), scale(z, -a * sv)),
                )
            }
        };
        let f = self.frame();
        Ok(SurfaceJet2 {
            point: checked(f.point(p))?,
            du: checked(f.vector(du))?,
            dv: checked(f.vector(dv))?,
            duu: checked(f.vector(duu))?,
            duv: checked(f.vector(duv))?,
            dvv: checked(f.vector(dvv))?,
        })
    }

    fn normal_at(&self, uv: UV) -> Result<Point3, GeometryError> {
        self.check_uv(uv)?;
        let (s, c) = uv[0].sin_cos();
        let (sv, cv) = uv[1].sin_cos();
        let n = match self {
            Self::Plane { .. } => [0.0, 0.0, 1.0],
            Self::Sphere { .. } | Self::Torus { .. } => [cv * c, cv * s, sv],
            Self::Cylinder { .. } => [c, s, 0.0],
            Self::Cone { semi_angle, .. } => {
                if uv[1] == 0.0 {
                    return Err(GeometryError::SingularParameterization);
                }
                [
                    semi_angle.cos() * c,
                    semi_angle.cos() * s,
                    -semi_angle.sin(),
                ]
            }
        };
        checked(self.frame().vector(n))
    }

    fn project(&self, p: Point3, hint: Option<UV>) -> Result<UV, GeometryError> {
        self.validate()?;
        checked(p)?;
        if let Some(h) = hint {
            for v in h {
                finite(v)?;
            }
        }
        let p = checked(self.frame().local(p))?;
        let radial = p[0].hypot(p[1]);
        let u = lift(p[1].atan2(p[0]), hint.map(|h| h[0]));
        let result = match self {
            Self::Plane { .. } => [p[0], p[1]],
            Self::Sphere { .. } => {
                if norm(p) == 0.0 {
                    return Err(GeometryError::SingularParameterization);
                }
                [u, p[2].atan2(radial)]
            }
            Self::Cylinder { .. } => {
                if radial == 0.0 {
                    return Err(GeometryError::SingularParameterization);
                }
                [u, p[2]]
            }
            Self::Cone { semi_angle, .. } => {
                let v = (radial * semi_angle.sin() + p[2] * semi_angle.cos()) * semi_angle.cos();
                if radial == 0.0 || v <= 0.0 {
                    return Err(GeometryError::SingularParameterization);
                }
                [u, v]
            }
            Self::Torus { major_radius, .. } => {
                if radial == 0.0 || (radial == *major_radius && p[2] == 0.0) {
                    return Err(GeometryError::SingularParameterization);
                }
                [
                    u,
                    lift(p[2].atan2(radial - major_radius), hint.map(|h| h[1])),
                ]
            }
        };
        for x in result {
            finite(x)?;
        }
        Ok(result)
    }

    fn charts(&self) -> &[SurfaceChart] {
        match self {
            Self::Plane { .. } => &PLANE_CHART,
            Self::Sphere { .. } => &SPHERE_CHART,
            Self::Cylinder { .. } => &CYLINDER_CHART,
            Self::Cone { .. } => &CONE_CHART,
            Self::Torus { .. } => &TORUS_CHART,
        }
    }

    fn enclose(&self, domain: UVBox) -> Result<PatchBounds, GeometryError> {
        for d in domain {
            Interval::new(d.lo, d.hi)?;
        }
        self.check_uv([domain[0].lo, domain[1].lo])?;
        self.check_uv([domain[0].hi, domain[1].hi])?;
        let [u, v] = domain;
        let zero = Interval::point(0.0)?;
        let c = u.cos()?;
        let s = u.sin()?;
        let local = match self {
            Self::Plane { .. } => [u, v, zero],
            Self::Sphere { radius, .. } => {
                let r = Interval::point(*radius)?;
                let cv = v.cos()?.mul(r)?;
                [c.mul(cv)?, s.mul(cv)?, v.sin()?.mul(r)?]
            }
            Self::Cylinder { radius, .. } => {
                let r = Interval::point(*radius)?;
                [c.mul(r)?, s.mul(r)?, v]
            }
            Self::Cone { semi_angle, .. } => {
                let angle = Interval::point(*semi_angle)?;
                let k = angle.sin()?.div(angle.cos()?)?;
                let r = v.mul(k)?;
                [c.mul(r)?, s.mul(r)?, v]
            }
            Self::Torus {
                major_radius,
                minor_radius,
                ..
            } => {
                let a = Interval::point(*minor_radius)?;
                let r = Interval::point(*major_radius)?.add(v.cos()?.mul(a)?)?;
                [c.mul(r)?, s.mul(r)?, v.sin()?.mul(a)?]
            }
        };
        self.frame().bounds(local)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum CurveGeometry {
    Line {
        origin: Point3,
        direction: Point3,
    },
    Circle {
        frame: Frame3,
        radius: f64,
    },
    Ellipse {
        frame: Frame3,
        major_radius: f64,
        minor_radius: f64,
    },
    Intersection {
        definition: u32,
    },
}

pub trait Curve {
    fn point_at(&self, t: f64) -> Result<Point3, GeometryError>;
    fn tangent_at(&self, t: f64) -> Result<Point3, GeometryError>;
    fn enclose(&self, range: Interval) -> Result<PatchBounds, GeometryError>;
}

impl CurveGeometry {
    pub fn validate(&self) -> Result<(), GeometryError> {
        match self {
            Self::Line { origin, direction } => {
                checked(*origin)?;
                checked(*direction)?;
                if (norm(*direction) - 1.0).abs() > 1e-12 {
                    return Err(GeometryError::InvalidGeometry(
                        "line direction must be unit length".into(),
                    ));
                }
            }
            Self::Circle { frame, radius } => {
                frame.validate()?;
                if !radius.is_finite() || *radius <= 0.0 {
                    return Err(GeometryError::InvalidGeometry(
                        "circle radius must be positive".into(),
                    ));
                }
            }
            Self::Ellipse {
                frame,
                major_radius: a,
                minor_radius: b,
            } => {
                frame.validate()?;
                if !a.is_finite() || !b.is_finite() || *b <= 0.0 || a < b {
                    return Err(GeometryError::InvalidGeometry(
                        "invalid ellipse radii".into(),
                    ));
                }
            }
            Self::Intersection { .. } => {}
        };
        Ok(())
    }

    pub(crate) fn elementary_point(&self, t: f64) -> Result<Point3, GeometryError> {
        self.validate()?;
        finite(t)?;
        checked(match self {
            Self::Line { origin, direction } => add(*origin, scale(*direction, t)),
            Self::Circle { frame, radius } => {
                frame.point([radius * t.cos(), radius * t.sin(), 0.0])
            }
            Self::Ellipse {
                frame,
                major_radius: a,
                minor_radius: b,
            } => frame.point([a * t.cos(), b * t.sin(), 0.0]),
            Self::Intersection { definition } => {
                return Err(GeometryError::MissingReference {
                    kind: "intersection".into(),
                    index: *definition,
                })
            }
        })
    }

    pub(crate) fn elementary_tangent(&self, t: f64) -> Result<Point3, GeometryError> {
        self.validate()?;
        finite(t)?;
        checked(match self {
            Self::Line { direction, .. } => *direction,
            Self::Circle { frame, radius } => {
                frame.vector([-radius * t.sin(), radius * t.cos(), 0.0])
            }
            Self::Ellipse {
                frame,
                major_radius: a,
                minor_radius: b,
            } => frame.vector([-a * t.sin(), b * t.cos(), 0.0]),
            Self::Intersection { definition } => {
                return Err(GeometryError::MissingReference {
                    kind: "intersection".into(),
                    index: *definition,
                })
            }
        })
    }

    pub(crate) fn elementary_bounds(&self, range: Interval) -> Result<PatchBounds, GeometryError> {
        self.validate()?;
        Interval::new(range.lo, range.hi)?;
        let zero = Interval::point(0.0)?;
        match self {
            Self::Line { origin, direction } => {
                let mut axes = [zero; 3];
                for i in 0..3 {
                    axes[i] = Interval::point(origin[i])?
                        .add(range.mul(Interval::point(direction[i])?)?)?;
                }
                Ok(PatchBounds { axes })
            }
            Self::Circle { frame, radius } => frame.bounds([
                range.cos()?.mul(Interval::point(*radius)?)?,
                range.sin()?.mul(Interval::point(*radius)?)?,
                zero,
            ]),
            Self::Ellipse {
                frame,
                major_radius: a,
                minor_radius: b,
            } => frame.bounds([
                range.cos()?.mul(Interval::point(*a)?)?,
                range.sin()?.mul(Interval::point(*b)?)?,
                zero,
            ]),
            Self::Intersection { definition } => Err(GeometryError::MissingReference {
                kind: "intersection".into(),
                index: *definition,
            }),
        }
    }

    pub fn sample_elementary(
        &self,
        range: Interval,
        deflection: f64,
        max_segments: usize,
    ) -> Result<Vec<Point3>, GeometryError> {
        self.validate()?;
        Interval::new(range.lo, range.hi)?;
        if !deflection.is_finite() || deflection <= 0.0 {
            return Err(GeometryError::InvalidGeometry(
                "deflection must be finite and positive".into(),
            ));
        }
        let bound = match self {
            Self::Line { .. } => 0.0,
            Self::Circle { radius, .. } => *radius,
            Self::Ellipse { major_radius, .. } => *major_radius,
            Self::Intersection { definition } => {
                return Err(GeometryError::MissingReference {
                    kind: "intersection".into(),
                    index: *definition,
                })
            }
        };
        let minimum = if bound > 0.0 && range.width() >= std::f64::consts::TAU - 1e-12 {
            3.0
        } else {
            1.0
        };
        let count = (range.width().abs() * (bound / (8.0 * deflection)).sqrt())
            .ceil()
            .max(minimum);
        if !count.is_finite() || count > max_segments as f64 {
            return Err(GeometryError::LimitExceeded(
                "curve tessellation segments".into(),
            ));
        }
        let n = count as usize;
        let mut samples = Vec::with_capacity(n + 1);
        for i in 0..=n {
            samples.push(
                self.elementary_point(range.lo + (range.hi - range.lo) * i as f64 / n as f64)?,
            );
        }
        Ok(samples)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn surfaces() -> Vec<SurfaceGeometry> {
        let frame = Frame3::from_axis([3.0, -2.0, 7.0], [1.0, 2.0, 3.0], [0.0, 1.0, 0.0]).unwrap();
        vec![
            SurfaceGeometry::Plane { frame },
            SurfaceGeometry::Sphere { frame, radius: 2.0 },
            SurfaceGeometry::Cylinder { frame, radius: 2.0 },
            SurfaceGeometry::Cone {
                frame,
                semi_angle: 0.4,
            },
            SurfaceGeometry::Torus {
                frame,
                major_radius: 3.0,
                minor_radius: 1.0,
            },
        ]
    }

    #[test]
    fn derivatives_projection_and_normals_for_all_families() {
        let uv = [0.7, 0.4];
        let h = 1e-5;
        for surface in surfaces() {
            let jet = surface.derivatives(uv).unwrap();
            let n = surface.normal_at(uv).unwrap();
            assert!((norm(n) - 1.0).abs() < 1e-12);
            assert!(dot(n, jet.du).abs() < 1e-12);
            assert!(dot(n, jet.dv).abs() < 1e-12);
            let projected = surface.project(jet.point, Some(uv)).unwrap();
            assert!((projected[0] - uv[0]).abs() < 1e-12);
            assert!((projected[1] - uv[1]).abs() < 1e-12);
            for dimension in 0..2 {
                let mut lo = uv;
                let mut hi = uv;
                lo[dimension] -= h;
                hi[dimension] += h;
                let jl = surface.derivatives(lo).unwrap();
                let jh = surface.derivatives(hi).unwrap();
                let derivative = scale(sub(jh.point, jl.point), 0.5 / h);
                assert!(
                    norm(sub(
                        derivative,
                        if dimension == 0 { jet.du } else { jet.dv }
                    )) < 1e-8
                );
                assert!(
                    norm(sub(
                        scale(sub(jh.du, jl.du), 0.5 / h),
                        if dimension == 0 { jet.duu } else { jet.duv }
                    )) < 1e-8
                );
                assert!(
                    norm(sub(
                        scale(sub(jh.dv, jl.dv), 0.5 / h),
                        if dimension == 0 { jet.duv } else { jet.dvv }
                    )) < 1e-8
                );
            }
        }
    }

    #[test]
    fn conservative_boxes_cover_transformed_patches() {
        let box_uv = [
            Interval::new(0.2, 1.8).unwrap(),
            Interval::new(0.1, 0.9).unwrap(),
        ];
        for surface in surfaces() {
            let bounds = surface.enclose(box_uv).unwrap();
            for i in 0..=24 {
                for j in 0..=24 {
                    let uv = [0.2 + 1.6 * i as f64 / 24.0, 0.1 + 0.8 * j as f64 / 24.0];
                    assert!(bounds.contains(surface.point_at(uv).unwrap()));
                }
            }
        }
    }

    #[test]
    fn circle_samples_meet_chord_error_at_multiple_lods() {
        let curve = CurveGeometry::Circle {
            frame: Frame3::IDENTITY,
            radius: 2.0,
        };
        let range = Interval::new(0.0, std::f64::consts::TAU).unwrap();
        for epsilon in [0.1, 0.01, 0.001] {
            let samples = curve.sample_elementary(range, epsilon, 100_000).unwrap();
            let n = samples.len() - 1;
            let sagitta = 2.0 * (1.0 - (std::f64::consts::PI / n as f64).cos());
            assert!(sagitta <= epsilon);
            assert!(norm(sub(samples[0], samples[n])) < 1e-12);
        }
    }

    #[test]
    fn invalid_frames_tori_and_apices_fail_explicitly() {
        Frame3::from_axis([0.0; 3], [0.0, 0.0, 1e-320], [1.0, 0.0, 0.0])
            .unwrap()
            .validate()
            .unwrap();
        let mut bad = Frame3::IDENTITY;
        bad.y = [0.0, -1.0, 0.0];
        assert!(bad.validate().is_err());
        assert!(SurfaceGeometry::Torus {
            frame: Frame3::IDENTITY,
            major_radius: 1.0,
            minor_radius: 1.0
        }
        .validate()
        .is_err());
        let cone = SurfaceGeometry::Cone {
            frame: Frame3::IDENTITY,
            semi_angle: 0.3,
        };
        assert_eq!(
            cone.normal_at([0.0, 0.0]),
            Err(GeometryError::SingularParameterization)
        );
        assert!(CurveGeometry::Circle {
            frame: Frame3::IDENTITY,
            radius: 1.0
        }
        .sample_elementary(Interval::new(0.0, 1.0).unwrap(), 0.0, 100)
        .is_err());
    }
}
