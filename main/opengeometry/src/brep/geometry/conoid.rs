use super::curve::CurveGeometry;
use super::turned::turned_axis;
use crate::brep::error::GeometryError;
use crate::brep::frame::{Frame3, UV};
use crate::math::{add, cross, dot, scale, sub, Interval, Point3};

const NEWTON_STEPS: usize = 32;

type LocalJet = (Point3, Point3, Point3, Point3, Point3, Point3);

pub(super) fn height(c: &[f64; 3], u: f64) -> [f64; 3] {
    let (s, co) = u.sin_cos();
    let h = c[0] + c[1] * co + c[2] * s;
    [h, c[2] * co - c[1] * s, c[0] - h]
}

pub(super) fn local_jet(c: &[f64; 3], uv: UV) -> LocalJet {
    let (s, co) = uv[0].sin_cos();
    let (radial, tangent, z) = ([co, s, 0.0], [-s, co, 0.0], [0.0, 0.0, 1.0]);
    let [h, slope, curvature] = height(c, uv[0]);
    (
        add(scale(radial, uv[1]), scale(z, h)),
        add(scale(tangent, uv[1]), scale(z, slope)),
        radial,
        add(scale(radial, -uv[1]), scale(z, curvature)),
        tangent,
        [0.0; 3],
    )
}

pub(super) fn validate(c: &[f64; 3]) -> Result<(), GeometryError> {
    if c.iter().all(|v| v.is_finite()) {
        Ok(())
    } else {
        Err(GeometryError::InvalidGeometry(
            "conoid coefficients must be finite".into(),
        ))
    }
}

pub fn conoid_ellipse(
    frame: Frame3,
    c: [f64; 3],
    radius: f64,
) -> Result<(CurveGeometry, f64), GeometryError> {
    frame.validate()?;
    validate(&c)?;
    if !radius.is_finite() || radius <= 0.0 {
        return Err(GeometryError::InvalidGeometry(
            "conoid ellipse radius must be positive".into(),
        ));
    }
    let slope = [c[1] / radius, c[2] / radius];
    let phase = slope[1].atan2(slope[0]);
    let (s, co) = phase.sin_cos();
    let rise = slope[0].hypot(slope[1]);
    let stretch = rise.hypot(1.0);
    let x = frame.vector([co / stretch, s / stretch, rise / stretch]);
    let y = frame.vector([-s, co, 0.0]);
    let curve = CurveGeometry::Ellipse {
        frame: Frame3::from_axis(frame.point([0.0, 0.0, c[0]]), cross(x, y), x)?,
        major_radius: radius * stretch,
        minor_radius: radius,
    };
    curve.validate()?;
    Ok((curve, phase))
}

pub(super) fn local_normal(c: &[f64; 3], uv: UV) -> Result<Point3, GeometryError> {
    let (s, co) = uv[0].sin_cos();
    let slope = height(c, uv[0])[1];
    let length = slope.hypot(uv[1]);
    if length == 0.0 || !length.is_finite() {
        return Err(GeometryError::SingularParameterization);
    }
    Ok([-slope * s / length, slope * co / length, -uv[1] / length])
}

pub(super) fn local_bounds(
    c: &[f64; 3],
    domain: [Interval; 2],
) -> Result<[Interval; 3], GeometryError> {
    let [u, v] = domain;
    let (co, s) = (u.cos()?, u.sin()?);
    let z = turned_axis([c[0], 0.0], [c[1], c[2]], u)?;
    Ok([co.mul_interval(v)?, s.mul_interval(v)?, z])
}

pub(super) fn closest(c: &[f64; 3], p: Point3, start: UV) -> Result<UV, GeometryError> {
    let mut uv = start;
    for _ in 0..NEWTON_STEPS {
        let (point, du, dv, duu, duv, dvv) = local_jet(c, uv);
        let r = sub(point, p);
        let g = [dot(du, r), dot(dv, r)];
        let metric = [dot(du, du), dot(du, dv), dot(dv, dv)];
        let full = [
            metric[0] + dot(duu, r),
            metric[1] + dot(duv, r),
            metric[2] + dot(dvv, r),
        ];
        let det = |h: [f64; 3]| h[0] * h[2] - h[1] * h[1];
        let h = if full[0] > 0.0 && det(full) > 0.0 {
            full
        } else {
            metric
        };
        let d = det(h);
        if d <= 0.0 || !d.is_finite() {
            return Err(GeometryError::SingularParameterization);
        }
        let step = [
            (h[2] * g[0] - h[1] * g[1]) / d,
            (h[0] * g[1] - h[1] * g[0]) / d,
        ];
        if !step.iter().all(|s| s.is_finite()) {
            return Err(GeometryError::SingularParameterization);
        }
        uv = [uv[0] - step[0].clamp(-0.5, 0.5), uv[1] - step[1]];
        let small = |s: f64, x: f64| s.abs() <= 4.0 * f64::EPSILON * (1.0 + x.abs());
        if small(step[0], uv[0]) && small(step[1], uv[1]) {
            break;
        }
    }
    Ok(uv)
}
