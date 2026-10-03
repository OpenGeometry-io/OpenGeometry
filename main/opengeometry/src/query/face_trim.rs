use crate::brep::{
    period_lifted_uv, BrepEnvelope, Face, GeometryError, Orientation, PcurveGeometry, Surface,
    SurfaceGeometry, UV,
};
use crate::math::{dot, norm};

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
    if matches!(surface, SurfaceGeometry::Cylinder { .. }) && face.trim.holes.len() == 1 {
        if let (Some(outer), Some(inner)) = (
            periodic_cylinder_band_limit(brep, face, face.trim.outer)?,
            periodic_cylinder_band_limit(brep, face, face.trim.holes[0])?,
        ) {
            let lo = outer.min(inner);
            let hi = outer.max(inner);
            if (uv[1] - lo).abs() <= uv_tolerance || (uv[1] - hi).abs() <= uv_tolerance {
                return Ok(None);
            }
            return Ok(Some(uv[1] > lo && uv[1] < hi));
        }
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

fn periodic_cylinder_band_limit(
    brep: &BrepEnvelope,
    face: &Face,
    loop_id: u32,
) -> Result<Option<f64>, GeometryError> {
    let loop_ = &brep.topology.loops[loop_id as usize];
    let use_ = &brep.topology.halfedges[loop_.start_halfedge as usize];
    if use_.next != Some(loop_.start_halfedge) || use_.from != use_.to {
        return Ok(None);
    }
    let edge = &brep.topology.edges[use_.edge as usize];
    let range = edge.geometry.range();
    let Some(pcurve) = use_.geometry_use.pcurve else {
        return Ok(None);
    };
    let PcurveGeometry::Line2 { direction, .. } = brep.geometry.pcurves[pcurve as usize] else {
        return Ok(None);
    };
    let period = brep.geometry.surface(face.surface)?.charts()[face.trim.chart as usize].periods[0];
    let Some(period) = period else {
        return Ok(None);
    };
    if direction[1].abs() * range.width() > brep.accuracy.geometric
        || ((direction[0] * range.width()).abs() - period).abs() > 4.0 * brep.accuracy.geometric
    {
        return Ok(None);
    }
    Ok(Some(lifted_uv(brep, loop_.start_halfedge, range.lo)?[1]))
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
    let uv = brep.geometry.pcurve_at(pcurve, parameter)?;
    let periods = brep.geometry.surface(face.surface)?.charts()[face.trim.chart as usize].periods;
    Ok(period_lifted_uv(
        uv,
        periods,
        use_.geometry_use.periodic_lift,
    ))
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
            PcurveGeometry::Line2 { direction, .. } => {
                let periods =
                    brep.geometry.surface(face.surface)?.charts()[face.trim.chart as usize].periods;
                if periods.iter().enumerate().any(|(axis, period)| {
                    period.is_some_and(|period| {
                        ((direction[axis] * range.width()).abs() - period).abs()
                            <= 4.0 * brep.accuracy.geometric
                    })
                }) {
                    64
                } else {
                    1
                }
            }
            PcurveGeometry::Conic2 { .. } => 64,
            PcurveGeometry::ProjectedCurve { .. } | PcurveGeometry::IntersectionSide { .. } => 128,
        };
        for index in 0..subdivisions {
            let fraction = if use_.geometry_use.sense == Orientation::Forward {
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
