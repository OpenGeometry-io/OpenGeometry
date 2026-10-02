use super::cells::Root;
use crate::brep::{unit, GeometryError, Surface, SurfaceGeometry};
use crate::math::{add, cross, dot, norm, scale, solve, sub};

pub(super) fn pseudo_arclength_correct(
    source: &SurfaceGeometry,
    target: &SurfaceGeometry,
    root: Root,
    tolerance: f64,
) -> Result<Root, GeometryError> {
    let predictor = root.point;
    let normal_cross = cross(source.normal_at(root.uv_a)?, target.normal_at(root.uv_b)?);
    if norm(normal_cross) <= 1.0e-8 {
        let point_a = source.point_at(root.uv_a)?;
        let point_b = target.point_at(root.uv_b)?;
        if norm(sub(point_a, point_b)) <= tolerance {
            return Ok(Root {
                point: scale(add(point_a, point_b), 0.5),
                ..root
            });
        }
        return Err(GeometryError::UnresolvedIntersection(
            "universal SSI tangent branch root exceeds its support residual".into(),
        ));
    }
    let tangent = unit(normal_cross)?;
    let mut parameters = [root.uv_a[0], root.uv_a[1], root.uv_b[0], root.uv_b[1]];
    for _ in 0..12 {
        let uv_a = [parameters[0], parameters[1]];
        let uv_b = [parameters[2], parameters[3]];
        let point_a = source.point_at(uv_a)?;
        let point_b = target.point_at(uv_b)?;
        let midpoint = scale(add(point_a, point_b), 0.5);
        let residual = sub(point_a, point_b);
        let constraint = dot(sub(midpoint, predictor), tangent);
        if norm(residual) <= tolerance * 0.25 && constraint.abs() <= tolerance * 0.25 {
            return Ok(Root {
                uv_a,
                uv_b,
                point: midpoint,
            });
        }
        let jet_a = source.derivatives(uv_a)?;
        let jet_b = target.derivatives(uv_b)?;
        let matrix = [
            [jet_a.du[0], jet_a.dv[0], -jet_b.du[0], -jet_b.dv[0]],
            [jet_a.du[1], jet_a.dv[1], -jet_b.du[1], -jet_b.dv[1]],
            [jet_a.du[2], jet_a.dv[2], -jet_b.du[2], -jet_b.dv[2]],
            [
                0.5 * dot(jet_a.du, tangent),
                0.5 * dot(jet_a.dv, tangent),
                0.5 * dot(jet_b.du, tangent),
                0.5 * dot(jet_b.dv, tangent),
            ],
        ];
        let correction = solve(
            matrix,
            [-residual[0], -residual[1], -residual[2], -constraint],
        )
        .map_err(|_| {
            GeometryError::UnresolvedIntersection(
                "universal SSI pseudo-arclength correction is singular".into(),
            )
        })?;
        if correction.minimum_scaled_pivot <= 256.0 * f64::EPSILON {
            return Err(GeometryError::UnresolvedIntersection(
                "universal SSI pseudo-arclength correction is ill-conditioned".into(),
            ));
        }
        for (parameter, delta) in parameters.iter_mut().zip(correction.value) {
            *parameter += delta;
        }
        if parameters.iter().any(|value| !value.is_finite()) {
            return Err(GeometryError::UnresolvedIntersection(
                "universal SSI pseudo-arclength correction left finite parameter space".into(),
            ));
        }
    }
    Err(GeometryError::UnresolvedIntersection(
        "universal SSI pseudo-arclength correction iteration budget exhausted".into(),
    ))
}
