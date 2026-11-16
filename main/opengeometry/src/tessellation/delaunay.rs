use std::collections::{BTreeSet, HashMap};
use std::f64::consts::PI;

const ANGLE_MARGIN: f64 = 1e-12;
const COLLINEAR: f64 = 1e-9;

type Pt = [f64; 2];
type Edge = (usize, usize);

pub(super) struct Triangulation<'a> {
    pub(super) triangles: Vec<[usize; 3]>,
    owners: HashMap<Edge, usize>,
    metric: [f64; 2],
    fixed: &'a BTreeSet<Edge>,
}

impl<'a> Triangulation<'a> {
    pub(super) fn new(
        triangles: Vec<[usize; 3]>,
        metric: [f64; 2],
        fixed: &'a BTreeSet<Edge>,
    ) -> Self {
        let mut owners = HashMap::with_capacity(triangles.len() * 3);
        for (index, triangle) in triangles.iter().enumerate() {
            for k in 0..3 {
                owners.insert((triangle[k], triangle[(k + 1) % 3]), index);
            }
        }
        Self {
            triangles,
            owners,
            metric,
            fixed,
        }
    }

    pub(super) fn free_edges(&self) -> Vec<Edge> {
        let mut edges = Vec::with_capacity(self.triangles.len() * 3);
        for triangle in &self.triangles {
            for k in 0..3 {
                let (a, b) = (triangle[k], triangle[(k + 1) % 3]);
                if a < b && self.is_free(a, b) {
                    edges.push((a, b));
                }
            }
        }
        edges
    }

    pub(super) fn has_edge(&self, a: usize, b: usize) -> bool {
        self.owners.contains_key(&(a, b)) && self.owners.contains_key(&(b, a))
    }

    pub(super) fn flip(&mut self, uv: &[Pt], mut stack: Vec<Edge>) -> Vec<Edge> {
        let mut created = Vec::new();
        while let Some((a, b)) = stack.pop() {
            let (Some(&t), Some(&u)) = (self.owners.get(&(a, b)), self.owners.get(&(b, a))) else {
                continue;
            };
            let r = opposite(self.triangles[t], a, b);
            let s = opposite(self.triangles[u], b, a);
            let point = |index: usize| self.scaled(uv[index]);
            if !self.is_free(a, b) || !should_flip(point(a), point(b), point(r), point(s)) {
                continue;
            }
            self.exchange([t, u], a, b, r, s);
            stack.extend([(a, s), (s, b), (b, r), (r, a)]);
            created.push((r, s));
        }
        created
    }

    pub(super) fn insert_on_boundary(
        &mut self,
        uv: &[Pt],
        from: usize,
        to: usize,
        m: usize,
    ) -> Vec<Edge> {
        let (a, b, t) = match (self.owners.get(&(from, to)), self.owners.get(&(to, from))) {
            (Some(&t), _) => (from, to, t),
            (_, Some(&t)) => (to, from, t),
            _ => return Vec::new(),
        };
        let r = opposite(self.triangles[t], a, b);
        let t2 = self.triangles.len();
        self.triangles[t] = [a, m, r];
        self.triangles.push([m, b, r]);
        self.owners.remove(&(a, b));
        for (edge, owner) in [
            ((a, m), t),
            ((m, r), t),
            ((m, b), t2),
            ((b, r), t2),
            ((r, m), t2),
        ] {
            self.owners.insert(edge, owner);
        }
        self.flip(uv, vec![(b, r), (r, a)])
    }

    fn exchange(&mut self, [t, u]: [usize; 2], a: usize, b: usize, r: usize, s: usize) {
        self.triangles[t] = [a, s, r];
        self.triangles[u] = [s, b, r];
        self.owners.remove(&(a, b));
        self.owners.remove(&(b, a));
        self.owners.insert((a, s), t);
        self.owners.insert((s, r), t);
        self.owners.insert((b, r), u);
        self.owners.insert((r, s), u);
    }

    pub(super) fn split(&mut self, uv: &[Pt], a: usize, b: usize, m: usize) -> Vec<Edge> {
        let (Some(&t), Some(&u)) = (self.owners.get(&(a, b)), self.owners.get(&(b, a))) else {
            return Vec::new();
        };
        let r = opposite(self.triangles[t], a, b);
        let s = opposite(self.triangles[u], b, a);
        let (t2, u2) = (self.triangles.len(), self.triangles.len() + 1);
        self.triangles[t] = [a, m, r];
        self.triangles[u] = [b, m, s];
        self.triangles.extend([[m, b, r], [m, a, s]]);
        self.owners.remove(&(a, b));
        self.owners.remove(&(b, a));
        for (edge, owner) in [
            ((a, m), t),
            ((m, r), t),
            ((m, b), t2),
            ((b, r), t2),
            ((r, m), t2),
            ((b, m), u),
            ((m, s), u),
            ((m, a), u2),
            ((a, s), u2),
            ((s, m), u2),
        ] {
            self.owners.insert(edge, owner);
        }
        let mut touched = vec![(a, m), (m, b), (m, r), (m, s)];
        touched.extend(self.flip(uv, vec![(b, r), (r, a), (a, s), (s, b)]));
        touched
    }

    fn is_free(&self, a: usize, b: usize) -> bool {
        !self.fixed.contains(&(a.min(b), a.max(b)))
    }

    fn scaled(&self, p: Pt) -> Pt {
        [p[0] * self.metric[0], p[1] * self.metric[1]]
    }
}

pub(super) fn straight_run_corners(
    uv: &[Pt],
    ring: std::ops::Range<usize>,
    metric: [f64; 2],
) -> Vec<usize> {
    let count = ring.len();
    let point = |offset: usize| {
        let p = uv[ring.start + offset % count];
        [p[0] * metric[0], p[1] * metric[1]]
    };
    let bends = |offset: usize| {
        let (previous, here, next) = (point(offset + count - 1), point(offset), point(offset + 1));
        sine(previous, next, here).abs() > COLLINEAR
            || (here[0] - previous[0]) * (next[0] - here[0])
                + (here[1] - previous[1]) * (next[1] - here[1])
                <= 0.0
    };
    let Some(first) = (0..count).find(|&offset| bends(offset)) else {
        return ring.collect();
    };
    let mut corners = vec![first];
    for step in 1..count {
        let offset = first + step;
        let last = corners[corners.len() - 1];
        let (previous, here, next) = (point(last), point(offset), point(offset + 1));
        let ahead = (here[0] - previous[0]) * (next[0] - here[0])
            + (here[1] - previous[1]) * (next[1] - here[1]);
        if sine(previous, next, here).abs() > COLLINEAR || ahead <= 0.0 {
            corners.push(offset % count);
        }
    }
    corners
        .into_iter()
        .map(|offset| ring.start + offset)
        .collect()
}

fn opposite(triangle: [usize; 3], a: usize, b: usize) -> usize {
    (0..3)
        .map(|k| triangle[k])
        .find(|&vertex| vertex != a && vertex != b)
        .unwrap_or(triangle[0])
}

fn should_flip(a: Pt, b: Pt, r: Pt, s: Pt) -> bool {
    convex(a, b, r, s) && angle(a, r, b) + angle(a, s, b) > PI + ANGLE_MARGIN
}

fn convex(a: Pt, b: Pt, r: Pt, s: Pt) -> bool {
    let side_a = sine(r, s, a);
    let side_b = sine(r, s, b);
    side_a.abs() > COLLINEAR && side_b.abs() > COLLINEAR && side_a * side_b < 0.0
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
