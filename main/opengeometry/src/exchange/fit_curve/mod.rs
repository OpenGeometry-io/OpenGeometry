#[cfg(test)]
mod tests;

use crate::brep::{
    bounds_chord_deviation, Curve, CurveGeometry, GeometryError, GeometryStore,
    IntersectionDefinition, IntersectionSide, Surface, TraceAnchor, UV,
};
use crate::math::{norm, sub, Interval, Point3};

#[derive(Clone, Debug)]
pub(super) struct FittedIntersectionCurve {
    pub(super) controls: Vec<Point3>,
    pcurve_a: Vec<UV>,
    pcurve_b: Vec<UV>,
    pub(super) knots: Vec<f64>,
    pub(super) multiplicities: Vec<usize>,
    pub(super) error_bound: f64,
}

struct FittedControls {
    controls: Vec<Point3>,
    pcurve_a: Vec<UV>,
    pcurve_b: Vec<UV>,
}

pub(super) fn fit_intersection_curve(
    store: &GeometryStore,
    definition_id: u32,
    tolerance: f64,
    max_segments: usize,
) -> Result<FittedIntersectionCurve, GeometryError> {
    if !tolerance.is_finite() || tolerance <= 0.0 || max_segments == 0 {
        return Err(GeometryError::InvalidGeometry(
            "export curve fitting requires positive tolerance and segment budget".into(),
        ));
    }
    let definition = store
        .intersections
        .get(definition_id as usize)
        .ok_or_else(|| GeometryError::MissingReference {
            kind: "intersection".into(),
            index: definition_id,
        })?;
    definition.validate(store)?;
    let first = definition.anchors.first().ok_or_else(|| {
        GeometryError::InvalidGeometry("intersection curve has no first anchor".into())
    })?;
    let last = definition.anchors.last().ok_or_else(|| {
        GeometryError::InvalidGeometry("intersection curve has no last anchor".into())
    })?;
    let curve_id = store
        .curves
        .iter()
        .position(|curve| {
            matches!(curve, CurveGeometry::Intersection { definition } if *definition == definition_id)
        })
        .ok_or_else(|| GeometryError::MissingReference {
            kind: "intersection curve".into(),
            index: definition_id,
        })?;
    let curve = store
        .curve(u32::try_from(curve_id).map_err(|_| {
            GeometryError::LimitExceeded("intersection curve identifiers".into())
        })?)?;
    let (accepted, achieved) = accepted_ranges(
        store,
        definition,
        &curve,
        first,
        last,
        tolerance,
        max_segments,
    )?;
    let FittedControls {
        controls,
        pcurve_a,
        pcurve_b,
    } = fitted_controls(store, definition, &accepted, tolerance)?;
    let segment_count = accepted.len();
    let knots = (0..=segment_count).map(|value| value as f64).collect();
    let multiplicities = (0..=segment_count)
        .map(|index| {
            if index == 0 || index == segment_count {
                4
            } else {
                3
            }
        })
        .collect();
    Ok(FittedIntersectionCurve {
        controls,
        pcurve_a,
        pcurve_b,
        knots,
        multiplicities,
        error_bound: achieved,
    })
}

fn accepted_ranges(
    store: &GeometryStore,
    definition: &IntersectionDefinition,
    curve: &impl Curve,
    first: &TraceAnchor,
    last: &TraceAnchor,
    tolerance: f64,
    max_segments: usize,
) -> Result<(Vec<Interval>, f64), GeometryError> {
    let mut stack = vec![(Interval::new(first.parameter, last.parameter)?, 0usize)];
    let mut accepted = Vec::new();
    let mut visits = 0usize;
    let mut achieved: f64 = 0.0;
    while let Some((range, depth)) = stack.pop() {
        visits = visits
            .checked_add(1)
            .ok_or_else(|| GeometryError::LimitExceeded("export curve fit visits".into()))?;
        if visits > max_segments.saturating_mul(2) {
            return Err(GeometryError::LimitExceeded(
                "export curve fit visits".into(),
            ));
        }
        let a = curve.point_at(range.lo)?;
        let b = curve.point_at(range.hi)?;
        let deviation = if let Some(bound) = definition.certified_chord_deviation(range, store)? {
            bound
        } else {
            bounds_chord_deviation(curve.enclose(range)?, a, b)
        };
        if deviation <= tolerance {
            achieved = achieved.max(deviation);
            accepted.push(range);
            continue;
        }
        if depth >= 48 || accepted.len() + stack.len() + 2 > max_segments {
            return Err(GeometryError::LimitExceeded(
                "export curve fit segments".into(),
            ));
        }
        let midpoint = range.midpoint();
        stack.push((Interval::new(midpoint, range.hi)?, depth + 1));
        stack.push((Interval::new(range.lo, midpoint)?, depth + 1));
    }
    accepted.sort_by(|a, b| a.lo.total_cmp(&b.lo));
    if accepted.is_empty() {
        return Err(GeometryError::UnresolvedIntersection(
            "export curve fitting produced no intervals".into(),
        ));
    }
    Ok((accepted, achieved))
}

fn fitted_controls(
    store: &GeometryStore,
    definition: &IntersectionDefinition,
    accepted: &[Interval],
    tolerance: f64,
) -> Result<FittedControls, GeometryError> {
    let supports = [
        store.surface(definition.surfaces[0])?,
        store.surface(definition.surfaces[1])?,
    ];
    let mut controls = Vec::with_capacity(accepted.len() * 3 + 1);
    let mut pcurve_a = Vec::with_capacity(accepted.len() * 3 + 1);
    let mut pcurve_b = Vec::with_capacity(accepted.len() * 3 + 1);
    for (index, range) in accepted.iter().enumerate() {
        let left = definition.evaluate(range.lo, store)?;
        let right = definition.evaluate(range.hi, store)?;
        let xyz = line_controls(left.point, right.point);
        let uv_a = line_controls(left.uv_a, right.uv_a);
        let uv_b = line_controls(left.uv_b, right.uv_b);
        for sample in 0..=4 {
            let fraction = sample as f64 / 4.0;
            let t = range.lo + range.width() * fraction;
            let authoritative = definition.evaluate(t, store)?;
            let fitted: Point3 = std::array::from_fn(|axis| {
                left.point[axis] * (1.0 - fraction) + right.point[axis] * fraction
            });
            if norm(sub(authoritative.point, fitted)) > tolerance {
                return Err(GeometryError::UnresolvedIntersection(
                    "export curve sample exceeds its certified fitting budget".into(),
                ));
            }
            for (surface, (from, to)) in supports
                .into_iter()
                .zip([(left.uv_a, right.uv_a), (left.uv_b, right.uv_b)])
            {
                let pcurve: UV =
                    std::array::from_fn(|axis| from[axis] * (1.0 - fraction) + to[axis] * fraction);
                if norm(sub(surface.point_at(pcurve)?, fitted))
                    > tolerance + definition.residual_tolerance
                {
                    return Err(GeometryError::UnresolvedIntersection(
                        "export pcurve sample misses the fitted spatial curve".into(),
                    ));
                }
                let uv = surface.project(fitted, None)?;
                if norm(sub(surface.point_at(uv)?, fitted))
                    > tolerance + definition.residual_tolerance
                {
                    return Err(GeometryError::UnresolvedIntersection(
                        "export curve sample exceeds a retained support budget".into(),
                    ));
                }
            }
        }
        let start = usize::from(index > 0);
        controls.extend_from_slice(&xyz[start..]);
        pcurve_a.extend_from_slice(&uv_a[start..]);
        pcurve_b.extend_from_slice(&uv_b[start..]);
    }
    Ok(FittedControls {
        controls,
        pcurve_a,
        pcurve_b,
    })
}

fn line_controls<const N: usize>(a: [f64; N], b: [f64; N]) -> [[f64; N]; 4] {
    [
        a,
        std::array::from_fn(|axis| (2.0 * a[axis] + b[axis]) / 3.0),
        std::array::from_fn(|axis| (a[axis] + 2.0 * b[axis]) / 3.0),
        b,
    ]
}

impl FittedIntersectionCurve {
    pub(super) fn pcurve(&self, side: IntersectionSide) -> &[[f64; 2]] {
        if side == IntersectionSide::A {
            &self.pcurve_a
        } else {
            &self.pcurve_b
        }
    }
}
