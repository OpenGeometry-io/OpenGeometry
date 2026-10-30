use crate::brep::{Frame3, Similarity3, GROUND};
use crate::geom2d::{self_intersects2, Pt2};
use crate::math::{add, cross, dot, norm, scale, sub, Point3};
use crate::operations::{invalid, OperationError};
use crate::primitives::{profile_self_intersects, ProfileEdge};
use std::f64::consts::TAU;

#[derive(Clone)]
pub(crate) enum ProfileLoop {
    Circle { frame: Frame3, radius: f64 },
    Lines(Vec<Point3>),
    Curves(Vec<LoopEdge>),
}

#[derive(Clone, Copy)]
pub(crate) struct LoopEdge {
    pub(crate) from: Point3,
    pub(crate) arc: Option<LoopArc>,
}

#[derive(Clone, Copy)]
pub(crate) struct LoopArc {
    pub(crate) center: Point3,
    pub(crate) axis: Point3,
    pub(crate) radius: f64,
    pub(crate) counter_clockwise: bool,
}

pub(crate) fn mapped_frame(frame: Frame3, mapping: Similarity3) -> Frame3 {
    Frame3 {
        origin: mapping.apply_point(frame.origin),
        x: mapping.apply_vector(frame.x),
        y: mapping.apply_vector(frame.y),
        z: mapping.apply_vector(frame.z),
    }
}

pub(super) fn profile_frame(
    profile: &ProfileLoop,
    tolerance: f64,
) -> Result<Frame3, OperationError> {
    match profile {
        ProfileLoop::Circle { frame, radius } => Ok(circle_frame(*frame, *radius, tolerance)),
        ProfileLoop::Lines(points) => lines_frame(points, tolerance),
        ProfileLoop::Curves(edges) => curves_frame(edges, tolerance),
    }
}

fn curves_frame(edges: &[LoopEdge], tolerance: f64) -> Result<Frame3, OperationError> {
    let points = edges
        .iter()
        .enumerate()
        .flat_map(|(index, edge)| {
            let to = edges[(index + 1) % edges.len()].from;
            std::iter::once(edge.from).chain(edge.arc.map(|arc| arc_midpoint(edge.from, to, arc)))
        })
        .collect::<Vec<_>>();
    let frame = lines_frame(&points, tolerance)?;
    if edges
        .iter()
        .filter_map(|edge| edge.arc)
        .any(|arc| arc.radius * norm(cross(arc.axis, frame.z)) > 4.0 * tolerance)
    {
        return Err(invalid("profile is not planar"));
    }
    Ok(frame)
}

fn turn_axis(arc: LoopArc) -> Point3 {
    if arc.counter_clockwise {
        arc.axis
    } else {
        scale(arc.axis, -1.0)
    }
}

fn arc_turn(from: Point3, to: Point3, arc: LoopArc) -> f64 {
    let a = sub(from, arc.center);
    let b = sub(to, arc.center);
    dot(cross(a, b), turn_axis(arc))
        .atan2(dot(a, b))
        .rem_euclid(TAU)
}

fn arc_midpoint(from: Point3, to: Point3, arc: LoopArc) -> Point3 {
    let half = arc_turn(from, to, arc) / 2.0;
    let radial = sub(from, arc.center);
    add(
        arc.center,
        add(
            scale(radial, half.cos()),
            scale(cross(turn_axis(arc), radial), half.sin()),
        ),
    )
}

fn circle_frame(frame: Frame3, radius: f64, tolerance: f64) -> Frame3 {
    let up = [0.0, 1.0, 0.0];
    if radius * norm(cross(frame.z, up)) <= 4.0 * tolerance && dot(frame.z, up) < 0.0 {
        return Frame3 {
            origin: frame.origin,
            x: frame.x,
            y: scale(frame.y, -1.0),
            z: scale(frame.z, -1.0),
        };
    }
    frame
}

fn lines_frame(points: &[Point3], tolerance: f64) -> Result<Frame3, OperationError> {
    let first = points[0];
    let normal =
        newell_normal(points, tolerance).ok_or_else(|| degenerate_error(points, tolerance))?;
    if points
        .iter()
        .any(|point| dot(sub(*point, first), normal).abs() > 4.0 * tolerance)
    {
        return Err(invalid("profile is not planar"));
    }
    if points
        .iter()
        .all(|point| (point[1] - first[1]).abs() <= 4.0 * tolerance)
    {
        return Ok(Frame3 {
            origin: [0.0, first[1], 0.0],
            ..GROUND
        });
    }
    Frame3::from_axis(first, normal, sub(points[1], first)).map_err(Into::into)
}

pub(super) fn newell_normal(points: &[Point3], tolerance: f64) -> Option<Point3> {
    let first = points[0];
    let sum = (1..points.len() - 1).fold([0.0; 3], |sum, index| {
        add(
            sum,
            cross(sub(points[index], first), sub(points[index + 1], first)),
        )
    });
    let length = norm(sum);
    let reach = norm(sub(farthest(points), first));
    (length > 4.0 * tolerance * reach).then(|| scale(sum, 1.0 / length))
}

fn degenerate_error(points: &[Point3], tolerance: f64) -> OperationError {
    let first = points[0];
    let direction = sub(farthest(points), first);
    let reach = norm(direction);
    if points
        .iter()
        .any(|point| norm(cross(sub(*point, first), direction)) > 4.0 * tolerance * reach)
    {
        invalid("profile self-intersects")
    } else {
        invalid("profile is collinear")
    }
}

fn farthest(points: &[Point3]) -> Point3 {
    let first = points[0];
    points
        .iter()
        .copied()
        .max_by(|a, b| norm(sub(*a, first)).total_cmp(&norm(sub(*b, first))))
        .unwrap_or(first)
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
        ProfileLoop::Curves(edges) => edges
            .iter()
            .enumerate()
            .map(|(index, edge)| curve_edge(*edge, edges[(index + 1) % edges.len()].from, frame))
            .collect(),
    }
}

fn curve_edge(edge: LoopEdge, to: Point3, frame: Frame3) -> ProfileEdge {
    let local = |point: Point3| {
        let local = frame.local(point);
        [local[0], local[1]]
    };
    let from = local(edge.from);
    let Some(arc) = edge.arc else {
        return ProfileEdge::Line {
            from,
            to: local(to),
        };
    };
    let center = local(arc.center);
    let sign = if arc.counter_clockwise == (dot(arc.axis, frame.z) > 0.0) {
        1.0
    } else {
        -1.0
    };
    ProfileEdge::Arc {
        center,
        radius: arc.radius,
        start_angle: (from[1] - center[1]).atan2(from[0] - center[0]),
        sweep_angle: sign * arc_turn(edge.from, to, arc),
    }
}

pub(super) fn check_lines(
    profile: &ProfileLoop,
    frame: Frame3,
    tolerance: f64,
) -> Result<(), OperationError> {
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
    if let ProfileLoop::Curves(_) = profile {
        if profile_self_intersects(&edges(profile, frame), true, tolerance) {
            return Err(invalid("profile self-intersects"));
        }
    }
    Ok(())
}
