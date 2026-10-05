use super::document::Document;
use super::lexer::{fields, reference, references};
use crate::vector::{dot, sub};
use std::f64::consts::TAU;

pub struct ParameterDirectionReport {
    pub assessed: usize,
    pub aligned: usize,
    pub reversed: usize,
    pub conic_assessed: usize,
    pub conic_aligned: usize,
    pub conic_reversed: usize,
}

impl Document {
    pub fn pcurveless_edges(&self) -> Result<usize, String> {
        let mut count = 0;
        for expression in self
            .entities
            .values()
            .filter(|expression| expression.starts_with("EDGE_CURVE("))
        {
            let args = fields(expression)?;
            let curve = self.entity(reference(&args[3])?)?;
            if !curve.starts_with("SURFACE_CURVE(") && !curve.starts_with("SEAM_CURVE(") {
                count += 1;
            }
        }
        Ok(count)
    }

    pub fn product_names(&self) -> Result<Vec<String>, String> {
        self.entities
            .values()
            .filter(|expression| expression.starts_with("PRODUCT("))
            .map(|expression| fields(expression).map(|args| args[0].clone()))
            .collect()
    }

    pub fn vertex_positions(&self) -> Result<Vec<[f64; 3]>, String> {
        self.entities
            .iter()
            .filter(|(_, expression)| expression.starts_with("VERTEX_POINT("))
            .map(|(id, _)| self.vertex(*id))
            .collect()
    }

    pub fn surface_radii(&self) -> Result<Vec<(String, Vec<f64>)>, String> {
        self.entities
            .values()
            .filter_map(|expression| {
                let kind = if expression.starts_with("CYLINDRICAL_SURFACE(") {
                    "cylinder"
                } else if expression.starts_with("SPHERICAL_SURFACE(") {
                    "sphere"
                } else if expression.starts_with("TOROIDAL_SURFACE(") {
                    "torus"
                } else {
                    return None;
                };
                Some(fields(expression).and_then(|args| {
                    args[2..]
                        .iter()
                        .map(|value| value.parse().map_err(|_| "invalid surface radius".into()))
                        .collect::<Result<Vec<_>, String>>()
                        .map(|radii| (kind.into(), radii))
                }))
            })
            .collect()
    }

    pub fn parameter_direction_report(&self) -> Result<ParameterDirectionReport, String> {
        let mut report = ParameterDirectionReport {
            assessed: 0,
            aligned: 0,
            reversed: 0,
            conic_assessed: 0,
            conic_aligned: 0,
            conic_reversed: 0,
        };
        for expression in self.entities.values() {
            if !expression.starts_with("SURFACE_CURVE(") && !expression.starts_with("SEAM_CURVE(") {
                continue;
            }
            let args = fields(expression)?;
            let curve = reference(&args[1])?;
            let curve_expression = self.entity(curve)?;
            let sampled = curve_expression.starts_with("LINE(")
                || curve_expression.starts_with("B_SPLINE_CURVE_WITH_KNOTS(");
            let conic =
                curve_expression.starts_with("CIRCLE(") || curve_expression.starts_with("ELLIPSE(");
            if !sampled && !conic {
                continue;
            }
            for pcurve in references(&args[2]) {
                let (surface, uv_curve) = self.pcurve_support(pcurve)?;
                if sampled {
                    self.add_sampled_direction(&mut report, curve, surface, uv_curve)?;
                } else {
                    let turn = self.conic_turn(curve, surface, uv_curve)?;
                    add_conic_direction(&mut report, pcurve, turn)?;
                }
            }
        }
        Ok(report)
    }

    fn add_sampled_direction(
        &self,
        report: &mut ParameterDirectionReport,
        curve: usize,
        surface: usize,
        uv_curve: usize,
    ) -> Result<(), String> {
        let sample_distance = |reverse: bool| -> Result<f64, String> {
            let mut total = 0.0;
            for parameter in [0.25, 0.75] {
                let uv =
                    self.pcurve_point(uv_curve, if reverse { 1.0 - parameter } else { parameter })?;
                let point = self.surface_point(surface, uv)?;
                let support = self.curve_point(curve, parameter)?;
                total += point
                    .iter()
                    .zip(support)
                    .map(|(a, b)| (a - b).powi(2))
                    .sum::<f64>();
            }
            Ok(total)
        };
        report.assessed += 1;
        if sample_distance(false)? <= sample_distance(true)? {
            report.aligned += 1;
        } else {
            report.reversed += 1;
        }
        Ok(())
    }

    fn conic_turn(&self, curve: usize, surface: usize, uv_curve: usize) -> Result<f64, String> {
        let conic = self.conic(curve)?;
        let mut angles = [0.0; 2];
        for (angle, parameter) in angles.iter_mut().zip([0.25, 0.25 + 1.0 / 64.0]) {
            let point = self.surface_point(surface, self.pcurve_point(uv_curve, parameter)?)?;
            let delta = sub(point, conic.origin);
            *angle = (dot(delta, conic.y) / conic.b).atan2(dot(delta, conic.x) / conic.a);
        }
        let turn = angles[1] - angles[0];
        Ok(turn - TAU * (turn / TAU).round())
    }
}

fn add_conic_direction(
    report: &mut ParameterDirectionReport,
    pcurve: usize,
    turn: f64,
) -> Result<(), String> {
    report.conic_assessed += 1;
    if turn > 0.0 {
        report.conic_aligned += 1;
    } else if turn < 0.0 {
        report.conic_reversed += 1;
    } else {
        return Err(format!("conic pcurve #{pcurve} has no direction"));
    }
    Ok(())
}
