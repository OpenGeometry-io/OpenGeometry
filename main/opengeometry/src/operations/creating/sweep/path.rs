use super::bend::{bend_frame, rotation, Bend, BendFrame};
use crate::brep::{unit, Accuracy};
use crate::math::{add, cross, dot, norm, sub, Point3};
use crate::operations::{invalid, OperationError};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SweepPath {
    pub(crate) points: Vec<Point3>,
    pub(crate) bends: Vec<Option<Bend>>,
    pub(crate) closed: bool,
}

pub(super) struct Course {
    pub(super) closed: bool,
    pub(super) points: Vec<Point3>,
    pub(super) starts: Vec<Point3>,
    pub(super) ends: Vec<Point3>,
    pub(super) bends: Vec<Option<BendFrame>>,
}

pub(super) fn checked_path(
    mut path: SweepPath,
    accuracy: Accuracy,
) -> Result<SweepPath, OperationError> {
    let finite = path.points.iter().flatten().all(|value| value.is_finite());
    let arcs = path.bends.iter().flatten().all(|bend| {
        bend.angle > 0.0
            && bend.angle <= std::f64::consts::TAU
            && bend.axis.iter().all(|v| v.is_finite())
    });
    let arcless = path.bends.iter().all(Option::is_none);
    let least = match (path.closed, arcless) {
        (false, _) => 2,
        (true, false) => 1,
        (true, true) => 3,
    };
    if path.points.len() < least
        || !finite
        || !arcs
        || path.bends.len() + usize::from(!path.closed) != path.points.len()
    {
        return Err(invalid("sweep path is invalid"));
    }
    split_long_bends(&mut path)?;
    remove_collinear_joints(&mut path, accuracy)?;
    Ok(path)
}

fn split_long_bends(path: &mut SweepPath) -> Result<(), OperationError> {
    let mut segment = 0;
    while segment < path.bends.len() {
        let Some(bend) = path.bends[segment] else {
            segment += 1;
            continue;
        };
        let pieces = (bend.angle / std::f64::consts::PI).ceil().max(1.0) as usize;
        let piece = Bend {
            angle: bend.angle / pieces as f64,
            ..bend
        };
        let start = sub(path.points[segment], bend.centre);
        let axis = unit(bend.axis)?;
        for step in 1..pieces {
            let turned = rotation(axis, piece.angle * step as f64, start);
            path.points.insert(segment + step, add(bend.centre, turned));
        }
        path.bends
            .splice(segment..=segment, std::iter::repeat_n(Some(piece), pieces));
        segment += pieces;
    }
    Ok(())
}

fn remove_collinear_joints(path: &mut SweepPath, accuracy: Accuracy) -> Result<(), OperationError> {
    let mut index = 1;
    while index < path.bends.len() {
        if path.bends[index - 1].is_some() || path.bends[index].is_some() {
            index += 1;
            continue;
        }
        let next = path.points[(index + 1) % path.points.len()];
        let [before, at] = [path.points[index - 1], path.points[index]];
        if line_joint_is_straight(before, at, next, accuracy)? {
            path.points.remove(index);
            path.bends.remove(index);
        } else {
            index += 1;
        }
    }
    let last = path.bends.len() - 1;
    if path.closed && path.bends[last].is_none() && path.bends[0].is_none() {
        line_joint_is_straight(path.points[last], path.points[0], path.points[1], accuracy)?;
    }
    Ok(())
}

fn line_joint_is_straight(
    before: Point3,
    at: Point3,
    next: Point3,
    accuracy: Accuracy,
) -> Result<bool, OperationError> {
    let (incoming, outgoing) = (sub(at, before), sub(next, at));
    if norm(incoming) <= 4.0 * accuracy.geometric || norm(outgoing) <= 4.0 * accuracy.geometric {
        return Err(invalid("sweep path has a short segment"));
    }
    let (u, v) = (unit(incoming)?, unit(outgoing)?);
    if norm(add(u, v)) <= 1e-9 {
        return Err(OperationError::SweepSelfIntersection(
            "sweep path reverses".into(),
        ));
    }
    Ok(norm(cross(u, v)) <= 1e-9 && dot(u, v) > 0.0)
}

pub(super) fn course(
    path: SweepPath,
    reach: f64,
    accuracy: Accuracy,
) -> Result<Course, OperationError> {
    let count = path.bends.len();
    let mut points = path.points;
    if path.closed {
        points.push(points[0]);
    }
    let mut course = Course {
        closed: path.closed,
        starts: Vec::with_capacity(count),
        ends: Vec::with_capacity(count),
        bends: Vec::with_capacity(count),
        points,
    };
    for (segment, bend) in path.bends.into_iter().enumerate() {
        let [from, to] = [course.points[segment], course.points[segment + 1]];
        let frame = bend
            .map(|bend| bend_frame(bend, from, reach, accuracy))
            .transpose()?;
        let [start, end] = match frame {
            Some(frame) => {
                let start = cross(frame.axis, frame.radial);
                [start, frame.rotated(start)]
            }
            None => {
                if norm(sub(to, from)) <= 4.0 * accuracy.geometric {
                    return Err(invalid("sweep path has a short segment"));
                }
                let direction = unit(sub(to, from))?;
                [direction, direction]
            }
        };
        course.starts.push(start);
        course.ends.push(end);
        course.bends.push(frame);
    }
    check_arc_joints(&course)?;
    Ok(course)
}

impl Course {
    pub(super) fn closing_bisector(&self) -> Result<Option<Point3>, OperationError> {
        let last = self.bends.len() - 1;
        let (incoming, outgoing) = (self.ends[last], self.starts[0]);
        if !self.closed
            || self.bends[last].is_some()
            || self.bends[0].is_some()
            || norm(cross(incoming, outgoing)) <= 1e-9
        {
            return Ok(None);
        }
        unit(add(incoming, outgoing))
            .map(Some)
            .map_err(|_| OperationError::SweepSelfIntersection("sweep path reverses".into()))
    }
}

fn check_arc_joints(course: &Course) -> Result<(), OperationError> {
    let count = course.bends.len();
    for joint in (1..count).chain(course.closed.then_some(0)) {
        let previous = (joint + count - 1) % count;
        if course.bends[previous].is_none() && course.bends[joint].is_none() {
            continue;
        }
        let (incoming, outgoing) = (course.ends[previous], course.starts[joint]);
        if norm(cross(incoming, outgoing)) > 1e-9 || dot(incoming, outgoing) <= 0.0 {
            return Err(invalid("path has a corner at an arc"));
        }
    }
    Ok(())
}
