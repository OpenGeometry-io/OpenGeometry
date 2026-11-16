use std::collections::{BTreeSet, HashMap};
use std::f64::consts::PI;

const ANGLE_MARGIN: f64 = 1e-12;

type Pt = [f64; 2];

pub(super) fn delaunay_flips(
    triangles: &mut [[usize; 3]],
    uv: &[Pt],
    metric: [f64; 2],
    fixed: &BTreeSet<(usize, usize)>,
) {
    let point = |index: usize| [uv[index][0] * metric[0], uv[index][1] * metric[1]];
    let mut owners = HashMap::with_capacity(triangles.len() * 3);
    let mut stack = Vec::with_capacity(triangles.len() * 3);
    for (index, triangle) in triangles.iter().enumerate() {
        for k in 0..3 {
            let (a, b) = (triangle[k], triangle[(k + 1) % 3]);
            owners.insert((a, b), index);
            push_free(&mut stack, fixed, a, b);
        }
    }
    while let Some((a, b)) = stack.pop() {
        let (Some(&t), Some(&u)) = (owners.get(&(a, b)), owners.get(&(b, a))) else {
            continue;
        };
        let r = opposite(triangles[t], a, b);
        let s = opposite(triangles[u], b, a);
        if !should_flip(point(a), point(b), point(r), point(s)) {
            continue;
        }
        triangles[t] = [a, s, r];
        triangles[u] = [s, b, r];
        owners.remove(&(a, b));
        owners.remove(&(b, a));
        owners.insert((a, s), t);
        owners.insert((s, r), t);
        owners.insert((b, r), u);
        owners.insert((r, s), u);
        for (p, q) in [(a, s), (s, b), (b, r), (r, a)] {
            push_free(&mut stack, fixed, p, q);
        }
    }
}

fn push_free(
    stack: &mut Vec<(usize, usize)>,
    fixed: &BTreeSet<(usize, usize)>,
    a: usize,
    b: usize,
) {
    let key = if a < b { (a, b) } else { (b, a) };
    if !fixed.contains(&key) {
        stack.push(key);
    }
}

fn opposite(triangle: [usize; 3], a: usize, b: usize) -> usize {
    (0..3)
        .map(|k| triangle[k])
        .find(|&vertex| vertex != a && vertex != b)
        .unwrap_or(triangle[0])
}

fn should_flip(a: Pt, b: Pt, r: Pt, s: Pt) -> bool {
    let side_a = sine(r, s, a);
    let side_b = sine(r, s, b);
    if side_a.abs() <= ANGLE_MARGIN || side_b.abs() <= ANGLE_MARGIN || side_a * side_b > 0.0 {
        return false;
    }
    angle(a, r, b) + angle(a, s, b) > PI + ANGLE_MARGIN
}

fn sine(from: Pt, to: Pt, p: Pt) -> f64 {
    let d = [to[0] - from[0], to[1] - from[1]];
    let e = [p[0] - from[0], p[1] - from[1]];
    let length = d[0].hypot(d[1]) * e[0].hypot(e[1]);
    if length == 0.0 {
        return 0.0;
    }
    (d[0] * e[1] - d[1] * e[0]) / length
}

fn angle(a: Pt, apex: Pt, b: Pt) -> f64 {
    let d = [a[0] - apex[0], a[1] - apex[1]];
    let e = [b[0] - apex[0], b[1] - apex[1]];
    (d[0] * e[1] - d[1] * e[0])
        .abs()
        .atan2(d[0] * e[0] + d[1] * e[1])
}
