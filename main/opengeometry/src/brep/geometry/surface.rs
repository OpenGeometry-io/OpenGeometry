use crate::brep::bounds::PatchBounds;
use crate::brep::error::GeometryError;
use crate::brep::frame::{checked, Frame3, UVBox, UV};
use crate::math::{add, finite, norm, scale, Interval, Point3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug)]
pub struct SurfaceChart {
    pub(crate) domain: [[f64; 2]; 2],
    pub(crate) periods: [Option<f64>; 2],
}
const PLANE_CHART: [SurfaceChart; 1] = [SurfaceChart {
    domain: [[-f64::MAX, f64::MAX]; 2],
    periods: [None, None],
}];
const SPHERE_CHART: [SurfaceChart; 1] = [SurfaceChart {
    domain: [
        [0.0, std::f64::consts::TAU],
        [-std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2],
    ],
    periods: [Some(std::f64::consts::TAU), None],
}];
const CYLINDER_CHART: [SurfaceChart; 1] = [SurfaceChart {
    domain: [[0.0, std::f64::consts::TAU], [-f64::MAX, f64::MAX]],
    periods: [Some(std::f64::consts::TAU), None],
}];
const CONE_CHART: [SurfaceChart; 1] = [SurfaceChart {
    domain: [[0.0, std::f64::consts::TAU], [0.0, f64::MAX]],
    periods: [Some(std::f64::consts::TAU), None],
}];
const TORUS_CHART: [SurfaceChart; 1] = [SurfaceChart {
    domain: [[0.0, std::f64::consts::TAU]; 2],
    periods: [Some(std::f64::consts::TAU); 2],
}];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    pub(super) point: Point3,
    pub(crate) du: Point3,
    pub(crate) dv: Point3,
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
    pub(crate) fn validate(&self) -> Result<(), GeometryError> {
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
                let cv = v.cos()?.mul_interval(r)?;
                [
                    c.mul_interval(cv)?,
                    s.mul_interval(cv)?,
                    v.sin()?.mul_interval(r)?,
                ]
            }
            Self::Cylinder { radius, .. } => {
                let r = Interval::point(*radius)?;
                [c.mul_interval(r)?, s.mul_interval(r)?, v]
            }
            Self::Cone { semi_angle, .. } => {
                let angle = Interval::point(*semi_angle)?;
                let k = angle.sin()?.div_interval(angle.cos()?)?;
                let r = v.mul_interval(k)?;
                [c.mul_interval(r)?, s.mul_interval(r)?, v]
            }
            Self::Torus {
                major_radius,
                minor_radius,
                ..
            } => {
                let a = Interval::point(*minor_radius)?;
                let r = Interval::point(*major_radius)?.add_interval(v.cos()?.mul_interval(a)?)?;
                [
                    c.mul_interval(r)?,
                    s.mul_interval(r)?,
                    v.sin()?.mul_interval(a)?,
                ]
            }
        };
        self.frame().bounds(local)
    }
}
