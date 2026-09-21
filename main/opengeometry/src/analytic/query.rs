use super::{
    geometry::{dot, norm, scale, sub, unit, Surface},
    topology::{BrepEnvelope, Face, PcurveGeometry},
    GeometryError, Point3, SurfaceGeometry, UV,
};
use crate::math::{
    interval::Interval,
    roots::{isolate_candidates, quadratic, QuadraticRoots},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointClassification {
    Inside,
    Outside,
    Boundary,
    Unknown,
}

fn lifted_uv(brep: &BrepEnvelope, halfedge: u32, parameter: f64) -> Result<UV, GeometryError> {
    let use_ = &brep.topology.halfedges[halfedge as usize];
    let face = &brep.topology.faces[use_
        .face
        .ok_or_else(|| GeometryError::InvalidTopology("trim query halfedge has no face".into()))?
        as usize];
    let pcurve = use_.geometry_use.pcurve.ok_or_else(|| {
        GeometryError::InvalidTopology("trim query halfedge has no pcurve".into())
    })?;
    let mut uv = brep.geometry.pcurve_at(pcurve, parameter)?;
    let periods = brep.geometry.surface(face.surface)?.charts()[face.trim.chart as usize].periods;
    for axis in 0..2 {
        uv[axis] += periods[axis].map_or(0.0, |period| {
            period * f64::from(use_.geometry_use.periodic_lift[axis])
        });
    }
    Ok(uv)
}

fn trim_samples(brep: &BrepEnvelope, face: &Face, loop_id: u32) -> Result<Vec<UV>, GeometryError> {
    let loop_ = &brep.topology.loops[loop_id as usize];
    let start = loop_.start_halfedge;
    let mut current = start;
    let mut points = Vec::new();
    for _ in 0..=brep.topology.halfedges.len() {
        let use_ = &brep.topology.halfedges[current as usize];
        if use_.face != Some(face.id) {
            return Err(GeometryError::InvalidTopology(
                "trim query crossed into another face".into(),
            ));
        }
        let edge = &brep.topology.edges[use_.edge as usize];
        let range = edge.geometry.range();
        let pcurve = use_.geometry_use.pcurve.ok_or_else(|| {
            GeometryError::InvalidTopology("trim query halfedge has no pcurve".into())
        })?;
        let subdivisions = match brep.geometry.pcurves[pcurve as usize] {
            PcurveGeometry::Line2 { .. } => 1,
            PcurveGeometry::Conic2 { .. } => 64,
            PcurveGeometry::ProjectedCurve { .. } | PcurveGeometry::IntersectionSide { .. } => 128,
        };
        for index in 0..subdivisions {
            let fraction = if use_.geometry_use.sense == super::topology::Orientation::Forward {
                index as f64 / subdivisions as f64
            } else {
                1.0 - index as f64 / subdivisions as f64
            };
            points.push(lifted_uv(
                brep,
                current,
                range.lo + range.width() * fraction,
            )?);
        }
        current = use_
            .next
            .ok_or_else(|| GeometryError::InvalidTopology("open trim loop in query".into()))?;
        if current == start {
            break;
        }
    }
    if current != start || points.len() < 3 {
        return Err(GeometryError::InvalidTopology(
            "trim query requires a closed nondegenerate loop".into(),
        ));
    }
    Ok(points)
}

fn segment_distance_2d(point: UV, a: UV, b: UV) -> f64 {
    let direction = [b[0] - a[0], b[1] - a[1]];
    let length_squared = dot(
        [direction[0], direction[1], 0.0],
        [direction[0], direction[1], 0.0],
    );
    if length_squared == 0.0 {
        return (point[0] - a[0]).hypot(point[1] - a[1]);
    }
    let parameter = (((point[0] - a[0]) * direction[0] + (point[1] - a[1]) * direction[1])
        / length_squared)
        .clamp(0.0, 1.0);
    (point[0] - a[0] - parameter * direction[0]).hypot(point[1] - a[1] - parameter * direction[1])
}

fn in_loop(point: UV, polygon: &[UV], tolerance: f64) -> Option<bool> {
    let mut inside = false;
    for index in 0..polygon.len() {
        let a = polygon[index];
        let b = polygon[(index + 1) % polygon.len()];
        if segment_distance_2d(point, a, b) <= tolerance {
            return None;
        }
        if (a[1] > point[1]) != (b[1] > point[1]) {
            let x = a[0] + (point[1] - a[1]) * (b[0] - a[0]) / (b[1] - a[1]);
            if x > point[0] {
                inside = !inside;
            }
        }
    }
    Some(inside)
}

fn lift_to_face(surface: &SurfaceGeometry, face: &Face, mut uv: UV) -> UV {
    let periods = surface.charts()[face.trim.chart as usize].periods;
    for axis in 0..2 {
        if let Some(period) = periods[axis] {
            let center = face.trim.uv_bounds[axis].midpoint();
            uv[axis] += ((center - uv[axis]) / period).round() * period;
        }
    }
    uv
}

pub(crate) fn face_contains_uv(
    brep: &BrepEnvelope,
    face: &Face,
    uv: UV,
) -> Result<Option<bool>, GeometryError> {
    let surface = brep.geometry.surface(face.surface)?;
    let uv = lift_to_face(surface, face, uv);
    let jet = surface.derivatives(uv)?;
    let metric = norm(jet.du).min(norm(jet.dv));
    let uv_tolerance = if metric > 0.0 {
        brep.accuracy.geometric / metric
    } else {
        return Ok(None);
    };
    if !face.trim.uv_bounds[0].contains(uv[0]) || !face.trim.uv_bounds[1].contains(uv[1]) {
        return Ok(Some(false));
    }
    let outer = trim_samples(brep, face, face.trim.outer)?;
    let Some(mut inside) = in_loop(uv, &outer, uv_tolerance) else {
        return Ok(None);
    };
    if !inside {
        return Ok(Some(false));
    }
    for hole in &face.trim.holes {
        let polygon = trim_samples(brep, face, *hole)?;
        match in_loop(uv, &polygon, uv_tolerance) {
            None => return Ok(None),
            Some(true) => inside = false,
            Some(false) => {}
        }
    }
    Ok(Some(inside))
}

fn polynomial_value(coefficients: &[f64], value: f64) -> f64 {
    coefficients
        .iter()
        .rev()
        .fold(0.0, |sum, coefficient| sum.mul_add(value, *coefficient))
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

fn support_roots(
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
        } => {
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
            let candidates = isolate_candidates(&coefficients, domain, tolerance, 100_000)
                .map_err(|error| {
                    GeometryError::UnresolvedIntersection(format!(
                        "torus ray root isolation: {error:?}"
                    ))
                })?;
            Some(
                candidates
                    .intervals
                    .into_iter()
                    .map(|candidate| refine_polynomial_root(&coefficients, candidate))
                    .collect(),
            )
        }
    };
    Ok(result.map(|roots| {
        roots
            .into_iter()
            .filter(|root| domain.contains(*root))
            .collect()
    }))
}

fn ray_box_domain(
    brep: &BrepEnvelope,
    origin: Point3,
    direction: Point3,
) -> Result<Option<Interval>, GeometryError> {
    let Some(bounds) = brep.bounds()? else {
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

fn classify_with_ray(
    brep: &BrepEnvelope,
    point: Point3,
    direction: Point3,
) -> Result<PointClassification, GeometryError> {
    let Some(domain) = ray_box_domain(brep, point, direction)? else {
        return Ok(PointClassification::Outside);
    };
    let mut hits: Vec<(f64, Point3)> = Vec::new();
    for face in &brep.topology.faces {
        let surface = brep.geometry.surface(face.surface)?;
        let Some(roots) = support_roots(
            surface,
            point,
            direction,
            domain,
            brep.accuracy.intersection,
        )?
        else {
            return Ok(PointClassification::Unknown);
        };
        for parameter in roots {
            let position = [
                point[0] + parameter * direction[0],
                point[1] + parameter * direction[1],
                point[2] + parameter * direction[2],
            ];
            let uv = match surface.project(position, None) {
                Ok(uv) => uv,
                Err(super::GeometryError::SingularParameterization) => {
                    return Ok(PointClassification::Unknown)
                }
                Err(error) => return Err(error),
            };
            if norm(sub(surface.point_at(uv)?, position)) > brep.accuracy.intersection {
                continue;
            }
            match face_contains_uv(brep, face, uv)? {
                Some(true) => {}
                Some(false) => continue,
                None => return Ok(PointClassification::Unknown),
            }
            let normal = scale(surface.normal_at(uv)?, face.sense.multiplier());
            if dot(normal, direction).abs() <= 64.0 * f64::EPSILON {
                return Ok(PointClassification::Unknown);
            }
            if parameter <= brep.accuracy.geometric {
                return Ok(PointClassification::Boundary);
            }
            if hits
                .iter()
                .all(|(existing, _)| (existing - parameter).abs() > brep.accuracy.geometric)
            {
                hits.push((parameter, position));
            }
        }
    }
    Ok(if hits.len() % 2 == 1 {
        PointClassification::Inside
    } else {
        PointClassification::Outside
    })
}

pub fn classify_point(
    brep: &BrepEnvelope,
    point: Point3,
) -> Result<PointClassification, GeometryError> {
    brep.validate()?;
    if point.into_iter().any(|value| !value.is_finite()) {
        return Err(GeometryError::InvalidGeometry(
            "point classification requires finite coordinates".into(),
        ));
    }
    let directions = [
        unit([1.0, 0.371, 0.127])?,
        unit([0.193, 1.0, 0.419])?,
        unit([0.311, 0.233, 1.0])?,
    ];
    for direction in directions {
        let result = classify_with_ray(brep, point, direction)?;
        if result != PointClassification::Unknown {
            return Ok(result);
        }
    }
    Ok(PointClassification::Unknown)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytic::{
        booleans::{boolean_brep, BooleanOp},
        primitives,
        topology::Accuracy,
        Frame3,
    };

    fn accuracy() -> Accuracy {
        Accuracy {
            geometric: 1e-8,
            intersection: 1e-9,
            tessellation: 0.01,
            exchange: 1e-5,
        }
    }

    #[test]
    fn analytic_ray_classification_covers_all_surface_families() {
        let bodies = [
            (
                primitives::cuboid("box".into(), Frame3::IDENTITY, [2.0; 3], accuracy()).unwrap(),
                [1.0, 1.0, 1.0],
                [3.0, 3.0, 3.0],
            ),
            (
                primitives::sphere("sphere".into(), Frame3::IDENTITY, 1.0, accuracy()).unwrap(),
                [0.0; 3],
                [2.0, 0.0, 0.0],
            ),
            (
                primitives::cylinder("cylinder".into(), Frame3::IDENTITY, 1.0, 2.0, accuracy())
                    .unwrap(),
                [0.0, 0.0, 1.0],
                [2.0, 0.0, 1.0],
            ),
            (
                primitives::cone("cone".into(), Frame3::IDENTITY, 1.0, 2.0, accuracy()).unwrap(),
                [0.0, 0.0, 1.0],
                [2.0, 0.0, 1.0],
            ),
            (
                primitives::torus("torus".into(), Frame3::IDENTITY, 3.0, 1.0, accuracy()).unwrap(),
                [3.0, 0.0, 0.0],
                [0.0; 3],
            ),
        ];
        for (body, inside, outside) in bodies {
            assert_eq!(
                classify_point(&body, inside).unwrap(),
                PointClassification::Inside,
                "{}",
                body.id
            );
            assert_eq!(
                classify_point(&body, outside).unwrap(),
                PointClassification::Outside,
                "{}",
                body.id
            );
        }
    }

    #[test]
    fn cavity_shell_uses_regularized_material_occupancy() {
        let host =
            primitives::torus("host".into(), Frame3::IDENTITY, 3.0, 1.0, accuracy()).unwrap();
        let cutter =
            primitives::torus("cutter".into(), Frame3::IDENTITY, 3.0, 0.4, accuracy()).unwrap();
        let result = boolean_brep(&host, &cutter, BooleanOp::Subtraction, "shell".into()).unwrap();
        assert_eq!(
            classify_point(&result.brep, [3.0, 0.0, 0.0]).unwrap(),
            PointClassification::Outside
        );
        assert_eq!(
            classify_point(&result.brep, [3.7, 0.0, 0.0]).unwrap(),
            PointClassification::Inside
        );
        assert_eq!(
            classify_point(&result.brep, [0.0; 3]).unwrap(),
            PointClassification::Outside
        );
    }
}
