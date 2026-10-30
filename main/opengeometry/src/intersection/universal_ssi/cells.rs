use super::implicit_bounds::implicit_value;
use crate::brep::{GeometryError, Surface, SurfaceGeometry, UVBox, UV};
use crate::math::{add, cross, norm, scale, sub, Point3};

#[derive(Clone, Copy)]
pub(super) struct Root {
    pub(super) uv_a: UV,
    pub(super) uv_b: UV,
    pub(super) point: Point3,
}

#[derive(Clone, Copy)]
pub(super) struct Segment {
    pub(super) ends: [Root; 2],
    pub(super) domain: UVBox,
}

struct AxisSamples {
    h: [f64; 2],
    center: f64,
    axis_values: Vec<[f64; 2]>,
}

pub(super) enum StationaryEvent {
    Contact(Point3),
    InteriorCritical,
}

pub(super) fn cell_segments(
    source: &SurfaceGeometry,
    target: &SurfaceGeometry,
    domain: UVBox,
    tolerance: f64,
) -> Result<Vec<Segment>, GeometryError> {
    let corners = [
        [domain[0].lo, domain[1].lo],
        [domain[0].hi, domain[1].lo],
        [domain[0].hi, domain[1].hi],
        [domain[0].lo, domain[1].hi],
    ];
    let mut values = [0.0; 4];
    for (index, uv) in corners.into_iter().enumerate() {
        values[index] = implicit_value(target, source.point_at(uv)?)?;
    }
    let edge_indices = [[0, 1], [1, 2], [2, 3], [3, 0]];
    let mut roots = Vec::new();
    for [a, b] in edge_indices {
        if let Some(root) = edge_root(
            source,
            target,
            [corners[a], corners[b]],
            [values[a], values[b]],
            tolerance,
        )? {
            if roots
                .iter()
                .all(|existing: &Root| norm(sub(existing.point, root.point)) > tolerance)
            {
                roots.push(root);
            }
        }
    }
    Ok(match roots.len() {
        0 => Vec::new(),
        1 => vec![Segment {
            ends: [roots[0], roots[0]],
            domain,
        }],
        2 => vec![Segment {
            ends: [roots[0], roots[1]],
            domain,
        }],
        3 => three_root_segments(&roots, domain)?,
        4 => four_root_segments(source, target, domain, values, &roots)?,
        _ => {
            return Err(GeometryError::UnresolvedIntersection(format!(
                "universal SSI cell contains a singular or unresolved branch event ({} edge roots)",
                roots.len()
            )))
        }
    })
}

fn edge_root(
    source: &SurfaceGeometry,
    target: &SurfaceGeometry,
    uv: [UV; 2],
    values: [f64; 2],
    tolerance: f64,
) -> Result<Option<Root>, GeometryError> {
    if values[0] != 0.0
        && values[1] != 0.0
        && values[0].is_sign_negative() == values[1].is_sign_negative()
    {
        return Ok(None);
    }
    support_root(
        source, target, uv[0], uv[1], values[0], values[1], tolerance,
    )
    .map(Some)
}

fn support_root(
    source: &SurfaceGeometry,
    target: &SurfaceGeometry,
    left_uv: UV,
    right_uv: UV,
    left_value: f64,
    right_value: f64,
    tolerance: f64,
) -> Result<Root, GeometryError> {
    let mut lo_uv = left_uv;
    let mut hi_uv = right_uv;
    let mut lo = left_value;
    let mut hi = right_value;
    let mut best = if lo.abs() <= hi.abs() { lo_uv } else { hi_uv };
    for _ in 0..80 {
        let uv = [(lo_uv[0] + hi_uv[0]) * 0.5, (lo_uv[1] + hi_uv[1]) * 0.5];
        let point = source.point_at(uv)?;
        let value = implicit_value(target, point)?;
        if value.abs() < implicit_value(target, source.point_at(best)?)?.abs() {
            best = uv;
        }
        let projected = target.project(point, None);
        if let Ok(uv_b) = projected {
            let target_point = target.point_at(uv_b)?;
            if norm(sub(point, target_point)) <= tolerance * 0.01 {
                return Ok(Root {
                    uv_a: uv,
                    uv_b,
                    point,
                });
            }
        }
        if lo == 0.0 {
            best = lo_uv;
            break;
        }
        if hi == 0.0 {
            best = hi_uv;
            break;
        }
        if lo.is_sign_negative() == value.is_sign_negative() {
            lo_uv = uv;
            lo = value;
        } else {
            hi_uv = uv;
            hi = value;
        }
    }
    let point = source.point_at(best)?;
    let uv_b = target.project(point, None).map_err(|_| {
        GeometryError::UnresolvedIntersection(
            "universal SSI root cannot be projected to its second support".into(),
        )
    })?;
    if norm(sub(point, target.point_at(uv_b)?)) > tolerance {
        return Err(GeometryError::UnresolvedIntersection(
            "universal SSI root residual exceeds the intersection budget".into(),
        ));
    }
    Ok(Root {
        uv_a: best,
        uv_b,
        point,
    })
}

fn three_root_segments(roots: &[Root], domain: UVBox) -> Result<Vec<Segment>, GeometryError> {
    let mut pair = [0, 1];
    let mut separation = 0.0;
    for first in 0..3 {
        for second in (first + 1)..3 {
            let candidate = norm(sub(roots[first].point, roots[second].point));
            if candidate > separation {
                separation = candidate;
                pair = [first, second];
            }
        }
    }
    let isolated = (0..3).find(|index| !pair.contains(index)).ok_or_else(|| {
        GeometryError::UnresolvedIntersection(
            "universal SSI could not isolate a grid-vertex event".into(),
        )
    })?;
    Ok(vec![
        Segment {
            ends: [roots[pair[0]], roots[pair[1]]],
            domain,
        },
        Segment {
            ends: [roots[isolated], roots[isolated]],
            domain,
        },
    ])
}

fn four_root_segments(
    source: &SurfaceGeometry,
    target: &SurfaceGeometry,
    domain: UVBox,
    values: [f64; 4],
    roots: &[Root],
) -> Result<Vec<Segment>, GeometryError> {
    let center = [domain[0].midpoint(), domain[1].midpoint()];
    let center_value = implicit_value(target, source.point_at(center)?)?;
    let same_as_corner = center_value.is_sign_negative() == values[0].is_sign_negative();
    Ok(if same_as_corner {
        vec![
            Segment {
                ends: [roots[0], roots[3]],
                domain,
            },
            Segment {
                ends: [roots[1], roots[2]],
                domain,
            },
        ]
    } else {
        vec![
            Segment {
                ends: [roots[0], roots[1]],
                domain,
            },
            Segment {
                ends: [roots[2], roots[3]],
                domain,
            },
        ]
    })
}

pub(super) fn stationary_event(
    source: &SurfaceGeometry,
    target: &SurfaceGeometry,
    domain: UVBox,
    tolerance: f64,
) -> Result<Option<StationaryEvent>, GeometryError> {
    let uv_a = [domain[0].midpoint(), domain[1].midpoint()];
    let value = |uv| implicit_value(target, source.point_at(uv)?);
    let Some(uv_a) = critical_uv(source, &value, domain, uv_a)? else {
        return Ok(None);
    };
    if (0..2).any(|axis| {
        uv_a[axis] < domain[axis].lo - tolerance || uv_a[axis] > domain[axis].hi + tolerance
    }) {
        return Ok(None);
    }
    let point_a = source.point_at(uv_a)?;
    let uv_b = match target.project(point_a, None) {
        Ok(uv) if target.parameter_in_domain(uv) => uv,
        _ => return Ok(Some(StationaryEvent::InteriorCritical)),
    };
    let point_b = target.point_at(uv_b)?;
    if norm(sub(point_a, point_b)) > tolerance {
        return Ok(Some(StationaryEvent::InteriorCritical));
    }
    let normal_a = source.normal_at(uv_a)?;
    let normal_b = target.normal_at(uv_b)?;
    if norm(cross(normal_a, normal_b)) > 1.0e-5 {
        return Ok(Some(StationaryEvent::InteriorCritical));
    }
    let samples = axis_samples(&value, domain, uv_a)?;
    let (diagonal, mixed) = second_derivatives(&value, uv_a, &samples)?;
    let determinant = diagonal[0] * diagonal[1] - mixed * mixed;
    let hessian_scale = diagonal[0]
        .abs()
        .max(diagonal[1].abs())
        .max(mixed.abs())
        .max(1.0);
    if determinant <= 1024.0 * f64::EPSILON * hessian_scale * hessian_scale {
        return Ok(Some(StationaryEvent::InteriorCritical));
    }
    Ok(Some(StationaryEvent::Contact(scale(
        add(point_a, point_b),
        0.5,
    ))))
}

fn critical_uv(
    source: &SurfaceGeometry,
    value: &impl Fn(UV) -> Result<f64, GeometryError>,
    domain: UVBox,
    mut uv_a: UV,
) -> Result<Option<UV>, GeometryError> {
    let mut converged = false;
    for _ in 0..16 {
        let samples = axis_samples(value, domain, uv_a)?;
        let AxisSamples { h, axis_values, .. } = &samples;
        let gradient =
            [0, 1].map(|axis| (axis_values[axis][1] - axis_values[axis][0]) / (2.0 * h[axis]));
        let (diagonal, mixed) = second_derivatives(value, uv_a, &samples)?;
        let determinant = diagonal[0] * diagonal[1] - mixed * mixed;
        if !determinant.is_finite() || determinant.abs() <= f64::EPSILON {
            return Ok(None);
        }
        let delta = [
            (diagonal[1] * gradient[0] - mixed * gradient[1]) / determinant,
            (diagonal[0] * gradient[1] - mixed * gradient[0]) / determinant,
        ];
        let candidate = [uv_a[0] - delta[0], uv_a[1] - delta[1]];
        if (0..2).any(|axis| {
            !candidate[axis].is_finite()
                || candidate[axis] < domain[axis].lo
                || candidate[axis] > domain[axis].hi
        }) {
            return Ok(None);
        }
        uv_a = candidate;
        if delta[0].abs() <= h[0] * 1.0e-3 && delta[1].abs() <= h[1] * 1.0e-3 {
            converged = true;
            break;
        }
    }
    if !converged || !source.parameter_in_domain(uv_a) {
        return Ok(None);
    }
    Ok(Some(uv_a))
}

fn axis_samples(
    value: &impl Fn(UV) -> Result<f64, GeometryError>,
    domain: UVBox,
    uv: UV,
) -> Result<AxisSamples, GeometryError> {
    let h = [0, 1].map(|axis| {
        (domain[axis].width() * 1.0e-3).max(f64::EPSILON.sqrt() * uv[axis].abs().max(1.0))
    });
    let center = value(uv)?;
    let axis_values = [0, 1]
        .map(|axis| {
            let mut low = uv;
            let mut high = uv;
            low[axis] -= h[axis];
            high[axis] += h[axis];
            Ok([value(low)?, value(high)?])
        })
        .into_iter()
        .collect::<Result<Vec<_>, GeometryError>>()?;
    Ok(AxisSamples {
        h,
        center,
        axis_values,
    })
}

fn second_derivatives(
    value: &impl Fn(UV) -> Result<f64, GeometryError>,
    uv: UV,
    samples: &AxisSamples,
) -> Result<([f64; 2], f64), GeometryError> {
    let AxisSamples {
        h,
        center,
        axis_values,
    } = samples;
    let diagonal = [0, 1].map(|axis| {
        (axis_values[axis][0] - 2.0 * center + axis_values[axis][1]) / (h[axis] * h[axis])
    });
    let mixed_values = [[-1.0, -1.0], [-1.0, 1.0], [1.0, -1.0], [1.0, 1.0]]
        .map(|offset| value([uv[0] + offset[0] * h[0], uv[1] + offset[1] * h[1]]))
        .into_iter()
        .collect::<Result<Vec<_>, GeometryError>>()?;
    let mixed = (mixed_values[3] - mixed_values[2] - mixed_values[1] + mixed_values[0])
        / (4.0 * h[0] * h[1]);
    Ok((diagonal, mixed))
}
