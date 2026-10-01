use super::curves::conic_point;
use super::lexer::{fields, keywords, reference, references};
use std::collections::BTreeMap;
use std::f64::consts::TAU;

pub struct Document {
    pub(super) entities: BTreeMap<usize, String>,
}

type Axis3 = ([f64; 3], [f64; 3], [f64; 3]);

impl Document {
    pub fn count(&self, kind: &str) -> usize {
        self.entities
            .values()
            .filter(|expression| keywords(expression).iter().any(|word| word == kind))
            .count()
    }

    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    pub fn entity(&self, id: usize) -> Result<&str, String> {
        self.entities
            .get(&id)
            .map(String::as_str)
            .ok_or_else(|| format!("missing entity #{id}"))
    }

    pub(super) fn tuple(&self, expression: &str) -> Result<Vec<f64>, String> {
        let start = expression
            .find('(')
            .ok_or("tuple lacks opening parenthesis")?;
        let end = expression
            .rfind(')')
            .ok_or("tuple lacks closing parenthesis")?;
        expression[start + 1..end]
            .split(',')
            .map(|value| value.trim().parse().map_err(|_| "invalid real".into()))
            .collect()
    }

    pub(super) fn coordinates(&self, id: usize) -> Result<[f64; 3], String> {
        let expression = self.entity(id)?;
        if !expression.starts_with("CARTESIAN_POINT(") && !expression.starts_with("DIRECTION(") {
            return Err(format!("#{id} is not a point or direction"));
        }
        let args = fields(expression)?;
        let values = self.tuple(&args[1])?;
        if values.len() != 3 {
            return Err(format!("#{id} is not three-dimensional"));
        }
        Ok([values[0], values[1], values[2]])
    }

    pub(super) fn vertex(&self, id: usize) -> Result<[f64; 3], String> {
        let expression = self.entity(id)?;
        if !expression.starts_with("VERTEX_POINT(") {
            return Err(format!("#{id} is not a vertex"));
        }
        let args = fields(expression)?;
        self.coordinates(reference(&args[1])?)
    }

    pub(super) fn axis(&self, id: usize) -> Result<Axis3, String> {
        let expression = self.entity(id)?;
        if !expression.starts_with("AXIS2_PLACEMENT_3D(") {
            return Err(format!("#{id} is not a 3D placement"));
        }
        let args = fields(expression)?;
        Ok((
            self.coordinates(reference(&args[1])?)?,
            self.coordinates(reference(&args[2])?)?,
            self.coordinates(reference(&args[3])?)?,
        ))
    }

    fn coordinates2(&self, id: usize) -> Result<[f64; 2], String> {
        let expression = self.entity(id)?;
        if !expression.starts_with("CARTESIAN_POINT(") && !expression.starts_with("DIRECTION(") {
            return Err(format!("#{id} is not a point or direction"));
        }
        let args = fields(expression)?;
        let values = self.tuple(&args[1])?;
        if values.len() != 2 {
            return Err(format!("#{id} is not two-dimensional"));
        }
        Ok([values[0], values[1]])
    }

    pub(super) fn spline_point(&self, id: usize, parameter: f64) -> Result<Vec<f64>, String> {
        let expression = self.entity(id)?;
        if !expression.starts_with("B_SPLINE_CURVE_WITH_KNOTS(") {
            return Err(format!("#{id} is not a spline"));
        }
        let args = fields(expression)?;
        let controls = references(&args[2]);
        if controls.len() < 4 || (controls.len() - 1) % 3 != 0 {
            return Err(format!("#{id} has invalid cubic controls"));
        }
        let segments = (controls.len() - 1) / 3;
        let position = parameter * segments as f64;
        let segment = (position.floor() as usize).min(segments - 1);
        let local = if parameter == 1.0 {
            1.0
        } else {
            position - segment as f64
        };
        let first = 3 * segment;
        let points = controls[first..first + 4]
            .iter()
            .map(|id| {
                let args = fields(self.entity(*id)?)?;
                self.tuple(&args[1])
            })
            .collect::<Result<Vec<_>, _>>()?;
        let dimension = points[0].len();
        if points.iter().any(|point| point.len() != dimension) {
            return Err("mixed spline dimensions".into());
        }
        let weights = [
            (1.0 - local).powi(3),
            3.0 * local * (1.0 - local).powi(2),
            3.0 * local.powi(2) * (1.0 - local),
            local.powi(3),
        ];
        Ok((0..dimension)
            .map(|axis| {
                (0..4)
                    .map(|index| weights[index] * points[index][axis])
                    .sum()
            })
            .collect())
    }

    pub(super) fn pcurve_point(&self, id: usize, parameter: f64) -> Result<[f64; 2], String> {
        let expression = self.entity(id)?;
        if expression.starts_with("B_SPLINE_CURVE_WITH_KNOTS(") {
            let point = self.spline_point(id, parameter)?;
            if point.len() != 2 {
                return Err("pcurve spline is not two-dimensional".into());
            }
            return Ok([point[0], point[1]]);
        }
        let args = fields(expression)?;
        if expression.starts_with("LINE(") {
            let origin = self.coordinates2(reference(&args[1])?)?;
            let vector = fields(self.entity(reference(&args[2])?)?)?;
            let direction = self.coordinates2(reference(&vector[1])?)?;
            let length: f64 = vector[2].parse().map_err(|_| "invalid vector length")?;
            return Ok([
                origin[0] + parameter * direction[0] * length,
                origin[1] + parameter * direction[1] * length,
            ]);
        }
        if expression.starts_with("CIRCLE(") || expression.starts_with("ELLIPSE(") {
            let placement = fields(self.entity(reference(&args[1])?)?)?;
            let origin = self.coordinates2(reference(&placement[1])?)?;
            let x = self.coordinates2(reference(&placement[2])?)?;
            let length = x[0].hypot(x[1]);
            if length == 0.0 {
                return Err("singular pcurve direction".into());
            }
            let x = [x[0] / length, x[1] / length];
            let y = [-x[1], x[0]];
            let major: f64 = args[2].parse().map_err(|_| "invalid pcurve radius")?;
            let minor: f64 = if expression.starts_with("CIRCLE(") {
                major
            } else {
                args[3].parse().map_err(|_| "invalid pcurve minor radius")?
            };
            return Ok(conic_point(origin, x, y, [major, minor], TAU * parameter));
        }
        Err(format!("#{id} is not a supported pcurve"))
    }

    pub(super) fn pcurve_support(&self, pcurve: usize) -> Result<(usize, usize), String> {
        let pcurve_args = fields(self.entity(pcurve)?)?;
        let surface = reference(&pcurve_args[1])?;
        let definition = reference(&pcurve_args[2])?;
        let representation = fields(self.entity(definition)?)?;
        let uv_curve = *references(&representation[1])
            .first()
            .ok_or("empty pcurve representation")?;
        Ok((surface, uv_curve))
    }

    pub(super) fn surface_point(&self, id: usize, uv: [f64; 2]) -> Result<[f64; 3], String> {
        let expression = self.entity(id)?;
        let args = fields(expression)?;
        let (origin, z, x) = self.axis(reference(&args[1])?)?;
        let y = [
            z[1] * x[2] - z[2] * x[1],
            z[2] * x[0] - z[0] * x[2],
            z[0] * x[1] - z[1] * x[0],
        ];
        let (sin_u, cos_u) = uv[0].sin_cos();
        let (sin_v, cos_v) = uv[1].sin_cos();
        let (radial, axial) = if expression.starts_with("PLANE(") {
            return Ok(std::array::from_fn(|axis| {
                origin[axis] + uv[0] * x[axis] + uv[1] * y[axis]
            }));
        } else if expression.starts_with("CYLINDRICAL_SURFACE(") {
            (
                args[2]
                    .parse::<f64>()
                    .map_err(|_| "invalid cylinder radius")?,
                uv[1],
            )
        } else if expression.starts_with("SPHERICAL_SURFACE(") {
            let radius: f64 = args[2].parse().map_err(|_| "invalid sphere radius")?;
            (radius * cos_v, radius * sin_v)
        } else if expression.starts_with("CONICAL_SURFACE(") {
            let angle: f64 = args[3].parse().map_err(|_| "invalid cone angle")?;
            (uv[1] * angle.sin(), uv[1] * angle.cos())
        } else if expression.starts_with("TOROIDAL_SURFACE(") {
            let major: f64 = args[2].parse().map_err(|_| "invalid torus major radius")?;
            let minor: f64 = args[3].parse().map_err(|_| "invalid torus minor radius")?;
            (major + minor * cos_v, minor * sin_v)
        } else {
            return Err(format!("#{id} is not a supported surface"));
        };
        Ok(std::array::from_fn(|axis| {
            origin[axis] + radial * (cos_u * x[axis] + sin_u * y[axis]) + axial * z[axis]
        }))
    }
}
