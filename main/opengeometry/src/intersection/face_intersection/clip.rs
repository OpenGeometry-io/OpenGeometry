use super::face_view::FaceView;
use super::trim::uv_in_bounds;
use crate::brep::{
    Accuracy, CurveGeometry, Face, GeometryError, GeometryStore, IntersectionSide, PcurveGeometry,
    Surface, UVBox,
};
use crate::intersection::ssi_result::SsiCurve;
use crate::math::Interval;
use crate::query::face_contains_uv;

struct BranchMembership<'a> {
    geometry: &'a GeometryStore,
    branch: &'a SsiCurve,
    faces: [(FaceView<'a>, &'a Face); 2],
    boxes_only: bool,
    tolerance: f64,
}

pub(super) fn clip_face_branch(
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
        CurveGeometry::Intersection { definition } => geometry.intersections[definition as usize]
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

    let membership = BranchMembership {
        geometry,
        branch,
        faces,
        boxes_only,
        tolerance: accuracy.intersection,
    };
    let mut intervals: Vec<Interval> = Vec::new();
    for (segment, pair) in anchors.windows(2).enumerate() {
        let subdivisions = 4;
        for part in 0..subdivisions {
            let lo = pair[0] + (pair[1] - pair[0]) * part as f64 / subdivisions as f64;
            let hi = pair[0] + (pair[1] - pair[0]) * (part + 1) as f64 / subdivisions as f64;
            let Some((start, end)) = clipped_span(&membership, bounds, segment, lo, hi)? else {
                continue;
            };
            if end - start <= accuracy.geometric {
                continue;
            }
            if let Some(previous) = intervals.last_mut() {
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

fn clipped_span(
    membership: &BranchMembership,
    bounds: [UVBox; 2],
    segment: usize,
    lo: f64,
    hi: f64,
) -> Result<Option<(f64, f64)>, GeometryError> {
    let BranchMembership {
        geometry,
        branch,
        faces,
        boxes_only,
        tolerance,
    } = *membership;
    let middle = 0.5 * (lo + hi);
    let lo_inside = branch_inside_faces(geometry, branch, faces, boxes_only, tolerance, lo)?;
    let middle_inside =
        branch_inside_faces(geometry, branch, faces, boxes_only, tolerance, middle)?;
    let hi_inside = branch_inside_faces(geometry, branch, faces, boxes_only, tolerance, hi)?;
    if !lo_inside && !middle_inside && !hi_inside {
        if matches!(
            geometry.curves[branch.curve as usize],
            CurveGeometry::Intersection { .. }
        ) && tube_may_enter_boxes(geometry, branch, segment, bounds)?
            && !span_clears_boxes(membership, bounds, lo, hi)?
            && !span_ends_on_boundary(membership, lo, hi)?
        {
            return Err(GeometryError::UnresolvedIntersection(
                "bounded SSI branch may cross a trim box between certified anchors".into(),
            ));
        }
        return Ok(None);
    }
    let start = if lo_inside {
        lo
    } else if middle_inside {
        transition_parameter(geometry, branch, faces, boxes_only, lo, middle, tolerance)?
    } else {
        transition_parameter(geometry, branch, faces, boxes_only, middle, hi, tolerance)?
    };
    let end = if hi_inside {
        hi
    } else if middle_inside {
        transition_parameter(geometry, branch, faces, boxes_only, hi, middle, tolerance)?
    } else {
        transition_parameter(geometry, branch, faces, boxes_only, middle, lo, tolerance)?
    };
    let (start, end) = if start <= end {
        (start, end)
    } else {
        (end, start)
    };
    Ok(Some((start, end)))
}

fn span_ends_on_boundary(
    membership: &BranchMembership,
    lo: f64,
    hi: f64,
) -> Result<bool, GeometryError> {
    if membership.boxes_only {
        return Ok(false);
    }
    for parameter in [lo, hi] {
        let mut touching = true;
        for side in 0..2 {
            let uv = membership
                .geometry
                .pcurve_at(membership.branch.pcurves[side], parameter)?;
            let (view, face) = membership.faces[side];
            touching &= face_contains_uv(view.brep, face, uv)? != Some(false);
        }
        if touching {
            return Ok(true);
        }
    }
    Ok(false)
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

fn tube_may_enter_boxes(
    geometry: &GeometryStore,
    branch: &SsiCurve,
    segment: usize,
    bounds: [UVBox; 2],
) -> Result<bool, GeometryError> {
    let CurveGeometry::Intersection { definition } = geometry.curves[branch.curve as usize] else {
        return Ok(true);
    };
    let definition = &geometry.intersections[definition as usize];
    let tube = definition.uv_tubes.get(segment).ok_or_else(|| {
        GeometryError::InvalidGeometry("intersection branch tube is missing".into())
    })?;
    let sides = definition_sides(geometry, branch);
    let periods = side_periods(geometry, definition.surfaces, sides)?;
    Ok((0..2).all(|side| {
        let first = 2 * sides[side];
        box_may_overlap(periods[side], [tube[first], tube[first + 1]], bounds[side])
    }))
}

fn span_clears_boxes(
    membership: &BranchMembership,
    bounds: [UVBox; 2],
    lo: f64,
    hi: f64,
) -> Result<bool, GeometryError> {
    let BranchMembership {
        geometry,
        branch,
        faces,
        boxes_only,
        tolerance,
    } = *membership;
    let CurveGeometry::Intersection { definition } = geometry.curves[branch.curve as usize] else {
        return Ok(false);
    };
    let surfaces = geometry.intersections[definition as usize].surfaces;
    let periods = side_periods(geometry, surfaces, definition_sides(geometry, branch))?;
    let mut spans = vec![(lo, hi, 8)];
    while let Some((lo, hi, depth)) = spans.pop() {
        let mut clear = false;
        for side in 0..2 {
            let piece = piece_box(geometry, branch.pcurves[side], lo, hi, tolerance)?;
            clear |= !box_may_overlap(periods[side], piece, bounds[side]);
        }
        if clear {
            continue;
        }
        let middle = 0.5 * (lo + hi);
        if depth == 0
            || branch_inside_faces(geometry, branch, faces, boxes_only, tolerance, middle)?
        {
            return Ok(false);
        }
        spans.push((lo, middle, depth - 1));
        spans.push((middle, hi, depth - 1));
    }
    Ok(true)
}

pub(super) fn piece_box(
    geometry: &GeometryStore,
    pcurve: u32,
    lo: f64,
    hi: f64,
    tolerance: f64,
) -> Result<[Interval; 2], GeometryError> {
    let [first, middle, last] = [lo, 0.5 * (lo + hi), hi].map(|t| geometry.pcurve_at(pcurve, t));
    let samples = [first?, middle?, last?];
    let low = [0, 1].map(|axis| {
        samples
            .iter()
            .map(|uv| uv[axis])
            .fold(f64::INFINITY, f64::min)
    });
    let high = [0, 1].map(|axis| {
        samples
            .iter()
            .map(|uv| uv[axis])
            .fold(f64::NEG_INFINITY, f64::max)
    });
    let padding = 2.0
        * (0..2)
            .map(|axis| high[axis] - low[axis])
            .fold(tolerance, f64::max);
    let [u, v] = [0, 1].map(|axis| Interval::new(low[axis] - padding, high[axis] + padding));
    Ok([u?, v?])
}

fn definition_sides(geometry: &GeometryStore, branch: &SsiCurve) -> [usize; 2] {
    [0, 1].map(
        |side| match geometry.pcurves[branch.pcurves[side] as usize] {
            PcurveGeometry::IntersectionSide {
                side: IntersectionSide::A,
                ..
            } => 0,
            PcurveGeometry::IntersectionSide {
                side: IntersectionSide::B,
                ..
            } => 1,
            _ => side,
        },
    )
}

fn side_periods(
    geometry: &GeometryStore,
    surfaces: [u32; 2],
    sides: [usize; 2],
) -> Result<[[Option<f64>; 2]; 2], GeometryError> {
    let mut periods = [[None; 2]; 2];
    for side in 0..2 {
        periods[side] = geometry.surface(surfaces[sides[side]])?.charts()[0].periods;
    }
    Ok(periods)
}

fn box_may_overlap(periods: [Option<f64>; 2], tube: [Interval; 2], face_bounds: UVBox) -> bool {
    (0..2).all(|axis| {
        let interval = tube[axis];
        let face = face_bounds[axis];
        periods[axis].map_or_else(
            || interval.lo <= face.hi && interval.hi >= face.lo,
            |period| {
                (-2..=2).any(|turn| {
                    let shift = f64::from(turn) * period;
                    interval.lo + shift <= face.hi && interval.hi + shift >= face.lo
                })
            },
        )
    })
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
