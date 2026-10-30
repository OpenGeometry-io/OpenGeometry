use super::bend::rotation;
use super::path::Course;
use crate::brep::Accuracy;
use crate::math::{add, dot, norm, scale, sub, Point3};
use crate::operations::creating::ProfileLoop;
use crate::operations::OperationError;

const CHORDS_PER_BEND: usize = 64;

pub(super) fn profile_reach(profile: &ProfileLoop, start: Point3) -> f64 {
    match profile {
        ProfileLoop::Lines(points) => points
            .iter()
            .map(|point| norm(sub(*point, start)))
            .fold(0.0_f64, f64::max),
        ProfileLoop::Circle { frame, radius } => norm(sub(frame.origin, start)) + radius,
        ProfileLoop::Curves(edges) => edges
            .iter()
            .map(|edge| match edge.arc {
                Some(arc) => norm(sub(arc.center, start)) + arc.radius,
                None => norm(sub(edge.from, start)),
            })
            .fold(0.0_f64, f64::max),
    }
}

pub(super) fn check_segment_overlap(
    reach: f64,
    course: &Course,
    accuracy: Accuracy,
) -> Result<(), OperationError> {
    let count = course.bends.len();
    let pieces = (0..count)
        .map(|segment| centre_line(course, segment))
        .collect::<Vec<_>>();
    for first in 0..count {
        for second in first + 2..count {
            if course.closed && first == 0 && second + 1 == count {
                continue;
            }
            if centre_line_distance(&pieces[first], &pieces[second])
                <= 2.0 * reach + 4.0 * accuracy.geometric
            {
                return Err(OperationError::SweepSelfIntersection(
                    "non-adjacent sweep segments overlap".into(),
                ));
            }
        }
    }
    Ok(())
}

fn centre_line(course: &Course, segment: usize) -> (Vec<Point3>, f64) {
    let [from, to] = [course.points[segment], course.points[segment + 1]];
    let Some(frame) = course.bends[segment] else {
        return (vec![from, to], 0.0);
    };
    let step = frame.angle / CHORDS_PER_BEND as f64;
    let start = sub(from, frame.centre);
    let chords = (0..=CHORDS_PER_BEND)
        .map(|index| {
            add(
                frame.centre,
                rotation(frame.axis, step * index as f64, start),
            )
        })
        .collect();
    (chords, frame.radius * (1.0 - (step / 2.0).cos()))
}

fn centre_line_distance(first: &(Vec<Point3>, f64), second: &(Vec<Point3>, f64)) -> f64 {
    let mut distance = f64::INFINITY;
    for a in first.0.windows(2) {
        for b in second.0.windows(2) {
            distance = distance.min(segment_distance(a[0], a[1], b[0], b[1]));
        }
    }
    distance - first.1 - second.1
}

fn segment_distance(a: Point3, b: Point3, c: Point3, d: Point3) -> f64 {
    let u = sub(b, a);
    let v = sub(d, c);
    let w = sub(a, c);
    let (aa, bb, cc, dd, ee) = (dot(u, u), dot(u, v), dot(v, v), dot(u, w), dot(v, w));
    let denominator = aa * cc - bb * bb;
    let mut s = if denominator.abs() <= 1e-20 {
        0.0
    } else {
        ((bb * ee - cc * dd) / denominator).clamp(0.0, 1.0)
    };
    let t = ((bb * s + ee) / cc).clamp(0.0, 1.0);
    s = ((bb * t - dd) / aa).clamp(0.0, 1.0);
    norm(sub(add(a, scale(u, s)), add(c, scale(v, t))))
}
