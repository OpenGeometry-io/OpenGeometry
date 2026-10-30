use crate::brep::{
    period_lifted_uv, BrepEnvelope, EdgeGeometry, Face, GeometryError, HalfEdge, Orientation,
    Surface, SurfaceGeometry,
};
use crate::math::Point3;

pub(super) fn trim_loop_samples(
    brep: &BrepEnvelope,
    face: &Face,
    loop_id: u32,
    samples: &[Vec<Point3>],
) -> Result<Vec<([f64; 2], Point3)>, GeometryError> {
    let periods = brep.geometry.surface(face.surface)?.charts()[face.trim.chart as usize].periods;
    let start = brep.topology.loops[loop_id as usize].start_halfedge;
    let mut current = start;
    let mut result = Vec::new();
    loop {
        let halfedge = &brep.topology.halfedges[current as usize];
        let edge = &brep.topology.edges[halfedge.edge as usize];
        let source = &samples[halfedge.edge as usize];
        let pcurve = halfedge
            .geometry_use
            .pcurve
            .ok_or_else(|| GeometryError::InvalidTopology("missing face pcurve".into()))?;
        let range = match edge.geometry {
            EdgeGeometry::Curve { range, .. } => range,
            EdgeGeometry::Collapsed { vertex } => {
                let parameter = if halfedge.geometry_use.sense == Orientation::Forward {
                    0.0
                } else {
                    1.0
                };
                let uv = lifted_uv(brep, halfedge, pcurve, parameter, periods)?;
                result.push((uv, brep.topology.vertices[vertex as usize].position));
                current = halfedge
                    .next
                    .ok_or_else(|| GeometryError::InvalidTopology("open face loop".into()))?;
                if current == start {
                    break;
                }
                continue;
            }
        };
        let segment_count = source.len().saturating_sub(1);
        if segment_count == 0 {
            return Err(GeometryError::InvalidTopology(
                "empty curved trim edge sample".into(),
            ));
        }
        for offset in 0..segment_count {
            let sample_index = if halfedge.geometry_use.sense == Orientation::Forward {
                offset
            } else {
                segment_count - offset
            };
            let parameter = range.lo + range.width() * sample_index as f64 / segment_count as f64;
            let uv = lifted_uv(brep, halfedge, pcurve, parameter, periods)?;
            result.push((uv, source[sample_index]));
        }
        current = halfedge
            .next
            .ok_or_else(|| GeometryError::InvalidTopology("open face loop".into()))?;
        if current == start {
            break;
        }
        if result.len()
            > brep
                .topology
                .halfedges
                .len()
                .saturating_mul(maximum_edge_samples(samples))
        {
            return Err(GeometryError::LimitExceeded(
                "curved trim loop samples".into(),
            ));
        }
    }
    if result.len() < 4 {
        return Err(GeometryError::InvalidTopology(
            "curved trim loop requires at least four samples".into(),
        ));
    }
    Ok(result)
}

fn lifted_uv(
    brep: &BrepEnvelope,
    halfedge: &HalfEdge,
    pcurve: u32,
    parameter: f64,
    periods: [Option<f64>; 2],
) -> Result<[f64; 2], GeometryError> {
    let uv = brep.geometry.pcurve_at(pcurve, parameter)?;
    Ok(period_lifted_uv(
        uv,
        periods,
        halfedge.geometry_use.periodic_lift,
    ))
}

fn maximum_edge_samples(samples: &[Vec<Point3>]) -> usize {
    samples.iter().map(Vec::len).max().unwrap_or(1)
}

pub(super) fn periodic_band_loop(
    face: &Face,
    periods: [Option<f64>; 2],
    loops: &[Vec<([f64; 2], Point3)>],
) -> Option<Vec<([f64; 2], Point3)>> {
    if loops.len() != 2 {
        return None;
    }
    for axis in 0..2 {
        let Some(period) = periods[axis] else {
            continue;
        };
        let windings = [
            periodic_ring_winding(&loops[0], axis, period),
            periodic_ring_winding(&loops[1], axis, period),
        ];
        if windings.iter().any(|winding| winding.abs() != 1) {
            continue;
        }
        let other = 1 - axis;
        if let Some(other_period) = periods[other] {
            if loops
                .iter()
                .any(|ring| periodic_ring_winding(ring, other, other_period) != 0)
            {
                continue;
            }
        }
        let seam = face.trim.uv_bounds[axis].lo;
        let mut first = open_periodic_ring(&loops[0], axis, period, seam, periods[other])?;
        let mut second = open_periodic_ring(&loops[1], axis, period, seam, periods[other])?;
        let first_mean = first.iter().map(|(uv, _)| uv[other]).sum::<f64>() / first.len() as f64;
        let second_mean = second.iter().map(|(uv, _)| uv[other]).sum::<f64>() / second.len() as f64;
        if first_mean > second_mean {
            std::mem::swap(&mut first, &mut second);
        }
        second.reverse();
        first.extend(second);
        return Some(first);
    }
    None
}

fn periodic_ring_winding(ring: &[([f64; 2], Point3)], axis: usize, period: f64) -> i32 {
    let delta = ring
        .iter()
        .zip(ring.iter().cycle().skip(1))
        .take(ring.len())
        .map(|(a, b)| {
            let raw = b.0[axis] - a.0[axis];
            raw - (raw / period).round() * period
        })
        .sum::<f64>();
    (delta / period).round() as i32
}

fn open_periodic_ring(
    ring: &[([f64; 2], Point3)],
    axis: usize,
    period: f64,
    seam: f64,
    other_period: Option<f64>,
) -> Option<Vec<([f64; 2], Point3)>> {
    let other = 1 - axis;
    let mut sorted = ring
        .iter()
        .map(|&(mut uv, point)| {
            uv[axis] = seam + (uv[axis] - seam).rem_euclid(period);
            (uv, point)
        })
        .collect::<Vec<_>>();
    sorted.sort_by(|a, b| a.0[axis].total_cmp(&b.0[axis]));
    let tolerance = 256.0 * f64::EPSILON * period.max(1.0);
    sorted.dedup_by(|a, b| (a.0[axis] - b.0[axis]).abs() <= tolerance);
    if sorted.len() < 3 {
        return None;
    }

    let first = sorted[0];
    let last = *sorted.last()?;
    let before = last.0[axis] - period;
    let span = first.0[axis] - before;
    if span <= tolerance {
        return None;
    }
    let t = ((seam - before) / span).clamp(0.0, 1.0);
    let mut other_delta = first.0[other] - last.0[other];
    if let Some(other_period) = other_period {
        other_delta -= (other_delta / other_period).round() * other_period;
    }
    let other_value = last.0[other] + other_delta * t;
    let seam_point = std::array::from_fn(|component| {
        last.1[component] + (first.1[component] - last.1[component]) * t
    });
    let mut seam_uv = first.0;
    seam_uv[axis] = seam;
    seam_uv[other] = other_value;

    let mut opened = Vec::with_capacity(sorted.len() + 2);
    opened.push((seam_uv, seam_point));
    opened.extend(
        sorted
            .into_iter()
            .filter(|(uv, _)| (uv[axis] - seam).abs() > tolerance),
    );
    let mut end_uv = seam_uv;
    end_uv[axis] += period;
    opened.push((end_uv, seam_point));
    Some(opened)
}

pub(super) fn merged_grid_coordinates(
    uniform: impl Iterator<Item = f64>,
    boundary: impl Iterator<Item = f64>,
) -> Vec<f64> {
    let mut values = uniform
        .map(|value| (value, false))
        .chain(boundary.map(|value| (value, true)))
        .collect::<Vec<_>>();
    values.sort_by(|a, b| a.0.total_cmp(&b.0));
    let magnitude = values
        .iter()
        .map(|(value, _)| value.abs())
        .fold(1.0_f64, f64::max);
    let tolerance = 128.0 * f64::EPSILON * magnitude;
    let mut merged: Vec<(f64, bool)> = Vec::with_capacity(values.len());
    for value in values {
        if let Some(last) = merged.last_mut() {
            if (value.0 - last.0).abs() <= tolerance {
                if value.1 {
                    *last = value;
                }
                continue;
            }
        }
        merged.push(value);
    }
    merged.into_iter().map(|(value, _)| value).collect()
}

pub(super) fn trimmed_surface_edge_deviation(
    surface: &SurfaceGeometry,
    uv: &[[f64; 2]],
    a: usize,
    b: usize,
) -> f64 {
    let du = (uv[b][0] - uv[a][0]).abs();
    let dv = (uv[b][1] - uv[a][1]).abs();
    let v = [uv[a][1].min(uv[b][1]), uv[a][1].max(uv[b][1])];
    let maximum_trig = |offset: f64, values: fn(f64) -> f64| {
        let period = std::f64::consts::PI;
        let contains_extremum =
            ((v[0] - offset) / period).ceil() <= ((v[1] - offset) / period).floor();
        if contains_extremum {
            1.0
        } else {
            values(v[0]).abs().max(values(v[1]).abs())
        }
    };
    let acceleration = match surface {
        SurfaceGeometry::Plane { .. } => 0.0,
        SurfaceGeometry::Sphere { radius, .. } => {
            let cos_v = maximum_trig(0.0, f64::cos);
            let sin_v = maximum_trig(std::f64::consts::FRAC_PI_2, f64::sin);
            radius * (cos_v * du * du + 2.0 * sin_v * du * dv + dv * dv)
        }
        SurfaceGeometry::Cylinder { radius, .. } => radius * du * du,
        SurfaceGeometry::Cone { semi_angle, .. } => {
            let slope = semi_angle.tan();
            let radius = uv[a][1].abs().max(uv[b][1].abs()) * slope;
            radius * du * du + 2.0 * slope * du * dv
        }
        SurfaceGeometry::Torus {
            major_radius,
            minor_radius,
            ..
        } => {
            (major_radius + minor_radius) * du * du
                + 2.0 * minor_radius * du * dv
                + minor_radius * dv * dv
        }
    };
    acceleration / 8.0
}
