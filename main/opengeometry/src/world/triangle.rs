use crate::analytic::geometry::{add, cross, dot, norm, scale, sub};
use crate::analytic::Point3;

pub(super) type Triangle = [Point3; 3];

const DEGENERATE: f64 = 1e-24;

pub(super) fn closest_point_on_triangle(p: Point3, [a, b, c]: Triangle) -> Point3 {
    let ab = sub(b, a);
    let ac = sub(c, a);
    let ap = sub(p, a);
    let d1 = dot(ab, ap);
    let d2 = dot(ac, ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = sub(p, b);
    let d3 = dot(ab, bp);
    let d4 = dot(ac, bp);
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return add(a, scale(ab, d1 / (d1 - d3)));
    }
    let cp = sub(p, c);
    let d5 = dot(ab, cp);
    let d6 = dot(ac, cp);
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return add(a, scale(ac, d2 / (d2 - d6)));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && d4 - d3 >= 0.0 && d5 - d6 >= 0.0 {
        return add(b, scale(sub(c, b), (d4 - d3) / ((d4 - d3) + (d5 - d6))));
    }
    let denominator = va + vb + vc;
    if denominator.abs() <= DEGENERATE {
        return a;
    }
    add(
        a,
        add(scale(ab, vb / denominator), scale(ac, vc / denominator)),
    )
}

pub(super) fn closest_points_on_segments(
    p1: Point3,
    q1: Point3,
    p2: Point3,
    q2: Point3,
) -> (Point3, Point3) {
    let d1 = sub(q1, p1);
    let d2 = sub(q2, p2);
    let r = sub(p1, p2);
    let a = dot(d1, d1);
    let e = dot(d2, d2);
    let f = dot(d2, r);
    let (s, t) = if a <= DEGENERATE && e <= DEGENERATE {
        (0.0, 0.0)
    } else if a <= DEGENERATE {
        (0.0, (f / e).clamp(0.0, 1.0))
    } else {
        let c = dot(d1, r);
        if e <= DEGENERATE {
            ((-c / a).clamp(0.0, 1.0), 0.0)
        } else {
            let b = dot(d1, d2);
            let denominator = a * e - b * b;
            let s = if denominator > DEGENERATE {
                ((b * f - c * e) / denominator).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let t = (b * s + f) / e;
            if t < 0.0 {
                ((-c / a).clamp(0.0, 1.0), 0.0)
            } else if t > 1.0 {
                (((b - c) / a).clamp(0.0, 1.0), 1.0)
            } else {
                (s, t)
            }
        }
    };
    (add(p1, scale(d1, s)), add(p2, scale(d2, t)))
}

pub(super) fn segment_hits_triangle(p: Point3, q: Point3, [a, b, c]: Triangle) -> Option<Point3> {
    let direction = sub(q, p);
    let edge1 = sub(b, a);
    let edge2 = sub(c, a);
    let h = cross(direction, edge2);
    let determinant = dot(edge1, h);
    if determinant.abs() <= DEGENERATE {
        return None;
    }
    let inverse = 1.0 / determinant;
    let s = sub(p, a);
    let u = inverse * dot(s, h);
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let qv = cross(s, edge1);
    let v = inverse * dot(direction, qv);
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = inverse * dot(edge2, qv);
    (0.0..=1.0)
        .contains(&t)
        .then(|| add(p, scale(direction, t)))
}

pub(super) fn triangle_distance(first: Triangle, second: Triangle) -> (f64, Point3, Point3) {
    for (edges_of, other) in [(first, second), (second, first)] {
        for index in 0..3 {
            if let Some(hit) =
                segment_hits_triangle(edges_of[index], edges_of[(index + 1) % 3], other)
            {
                return (0.0, hit, hit);
            }
        }
    }
    let mut best = (f64::INFINITY, first[0], second[0]);
    let mut consider = |on_first: Point3, on_second: Point3| {
        let distance = norm(sub(on_first, on_second));
        if distance < best.0 {
            best = (distance, on_first, on_second);
        }
    };
    for vertex in first {
        consider(vertex, closest_point_on_triangle(vertex, second));
    }
    for vertex in second {
        consider(closest_point_on_triangle(vertex, first), vertex);
    }
    for i in 0..3 {
        for j in 0..3 {
            let (on_first, on_second) = closest_points_on_segments(
                first[i],
                first[(i + 1) % 3],
                second[j],
                second[(j + 1) % 3],
            );
            consider(on_first, on_second);
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLOOR: Triangle = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-12
    }

    #[test]
    fn a_triangle_above_another_is_its_height_away() {
        let above = [[0.2, 0.2, 0.5], [0.6, 0.2, 0.5], [0.2, 0.6, 0.5]];
        assert!(close(triangle_distance(FLOOR, above).0, 0.5));
    }

    #[test]
    fn crossing_triangles_are_zero_distance() {
        let crossing = [[0.2, 0.2, -1.0], [0.2, 0.2, 1.0], [0.3, 0.1, 0.0]];
        assert_eq!(triangle_distance(FLOOR, crossing).0, 0.0);
    }

    #[test]
    fn skew_triangles_report_their_closest_points() {
        let skew = [[2.0, -1.0, 1.0], [2.0, 1.0, 1.0], [3.0, 0.0, 1.0]];
        let (distance, on_first, on_second) = triangle_distance(FLOOR, skew);
        assert!(close(distance, 2.0_f64.sqrt()));
        assert_eq!(on_first, [1.0, 0.0, 0.0]);
        assert_eq!(on_second, [2.0, 0.0, 1.0]);
    }

    #[test]
    fn coplanar_triangles_side_by_side_are_the_edge_gap_apart() {
        let beside = [[1.5, 0.0, 0.0], [2.5, 0.0, 0.0], [1.5, 1.0, 0.0]];
        assert!(close(triangle_distance(FLOOR, beside).0, 0.5));
    }

    #[test]
    fn a_point_above_the_interior_projects_straight_down() {
        assert_eq!(
            closest_point_on_triangle([0.25, 0.25, 3.0], FLOOR),
            [0.25, 0.25, 0.0]
        );
    }
}
