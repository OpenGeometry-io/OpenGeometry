use crate::brep::GeometryError;
use crate::geom2d::{point_in_ring2, segments_cross2, self_intersects2, Pt2};

fn signed_area(points: &[[f64; 2]]) -> f64 {
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .map(|(a, b)| a[0] * b[1] - b[0] * a[1])
        .sum::<f64>()
        * 0.5
}

pub(crate) fn orient_profile_loop(mut points: Vec<[f64; 2]>, ccw: bool) -> Vec<[f64; 2]> {
    if (signed_area(&points) > 0.0) != ccw {
        points.reverse();
    }
    points
}

fn segment_distance(point: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let edge = [b[0] - a[0], b[1] - a[1]];
    let length_squared = edge[0].mul_add(edge[0], edge[1] * edge[1]);
    if length_squared == 0.0 {
        return (point[0] - a[0]).hypot(point[1] - a[1]);
    }
    let t = (((point[0] - a[0]) * edge[0] + (point[1] - a[1]) * edge[1]) / length_squared)
        .clamp(0.0, 1.0);
    (point[0] - (a[0] + t * edge[0])).hypot(point[1] - (a[1] + t * edge[1]))
}

pub(crate) fn loops_touch(a: &[[f64; 2]], b: &[[f64; 2]], tolerance: f64) -> bool {
    a.iter().any(|point| {
        b.iter()
            .zip(b.iter().cycle().skip(1))
            .any(|(from, to)| segment_distance(*point, *from, *to) <= tolerance)
    }) || b.iter().any(|point| {
        a.iter()
            .zip(a.iter().cycle().skip(1))
            .any(|(from, to)| segment_distance(*point, *from, *to) <= tolerance)
    }) || a.iter().zip(a.iter().cycle().skip(1)).any(|(a0, a1)| {
        b.iter().zip(b.iter().cycle().skip(1)).any(|(b0, b1)| {
            segments_cross2(
                Pt2::new(a0[0], a0[1]),
                Pt2::new(a1[0], a1[1]),
                Pt2::new(b0[0], b0[1]),
                Pt2::new(b1[0], b1[1]),
                tolerance,
            )
        })
    })
}

pub(crate) fn validate_profile_loop(
    points: &[[f64; 2]],
    tolerance: f64,
    label: &str,
) -> Result<(), GeometryError> {
    if points.len() < 3 {
        return Err(GeometryError::InvalidGeometry(format!(
            "{label} requires at least three vertices"
        )));
    }
    if points
        .iter()
        .flatten()
        .any(|coordinate| !coordinate.is_finite())
    {
        return Err(GeometryError::InvalidGeometry(format!(
            "{label} contains a non-finite coordinate"
        )));
    }
    let resolution = 4.0 * tolerance;
    if points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .any(|(a, b)| (a[0] - b[0]).hypot(a[1] - b[1]) <= resolution)
    {
        return Err(GeometryError::UnresolvedIntersection(format!(
            "{label} contains an edge below geometric resolution"
        )));
    }
    if signed_area(points).abs() <= resolution * resolution {
        return Err(GeometryError::UnresolvedIntersection(format!(
            "{label} area is below geometric resolution"
        )));
    }
    let planar = points
        .iter()
        .map(|point| Pt2::new(point[0], point[1]))
        .collect::<Vec<_>>();
    if self_intersects2(&planar, tolerance) {
        return Err(GeometryError::InvalidGeometry(format!(
            "{label} self-intersects"
        )));
    }
    Ok(())
}

pub(crate) fn profile_point_inside(point: [f64; 2], loop_points: &[[f64; 2]]) -> bool {
    let ring = loop_points
        .iter()
        .map(|point| Pt2::new(point[0], point[1]))
        .collect::<Vec<_>>();
    point_in_ring2(Pt2::new(point[0], point[1]), &ring)
}
