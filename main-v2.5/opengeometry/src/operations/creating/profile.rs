use crate::brep::{Frame3, Similarity3, GROUND};
use crate::geom2d::{self_intersects2, Pt2};
use crate::math::{cross, dot, norm, scale, sub, Point3};
use crate::operations::invalid;
use crate::primitives::ProfileEdge;
use crate::world_graph::GraphError;

#[derive(Clone)]
pub(crate) enum ProfileLoop {
    Circle { frame: Frame3, radius: f64 },
    Lines(Vec<Point3>),
}

pub(crate) fn mapped_frame(frame: Frame3, mapping: Similarity3) -> Frame3 {
    Frame3 {
        origin: mapping.apply_point(frame.origin),
        x: mapping.apply_vector(frame.x),
        y: mapping.apply_vector(frame.y),
        z: mapping.apply_vector(frame.z),
    }
}

pub(super) fn profile_frame(profile: &ProfileLoop, tolerance: f64) -> Result<Frame3, GraphError> {
    match profile {
        ProfileLoop::Circle { frame, .. } => Ok(*frame),
        ProfileLoop::Lines(points) => {
            let first = points[0];
            let mut normal = None;
            for index in 1..points.len() - 1 {
                let cross_product = cross(sub(points[index], first), sub(points[index + 1], first));
                if norm(cross_product) > 4.0 * tolerance * norm(sub(points[index], first)) {
                    normal = Some(scale(cross_product, 1.0 / norm(cross_product)));
                    break;
                }
            }
            let normal = normal.ok_or_else(|| invalid("profile is collinear"))?;
            for point in points {
                if dot(sub(*point, first), normal).abs() > 4.0 * tolerance {
                    return Err(invalid("profile is not planar"));
                }
            }
            if (normal[1].abs() - 1.0).abs() <= 1e-9 {
                return Ok(Frame3 {
                    origin: [0.0, first[1], 0.0],
                    ..GROUND
                });
            }
            Frame3::from_axis(first, normal, sub(points[1], first)).map_err(Into::into)
        }
    }
}

fn circle_edges(frame: Frame3, radius: f64, base: Frame3) -> Vec<ProfileEdge> {
    let centre = base.local(frame.origin);
    let x = [dot(frame.x, base.x), dot(frame.x, base.y)];
    let start_angle = x[1].atan2(x[0]);
    vec![
        ProfileEdge::Arc {
            center: [centre[0], centre[1]],
            radius,
            start_angle,
            sweep_angle: std::f64::consts::PI,
        },
        ProfileEdge::Arc {
            center: [centre[0], centre[1]],
            radius,
            start_angle: start_angle + std::f64::consts::PI,
            sweep_angle: std::f64::consts::PI,
        },
    ]
}

pub(super) fn edges(profile: &ProfileLoop, frame: Frame3) -> Vec<ProfileEdge> {
    match profile {
        ProfileLoop::Circle {
            frame: circle,
            radius,
        } => circle_edges(*circle, *radius, frame),
        ProfileLoop::Lines(points) => points
            .iter()
            .enumerate()
            .map(|(index, point)| {
                let from = frame.local(*point);
                let to = frame.local(points[(index + 1) % points.len()]);
                ProfileEdge::Line {
                    from: [from[0], from[1]],
                    to: [to[0], to[1]],
                }
            })
            .collect(),
    }
}

pub(super) fn check_lines(
    profile: &ProfileLoop,
    frame: Frame3,
    tolerance: f64,
) -> Result<(), GraphError> {
    if let ProfileLoop::Lines(points) = profile {
        let ring = points
            .iter()
            .map(|point| {
                let local = frame.local(*point);
                Pt2::new(local[0], local[1])
            })
            .collect::<Vec<_>>();
        if self_intersects2(&ring, tolerance) {
            return Err(invalid("profile self-intersects"));
        }
    }
    Ok(())
}
