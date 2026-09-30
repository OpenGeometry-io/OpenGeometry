use super::profile::{check_lines, edges, profile_frame, ProfileLoop};
use crate::brep::{Accuracy, BrepEnvelope, Frame3};
use crate::math::{add, dot, norm, scale, sub, Point3};
use crate::operations::{invalid, OperationError};
use crate::primitives;

pub(crate) fn build(
    id: String,
    outer: ProfileLoop,
    holes: Vec<ProfileLoop>,
    distance: f64,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, OperationError> {
    if !distance.is_finite() || distance.abs() <= 4.0 * accuracy.geometric {
        return Err(invalid("extrusion distance is below geometric resolution"));
    }
    let mut frame = profile_frame(&outer, accuracy.geometric)?;
    check_lines(&outer, frame, accuracy.geometric)?;
    check_holes(&holes, frame, accuracy)?;
    let original_origin = frame.origin;
    if distance < 0.0 {
        frame.origin = add(frame.origin, scale(frame.z, distance));
    }
    let height = distance.abs();
    let result = match (&outer, holes.as_slice()) {
        (ProfileLoop::Circle { radius, .. }, []) => {
            primitives::cylinder(id, frame, *radius, height, accuracy)?
        }
        (
            ProfileLoop::Circle { radius, .. },
            [ProfileLoop::Circle {
                frame: inner_frame,
                radius: inner_radius,
            }],
        ) if norm(sub(inner_frame.origin, original_origin)) <= 4.0 * accuracy.geometric => {
            primitives::annular_cylinder(id, frame, *inner_radius, *radius, height, accuracy)?
        }
        (ProfileLoop::Lines(outer_points), _)
            if holes
                .iter()
                .all(|hole| matches!(hole, ProfileLoop::Lines(_))) =>
        {
            let mapped = |points: &[Point3]| {
                points
                    .iter()
                    .map(|point| {
                        let local = frame.local(*point);
                        [local[0], local[1]]
                    })
                    .collect()
            };
            primitives::linear_extrusion(
                id,
                frame,
                mapped(outer_points),
                holes
                    .iter()
                    .filter_map(|hole| match hole {
                        ProfileLoop::Lines(points) => Some(mapped(points)),
                        _ => None,
                    })
                    .collect(),
                height,
                accuracy,
            )?
        }
        _ => primitives::arc_edged_extrusion_with_holes(
            id,
            frame,
            edges(&outer, frame),
            holes.iter().map(|hole| edges(hole, frame)).collect(),
            height,
            accuracy,
        )?,
    };
    Ok(result)
}

fn check_holes(
    holes: &[ProfileLoop],
    frame: Frame3,
    accuracy: Accuracy,
) -> Result<(), OperationError> {
    for hole in holes {
        check_lines(hole, frame, accuracy.geometric)?;
        match hole {
            ProfileLoop::Circle {
                frame: circle,
                radius,
            } => {
                if !radius.is_finite() || *radius <= 4.0 * accuracy.geometric {
                    return Err(invalid("hole radius is below geometric resolution"));
                }
                if dot(sub(circle.origin, frame.origin), frame.z).abs() > 4.0 * accuracy.geometric
                    || dot(circle.z, frame.z).abs() < 1.0 - 1e-9
                {
                    return Err(invalid("hole is not coplanar"));
                }
            }
            ProfileLoop::Lines(points) => {
                for point in points {
                    if dot(sub(*point, frame.origin), frame.z).abs() > 4.0 * accuracy.geometric {
                        return Err(invalid("hole is not coplanar"));
                    }
                }
            }
        }
    }
    Ok(())
}
