use super::{
    geometry::{add, cross, dot, norm, scale, sub, unit, PatchBounds, Surface, UVBox},
    intersection::{IntersectionDefinition, SsiBudget, TraceAnchor},
    ssi::{SsiCurve, SsiResult},
    topology::{Accuracy, GeometryStore, IntersectionSide, PcurveGeometry},
    CurveGeometry, GeometryError, Point3, SurfaceGeometry, UV,
};
use crate::math::{interval::Interval, solve::solve};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy)]
struct Root {
    uv_a: UV,
    uv_b: UV,
    point: Point3,
}

#[derive(Clone, Copy)]
struct Segment {
    ends: [Root; 2],
    domain: UVBox,
}

fn pseudo_arclength_correct(
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

fn local_intervals(
    surface: &SurfaceGeometry,
    bounds: PatchBounds,
) -> Result<[Interval; 3], GeometryError> {
    let frame = surface.frame();
    let mut local = [Interval::point(0.0)?; 3];
    for (axis, direction) in [frame.x, frame.y, frame.z].into_iter().enumerate() {
        let mut coordinate = Interval::point(0.0)?;
        for world_axis in 0..3 {
            coordinate = coordinate.add(
                bounds.axes[world_axis]
                    .sub(Interval::point(frame.origin[world_axis])?)?
                    .mul(Interval::point(direction[world_axis])?)?,
            )?;
        }
        local[axis] = coordinate;
    }
    Ok(local)
}

fn implicit_interval(
    surface: &SurfaceGeometry,
    bounds: PatchBounds,
) -> Result<Interval, GeometryError> {
    let [x, y, z] = local_intervals(surface, bounds)?;
    let x2 = x.square()?;
    let y2 = y.square()?;
    let z2 = z.square()?;
    Ok(match surface {
        SurfaceGeometry::Plane { .. } => z,
        SurfaceGeometry::Sphere { radius, .. } => x2
            .add(y2)?
            .add(z2)?
            .sub(Interval::point(radius * radius)?)?,
        SurfaceGeometry::Cylinder { radius, .. } => {
            x2.add(y2)?.sub(Interval::point(radius * radius)?)?
        }
        SurfaceGeometry::Cone { semi_angle, .. } => {
            if z.hi < 0.0 {
                return Interval::point(1.0).map_err(Into::into);
            }
            let slope = semi_angle.tan();
            x2.add(y2)?.sub(z2.mul(Interval::point(slope * slope)?)?)?
        }
        SurfaceGeometry::Torus {
            major_radius,
            minor_radius,
            ..
        } => {
            let radial = x2.add(y2)?;
            Interval::new(radial.lo.max(0.0), radial.hi)?
                .sqrt()?
                .sub(Interval::point(*major_radius)?)?
                .square()?
                .add(z2)?
                .sub(Interval::point(minor_radius * minor_radius)?)?
        }
    })
}

fn implicit_value(surface: &SurfaceGeometry, point: Point3) -> Result<f64, GeometryError> {
    let [x, y, z] = surface.frame().local(point);
    let value = match surface {
        SurfaceGeometry::Plane { .. } => z,
        SurfaceGeometry::Sphere { radius, .. } => {
            x.mul_add(x, y.mul_add(y, z * z)) - radius * radius
        }
        SurfaceGeometry::Cylinder { radius, .. } => x.mul_add(x, y * y) - radius * radius,
        SurfaceGeometry::Cone { semi_angle, .. } => {
            if z < 0.0 {
                return Ok(x.mul_add(x, y.mul_add(y, z * z)) + 1.0);
            }
            x.mul_add(x, y * y) - z * z * semi_angle.tan().powi(2)
        }
        SurfaceGeometry::Torus {
            major_radius,
            minor_radius,
            ..
        } => {
            let radial = x.mul_add(x, y * y);
            (radial.sqrt() - major_radius).powi(2) + z * z - minor_radius * minor_radius
        }
    };
    if value.is_finite() {
        Ok(value)
    } else {
        Err(GeometryError::UnresolvedIntersection(
            "universal SSI implicit evaluation exceeded numerical range".into(),
        ))
    }
}

fn surface_scale(surface: &SurfaceGeometry) -> f64 {
    match surface {
        SurfaceGeometry::Plane { .. } => 1.0,
        SurfaceGeometry::Sphere { radius, .. } | SurfaceGeometry::Cylinder { radius, .. } => {
            *radius
        }
        SurfaceGeometry::Cone { .. } => 1.0,
        SurfaceGeometry::Torus {
            major_radius,
            minor_radius,
            ..
        } => major_radius + minor_radius,
    }
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

fn cell_segments(
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
        3 => {
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
            vec![
                Segment {
                    ends: [roots[pair[0]], roots[pair[1]]],
                    domain,
                },
                Segment {
                    ends: [roots[isolated], roots[isolated]],
                    domain,
                },
            ]
        }
        4 => {
            let center = [domain[0].midpoint(), domain[1].midpoint()];
            let center_value = implicit_value(target, source.point_at(center)?)?;
            let same_as_corner = center_value.is_sign_negative() == values[0].is_sign_negative();
            if same_as_corner {
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
            }
        }
        _ => {
            return Err(GeometryError::UnresolvedIntersection(format!(
                "universal SSI cell contains a singular or unresolved branch event ({} edge roots)",
                roots.len()
            )))
        }
    })
}

enum StationaryEvent {
    Contact(Point3),
    InteriorCritical,
}

fn stationary_event(
    source: &SurfaceGeometry,
    target: &SurfaceGeometry,
    domain: UVBox,
    tolerance: f64,
) -> Result<Option<StationaryEvent>, GeometryError> {
    let mut uv_a = [domain[0].midpoint(), domain[1].midpoint()];
    let value = |uv| implicit_value(target, source.point_at(uv)?);
    let mut converged = false;
    for _ in 0..16 {
        let h = [0, 1].map(|axis| {
            (domain[axis].width() * 1.0e-3).max(f64::EPSILON.sqrt() * uv_a[axis].abs().max(1.0))
        });
        let center = value(uv_a)?;
        let axis_values = [0, 1]
            .map(|axis| {
                let mut low = uv_a;
                let mut high = uv_a;
                low[axis] -= h[axis];
                high[axis] += h[axis];
                Ok([value(low)?, value(high)?])
            })
            .into_iter()
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let gradient =
            [0, 1].map(|axis| (axis_values[axis][1] - axis_values[axis][0]) / (2.0 * h[axis]));
        let diagonal = [0, 1].map(|axis| {
            (axis_values[axis][0] - 2.0 * center + axis_values[axis][1]) / (h[axis] * h[axis])
        });
        let mixed = {
            let values = [[-1.0, -1.0], [-1.0, 1.0], [1.0, -1.0], [1.0, 1.0]]
                .map(|offset| value([uv_a[0] + offset[0] * h[0], uv_a[1] + offset[1] * h[1]]))
                .into_iter()
                .collect::<Result<Vec<_>, GeometryError>>()?;
            (values[3] - values[2] - values[1] + values[0]) / (4.0 * h[0] * h[1])
        };
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
    let h = [0, 1].map(|axis| {
        (domain[axis].width() * 1.0e-3).max(f64::EPSILON.sqrt() * uv_a[axis].abs().max(1.0))
    });
    let center = value(uv_a)?;
    let axis_values = [0, 1]
        .map(|axis| {
            let mut low = uv_a;
            let mut high = uv_a;
            low[axis] -= h[axis];
            high[axis] += h[axis];
            Ok([value(low)?, value(high)?])
        })
        .into_iter()
        .collect::<Result<Vec<_>, GeometryError>>()?;
    let diagonal = [0, 1].map(|axis| {
        (axis_values[axis][0] - 2.0 * center + axis_values[axis][1]) / (h[axis] * h[axis])
    });
    let mixed_values = [[-1.0, -1.0], [-1.0, 1.0], [1.0, -1.0], [1.0, 1.0]]
        .map(|offset| value([uv_a[0] + offset[0] * h[0], uv_a[1] + offset[1] * h[1]]))
        .into_iter()
        .collect::<Result<Vec<_>, GeometryError>>()?;
    let mixed = (mixed_values[3] - mixed_values[2] - mixed_values[1] + mixed_values[0])
        / (4.0 * h[0] * h[1]);
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

fn split(domain: UVBox, axis: usize) -> Result<[UVBox; 2], GeometryError> {
    let midpoint = domain[axis].midpoint();
    let mut left = domain;
    let mut right = domain;
    left[axis] = Interval::new(domain[axis].lo, midpoint)?;
    right[axis] = Interval::new(midpoint, domain[axis].hi)?;
    Ok([left, right])
}

fn bounds_diameter(bounds: PatchBounds) -> f64 {
    bounds
        .axes
        .iter()
        .map(|axis| axis.width() * axis.width())
        .sum::<f64>()
        .sqrt()
}

fn bounds_overlap(a: PatchBounds, b: PatchBounds, tolerance: f64) -> bool {
    (0..3).all(|axis| {
        a.axes[axis].lo <= b.axes[axis].hi + tolerance
            && a.axes[axis].hi + tolerance >= b.axes[axis].lo
    })
}

fn find(parent: &mut [usize], value: usize) -> usize {
    let mut root = value;
    while parent[root] != root {
        root = parent[root];
    }
    let mut current = value;
    while parent[current] != current {
        let next = parent[current];
        parent[current] = root;
        current = next;
    }
    root
}

fn union(parent: &mut [usize], left: usize, right: usize) {
    let left = find(parent, left);
    let right = find(parent, right);
    if left != right {
        parent[right] = left;
    }
}

fn segment_endpoint_tangent(
    source: &SurfaceGeometry,
    target: &SurfaceGeometry,
    segment: Segment,
    end: usize,
) -> Result<Point3, GeometryError> {
    let root = segment.ends[end];
    let transverse = cross(source.normal_at(root.uv_a)?, target.normal_at(root.uv_b)?);
    if norm(transverse) > 1.0e-8 {
        unit(transverse)
    } else {
        unit(sub(segment.ends[1 - end].point, root.point)).map_err(|_| {
            GeometryError::UnresolvedIntersection(
                "universal SSI tangent segment has no resolvable direction".into(),
            )
        })
    }
}

fn normalized_key(uv: UV, domain: UVBox, periods: [Option<f64>; 2], quantum: [f64; 2]) -> [i64; 2] {
    std::array::from_fn(|axis| {
        let value = periods[axis].map_or(uv[axis], |period| {
            let lifted = (uv[axis] - domain[axis].lo).rem_euclid(period);
            if (period - lifted).abs() <= quantum[axis] {
                domain[axis].lo
            } else {
                domain[axis].lo + lifted
            }
        });
        (value / quantum[axis]).round() as i64
    })
}

fn unwrap(value: f64, previous: f64, period: Option<f64>) -> f64 {
    period.map_or(value, |period| {
        value + ((previous - value) / period).round() * period
    })
}

fn branch_definition(
    mut roots: Vec<Root>,
    mut source_domains: Vec<UVBox>,
    surfaces: [u32; 2],
    store: &mut GeometryStore,
    accuracy: Accuracy,
) -> Result<SsiCurve, GeometryError> {
    if source_domains.len() + 1 != roots.len() {
        return Err(GeometryError::InvalidGeometry(
            "universal SSI branch domains do not match its roots".into(),
        ));
    }
    let source = store.surface(surfaces[0])?.clone();
    let target = store.surface(surfaces[1])?.clone();
    roots = roots
        .into_iter()
        .map(|root| pseudo_arclength_correct(&source, &target, root, accuracy.intersection))
        .collect::<Result<Vec<_>, _>>()?;
    let periods_a = store.surface(surfaces[0])?.charts()[0].periods;
    let periods_b = store.surface(surfaces[1])?.charts()[0].periods;
    for index in 1..roots.len() {
        for axis in 0..2 {
            roots[index].uv_a[axis] = unwrap(
                roots[index].uv_a[axis],
                roots[index - 1].uv_a[axis],
                periods_a[axis],
            );
            roots[index].uv_b[axis] = unwrap(
                roots[index].uv_b[axis],
                roots[index - 1].uv_b[axis],
                periods_b[axis],
            );
        }
    }
    for (index, domain) in source_domains.iter_mut().enumerate() {
        for axis in 0..2 {
            if let Some(period) = periods_a[axis] {
                let branch_midpoint = 0.5 * (roots[index].uv_a[axis] + roots[index + 1].uv_a[axis]);
                let shift = ((branch_midpoint - domain[axis].midpoint()) / period).round() * period;
                domain[axis] = Interval::new(domain[axis].lo + shift, domain[axis].hi + shift)?;
            }
        }
    }
    let mut parameter = 0.0;
    let mut anchors = Vec::with_capacity(roots.len());
    for (index, root) in roots.iter().enumerate() {
        if index > 0 {
            parameter += norm(sub(root.point, roots[index - 1].point));
        }
        anchors.push(TraceAnchor {
            parameter,
            point: root.point,
            uv_a: root.uv_a,
            uv_b: root.uv_b,
        });
    }
    if anchors.len() < 2 || parameter <= 4.0 * accuracy.geometric {
        return Err(GeometryError::UnresolvedIntersection(
            "universal SSI branch is below geometric resolution".into(),
        ));
    }
    let mut uv_tubes = Vec::with_capacity(anchors.len() - 1);
    for (index, pair) in anchors.windows(2).enumerate() {
        let coordinates = [
            [pair[0].uv_a[0], pair[1].uv_a[0]],
            [pair[0].uv_a[1], pair[1].uv_a[1]],
            [pair[0].uv_b[0], pair[1].uv_b[0]],
            [pair[0].uv_b[1], pair[1].uv_b[1]],
        ];
        let mut intervals = coordinates.map(|values| {
            let padding = (values[1] - values[0]).abs().max(accuracy.intersection) * 2.0;
            Interval::new(
                values[0].min(values[1]) - padding,
                values[0].max(values[1]) + padding,
            )
        });
        for axis in 0..2 {
            let domain = source_domains[index][axis];
            let padding =
                domain.width() * 0.05 + 16.0 * f64::EPSILON * domain.lo.abs().max(domain.hi.abs());
            intervals[axis] = Interval::new(
                domain
                    .lo
                    .min(coordinates[axis][0])
                    .min(coordinates[axis][1])
                    - padding,
                domain
                    .hi
                    .max(coordinates[axis][0])
                    .max(coordinates[axis][1])
                    + padding,
            );
        }
        let [u0, v0, u1, v1] = intervals;
        uv_tubes.push([u0?, v0?, u1?, v1?]);
    }
    let definition = store.intersections.len() as u32;
    store.intersections.push(IntersectionDefinition {
        surfaces,
        anchors,
        uv_tubes,
        residual_tolerance: accuracy.intersection,
    });
    let curve = store.curves.len() as u32;
    store
        .curves
        .push(CurveGeometry::Intersection { definition });
    let pcurve_a = store.pcurves.len() as u32;
    store.pcurves.push(PcurveGeometry::IntersectionSide {
        definition,
        side: IntersectionSide::A,
    });
    let pcurve_b = store.pcurves.len() as u32;
    store.pcurves.push(PcurveGeometry::IntersectionSide {
        definition,
        side: IntersectionSide::B,
    });
    let domain = Interval::new(0.0, parameter)?;
    store.intersections[definition as usize].validate(store)?;
    Ok(SsiCurve {
        curve,
        pcurves: [pcurve_a, pcurve_b],
        domain: Some(domain),
    })
}

pub fn intersect_patches(
    store: &mut GeometryStore,
    surfaces: [u32; 2],
    domains: [UVBox; 2],
    accuracy: Accuracy,
    budget: SsiBudget,
) -> Result<SsiResult, GeometryError> {
    intersect_patches_from(store, surfaces, domains, 0, accuracy, budget)
}

pub fn intersect_patches_from(
    store: &mut GeometryStore,
    surfaces: [u32; 2],
    domains: [UVBox; 2],
    source_side: usize,
    accuracy: Accuracy,
    budget: SsiBudget,
) -> Result<SsiResult, GeometryError> {
    if source_side > 1 {
        return Err(GeometryError::InvalidGeometry(
            "universal SSI source side must be zero or one".into(),
        ));
    }
    let ordered_surfaces = if source_side == 0 {
        surfaces
    } else {
        [surfaces[1], surfaces[0]]
    };
    let ordered_domains = if source_side == 0 {
        domains
    } else {
        [domains[1], domains[0]]
    };
    accuracy.validate()?;
    budget.validate()?;
    for domain in ordered_domains {
        for axis in domain {
            Interval::new(axis.lo, axis.hi)?;
            if axis.width() <= 0.0 {
                return Err(GeometryError::InvalidGeometry(
                    "universal SSI patch domain has no area".into(),
                ));
            }
        }
    }
    let source = store.surface(ordered_surfaces[0])?.clone();
    let target = store.surface(ordered_surfaces[1])?.clone();
    source.validate()?;
    target.validate()?;
    let source_bounds = source.enclose(ordered_domains[0])?;
    let target_bounds = target.enclose(ordered_domains[1])?;
    let scale = bounds_diameter(source_bounds)
        .max(surface_scale(&source))
        .max(surface_scale(&target))
        .max(accuracy.geometric);
    let target_cell = (scale / 64.0).max(16.0 * accuracy.geometric);
    let mut stack = vec![(ordered_domains[0], 0usize)];
    let mut visits = 0usize;
    let mut segments = Vec::new();
    let mut contacts = Vec::new();
    let mut unresolved = false;
    while let Some((domain, depth)) = stack.pop() {
        visits = visits
            .checked_add(1)
            .ok_or_else(|| GeometryError::LimitExceeded("SSI patch visits".into()))?;
        if visits > budget.max_patch_pairs {
            return Err(GeometryError::UnresolvedIntersection(
                "universal SSI patch-pair visit budget exhausted".into(),
            ));
        }
        let bounds = source.enclose(domain)?;
        if !bounds_overlap(bounds, target_bounds, accuracy.intersection) {
            continue;
        }
        if !implicit_interval(&target, bounds)?.contains(0.0) {
            continue;
        }
        if bounds_diameter(bounds) <= target_cell || depth == budget.max_subdivision_depth {
            let found = cell_segments(&source, &target, domain, accuracy.intersection)?;
            if found.is_empty() {
                match stationary_event(&source, &target, domain, accuracy.intersection)? {
                    Some(StationaryEvent::Contact(contact)) => {
                        if contacts
                            .iter()
                            .all(|existing| norm(sub(*existing, contact)) > accuracy.intersection)
                        {
                            contacts.push(contact);
                        }
                    }
                    Some(StationaryEvent::InteriorCritical)
                        if depth == budget.max_subdivision_depth =>
                    {
                        unresolved = true;
                    }
                    Some(StationaryEvent::InteriorCritical) => {
                        let axis = usize::from(domain[1].width() > domain[0].width());
                        let children = split(domain, axis)?;
                        stack.push((children[1], depth + 1));
                        stack.push((children[0], depth + 1));
                    }
                    None => {}
                }
            } else {
                segments.extend(found);
            }
            continue;
        }
        let axis = usize::from(domain[1].width() > domain[0].width());
        let children = split(domain, axis)?;
        stack.push((children[1], depth + 1));
        stack.push((children[0], depth + 1));
    }
    if unresolved {
        return Err(GeometryError::UnresolvedIntersection(
            "universal SSI could not certify an interval cell containing a possible tangency or sub-cell branch".into(),
        ));
    }
    let mut result = SsiResult {
        contacts,
        ..SsiResult::default()
    };
    if segments.is_empty() {
        return Ok(result);
    }

    let periods = source.charts()[0].periods;
    let quantum = [
        (ordered_domains[0][0].width() / 1_000_000.0).max(accuracy.geometric / scale),
        (ordered_domains[0][1].width() / 1_000_000.0).max(accuracy.geometric / scale),
    ];
    let mut unique = Vec::new();
    let mut segment_keys = HashSet::new();
    for segment in segments {
        let mut keys = segment
            .ends
            .map(|root| normalized_key(root.uv_a, ordered_domains[0], periods, quantum));
        if keys[0] == keys[1] {
            continue;
        }
        if keys[1] < keys[0] {
            keys.swap(0, 1);
        }
        if segment_keys.insert(keys) {
            unique.push(segment);
        }
    }
    let endpoint_count = unique.len() * 2;
    let mut parent = (0..endpoint_count).collect::<Vec<_>>();
    let stitch_tolerance =
        (4.0 * accuracy.geometric.max(accuracy.intersection)).max(target_cell * 0.01);
    let mut buckets: HashMap<[i64; 3], Vec<usize>> = HashMap::new();
    for endpoint in 0..endpoint_count {
        let root = unique[endpoint / 2].ends[endpoint % 2];
        let key = root
            .point
            .map(|coordinate| (coordinate / stitch_tolerance).floor() as i64);
        let tangent =
            segment_endpoint_tangent(&source, &target, unique[endpoint / 2], endpoint % 2)?;
        for x in (key[0] - 1)..=(key[0] + 1) {
            for y in (key[1] - 1)..=(key[1] + 1) {
                for z in (key[2] - 1)..=(key[2] + 1) {
                    for &candidate in buckets.get(&[x, y, z]).into_iter().flatten() {
                        let other = unique[candidate / 2].ends[candidate % 2];
                        if norm(sub(root.point, other.point)) > stitch_tolerance {
                            continue;
                        }
                        let other_tangent = segment_endpoint_tangent(
                            &source,
                            &target,
                            unique[candidate / 2],
                            candidate % 2,
                        )?;
                        if dot(tangent, other_tangent).abs() >= 0.99 {
                            union(&mut parent, endpoint, candidate);
                        }
                    }
                }
            }
        }
        buckets.entry(key).or_default().push(endpoint);
    }
    let collapsed_segments = (0..unique.len())
        .map(|index| find(&mut parent, index * 2) == find(&mut parent, index * 2 + 1))
        .collect::<Vec<_>>();
    let mut adjacency: HashMap<usize, Vec<(usize, usize)>> = HashMap::new();
    for index in 0..unique.len() {
        if collapsed_segments[index] {
            continue;
        }
        for end in 0..2 {
            let node = find(&mut parent, index * 2 + end);
            adjacency.entry(node).or_default().push((index, end));
        }
    }
    if adjacency.values().any(|edges| edges.len() > 2) {
        return Err(GeometryError::UnresolvedIntersection(
            "universal SSI found a singular branch junction".into(),
        ));
    }
    let mut used = collapsed_segments;
    for start in 0..unique.len() {
        if used[start] {
            continue;
        }
        let endpoint = (0..2)
            .find(|&end| {
                let node = find(&mut parent, start * 2 + end);
                adjacency.get(&node).is_some_and(|edges| edges.len() == 1)
            })
            .unwrap_or(0);
        let mut branch = Vec::new();
        let mut source_domains = Vec::new();
        let mut current = start;
        let mut from_end = endpoint;
        loop {
            if used[current] {
                break;
            }
            used[current] = true;
            let segment = unique[current];
            if branch.is_empty() {
                branch.push(segment.ends[from_end]);
            }
            let to_end = 1 - from_end;
            branch.push(segment.ends[to_end]);
            source_domains.push(segment.domain);
            if branch.len() > budget.max_steps_per_branch {
                return Err(GeometryError::UnresolvedIntersection(
                    "universal SSI branch step budget exhausted".into(),
                ));
            }
            let node = find(&mut parent, current * 2 + to_end);
            let Some(next) = adjacency
                .get(&node)
                .and_then(|edges| edges.iter().copied().find(|(index, _)| !used[*index]))
            else {
                break;
            };
            current = next.0;
            from_end = next.1;
        }
        if branch.len() >= 2 {
            result.curves.push(branch_definition(
                branch,
                source_domains,
                ordered_surfaces,
                store,
                accuracy,
            )?);
        }
    }
    if source_side == 1 {
        for curve in &mut result.curves {
            curve.pcurves.swap(0, 1);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytic::{Curve, Frame3};

    fn accuracy() -> Accuracy {
        Accuracy {
            geometric: 1e-5,
            intersection: 2.5e-6,
            tessellation: 1e-3,
            exchange: 1e-5,
        }
    }

    #[test]
    fn oblique_plane_torus_is_stored_as_corrected_numerical_branches() {
        let mut store = GeometryStore::new();
        let plane =
            Frame3::from_axis([0.0, 0.0, 0.15], [0.35, -0.2, 1.0], [1.0, 0.0, 0.0]).unwrap();
        store.surfaces = vec![
            SurfaceGeometry::Torus {
                frame: Frame3::IDENTITY,
                major_radius: 2.0,
                minor_radius: 0.5,
            },
            SurfaceGeometry::Plane { frame: plane },
        ];
        let tau = std::f64::consts::TAU;
        let result = intersect_patches(
            &mut store,
            [0, 1],
            [
                [Interval::new(0.0, tau).unwrap(); 2],
                [Interval::new(-4.0, 4.0).unwrap(); 2],
            ],
            accuracy(),
            SsiBudget::default(),
        )
        .unwrap();
        assert!(!result.curves.is_empty());
        for branch in result.curves {
            assert!(matches!(
                store.curves[branch.curve as usize],
                CurveGeometry::Intersection { .. }
            ));
            let domain = branch.domain.unwrap();
            for index in 0..=16 {
                let t = domain.lo + domain.width() * index as f64 / 16.0;
                let point = store.curve(branch.curve).unwrap().point_at(t).unwrap();
                for surface in &store.surfaces {
                    let uv = surface.project(point, None).unwrap();
                    assert!(
                        norm(sub(surface.point_at(uv).unwrap(), point)) <= accuracy().intersection
                    );
                }
            }
        }
    }

    #[test]
    fn disjoint_bounded_patches_return_empty_without_fabricating_a_branch() {
        let mut store = GeometryStore::new();
        store.surfaces = vec![
            SurfaceGeometry::Sphere {
                frame: Frame3::IDENTITY,
                radius: 1.0,
            },
            SurfaceGeometry::Plane {
                frame: Frame3 {
                    origin: [0.0, 0.0, 2.0],
                    ..Frame3::IDENTITY
                },
            },
        ];
        let result = intersect_patches(
            &mut store,
            [0, 1],
            [
                [
                    Interval::new(0.0, std::f64::consts::TAU).unwrap(),
                    Interval::new(-std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2)
                        .unwrap(),
                ],
                [Interval::new(-2.0, 2.0).unwrap(); 2],
            ],
            accuracy(),
            SsiBudget::default(),
        )
        .unwrap();
        assert!(result.curves.is_empty());
        assert!(result.contacts.is_empty());
    }

    #[test]
    fn terminal_subdivision_reports_an_isolated_torus_plane_tangency() {
        let u: f64 = 0.37;
        let v: f64 = 0.41;
        let (sin_u, cos_u) = u.sin_cos();
        let (sin_v, cos_v) = v.sin_cos();
        let normal = [cos_v * cos_u, cos_v * sin_u, sin_v];
        let radial = 2.0 + 0.5 * cos_v;
        let contact = [radial * cos_u, radial * sin_u, 0.5 * sin_v];
        let mut store = GeometryStore::new();
        store.surfaces = vec![
            SurfaceGeometry::Torus {
                frame: Frame3::IDENTITY,
                major_radius: 2.0,
                minor_radius: 0.5,
            },
            SurfaceGeometry::Plane {
                frame: Frame3::from_axis(contact, normal, [0.0, 0.0, 1.0]).unwrap(),
            },
        ];
        let tau = std::f64::consts::TAU;
        let result = intersect_patches(
            &mut store,
            [0, 1],
            [
                [Interval::new(0.0, tau).unwrap(); 2],
                [Interval::new(-3.0, 3.0).unwrap(); 2],
            ],
            accuracy(),
            SsiBudget::default(),
        )
        .unwrap();
        assert!(result.curves.is_empty());
        assert_eq!(result.contacts.len(), 1);
        assert!(norm(sub(result.contacts[0], contact)) <= accuracy().intersection);
    }

    #[test]
    fn tangent_generator_is_not_collapsed_into_an_isolated_contact() {
        let mut store = GeometryStore::new();
        store.surfaces = vec![
            SurfaceGeometry::Cylinder {
                frame: Frame3::IDENTITY,
                radius: 1.0,
            },
            SurfaceGeometry::Plane {
                frame: Frame3::from_axis([1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0])
                    .unwrap(),
            },
        ];
        let result = intersect_patches(
            &mut store,
            [0, 1],
            [
                [
                    Interval::new(-0.5, 0.5).unwrap(),
                    Interval::new(-1.0, 1.0).unwrap(),
                ],
                [Interval::new(-2.0, 2.0).unwrap(); 2],
            ],
            accuracy(),
            SsiBudget::default(),
        )
        .unwrap();
        assert!(!result.curves.is_empty());
        assert!(result.contacts.is_empty());
    }

    #[test]
    fn target_patch_bounds_prune_intersections_on_the_unbounded_support() {
        let mut store = GeometryStore::new();
        store.surfaces = vec![
            SurfaceGeometry::Cylinder {
                frame: Frame3::IDENTITY,
                radius: 1.0,
            },
            SurfaceGeometry::Cylinder {
                frame: Frame3::from_axis([0.0, 0.0, 0.2], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0])
                    .unwrap(),
                radius: 0.6,
            },
        ];
        let tau = std::f64::consts::TAU;
        let result = intersect_patches(
            &mut store,
            [0, 1],
            [
                [
                    Interval::new(0.0, tau).unwrap(),
                    Interval::new(-1.0, 1.0).unwrap(),
                ],
                [
                    Interval::new(0.0, tau).unwrap(),
                    Interval::new(10.0, 12.0).unwrap(),
                ],
            ],
            accuracy(),
            SsiBudget::default(),
        )
        .unwrap();
        assert!(result.curves.is_empty());
        assert!(result.contacts.is_empty());
    }
}
