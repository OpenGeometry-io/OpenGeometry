use super::super::delaunay::{straight_run_corners, Triangulation};
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
    let fan = (1..count - 1).map(|k| [0, k, k + 1]).collect::<Vec<_>>();
    let area = fan.iter().map(|&t| signed_area(&uv, t)).sum::<f64>();
    let fixed = ring_edges(count);
    let mut triangulation = Triangulation::new(fan, [1.0, 4.0], &fixed);
    triangulation.flip(&uv, triangulation.free_edges());
    let triangles = triangulation.triangles;
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
    let mut triangulation = Triangulation::new(vec![[0, 1, 2], [0, 2, 3]], [1.0, 1.0], &fixed);
    triangulation.flip(&uv, triangulation.free_edges());
    let triangles = triangulation.triangles;
    assert!(triangles.iter().all(|&t| signed_area(&uv, t) > 0.0));
    assert!(triangles.iter().all(|t| t.contains(&1) && t.contains(&3)));
}

#[test]
fn a_split_keeps_the_triangulation_whole_and_delaunay() {
    let count = 12;
    let mut uv = ring(count);
    let fixed = ring_edges(count);
    let fan = (1..count - 1).map(|k| [0, k, k + 1]).collect::<Vec<_>>();
    let mut triangulation = Triangulation::new(fan, [1.0, 4.0], &fixed);
    triangulation.flip(&uv, triangulation.free_edges());
    let area = |t: &Triangulation| {
        t.triangles
            .iter()
            .map(|&x| signed_area(&uv, x))
            .sum::<f64>()
    };
    let before = area(&triangulation);
    let (a, b) = triangulation.free_edges()[0];
    uv.push([(uv[a][0] + uv[b][0]) * 0.5, (uv[a][1] + uv[b][1]) * 0.5]);
    let touched = triangulation.split(&uv, a, b, count);
    assert!(!triangulation.has_edge(a, b));
    assert!(touched.iter().all(|&(p, q)| triangulation.has_edge(p, q)));
    assert_eq!(triangulation.triangles.len(), count);
    let scaled = uv.iter().map(|p| [p[0], 4.0 * p[1]]).collect::<Vec<_>>();
    for &t in &triangulation.triangles {
        assert!(signed_area(&uv, t) > 0.0);
        let corners = t.map(|index| scaled[index]);
        for (index, &point) in scaled.iter().enumerate() {
            assert!(t.contains(&index) || !inside_circumcircle(corners, point));
        }
    }
    let after = triangulation
        .triangles
        .iter()
        .map(|&x| signed_area(&uv, x))
        .sum::<f64>();
    assert!((after - before).abs() < 1e-12);
}

#[test]
fn samples_along_straight_runs_are_inserted_on_the_boundary() {
    let uv = vec![
        [0.0, 0.0],
        [1.0, 0.0],
        [2.0, 0.0],
        [3.0, 0.0],
        [3.0, 1.0],
        [3.0, 2.0],
        [0.0, 2.0],
        [0.0, 1.0],
    ];
    let corners = straight_run_corners(&uv, 0..uv.len(), [1.0, 1.0]);
    assert_eq!(corners, vec![0, 3, 5, 6]);
    let fixed = ring_edges(uv.len());
    let mut triangulation = Triangulation::new(vec![[0, 3, 5], [0, 5, 6]], [1.0, 1.0], &fixed);
    for (from, to, m) in [(0, 3, 1), (1, 3, 2), (3, 5, 4), (6, 0, 7)] {
        triangulation.insert_on_boundary(&uv, from, to, m);
    }
    let triangles = triangulation.triangles;
    assert_eq!(triangles.len(), uv.len() - 2);
    assert!(triangles.iter().all(|&t| signed_area(&uv, t) > 0.0));
    let area = triangles.iter().map(|&t| signed_area(&uv, t)).sum::<f64>();
    assert!((area - 6.0).abs() < 1e-12);
}
