use crate::brep::{BrepEnvelope, GeometryError, SurfaceGeometry};
use crate::math::{dot, isolate_candidates, quadratic, Interval, Point3, QuadraticRoots};

pub(super) fn support_roots(
    surface: &SurfaceGeometry,
    origin: Point3,
    direction: Point3,
    domain: Interval,
    tolerance: f64,
) -> Result<Option<Vec<f64>>, GeometryError> {
    let o = surface.frame().local(origin);
    let d = [
        dot(direction, surface.frame().x),
        dot(direction, surface.frame().y),
        dot(direction, surface.frame().z),
    ];
    let quadratic_roots = |a, b, c| -> Result<Option<Vec<f64>>, GeometryError> {
        Ok(Some(match quadratic(a, b, c)? {
            QuadraticRoots::Empty => Vec::new(),
            QuadraticRoots::One(root) => vec![root],
            QuadraticRoots::Two(roots) => roots.to_vec(),
            QuadraticRoots::IdenticallyZero => return Ok(None),
        }))
    };
    let result = match surface {
        SurfaceGeometry::Plane { .. } => {
            if d[2] == 0.0 {
                if o[2].abs() <= tolerance {
                    None
                } else {
                    Some(Vec::new())
                }
            } else {
                Some(vec![-o[2] / d[2]])
            }
        }
        SurfaceGeometry::Sphere { radius, .. } => {
            quadratic_roots(dot(d, d), 2.0 * dot(o, d), dot(o, o) - radius * radius)?
        }
        SurfaceGeometry::Cylinder { radius, .. } => quadratic_roots(
            d[0].mul_add(d[0], d[1] * d[1]),
            2.0 * o[0].mul_add(d[0], o[1] * d[1]),
            o[0].mul_add(o[0], o[1] * o[1]) - radius * radius,
        )?,
        SurfaceGeometry::Cone { semi_angle, .. } => {
            let k2 = semi_angle.tan().powi(2);
            quadratic_roots(
                d[0].mul_add(d[0], d[1] * d[1]) - k2 * d[2] * d[2],
                2.0 * (o[0].mul_add(d[0], o[1] * d[1]) - k2 * o[2] * d[2]),
                o[0].mul_add(o[0], o[1] * o[1]) - k2 * o[2] * o[2],
            )?
            .map(|roots| {
                roots
                    .into_iter()
                    .filter(|root| o[2] + root * d[2] >= -tolerance)
                    .collect()
            })
        }
        SurfaceGeometry::Torus {
            major_radius,
            minor_radius,
            ..
        } => Some(torus_roots(
            o,
            d,
            *major_radius,
            *minor_radius,
            domain,
            tolerance,
        )?),
    };
    Ok(result.map(|roots| {
        roots
            .into_iter()
            .filter(|root| domain.contains(*root))
            .collect()
    }))
}

fn torus_roots(
    o: Point3,
    d: Point3,
    major_radius: f64,
    minor_radius: f64,
    domain: Interval,
    tolerance: f64,
) -> Result<Vec<f64>, GeometryError> {
    let radial = [
        o[0].mul_add(o[0], o[1] * o[1]),
        2.0 * o[0].mul_add(d[0], o[1] * d[1]),
        d[0].mul_add(d[0], d[1] * d[1]),
    ];
    let sum = [
        dot(o, o) + major_radius * major_radius - minor_radius * minor_radius,
        2.0 * dot(o, d),
        dot(d, d),
    ];
    let r4 = 4.0 * major_radius * major_radius;
    let coefficients = [
        sum[0] * sum[0] - r4 * radial[0],
        2.0 * sum[0] * sum[1] - r4 * radial[1],
        sum[1] * sum[1] + 2.0 * sum[0] * sum[2] - r4 * radial[2],
        2.0 * sum[1] * sum[2],
        sum[2] * sum[2],
    ];
    let candidates =
        isolate_candidates(&coefficients, domain, tolerance, 100_000).map_err(|error| {
            GeometryError::UnresolvedIntersection(format!("torus ray root isolation: {error:?}"))
        })?;
    Ok(candidates
        .intervals
        .into_iter()
        .map(|candidate| refine_polynomial_root(&coefficients, candidate))
        .collect())
}

fn refine_polynomial_root(coefficients: &[f64], interval: Interval) -> f64 {
    let mut t = interval.midpoint();
    let derivative = coefficients
        .iter()
        .enumerate()
        .skip(1)
        .map(|(degree, coefficient)| *coefficient * degree as f64)
        .collect::<Vec<_>>();
    for _ in 0..12 {
        let value = polynomial_value(coefficients, t);
        let slope = polynomial_value(&derivative, t);
        if slope == 0.0 || !slope.is_finite() {
            break;
        }
        let candidate = t - value / slope;
        if !candidate.is_finite() || candidate < interval.lo || candidate > interval.hi {
            break;
        }
        t = candidate;
    }
    t
}

fn polynomial_value(coefficients: &[f64], value: f64) -> f64 {
    coefficients
        .iter()
        .rev()
        .fold(0.0, |sum, coefficient| sum.mul_add(value, *coefficient))
}

pub(super) fn ray_box_domain(
    brep: &BrepEnvelope,
    origin: Point3,
    direction: Point3,
) -> Result<Option<Interval>, GeometryError> {
    let Some(bounds) = brep.bounds_unchecked()? else {
        return Ok(None);
    };
    let mut lo: f64 = 0.0;
    let mut hi = f64::MAX;
    for axis in 0..3 {
        if direction[axis] == 0.0 {
            if !bounds.axes[axis].contains(origin[axis]) {
                return Ok(None);
            }
            continue;
        }
        let mut a = (bounds.axes[axis].lo - origin[axis]) / direction[axis];
        let mut b = (bounds.axes[axis].hi - origin[axis]) / direction[axis];
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
        lo = lo.max(a);
        hi = hi.min(b);
    }
    if hi < lo.max(0.0) {
        Ok(None)
    } else {
        Ok(Some(Interval::new(lo.max(0.0), hi)?))
    }
}
