use super::{
    geometry::{dot, norm, sub, PatchBounds, Surface},
    topology::{GeometryStore, IntersectionSide},
    Curve, GeometryError, Point3, UV,
};
use crate::math::interval::Interval;

#[derive(Clone, Debug)]
pub struct FittedIntersectionCurve {
    pub controls: Vec<Point3>,
    pub pcurve_a: Vec<UV>,
    pub pcurve_b: Vec<UV>,
    pub knots: Vec<f64>,
    pub multiplicities: Vec<usize>,
    pub error_bound: f64,
}

fn point_segment_distance(point: Point3, a: Point3, b: Point3) -> f64 {
    let chord = sub(b, a);
    let length_squared = dot(chord, chord);
    if length_squared == 0.0 {
        return norm(sub(point, a));
    }
    let parameter = (dot(sub(point, a), chord) / length_squared).clamp(0.0, 1.0);
    norm(sub(
        point,
        std::array::from_fn(|axis| a[axis] + parameter * chord[axis]),
    ))
}

fn bounds_chord_deviation(bounds: PatchBounds, a: Point3, b: Point3) -> f64 {
    let mut maximum: f64 = 0.0;
    for mask in 0..8 {
        let corner = std::array::from_fn(|axis| {
            if mask & (1 << axis) == 0 {
                bounds.axes[axis].lo
            } else {
                bounds.axes[axis].hi
            }
        });
        maximum = maximum.max(point_segment_distance(corner, a, b));
    }
    maximum
}

fn line_controls<const N: usize>(a: [f64; N], b: [f64; N]) -> [[f64; N]; 4] {
    [
        a,
        std::array::from_fn(|axis| (2.0 * a[axis] + b[axis]) / 3.0),
        std::array::from_fn(|axis| (a[axis] + 2.0 * b[axis]) / 3.0),
        b,
    ]
}

pub fn fit_intersection_curve(
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
            matches!(curve, super::CurveGeometry::Intersection { definition } if *definition == definition_id)
        })
        .ok_or_else(|| GeometryError::MissingReference {
            kind: "intersection curve".into(),
            index: definition_id,
        })?;
    let curve = store
        .curve(u32::try_from(curve_id).map_err(|_| {
            GeometryError::LimitExceeded("intersection curve identifiers".into())
        })?)?;
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

impl FittedIntersectionCurve {
    pub fn pcurve(&self, side: IntersectionSide) -> &[[f64; 2]] {
        if side == IntersectionSide::A {
            &self.pcurve_a
        } else {
            &self.pcurve_b
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytic::{
        intersection::SsiBudget, topology::Accuracy, universal_ssi::intersect_patches, Frame3,
        SurfaceGeometry,
    };

    #[test]
    fn fitted_numerical_curve_has_cubic_shape_and_support_bound() {
        let mut store = GeometryStore::new();
        store.surfaces = vec![
            SurfaceGeometry::Torus {
                frame: Frame3::IDENTITY,
                major_radius: 2.0,
                minor_radius: 0.5,
            },
            SurfaceGeometry::Plane {
                frame: Frame3::from_axis([0.0, 0.0, 0.15], [0.35, -0.2, 1.0], [1.0, 0.0, 0.0])
                    .unwrap(),
            },
        ];
        let tau = std::f64::consts::TAU;
        let result = intersect_patches(
            &mut store,
            [0, 1],
            [
                [Interval::new(0.0, tau).unwrap(); 2],
                [Interval::new(-4.0, 4.0).unwrap(); 2],
            ],
            Accuracy {
                geometric: 1e-5,
                intersection: 2.5e-6,
                tessellation: 1e-3,
                exchange: 0.05,
            },
            SsiBudget::default(),
        )
        .unwrap();
        let definition = match store.curves[result.curves[0].curve as usize] {
            super::super::CurveGeometry::Intersection { definition } => definition,
            _ => panic!("universal SSI must create an intersection definition"),
        };
        let fitted = fit_intersection_curve(&store, definition, 1.0, 20_000).unwrap();
        assert!(fitted.controls.len() >= 4);
        assert_eq!((fitted.controls.len() - 1) % 3, 0);
        assert_eq!(fitted.controls.len(), fitted.pcurve_a.len());
        assert_eq!(fitted.controls.len(), fitted.pcurve_b.len());
        assert!(fitted.error_bound <= 1.0);
        assert_eq!(fitted.knots.len(), fitted.multiplicities.len());
        assert_eq!(fitted.multiplicities.first(), Some(&4));
        assert_eq!(fitted.multiplicities.last(), Some(&4));
    }
}
