use super::super::delaunay::delaunay_flips;
use std::collections::BTreeSet;
use std::f64::consts::TAU;

fn ring(count: usize) -> Vec<[f64; 2]> {
    (0..count)
        .map(|index| {
            let angle = TAU * index as f64 / count as f64;
            [angle.cos(), 0.25 * angle.sin()]
        })
        .collect()
}

fn ring_edges(count: usize) -> BTreeSet<(usize, usize)> {
    (0..count)
        .map(|index| {
            let next = (index + 1) % count;
            (index.min(next), index.max(next))
        })
        .collect()
}

fn signed_area(uv: &[[f64; 2]], [a, b, c]: [usize; 3]) -> f64 {
    let d = [uv[b][0] - uv[a][0], uv[b][1] - uv[a][1]];
    let e = [uv[c][0] - uv[a][0], uv[c][1] - uv[a][1]];
    0.5 * (d[0] * e[1] - d[1] * e[0])
}

fn inside_circumcircle(p: [[f64; 2]; 3], q: [f64; 2]) -> bool {
    let rows = p.map(|v| {
        let (x, y) = (v[0] - q[0], v[1] - q[1]);
        [x, y, x * x + y * y]
    });
    let det = rows[0][0] * (rows[1][1] * rows[2][2] - rows[1][2] * rows[2][1])
        - rows[0][1] * (rows[1][0] * rows[2][2] - rows[1][2] * rows[2][0])
        + rows[0][2] * (rows[1][0] * rows[2][1] - rows[1][1] * rows[2][0]);
    det > 1e-9
}

#[test]
fn flips_turn_a_fan_of_needles_into_a_delaunay_triangulation() {
    let count = 24;
    let uv = ring(count);
    let mut triangles = (1..count - 1).map(|k| [0, k, k + 1]).collect::<Vec<_>>();
    let area = triangles.iter().map(|&t| signed_area(&uv, t)).sum::<f64>();
    delaunay_flips(&mut triangles, &uv, [1.0, 4.0], &ring_edges(count));
    let scaled = uv.iter().map(|p| [p[0], 4.0 * p[1]]).collect::<Vec<_>>();
    assert!((triangles.iter().map(|&t| signed_area(&uv, t)).sum::<f64>() - area).abs() < 1e-12);
    for &t in &triangles {
        assert!(signed_area(&uv, t) > 0.0);
        let corners = t.map(|index| scaled[index]);
        for (index, &point) in scaled.iter().enumerate() {
            assert!(t.contains(&index) || !inside_circumcircle(corners, point));
        }
    }
}

#[test]
fn flips_remove_a_flat_triangle_along_a_straight_boundary() {
    let uv = vec![[0.0, 0.0], [1.0, 0.0], [2.0, 0.0], [1.0, 1.0]];
    let fixed = BTreeSet::from([(0, 1), (1, 2), (2, 3), (0, 3)]);
    let mut triangles = vec![[0, 1, 2], [0, 2, 3]];
    delaunay_flips(&mut triangles, &uv, [1.0, 1.0], &fixed);
    assert!(triangles.iter().all(|&t| signed_area(&uv, t) > 0.0));
    assert!(triangles.iter().all(|t| t.contains(&1) && t.contains(&3)));
}
