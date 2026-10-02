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
    for side in 0..2 {
        let definition_side = match geometry.pcurves[branch.pcurves[side] as usize] {
            PcurveGeometry::IntersectionSide {
                side: IntersectionSide::A,
                ..
            } => 0,
            PcurveGeometry::IntersectionSide {
                side: IntersectionSide::B,
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
