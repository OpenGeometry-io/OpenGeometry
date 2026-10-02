use super::cells::{Root, Segment};
use super::correction::pseudo_arclength_correct;
use crate::brep::{
    unit, Accuracy, CurveGeometry, GeometryError, GeometryStore, IntersectionDefinition,
    IntersectionSide, PcurveGeometry, Surface, SurfaceGeometry, TraceAnchor, UVBox, UV,
};
use crate::intersection::ssi_result::SsiCurve;
use crate::math::{cross, norm, sub, Interval, Point3};

pub(super) fn segment_endpoint_tangent(
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

pub(super) fn normalized_key(
    uv: UV,
    domain: UVBox,
    periods: [Option<f64>; 2],
    quantum: [f64; 2],
) -> [i64; 2] {
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

pub(super) fn branch_definition(
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
    unwrap_root_angles(&mut roots, periods_a, periods_b);
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
    let uv_tubes = branch_uv_tubes(&anchors, &source_domains, accuracy)?;
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

fn unwrap_root_angles(
    roots: &mut [Root],
    periods_a: [Option<f64>; 2],
    periods_b: [Option<f64>; 2],
) {
    for index in 1..roots.len() {
        for axis in 0..2 {
            roots[index].uv_a[axis] = unwrap_angle(
                roots[index].uv_a[axis],
                roots[index - 1].uv_a[axis],
                periods_a[axis],
            );
            roots[index].uv_b[axis] = unwrap_angle(
                roots[index].uv_b[axis],
                roots[index - 1].uv_b[axis],
                periods_b[axis],
            );
        }
    }
}

fn unwrap_angle(value: f64, previous: f64, period: Option<f64>) -> f64 {
    period.map_or(value, |period| {
        value + ((previous - value) / period).round() * period
    })
}

fn branch_uv_tubes(
    anchors: &[TraceAnchor],
    source_domains: &[UVBox],
    accuracy: Accuracy,
) -> Result<Vec<[Interval; 4]>, GeometryError> {
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
    Ok(uv_tubes)
}
