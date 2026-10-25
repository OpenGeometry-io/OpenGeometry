use super::bend::{bend_frame, rotation, Bend, BendFrame};
use crate::brep::{unit, Accuracy};
use crate::math::{add, cross, dot, norm, sub, Point3};
use crate::operations::{invalid, OperationError};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SweepPath {
    pub(crate) points: Vec<Point3>,
    pub(crate) bends: Vec<Option<Bend>>,
}

pub(super) struct Course {
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
    if path.points.len() < 2 || !finite || !arcs || path.bends.len() + 1 != path.points.len() {
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
    while index + 1 < path.points.len() {
        if path.bends[index - 1].is_some() || path.bends[index].is_some() {
            index += 1;
            continue;
        }
        let points = &path.points;
        let before = sub(points[index], points[index - 1]);
        let after = sub(points[index + 1], points[index]);
        if norm(before) <= 4.0 * accuracy.geometric || norm(after) <= 4.0 * accuracy.geometric {
            return Err(invalid("sweep path has a short segment"));
        }
        let (u, v) = (unit(before)?, unit(after)?);
        if norm(add(u, v)) <= 1e-9 {
            return Err(OperationError::SweepSelfIntersection(
                "sweep path reverses".into(),
            ));
        }
        if norm(cross(u, v)) <= 1e-9 && dot(u, v) > 0.0 {
            path.points.remove(index);
            path.bends.remove(index);
        } else {
            index += 1;
        }
    }
    Ok(())
}

pub(super) fn course(
    path: SweepPath,
    reach: f64,
    accuracy: Accuracy,
) -> Result<Course, OperationError> {
    let count = path.bends.len();
    let mut course = Course {
        starts: Vec::with_capacity(count),
        ends: Vec::with_capacity(count),
        bends: Vec::with_capacity(count),
        points: path.points,
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

fn check_arc_joints(course: &Course) -> Result<(), OperationError> {
    for joint in 1..course.bends.len() {
        if course.bends[joint - 1].is_none() && course.bends[joint].is_none() {
            continue;
        }
        let (incoming, outgoing) = (course.ends[joint - 1], course.starts[joint]);
        if norm(cross(incoming, outgoing)) > 1e-9 || dot(incoming, outgoing) <= 0.0 {
            return Err(invalid("path has a corner at an arc"));
        }
    }
    Ok(())
}
