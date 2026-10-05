use crate::brep::{
    BrepEnvelope, Face, GeometryError, GeometryStore, PatchBounds, PcurveGeometry, Surface, UVBox,
};
use crate::math::{quadratic, Interval, QuadraticRoots};

pub(super) fn face_patch_bounds(
    brep: &BrepEnvelope,
    face: &Face,
) -> Result<PatchBounds, GeometryError> {
    brep.geometry
        .surface(face.surface)?
        .enclose(face.trim.uv_bounds)
}

pub(super) fn clip_coordinate(
    origin: f64,
    direction: f64,
    bounds: Interval,
    range: &mut [f64; 2],
) -> bool {
    if direction == 0.0 {
        return bounds.contains(origin);
    }
    let mut lo = (bounds.lo - origin) / direction;
    let mut hi = (bounds.hi - origin) / direction;
    if lo > hi {
        std::mem::swap(&mut lo, &mut hi);
    }
    range[0] = range[0].max(lo);
    range[1] = range[1].min(hi);
    range[0] <= range[1]
}

pub(super) fn pcurve_line(
    store: &GeometryStore,
    id: u32,
) -> Result<([f64; 2], [f64; 2]), GeometryError> {
    match store.pcurves.get(id as usize) {
        Some(PcurveGeometry::Line2 { origin, direction }) => Ok((*origin, *direction)),
        _ => Err(GeometryError::CoverageGap {
            families: ["plane trim".into(), "nonlinear intersection pcurve".into()],
        }),
    }
}

pub(super) fn boundary_parameters(
    brep: &BrepEnvelope,
    face_id: u32,
    line_origin: [f64; 2],
    line_direction: [f64; 2],
    output: &mut Vec<f64>,
) -> Result<(), GeometryError> {
    let face = brep.topology.faces.get(face_id as usize).ok_or_else(|| {
        GeometryError::MissingReference {
            kind: "face".into(),
            index: face_id,
        }
    })?;
    for loop_id in std::iter::once(face.trim.outer).chain(face.trim.holes.iter().copied()) {
        let loop_ = &brep.topology.loops[loop_id as usize];
        let start = loop_.start_halfedge;
        let mut current = start;
        for _ in 0..=brep.topology.halfedges.len() {
            let use_ = &brep.topology.halfedges[current as usize];
            let pcurve_id = use_.geometry_use.pcurve.ok_or_else(|| {
                GeometryError::InvalidTopology("trim boundary is missing its pcurve".into())
            })?;
            let edge_range = brep.topology.edges[use_.edge as usize].geometry.range();
            add_pcurve_crossings(
                &brep.geometry.pcurves[pcurve_id as usize],
                line_origin,
                line_direction,
                edge_range,
                output,
            )?;
            current = use_.next.ok_or_else(|| {
                GeometryError::InvalidTopology("open face loop during intersection".into())
            })?;
            if current == start {
                break;
            }
        }
        if current != start {
            return Err(GeometryError::InvalidTopology(
                "face loop does not close during intersection".into(),
            ));
        }
    }
    Ok(())
}

fn add_pcurve_crossings(
    pcurve: &PcurveGeometry,
    line_origin: [f64; 2],
    line_direction: [f64; 2],
    edge_range: Interval,
    output: &mut Vec<f64>,
) -> Result<(), GeometryError> {
    let delta_cross = |a: [f64; 2], b: [f64; 2]| a[0] * b[1] - a[1] * b[0];
    match pcurve {
        PcurveGeometry::Line2 { origin, direction } => {
            let determinant = delta_cross(line_direction, *direction);
            if determinant != 0.0 {
                let delta = [origin[0] - line_origin[0], origin[1] - line_origin[1]];
                let t = delta_cross(delta, *direction) / determinant;
                let s = delta_cross(delta, line_direction) / determinant;
                if edge_range.contains(s) {
                    output.push(t);
                }
            }
        }
        PcurveGeometry::Conic2 {
            origin,
            axis_a,
            axis_b,
        } => {
            let determinant = delta_cross(*axis_a, *axis_b);
            if determinant == 0.0 {
                return Err(GeometryError::InvalidGeometry(
                    "degenerate planar conic trim".into(),
                ));
            }
            let inverse = |vector: [f64; 2]| {
                [
                    delta_cross(vector, *axis_b) / determinant,
                    delta_cross(*axis_a, vector) / determinant,
                ]
            };
            let offset = inverse([line_origin[0] - origin[0], line_origin[1] - origin[1]]);
            let rate = inverse(line_direction);
            let roots = quadratic(
                rate[0] * rate[0] + rate[1] * rate[1],
                2.0 * (offset[0] * rate[0] + offset[1] * rate[1]),
                offset[0] * offset[0] + offset[1] * offset[1] - 1.0,
            )?;
            let candidates: Vec<f64> = match roots {
                QuadraticRoots::Empty => Vec::new(),
                QuadraticRoots::One(root) => vec![root],
                QuadraticRoots::Two(roots) => roots.to_vec(),
                QuadraticRoots::IdenticallyZero => {
                    return Err(GeometryError::UnresolvedIntersection(
                        "intersection curve coincides with a conic trim".into(),
                    ))
                }
            };
            for t in candidates {
                let coordinates = [offset[0] + rate[0] * t, offset[1] + rate[1] * t];
                let parameter = coordinates[1].atan2(coordinates[0]);
                let tau = std::f64::consts::TAU;
                if (-2..=2).any(|turn| edge_range.contains(parameter + f64::from(turn) * tau)) {
                    output.push(t);
                }
            }
        }
        PcurveGeometry::ProjectedCurve { .. } | PcurveGeometry::IntersectionSide { .. } => {
            return Err(GeometryError::CoverageGap {
                families: ["plane trim".into(), "general numerical boundary".into()],
            })
        }
    }
    Ok(())
}

pub(super) fn uv_in_bounds(
    geometry: &GeometryStore,
    surface: u32,
    bounds: UVBox,
    mut uv: [f64; 2],
    tolerance: f64,
) -> Result<bool, GeometryError> {
    let periods = geometry.surface(surface)?.charts()[0].periods;
    for axis in 0..2 {
        uv[axis] = periodic_value(uv[axis], bounds[axis], periods[axis]);
        if uv[axis] < bounds[axis].lo - tolerance || uv[axis] > bounds[axis].hi + tolerance {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn periodic_value(value: f64, bounds: Interval, period: Option<f64>) -> f64 {
    period.map_or(value, |period| {
        value + ((bounds.midpoint() - value) / period).round() * period
    })
}

pub(super) fn is_uv_box_trim(brep: &BrepEnvelope, face: &Face) -> Result<bool, GeometryError> {
    if !face.trim.holes.is_empty() {
        return Ok(false);
    }
    let surface = brep.geometry.surface(face.surface)?;
    let periods = surface.charts()[face.trim.chart as usize].periods;
    let tolerance = brep.accuracy.intersection;
    let mut sides = [false; 4];
    for halfedge in loop_halfedges(brep, face.trim.outer)? {
        let use_ = &brep.topology.halfedges[halfedge as usize];
        let pcurve = use_.geometry_use.pcurve.ok_or_else(|| {
            GeometryError::InvalidTopology("trim boundary is missing its pcurve".into())
        })?;
        let PcurveGeometry::Line2 { origin, direction } = brep.geometry.pcurves[pcurve as usize]
        else {
            return Ok(false);
        };
        let range = brep.topology.edges[use_.edge as usize].geometry.range();
        let mut ends: [[f64; 2]; 2] = [range.lo, range.hi].map(|parameter| {
            std::array::from_fn(|axis| origin[axis] + direction[axis] * parameter)
        });
        for end in &mut ends {
            for axis in 0..2 {
                end[axis] = periodic_value(end[axis], face.trim.uv_bounds[axis], periods[axis]);
            }
        }
        let delta = [ends[1][0] - ends[0][0], ends[1][1] - ends[0][1]];
        let constant_axis = if delta[0].abs() <= tolerance {
            0
        } else if delta[1].abs() <= tolerance {
            1
        } else {
            return Ok(false);
        };
        let varying_axis = 1 - constant_axis;
        let constant = 0.5 * (ends[0][constant_axis] + ends[1][constant_axis]);
        let bounds = face.trim.uv_bounds[constant_axis];
        let side = if (constant - bounds.lo).abs() <= tolerance {
            2 * constant_axis
        } else if (constant - bounds.hi).abs() <= tolerance {
            2 * constant_axis + 1
        } else {
            return Ok(false);
        };
        if ends.iter().any(|end| {
            end[varying_axis] < face.trim.uv_bounds[varying_axis].lo - tolerance
                || end[varying_axis] > face.trim.uv_bounds[varying_axis].hi + tolerance
        }) {
            return Ok(false);
        }
        sides[side] = true;
    }
    Ok(sides.into_iter().all(|present| present))
}

fn loop_halfedges(brep: &BrepEnvelope, loop_id: u32) -> Result<Vec<u32>, GeometryError> {
    let loop_ = brep.topology.loops.get(loop_id as usize).ok_or_else(|| {
        GeometryError::MissingReference {
            kind: "loop".into(),
            index: loop_id,
        }
    })?;
    let mut current = loop_.start_halfedge;
    let mut result = Vec::new();
    for _ in 0..=brep.topology.halfedges.len() {
        result.push(current);
        current = brep.topology.halfedges[current as usize]
            .next
            .ok_or_else(|| GeometryError::InvalidTopology("open face loop during SSI".into()))?;
        if current == loop_.start_halfedge {
            return Ok(result);
        }
    }
    Err(GeometryError::InvalidTopology(
        "face loop does not close during SSI".into(),
    ))
}
