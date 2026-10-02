use super::profile::{edges, ProfileLoop};
use crate::brep::Frame3;
use crate::math::{cross, dot, norm, sub};
use crate::operations::{invalid, OperationError};
use crate::primitives::{
    arc_profile_contains, loops_touch, profile_edge_intersections, profile_point_inside,
    ProfileEdge,
};

struct PlanarLoop {
    edges: Vec<ProfileEdge>,
    points: Option<Vec<[f64; 2]>>,
    probe: [f64; 2],
}

pub(super) fn check_circle_hole(
    circle: Frame3,
    radius: f64,
    frame: Frame3,
    tolerance: f64,
) -> Result<(), OperationError> {
    if dot(sub(circle.origin, frame.origin), frame.z).abs() > 4.0 * tolerance
        || radius * norm(cross(circle.z, frame.z)) > 4.0 * tolerance
    {
        return Err(invalid("hole is not coplanar"));
    }
    if !radius.is_finite() || radius <= 4.0 * tolerance {
        return Err(invalid("hole radius is below geometric resolution"));
    }
    Ok(())
}

pub(super) fn check_annular(
    outer: &ProfileLoop,
    holes: &[ProfileLoop],
    tolerance: f64,
) -> Result<(), OperationError> {
    if let (
        ProfileLoop::Circle {
            frame: outer_frame,
            radius,
        },
        [ProfileLoop::Circle {
            frame: inner_frame,
            radius: inner_radius,
        }],
    ) = (outer, holes)
    {
        if norm(sub(inner_frame.origin, outer_frame.origin)) <= 4.0 * tolerance
            && inner_radius + 4.0 * tolerance >= *radius
        {
            return Err(invalid("hole radius is not smaller than the profile"));
        }
    }
    Ok(())
}

pub(super) fn check_layout(
    outer: &ProfileLoop,
    holes: &[ProfileLoop],
    frame: Frame3,
    tolerance: f64,
) -> Result<(), OperationError> {
    let resolution = 4.0 * tolerance;
    let outer = planar_loop(outer, frame);
    let holes = holes
        .iter()
        .map(|hole| planar_loop(hole, frame))
        .collect::<Vec<_>>();
    check_inside_outer(&outer, &holes, resolution)?;
    check_holes_apart(&holes, resolution)
}

fn check_inside_outer(
    outer: &PlanarLoop,
    holes: &[PlanarLoop],
    resolution: f64,
) -> Result<(), OperationError> {
    for hole in holes {
        if !contains(outer, hole.probe, resolution) {
            return Err(invalid("hole lies outside the profile"));
        }
        if touch(outer, hole, resolution) {
            return Err(invalid("hole touches the profile"));
        }
    }
    Ok(())
}

fn check_holes_apart(holes: &[PlanarLoop], resolution: f64) -> Result<(), OperationError> {
    for (index, a) in holes.iter().enumerate() {
        for b in &holes[index + 1..] {
            if touch(a, b, resolution) {
                return Err(invalid("holes overlap"));
            }
            if contains(a, b.probe, resolution) || contains(b, a.probe, resolution) {
                return Err(invalid("holes are nested"));
            }
        }
    }
    Ok(())
}

fn planar_loop(profile: &ProfileLoop, frame: Frame3) -> PlanarLoop {
    let edges = edges(profile, frame);
    match profile {
        ProfileLoop::Circle { frame: circle, .. } => {
            let centre = frame.local(circle.origin);
            PlanarLoop {
                edges,
                points: None,
                probe: [centre[0], centre[1]],
            }
        }
        ProfileLoop::Lines(_) => {
            let points = edges
                .iter()
                .map(|edge| edge.endpoints().0)
                .collect::<Vec<_>>();
            PlanarLoop {
                probe: points[0],
                points: Some(points),
                edges,
            }
        }
    }
}

fn contains(container: &PlanarLoop, point: [f64; 2], resolution: f64) -> bool {
    match &container.points {
        Some(points) => profile_point_inside(point, points),
        None => arc_profile_contains(&container.edges, point, resolution),
    }
}

fn touch(a: &PlanarLoop, b: &PlanarLoop, resolution: f64) -> bool {
    match (&a.points, &b.points) {
        (Some(a), Some(b)) => loops_touch(a, b, resolution),
        _ => a.edges.iter().any(|edge| {
            b.edges
                .iter()
                .any(|other| !profile_edge_intersections(edge, other, resolution).is_empty())
        }),
    }
}
