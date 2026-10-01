use super::document::Document;
use super::lexer::{fields, reference, references};
use std::f64::consts::{FRAC_PI_2, TAU};

pub(super) struct Conic3 {
    pub(super) origin: [f64; 3],
    pub(super) x: [f64; 3],
    pub(super) y: [f64; 3],
    pub(super) z: [f64; 3],
    pub(super) a: f64,
    pub(super) b: f64,
}

impl Document {
    pub(super) fn curve_distance(&self, id: usize, point: [f64; 3]) -> Result<f64, String> {
        let expression = self.entity(id)?;
        let args = fields(expression)?;
        if expression.starts_with("LINE(") {
            let origin = self.coordinates(reference(&args[1])?)?;
            let vector = fields(self.entity(reference(&args[2])?)?)?;
            let direction = self.coordinates(reference(&vector[1])?)?;
            let delta = sub(point, origin);
            let projection = dot(delta, direction);
            return Ok(norm(std::array::from_fn(|index| {
                delta[index] - projection * direction[index]
            })));
        }
        if expression.starts_with("CIRCLE(") || expression.starts_with("ELLIPSE(") {
            let conic = self.conic(id)?;
            let delta = sub(point, conic.origin);
            let (px, py, pz) = (
                dot(delta, conic.x),
                dot(delta, conic.y),
                dot(delta, conic.z),
            );
            if expression.starts_with("CIRCLE(") {
                return Ok((pz * pz + (px.hypot(py) - conic.a).powi(2)).sqrt());
            }
            return Ok(ellipse_distance(px, py, pz, conic.a, conic.b));
        }
        if expression.starts_with("B_SPLINE_CURVE_WITH_KNOTS(") {
            let controls = references(&args[2]);
            let first = self.coordinates(*controls.first().ok_or("empty spline controls")?)?;
            let last = self.coordinates(*controls.last().ok_or("empty spline controls")?)?;
            return Ok(norm(sub(point, first)).min(norm(sub(point, last))));
        }
        Err(format!("#{id} is not an edge curve support"))
    }

    pub(super) fn curve_point(&self, id: usize, parameter: f64) -> Result<[f64; 3], String> {
        let expression = self.entity(id)?;
        if expression.starts_with("B_SPLINE_CURVE_WITH_KNOTS(") {
            let point = self.spline_point(id, parameter)?;
            if point.len() != 3 {
                return Err("3D spline is not three-dimensional".into());
            }
            return Ok([point[0], point[1], point[2]]);
        }
        if expression.starts_with("LINE(") {
            let args = fields(expression)?;
            let origin = self.coordinates(reference(&args[1])?)?;
            let vector = fields(self.entity(reference(&args[2])?)?)?;
            let direction = self.coordinates(reference(&vector[1])?)?;
            let length: f64 = vector[2].parse().map_err(|_| "invalid vector length")?;
            return Ok(std::array::from_fn(|axis| {
                origin[axis] + parameter * length * direction[axis]
            }));
        }
        if expression.starts_with("CIRCLE(") || expression.starts_with("ELLIPSE(") {
            let conic = self.conic(id)?;
            let radii = [conic.a, conic.b];
            return Ok(conic_point(
                conic.origin,
                conic.x,
                conic.y,
                radii,
                TAU * parameter,
            ));
        }
        Err(format!("#{id} has no comparable parameterization"))
    }

    pub(super) fn conic(&self, id: usize) -> Result<Conic3, String> {
        let expression = self.entity(id)?;
        let args = fields(expression)?;
        let (origin, z, x) = self.axis(reference(&args[1])?)?;
        let a: f64 = args[2].parse().map_err(|_| "invalid conic radius")?;
        let b = if expression.starts_with("ELLIPSE(") {
            args[3].parse().map_err(|_| "invalid ellipse radius")?
        } else {
            a
        };
        Ok(Conic3 {
            origin,
            x,
            y: cross(z, x),
            z,
            a,
            b,
        })
    }
}

pub(super) fn conic_point<const N: usize>(
    origin: [f64; N],
    x: [f64; N],
    y: [f64; N],
    radii: [f64; 2],
    angle: f64,
) -> [f64; N] {
    let (sin, cos) = angle.sin_cos();
    std::array::from_fn(|axis| origin[axis] + radii[0] * cos * x[axis] + radii[1] * sin * y[axis])
}

fn ellipse_distance(px: f64, py: f64, pz: f64, a: f64, b: f64) -> f64 {
    let (x, y) = (px.abs(), py.abs());
    let mut t = (a * y).atan2(b * x);
    for _ in 0..32 {
        let (sin, cos) = t.sin_cos();
        let slope = (b * b - a * a) * sin * cos + a * x * sin - b * y * cos;
        let bend = (b * b - a * a) * (2.0 * t).cos() + a * x * cos + b * y * sin;
        let next = (t - slope / bend).clamp(0.0, FRAC_PI_2);
        if next == t {
            break;
        }
        t = next;
    }
    let (sin, cos) = t.sin_cos();
    (pz * pz + (a * cos - x).powi(2) + (b * sin - y).powi(2)).sqrt()
}

pub(super) fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|index| a[index] - b[index])
}

pub(super) fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|index| a[index] * b[index]).sum()
}

fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
