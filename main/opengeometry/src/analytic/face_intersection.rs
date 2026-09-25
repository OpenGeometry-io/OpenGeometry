use super::{
    geometry::{PatchBounds, UVBox},
    intersection::SsiBudget,
    query::face_contains_uv,
    ssi::{intersect_surfaces, SsiCurve, SsiResult},
    topology::{Accuracy, BrepEnvelope, Face, GeometryStore, PcurveGeometry},
    universal_ssi, Curve, GeometryError, Point3, Surface, SurfaceGeometry,
};
use crate::math::{
    interval::Interval,
    roots::{quadratic, QuadraticRoots},
};

#[derive(Clone, Copy)]
pub struct FaceView<'a> {
    pub brep: &'a BrepEnvelope,
    pub face: u32,
}

pub struct IntersectionBranch {
    pub curve: u32,
    pub pcurves: [u32; 2],
    pub range: Interval,
    pub endpoints: [Point3; 2],
}

pub struct IntersectionGraph {
    pub geometry: GeometryStore,
    pub branches: Vec<IntersectionBranch>,
    pub contacts: Vec<Point3>,
    pub coincident: bool,
}

pub struct FacePairIntersection {
    pub faces: [u32; 2],
    pub graph: IntersectionGraph,
}

pub struct BodyIntersectionGraph {
    pub pairs: Vec<FacePairIntersection>,
}

fn combined_accuracy(a: Accuracy, b: Accuracy) -> Accuracy {
    Accuracy {
        geometric: a.geometric.max(b.geometric),
        intersection: a.intersection.max(b.intersection),
        tessellation: a.tessellation.max(b.tessellation),
        exchange: a.exchange.max(b.exchange),
    }
}

fn patch_bounds_overlap(a: PatchBounds, b: PatchBounds, tolerance: f64) -> bool {
    (0..3).all(|axis| {
        a.axes[axis].lo <= b.axes[axis].hi + tolerance
            && a.axes[axis].hi + tolerance >= b.axes[axis].lo
    })
}

fn face_patch_bounds(brep: &BrepEnvelope, face: &Face) -> Result<PatchBounds, GeometryError> {
    brep.geometry
        .surface(face.surface)?
        .enclose(face.trim.uv_bounds)
}

fn clip_coordinate(origin: f64, direction: f64, bounds: Interval, range: &mut [f64; 2]) -> bool {
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

fn pcurve_line(store: &GeometryStore, id: u32) -> Result<([f64; 2], [f64; 2]), GeometryError> {
    match store.pcurves.get(id as usize) {
        Some(PcurveGeometry::Line2 { origin, direction }) => Ok((*origin, *direction)),
        _ => Err(GeometryError::CoverageGap {
            families: ["plane trim".into(), "nonlinear intersection pcurve".into()],
        }),
    }
}

fn boundary_parameters(
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
            let delta_cross = |a: [f64; 2], b: [f64; 2]| a[0] * b[1] - a[1] * b[0];
            match &brep.geometry.pcurves[pcurve_id as usize] {
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
                        if (-2..=2)
                            .any(|turn| edge_range.contains(parameter + f64::from(turn) * tau))
                        {
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

fn periodic_value(value: f64, bounds: Interval, period: Option<f64>) -> f64 {
    period.map_or(value, |period| {
        value + ((bounds.midpoint() - value) / period).round() * period
    })
}

fn uv_in_bounds(
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

fn is_uv_box_trim(brep: &BrepEnvelope, face: &Face) -> Result<bool, GeometryError> {
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

fn branch_inside_faces(
    geometry: &GeometryStore,
    branch: &SsiCurve,
    faces: [(FaceView<'_>, &Face); 2],
    boxes_only: bool,
    tolerance: f64,
    parameter: f64,
) -> Result<bool, GeometryError> {
    for side in 0..2 {
        let uv = geometry.pcurve_at(branch.pcurves[side], parameter)?;
        let inside = if boxes_only {
            uv_in_bounds(
                geometry,
                side as u32,
                faces[side].1.trim.uv_bounds,
                uv,
                tolerance,
            )?
        } else {
            face_contains_uv(faces[side].0.brep, faces[side].1, uv)? == Some(true)
        };
        if !inside {
            return Ok(false);
        }
    }
    Ok(true)
}

fn transition_parameter(
    geometry: &GeometryStore,
    branch: &SsiCurve,
    faces: [(FaceView<'_>, &Face); 2],
    boxes_only: bool,
    mut outside: f64,
    mut inside: f64,
    tolerance: f64,
) -> Result<f64, GeometryError> {
    for _ in 0..64 {
        let middle = 0.5 * (outside + inside);
        if branch_inside_faces(geometry, branch, faces, boxes_only, tolerance, middle)? {
            inside = middle;
        } else {
            outside = middle;
        }
        if (inside - outside).abs() <= f64::EPSILON * middle.abs().max(1.0) * 16.0 {
            break;
        }
    }
    Ok(inside)
}

fn tube_may_enter_boxes(
    geometry: &GeometryStore,
    branch: &SsiCurve,
    segment: usize,
    bounds: [UVBox; 2],
) -> Result<bool, GeometryError> {
    let super::CurveGeometry::Intersection { definition } = geometry.curves[branch.curve as usize]
    else {
        return Ok(true);
    };
    let definition = &geometry.intersections[definition as usize];
    let tube = definition.uv_tubes.get(segment).ok_or_else(|| {
        GeometryError::InvalidGeometry("intersection branch tube is missing".into())
    })?;
    for side in 0..2 {
        let definition_side = match geometry.pcurves[branch.pcurves[side] as usize] {
            PcurveGeometry::IntersectionSide {
                side: super::topology::IntersectionSide::A,
                ..
            } => 0,
            PcurveGeometry::IntersectionSide {
                side: super::topology::IntersectionSide::B,
                ..
            } => 1,
            _ => side,
        };
        let periods = geometry
            .surface(definition.surfaces[definition_side])?
            .charts()[0]
            .periods;
        for axis in 0..2 {
            let interval = tube[2 * definition_side + axis];
            let face_bounds = bounds[side][axis];
            let overlaps = periods[axis].map_or_else(
                || interval.lo <= face_bounds.hi && interval.hi >= face_bounds.lo,
                |period| {
                    (-2..=2).any(|turn| {
                        let shift = f64::from(turn) * period;
                        interval.lo + shift <= face_bounds.hi
                            && interval.hi + shift >= face_bounds.lo
                    })
                },
            );
            if !overlaps {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn clip_face_branch(
    geometry: &GeometryStore,
    branch: &SsiCurve,
    faces: [(FaceView<'_>, &Face); 2],
    boxes_only: bool,
    bounds: [UVBox; 2],
    accuracy: Accuracy,
) -> Result<Vec<Interval>, GeometryError> {
    let domain = branch.domain.ok_or_else(|| GeometryError::CoverageGap {
        families: [
            "bounded face trim".into(),
            "unbounded intersection curve".into(),
        ],
    })?;
    let mut anchors = match geometry.curves[branch.curve as usize] {
        super::CurveGeometry::Intersection { definition } => geometry.intersections
            [definition as usize]
            .anchors
            .iter()
            .map(|anchor| anchor.parameter)
            .collect::<Vec<_>>(),
        _ => (0..=256)
            .map(|index| domain.lo + domain.width() * index as f64 / 256.0)
            .collect(),
    };
    anchors.retain(|parameter| domain.contains(*parameter));
    anchors.push(domain.lo);
    anchors.push(domain.hi);
    anchors.sort_by(f64::total_cmp);
    anchors.dedup_by(|left, right| (*left - *right).abs() <= f64::EPSILON * 16.0);

    let mut intervals: Vec<Interval> = Vec::new();
    for (segment, pair) in anchors.windows(2).enumerate() {
        let subdivisions = 4;
        for part in 0..subdivisions {
            let lo = pair[0] + (pair[1] - pair[0]) * part as f64 / subdivisions as f64;
            let hi = pair[0] + (pair[1] - pair[0]) * (part + 1) as f64 / subdivisions as f64;
            let middle = 0.5 * (lo + hi);
            let lo_inside = branch_inside_faces(
                geometry,
                branch,
                faces,
                boxes_only,
                accuracy.intersection,
                lo,
            )?;
            let middle_inside = branch_inside_faces(
                geometry,
                branch,
                faces,
                boxes_only,
                accuracy.intersection,
                middle,
            )?;
            let hi_inside = branch_inside_faces(
                geometry,
                branch,
                faces,
                boxes_only,
                accuracy.intersection,
                hi,
            )?;
            if !lo_inside && !middle_inside && !hi_inside {
                if matches!(
                    geometry.curves[branch.curve as usize],
                    super::CurveGeometry::Intersection { .. }
                ) && tube_may_enter_boxes(geometry, branch, segment, bounds)?
                {
                    return Err(GeometryError::UnresolvedIntersection(
                        "bounded SSI branch may cross a trim box between certified anchors".into(),
                    ));
                }
                continue;
            }
            let start = if lo_inside {
                lo
            } else if middle_inside {
                transition_parameter(
                    geometry,
                    branch,
                    faces,
                    boxes_only,
                    lo,
                    middle,
                    accuracy.intersection,
                )?
            } else {
                transition_parameter(
                    geometry,
                    branch,
                    faces,
                    boxes_only,
                    middle,
                    hi,
                    accuracy.intersection,
                )?
            };
            let end = if hi_inside {
                hi
            } else if middle_inside {
                transition_parameter(
                    geometry,
                    branch,
                    faces,
                    boxes_only,
                    hi,
                    middle,
                    accuracy.intersection,
                )?
            } else {
                transition_parameter(
                    geometry,
                    branch,
                    faces,
                    boxes_only,
                    middle,
                    lo,
                    accuracy.intersection,
                )?
            };
            let (start, end) = if start <= end {
                (start, end)
            } else {
                (end, start)
            };
            if end - start <= accuracy.geometric {
                continue;
            }
            if let Some(previous) = intervals.last_mut() {
                // Each trim transition can carry one geometric tolerance of
                // correction, so their shared boundary may differ by two.
                if start - previous.hi <= accuracy.intersection.max(2.0 * accuracy.geometric) {
                    *previous = Interval::new(previous.lo, end.max(previous.hi))?;
                    continue;
                }
            }
            intervals.push(Interval::new(start, end)?);
        }
    }
    Ok(intervals)
}

fn bound_plane_cylinder_generator(
    geometry: &GeometryStore,
    branch: &SsiCurve,
    faces: [&Face; 2],
    accuracy: Accuracy,
) -> Result<Option<Interval>, GeometryError> {
    let super::CurveGeometry::Line { origin, direction } = geometry.curves[branch.curve as usize]
    else {
        return Err(GeometryError::CoverageGap {
            families: ["plane/cylinder trim".into(), "nonlinear generator".into()],
        });
    };
    let mut range = [-f64::MAX.sqrt(), f64::MAX.sqrt()];
    for side in 0..2 {
        let surface = geometry.surface(side as u32)?;
        let start = surface.project(origin, None)?;
        let next = surface.project(super::geometry::add(origin, direction), Some(start))?;
        let periods = surface.charts()[0].periods;
        for axis in 0..2 {
            let bounds = faces[side].trim.uv_bounds[axis];
            let initial = periodic_value(start[axis], bounds, periods[axis]);
            let later = periodic_value(next[axis], bounds, periods[axis]);
            let mut rate = later - initial;
            if matches!(surface, SurfaceGeometry::Cylinder { .. })
                && axis == 0
                && rate.abs() > accuracy.intersection
            {
                return Err(GeometryError::CoverageGap {
                    families: ["plane/cylinder trim".into(), "nonaxial generator".into()],
                });
            }
            if matches!(surface, SurfaceGeometry::Cylinder { .. }) && axis == 0 {
                rate = 0.0;
            }
            if !clip_coordinate(initial, rate, bounds, &mut range) {
                return Ok(None);
            }
        }
    }
    if !range[0].is_finite() || !range[1].is_finite() || range[1] - range[0] <= accuracy.geometric {
        return Ok(None);
    }
    Ok(Some(Interval::new(range[0], range[1])?))
}

fn intersect_bounded_surfaces(
    geometry: &mut GeometryStore,
    faces: [&Face; 2],
    accuracy: Accuracy,
) -> Result<SsiResult, GeometryError> {
    let planar_pair = geometry
        .surfaces
        .iter()
        .take(2)
        .all(|surface| matches!(surface, SurfaceGeometry::Plane { .. }));
    let plane_cylinder_pair = geometry.surfaces.len() >= 2
        && matches!(
            (&geometry.surfaces[0], &geometry.surfaces[1]),
            (
                SurfaceGeometry::Plane { .. },
                SurfaceGeometry::Cylinder { .. }
            ) | (
                SurfaceGeometry::Cylinder { .. },
                SurfaceGeometry::Plane { .. }
            )
        );
    match intersect_surfaces(geometry, 0, 1, accuracy) {
        Ok(mut result) if plane_cylinder_pair => {
            let mut bounded = Vec::new();
            for mut branch in result.curves {
                if branch.domain.is_none() {
                    branch.domain =
                        bound_plane_cylinder_generator(geometry, &branch, faces, accuracy)?;
                }
                if branch.domain.is_some() {
                    bounded.push(branch);
                }
            }
            result.curves = bounded;
            Ok(result)
        }
        Ok(result) if planar_pair || result.curves.iter().all(|curve| curve.domain.is_some()) => {
            Ok(result)
        }
        Ok(_) | Err(GeometryError::CoverageGap { .. }) => universal_ssi::intersect_patches(
            geometry,
            [0, 1],
            [faces[0].trim.uv_bounds, faces[1].trim.uv_bounds],
            accuracy,
            SsiBudget::default(),
        ),
        Err(error) => Err(error),
    }
}

pub fn intersect_faces(
    a: FaceView<'_>,
    b: FaceView<'_>,
) -> Result<IntersectionGraph, GeometryError> {
    a.brep.validate()?;
    b.brep.validate()?;
    let face_a = a.brep.topology.faces.get(a.face as usize).ok_or_else(|| {
        GeometryError::MissingReference {
            kind: "face".into(),
            index: a.face,
        }
    })?;
    let face_b = b.brep.topology.faces.get(b.face as usize).ok_or_else(|| {
        GeometryError::MissingReference {
            kind: "face".into(),
            index: b.face,
        }
    })?;
    let planar = matches!(
        a.brep.geometry.surfaces[face_a.surface as usize],
        SurfaceGeometry::Plane { .. }
    ) && matches!(
        b.brep.geometry.surfaces[face_b.surface as usize],
        SurfaceGeometry::Plane { .. }
    );
    let boxes_only = !planar && is_uv_box_trim(a.brep, face_a)? && is_uv_box_trim(b.brep, face_b)?;
    let mut geometry = GeometryStore::new();
    geometry
        .surfaces
        .push(a.brep.geometry.surfaces[face_a.surface as usize].clone());
    geometry
        .surfaces
        .push(b.brep.geometry.surfaces[face_b.surface as usize].clone());
    let accuracy = combined_accuracy(a.brep.accuracy, b.brep.accuracy);
    let result = intersect_bounded_surfaces(&mut geometry, [face_a, face_b], accuracy)?;
    let contacts = result
        .contacts
        .into_iter()
        .filter_map(|point| {
            let ua = geometry.surfaces[0].project(point, None).ok()?;
            let ub = geometry.surfaces[1].project(point, None).ok()?;
            (face_contains_uv(a.brep, face_a, ua).ok()? == Some(true)
                && face_contains_uv(b.brep, face_b, ub).ok()? == Some(true))
            .then_some(point)
        })
        .collect();
    let mut branches = Vec::new();
    for branch in result.curves {
        if !planar {
            for range in clip_face_branch(
                &geometry,
                &branch,
                [(a, face_a), (b, face_b)],
                boxes_only,
                [face_a.trim.uv_bounds, face_b.trim.uv_bounds],
                accuracy,
            )? {
                let curve = geometry.curve(branch.curve)?;
                branches.push(IntersectionBranch {
                    curve: branch.curve,
                    pcurves: branch.pcurves,
                    range,
                    endpoints: [curve.point_at(range.lo)?, curve.point_at(range.hi)?],
                });
            }
            continue;
        }
        let (origin_a, direction_a) = pcurve_line(&geometry, branch.pcurves[0])?;
        let (origin_b, direction_b) = pcurve_line(&geometry, branch.pcurves[1])?;
        let mut range = branch
            .domain
            .map_or([-f64::MAX.sqrt(), f64::MAX.sqrt()], |range| {
                [range.lo, range.hi]
            });
        for (origin, direction, face) in [
            (origin_a, direction_a, face_a),
            (origin_b, direction_b, face_b),
        ] {
            for axis in 0..2 {
                if !clip_coordinate(
                    origin[axis],
                    direction[axis],
                    face.trim.uv_bounds[axis],
                    &mut range,
                ) {
                    range[0] = 1.0;
                    range[1] = 0.0;
                    break;
                }
            }
        }
        if range[0] > range[1] {
            continue;
        }
        let mut splits = vec![range[0], range[1]];
        boundary_parameters(a.brep, a.face, origin_a, direction_a, &mut splits)?;
        boundary_parameters(b.brep, b.face, origin_b, direction_b, &mut splits)?;
        splits.retain(|value| *value >= range[0] && *value <= range[1] && value.is_finite());
        splits.sort_by(f64::total_cmp);
        splits.dedup_by(|left, right| (*left - *right).abs() <= accuracy.intersection);
        for interval in splits.windows(2) {
            if interval[1] - interval[0] <= accuracy.geometric {
                continue;
            }
            let midpoint = interval[0] / 2.0 + interval[1] / 2.0;
            let uv_a = geometry.pcurve_at(branch.pcurves[0], midpoint)?;
            let uv_b = geometry.pcurve_at(branch.pcurves[1], midpoint)?;
            if face_contains_uv(a.brep, face_a, uv_a)? != Some(true)
                || face_contains_uv(b.brep, face_b, uv_b)? != Some(true)
            {
                continue;
            }
            let range = Interval::new(interval[0], interval[1])?;
            let curve = geometry.curve(branch.curve)?;
            branches.push(IntersectionBranch {
                curve: branch.curve,
                pcurves: branch.pcurves,
                range,
                endpoints: [curve.point_at(range.lo)?, curve.point_at(range.hi)?],
            });
        }
    }
    Ok(IntersectionGraph {
        geometry,
        branches,
        contacts,
        coincident: result.coincident,
    })
}

pub fn intersect_breps(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
) -> Result<BodyIntersectionGraph, GeometryError> {
    a.validate()?;
    b.validate()?;
    let accuracy = combined_accuracy(a.accuracy, b.accuracy);
    let bounds_a = a
        .topology
        .faces
        .iter()
        .map(|face| face_patch_bounds(a, face))
        .collect::<Result<Vec<_>, _>>()?;
    let bounds_b = b
        .topology
        .faces
        .iter()
        .map(|face| face_patch_bounds(b, face))
        .collect::<Result<Vec<_>, _>>()?;
    let mut pairs = Vec::new();
    for (face_a, bounds_a) in a.topology.faces.iter().zip(&bounds_a) {
        for (face_b, bounds_b) in b.topology.faces.iter().zip(&bounds_b) {
            if !patch_bounds_overlap(*bounds_a, *bounds_b, accuracy.intersection) {
                continue;
            }
            let graph = intersect_faces(
                FaceView {
                    brep: a,
                    face: face_a.id,
                },
                FaceView {
                    brep: b,
                    face: face_b.id,
                },
            )
            .map_err(|error| match error {
                GeometryError::UnresolvedIntersection(message) => {
                    GeometryError::UnresolvedIntersection(format!(
                        "face pair {}:{}: {message}",
                        face_a.id, face_b.id
                    ))
                }
                other => other,
            })?;
            if graph.coincident || !graph.branches.is_empty() || !graph.contacts.is_empty() {
                pairs.push(FacePairIntersection {
                    faces: [face_a.id, face_b.id],
                    graph,
                });
            }
        }
    }
    Ok(BodyIntersectionGraph { pairs })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytic::{
        geometry::{norm, sub},
        primitives,
        topology::Accuracy,
        CurveGeometry, Frame3,
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
    fn plane_face_graph_clips_the_shared_curve_to_both_trims() {
        let a = primitives::cuboid("a".into(), Frame3::IDENTITY, [2.0; 3], accuracy()).unwrap();
        let b = primitives::cuboid(
            "b".into(),
            Frame3 {
                origin: [-1.0, 0.5, 1.0],
                ..Frame3::IDENTITY
            },
            [2.0; 3],
            accuracy(),
        )
        .unwrap();
        let graph = intersect_faces(
            FaceView { brep: &a, face: 1 },
            FaceView { brep: &b, face: 3 },
        )
        .unwrap();
        assert_eq!(graph.branches.len(), 1);
        let mut endpoints = graph.branches[0].endpoints;
        endpoints.sort_by(|a, b| a[1].total_cmp(&b[1]));
        for (point, expected_y) in endpoints.into_iter().zip([0.5, 2.0]) {
            assert!((point[0] - 1.0).abs() <= accuracy().intersection);
            assert!((point[1] - expected_y).abs() <= accuracy().intersection);
            assert!((point[2] - 2.0).abs() <= accuracy().intersection);
        }
    }

    #[test]
    fn vertical_plane_cylinder_generators_remain_exact_and_bounded() {
        let quarter = std::f64::consts::FRAC_PI_2;
        let host = primitives::arc_edged_extrusion(
            "arc-profile".into(),
            Frame3::IDENTITY,
            vec![
                primitives::ProfileEdge::Arc {
                    center: [0.0, 0.0],
                    radius: 2.0,
                    start_angle: 0.0,
                    sweep_angle: quarter,
                },
                primitives::ProfileEdge::Line {
                    from: [0.0, 2.0],
                    to: [0.0, 1.5],
                },
                primitives::ProfileEdge::Arc {
                    center: [0.0, 0.0],
                    radius: 1.5,
                    start_angle: quarter,
                    sweep_angle: -quarter,
                },
                primitives::ProfileEdge::Line {
                    from: [1.5, 0.0],
                    to: [2.0, 0.0],
                },
            ],
            3.0,
            accuracy(),
        )
        .unwrap();
        let cutter = primitives::cuboid(
            "opening".into(),
            Frame3 {
                origin: [1.1, 0.9, 0.5],
                ..Frame3::IDENTITY
            },
            [1.0, 0.35, 1.5],
            accuracy(),
        )
        .unwrap();
        let graph = intersect_breps(&host, &cutter).unwrap();
        assert!(!graph.pairs.is_empty());
        let branches = graph
            .pairs
            .iter()
            .flat_map(|pair| &pair.graph.branches)
            .collect::<Vec<_>>();
        assert!(!branches.is_empty());
        for pair in &graph.pairs {
            for branch in &pair.graph.branches {
                assert!(branch.range.lo.is_finite() && branch.range.hi.is_finite());
                assert!(!matches!(
                    pair.graph.geometry.curves[branch.curve as usize],
                    CurveGeometry::Intersection { .. }
                ));
            }
        }
    }

    #[test]
    fn bounded_nonparallel_cylinder_faces_use_universal_ssi() {
        let accuracy = Accuracy {
            geometric: 1e-5,
            intersection: 2.5e-6,
            tessellation: 0.01,
            exchange: 1e-5,
        };
        let a = primitives::cylinder(
            "a".into(),
            Frame3 {
                origin: [0.0, 0.0, -2.0],
                ..Frame3::IDENTITY
            },
            1.0,
            4.0,
            accuracy,
        )
        .unwrap();
        let b_frame =
            Frame3::from_axis([-1.0, 0.0, 0.2], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
        let b = primitives::cylinder("b".into(), b_frame, 0.6, 2.0, accuracy).unwrap();
        let graph = intersect_faces(
            FaceView { brep: &a, face: 0 },
            FaceView { brep: &b, face: 0 },
        )
        .unwrap();
        assert!(!graph.branches.is_empty());
        for branch in &graph.branches {
            assert!(matches!(
                graph.geometry.curves[branch.curve as usize],
                CurveGeometry::Intersection { .. }
            ));
            let parameter = branch.range.midpoint();
            let point = graph
                .geometry
                .curve(branch.curve)
                .unwrap()
                .point_at(parameter)
                .unwrap();
            for side in 0..2 {
                let uv = graph
                    .geometry
                    .pcurve_at(branch.pcurves[side], parameter)
                    .unwrap();
                let support = graph.geometry.surfaces[side].point_at(uv).unwrap();
                assert!(norm(sub(point, support)) <= accuracy.intersection);
            }
        }
    }

    #[test]
    fn body_graph_uses_patch_bounds_before_face_pair_ssi() {
        let accuracy = Accuracy {
            geometric: 1e-5,
            intersection: 2.5e-6,
            tessellation: 0.01,
            exchange: 1e-5,
        };
        let a = primitives::cylinder(
            "a".into(),
            Frame3 {
                origin: [0.0, 0.0, -2.0],
                ..Frame3::IDENTITY
            },
            1.0,
            4.0,
            accuracy,
        )
        .unwrap();
        let b = primitives::cylinder(
            "b".into(),
            Frame3::from_axis([-2.0, 0.0, 0.2], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap(),
            0.6,
            4.0,
            accuracy,
        )
        .unwrap();
        let graph = intersect_breps(&a, &b).unwrap();
        assert_eq!(graph.pairs.len(), 1);
        assert_eq!(graph.pairs[0].faces, [0, 0]);
        assert_eq!(graph.pairs[0].graph.branches.len(), 2);
    }
}
