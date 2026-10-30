use super::band_chain::seam_twin_face;
use crate::brep::{
    BrepEnvelope, CurveGeometry, EdgeGeometry, Frame3, GeometryError, Orientation, PcurveGeometry,
    Surface, SurfaceGeometry,
};
use crate::math::{dot, Interval, Point3};
use crate::operations::modifying::boolean::assembly::widen_trim_bounds;

pub(super) struct Step {
    pub(super) edge: u32,
    pub(super) from: u32,
    pub(super) to: u32,
    pub(super) sense: Orientation,
    pub(super) pcurve: PcurveGeometry,
}

pub(super) struct Directed {
    pub(super) from: u32,
    pub(super) to: u32,
    pub(super) steps: Vec<Step>,
}

pub(super) fn clear_band_seam(
    brep: &mut BrepEnvelope,
    face: u32,
    hole: u32,
) -> Result<bool, GeometryError> {
    let surface = brep.topology.faces[face as usize].surface;
    let SurfaceGeometry::Cylinder { frame, .. } = brep.geometry.surfaces[surface as usize] else {
        return Ok(true);
    };
    let Some(seam) = (0..brep.topology.edges.len() as u32).find(|edge| {
        let record = &brep.topology.edges[*edge as usize];
        record.chart_seam && seam_twin_face(brep, record) == Some(face)
    }) else {
        return Ok(true);
    };
    let tau = std::f64::consts::TAU;
    let mut loops = brep.topology.faces[face as usize].trim.holes.clone();
    loops.push(hole);
    let angles = loop_angles(brep, face, &loops)?;
    let seam_angle = brep
        .geometry
        .surface(surface)?
        .project(seam_origin(brep, seam)?, None)?[0];
    let Some((middle, gap)) = clear_seam_angle(&angles, seam_angle, tau) else {
        return Ok(true);
    };
    if gap <= 1e-3 {
        return Ok(false);
    }
    let delta = (middle - seam_angle + std::f64::consts::PI).rem_euclid(tau) - std::f64::consts::PI;
    rotate_seam(brep, face, seam, frame, delta)
}

pub(super) fn clear_seam_angle(angles: &[f64], seam: f64, period: f64) -> Option<(f64, f64)> {
    let mut sorted = angles
        .iter()
        .map(|angle| angle.rem_euclid(period))
        .collect::<Vec<_>>();
    sorted.sort_by(f64::total_cmp);
    let (start, gap) = (0..sorted.len())
        .map(|index| {
            let next = sorted[(index + 1) % sorted.len()]
                + if index + 1 == sorted.len() {
                    period
                } else {
                    0.0
                };
            (sorted[index], next - sorted[index])
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))?;
    let offset = (seam - start).rem_euclid(period);
    (offset <= 0.0 || offset >= gap).then_some(((start + 0.5 * gap).rem_euclid(period), gap))
}

fn seam_origin(brep: &BrepEnvelope, seam: u32) -> Result<Point3, GeometryError> {
    let EdgeGeometry::Curve { curve, .. } = brep.topology.edges[seam as usize].geometry else {
        return Err(GeometryError::InvalidTopology(
            "seam edge is collapsed".into(),
        ));
    };
    match brep.geometry.curves[curve as usize] {
        CurveGeometry::Line { origin, .. } => Ok(origin),
        _ => Err(GeometryError::InvalidTopology(
            "seam edge is not a line".into(),
        )),
    }
}

fn loop_angles(brep: &BrepEnvelope, face: u32, loops: &[u32]) -> Result<Vec<f64>, GeometryError> {
    let record = &brep.topology.faces[face as usize];
    let periods = brep.geometry.surface(record.surface)?.charts()[0].periods;
    let mut angles = Vec::new();
    for halfedge in &brep.topology.halfedges {
        let in_hole = halfedge
            .loop_ref
            .is_some_and(|loop_id| loops.contains(&loop_id));
        let (true, Some(pcurve), EdgeGeometry::Curve { range, .. }) = (
            in_hole,
            halfedge.geometry_use.pcurve,
            &brep.topology.edges[halfedge.edge as usize].geometry,
        ) else {
            continue;
        };
        let lift = f64::from(halfedge.geometry_use.periodic_lift[0]) * periods[0].unwrap_or(0.0);
        let angle = |parameter: f64| Ok(brep.geometry.pcurve_at(pcurve, parameter)?[0] + lift);
        angles.extend(refined_extremes(angle, range.lo, range.hi)?);
    }
    Ok(angles)
}

pub(super) fn refined_extremes(
    value: impl Fn(f64) -> Result<f64, GeometryError>,
    lo: f64,
    hi: f64,
) -> Result<[f64; 2], GeometryError> {
    let samples = (0..=8)
        .map(|step| {
            let parameter = (lo + (hi - lo) * f64::from(step) / 8.0).min(hi);
            value(parameter).map(|angle| (parameter, angle))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut extremes = [0.0; 2];
    for (slot, sign) in [(0, 1.0), (1, -1.0)] {
        let best = (0..samples.len())
            .min_by(|a, b| (sign * samples[*a].1).total_cmp(&(sign * samples[*b].1)))
            .unwrap_or(0);
        let (mut a, mut b) = (
            samples[best.saturating_sub(1)].0,
            samples[(best + 1).min(samples.len() - 1)].0,
        );
        for _ in 0..40 {
            let (first, second) = (a + (b - a) / 3.0, b - (b - a) / 3.0);
            if sign * value(first)? < sign * value(second)? {
                b = second;
            } else {
                a = first;
            }
        }
        let refined = sign * value(0.5 * (a + b))?;
        extremes[slot] = sign * refined.min(sign * samples[best].1);
    }
    Ok(extremes)
}

fn rotate_seam(
    brep: &mut BrepEnvelope,
    face: u32,
    seam: u32,
    frame: Frame3,
    delta: f64,
) -> Result<bool, GeometryError> {
    let (sin, cos) = delta.sin_cos();
    let turn = |point: Point3| {
        let local = frame.local(point);
        frame.point([
            local[0] * cos - local[1] * sin,
            local[0] * sin + local[1] * cos,
            local[2],
        ])
    };
    let outer = brep.topology.faces[face as usize].trim.outer;
    let members = brep
        .topology
        .halfedges
        .iter()
        .filter(|halfedge| halfedge.loop_ref == Some(outer))
        .map(|halfedge| (halfedge.id, halfedge.edge, halfedge.from, halfedge.to))
        .collect::<Vec<_>>();
    if members.len() != 4 {
        return Ok(false);
    }
    for (halfedge, edge, from, to) in members {
        let EdgeGeometry::Curve { curve, range } = brep.topology.edges[edge as usize].geometry
        else {
            return Ok(false);
        };
        if edge == seam {
            let Some(PcurveGeometry::Line2 { origin, .. }) = brep.topology.halfedges
                [halfedge as usize]
                .geometry_use
                .pcurve
                .and_then(|pcurve| brep.geometry.pcurves.get_mut(pcurve as usize))
            else {
                return Ok(false);
            };
            origin[0] += delta;
            continue;
        }
        let CurveGeometry::Circle { frame: circle, .. } = brep.geometry.curves[curve as usize]
        else {
            return Ok(false);
        };
        if from != to {
            return Ok(false);
        }
        let turned = if dot(circle.z, frame.z) > 0.0 {
            delta
        } else {
            -delta
        };
        brep.topology.edges[edge as usize].geometry = EdgeGeometry::Curve {
            curve,
            range: Interval::new(range.lo + turned, range.hi + turned)?,
        };
        let vertex = &mut brep.topology.vertices[from as usize];
        vertex.position = turn(vertex.position);
    }
    let EdgeGeometry::Curve { curve, .. } = brep.topology.edges[seam as usize].geometry else {
        return Ok(false);
    };
    let CurveGeometry::Line { origin, .. } = &mut brep.geometry.curves[curve as usize] else {
        return Ok(false);
    };
    *origin = turn(*origin);
    let bounds = &mut brep.topology.faces[face as usize].trim.uv_bounds[0];
    *bounds = Interval::new(bounds.lo + delta, bounds.hi + delta)?;
    relift_holes(brep, face)?;
    Ok(true)
}

fn relift_holes(brep: &mut BrepEnvelope, face: u32) -> Result<(), GeometryError> {
    let holes = &brep.topology.faces[face as usize].trim.holes;
    let members = brep
        .topology
        .halfedges
        .iter()
        .filter(|halfedge| {
            halfedge
                .loop_ref
                .is_some_and(|loop_id| holes.contains(&loop_id))
        })
        .map(|halfedge| halfedge.id)
        .collect::<Vec<_>>();
    relift(brep, face, &members)
}

pub(super) fn relift(
    brep: &mut BrepEnvelope,
    face: u32,
    members: &[u32],
) -> Result<(), GeometryError> {
    let bounds = brep.topology.faces[face as usize].trim.uv_bounds[0];
    let tau = std::f64::consts::TAU;
    for &index in members {
        let index = index as usize;
        let halfedge = &brep.topology.halfedges[index];
        let (Some(pcurve), EdgeGeometry::Curve { range, .. }) = (
            halfedge.geometry_use.pcurve,
            &brep.topology.edges[halfedge.edge as usize].geometry,
        ) else {
            continue;
        };
        let lift = halfedge.geometry_use.periodic_lift[0];
        let middle = brep
            .geometry
            .pcurve_at(pcurve, 0.5 * (range.lo + range.hi))?[0];
        let turns = ((bounds.lo + 0.5 * bounds.width() - middle) / tau).round() as i32;
        brep.topology.halfedges[index].geometry_use.periodic_lift[0] =
            if (bounds.lo..=bounds.hi).contains(&(middle + f64::from(lift) * tau)) {
                lift
            } else {
                turns
            };
    }
    Ok(())
}

pub(super) fn set_periodic_lifts(
    brep: &mut BrepEnvelope,
    loop_id: u32,
    widen: bool,
) -> Result<(), GeometryError> {
    let face_id = brep.topology.loops[loop_id as usize].face_ref;
    let face = &brep.topology.faces[face_id as usize];
    let bounds = face.trim.uv_bounds;
    let periods = brep.geometry.surface(face.surface)?.charts()[0].periods;
    let (members, mut extent) = lift_loop(brep, loop_id, periods, bounds)?;
    if !widen {
        return Ok(());
    }
    for axis in 0..2 {
        let Some(period) = periods[axis] else {
            continue;
        };
        let turns = if extent[axis][1] > bounds[axis].hi + 1e-12 * period {
            -1
        } else if extent[axis][0] < bounds[axis].lo - 1e-12 * period {
            1
        } else {
            0
        };
        for halfedge in &members {
            brep.topology.halfedges[*halfedge as usize]
                .geometry_use
                .periodic_lift[axis] += turns;
        }
        let shift = f64::from(turns) * period;
        extent[axis] = [extent[axis][0] + shift, extent[axis][1] + shift];
    }
    let pad = brep.accuracy.geometric;
    let [u, v] = extent.map(|[lo, hi]| Interval::new(lo - pad, hi + pad));
    widen_trim_bounds(brep, face_id, [u?, v?])
}

fn lift_loop(
    brep: &mut BrepEnvelope,
    loop_id: u32,
    periods: [Option<f64>; 2],
    bounds: [Interval; 2],
) -> Result<(Vec<u32>, [[f64; 2]; 2]), GeometryError> {
    let start = brep.topology.loops[loop_id as usize].start_halfedge;
    let mut previous = bounds.map(|interval| 0.5 * (interval.lo + interval.hi));
    let mut extent = [[f64::INFINITY, f64::NEG_INFINITY]; 2];
    let mut members = Vec::new();
    let mut cursor = start;
    loop {
        let record = &brep.topology.halfedges[cursor as usize];
        let (EdgeGeometry::Curve { range, .. }, Some(pcurve)) = (
            &brep.topology.edges[record.edge as usize].geometry,
            record.geometry_use.pcurve,
        ) else {
            return Err(GeometryError::InvalidTopology(
                "chain cut loop has a collapsed edge".into(),
            ));
        };
        let (first, last) = if record.geometry_use.sense == Orientation::Forward {
            (range.lo, range.hi)
        } else {
            (range.hi, range.lo)
        };
        let begin = brep.geometry.pcurve_at(pcurve, first)?;
        let lift: [i32; 2] = std::array::from_fn(|axis| {
            periods[axis].map_or(0, |period| {
                ((previous[axis] - begin[axis]) / period).round() as i32
            })
        });
        let shift = |uv: [f64; 2]| -> [f64; 2] {
            std::array::from_fn(|axis| {
                uv[axis] + f64::from(lift[axis]) * periods[axis].unwrap_or(0.0)
            })
        };
        for step in 0..=8 {
            let parameter =
                (first + (last - first) * f64::from(step) / 8.0).clamp(range.lo, range.hi);
            let uv = shift(brep.geometry.pcurve_at(pcurve, parameter)?);
            for axis in 0..2 {
                extent[axis] = [extent[axis][0].min(uv[axis]), extent[axis][1].max(uv[axis])];
            }
        }
        previous = shift(brep.geometry.pcurve_at(pcurve, last)?);
        let next = record.next;
        brep.topology.halfedges[cursor as usize]
            .geometry_use
            .periodic_lift = lift;
        members.push(cursor);
        cursor =
            next.ok_or_else(|| GeometryError::InvalidTopology("chain cut loop is open".into()))?;
        if cursor == start {
            return Ok((members, extent));
        }
    }
}
