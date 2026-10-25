use super::poly2d::{
    cross2, dedupe_ring, dot2, point_on_edge_t, ring_inside2, segment_cross_t2, signed_area2,
    winding_number2, Edge2, Pt2,
};
use crate::brep::GeometryError;

pub(crate) struct RingRegion {
    pub(crate) outer: Vec<Pt2>,
    pub(crate) holes: Vec<Vec<Pt2>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PlanarBooleanOp {
    Union,
    Intersection,
    Subtraction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RegionBooleanError {
    OpenBoundary,
    OrphanHole,
}

struct WeldedBoundary {
    edges: Vec<Edge2>,
    ends: Vec<(usize, usize)>,
    vertex_count: usize,
}

struct VertexWeld {
    reach: f64,
    vertices: Vec<Pt2>,
    cells: std::collections::HashMap<(i64, i64), Vec<usize>>,
}

impl std::fmt::Display for RegionBooleanError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::OpenBoundary => "Boolean regions produced an open boundary",
            Self::OrphanHole => "Boolean regions produced a hole outside every region",
        })
    }
}

impl From<RegionBooleanError> for GeometryError {
    fn from(error: RegionBooleanError) -> Self {
        Self::InvalidTopology(match error {
            RegionBooleanError::OpenBoundary => {
                "layered planar Boolean has an open boundary".into()
            }
            RegionBooleanError::OrphanHole => error.to_string(),
        })
    }
}

pub(crate) fn probe_reach(mid: Pt2, len: f64, most: f64, sub_edges: &[Edge2], eps: f64) -> f64 {
    let mut clearance = 2.0 * (len * 0.25).min(most);
    for &(a, b) in sub_edges {
        if a.x.min(b.x) - mid.x >= clearance
            || mid.x - a.x.max(b.x) >= clearance
            || a.z.min(b.z) - mid.z >= clearance
            || mid.z - a.z.max(b.z) >= clearance
        {
            continue;
        }
        let distance = distance_to_segment(mid, a, b);
        if distance > eps && distance < clearance {
            clearance = distance;
        }
    }
    clearance * 0.5
}

fn distance_to_segment(point: Pt2, a: Pt2, b: Pt2) -> f64 {
    let along = Pt2::new(b.x - a.x, b.z - a.z);
    let length_squared = dot2(along, along);
    let t = if length_squared > 0.0 {
        (dot2(Pt2::new(point.x - a.x, point.z - a.z), along) / length_squared).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (a.x + t * along.x - point.x).hypot(a.z + t * along.z - point.z)
}

pub(crate) fn ring_is_degenerate(ring: &[Pt2], eps: f64) -> bool {
    let perimeter: f64 = (0..ring.len())
        .map(|index| {
            let next = ring[(index + 1) % ring.len()];
            (ring[index].x - next.x).hypot(ring[index].z - next.z)
        })
        .sum();
    ring.len() < 3 || signed_area2(ring).abs() < eps * perimeter * 0.5
}

pub(crate) fn boolean_oriented_regions(
    a: &[Vec<Pt2>],
    b: &[Vec<Pt2>],
    operation: PlanarBooleanOp,
    eps: f64,
) -> Result<Vec<RingRegion>, RegionBooleanError> {
    let mut contours = Vec::with_capacity(a.len() + b.len());
    contours.extend_from_slice(a);
    contours.extend_from_slice(b);
    winding_regions_by(&contours, eps, |point| {
        let in_a = a
            .iter()
            .map(|ring| winding_number2(point, ring))
            .sum::<i32>()
            != 0;
        let in_b = b
            .iter()
            .map(|ring| winding_number2(point, ring))
            .sum::<i32>()
            != 0;
        match operation {
            PlanarBooleanOp::Union => in_a || in_b,
            PlanarBooleanOp::Intersection => in_a && in_b,
            PlanarBooleanOp::Subtraction => in_a && !in_b,
        }
    })
}

fn winding_regions_by(
    polys: &[Vec<Pt2>],
    eps: f64,
    filled: impl Fn(Pt2) -> bool,
) -> Result<Vec<RingRegion>, RegionBooleanError> {
    let mut edges: Vec<Edge2> = Vec::new();
    for poly in polys {
        let m = poly.len();
        for i in 0..m {
            let a = poly[i];
            let b = poly[(i + 1) % m];
            if (a.x - b.x).hypot(a.z - b.z) > eps {
                edges.push((a, b));
            }
        }
    }
    regions_from_edges_by(&edges, eps, filled)
}

pub(crate) fn regions_from_edges_by(
    edges: &[Edge2],
    eps: f64,
    filled: impl Fn(Pt2) -> bool,
) -> Result<Vec<RingRegion>, RegionBooleanError> {
    if edges.is_empty() {
        return Ok(Vec::new());
    }
    let split = subdivided_edges(edges, eps);
    let boundary = welded_boundary(boundary_edges(&split, eps, filled), eps);
    if boundary.edges.is_empty() {
        return Ok(Vec::new());
    }

    let raw_loops = boundary_loops(&boundary, eps)?;
    nested_regions(raw_loops, eps)
}

fn subdivided_edges(edges: &[Edge2], eps: f64) -> Vec<Edge2> {
    let verts: Vec<Pt2> = edges.iter().flat_map(|&(a, b)| [a, b]).collect();

    let mut split: Vec<Edge2> = Vec::new();
    for (i, &(ea, eb)) in edges.iter().enumerate() {
        let rx = eb.x - ea.x;
        let rz = eb.z - ea.z;
        let mut ts: Vec<f64> = vec![0.0, 1.0];
        for (j, &(fa, fb)) in edges.iter().enumerate() {
            if i != j {
                if let Some((_, ta, _)) = segment_cross_t2(ea, eb, fa, fb, eps) {
                    ts.push(ta);
                }
            }
        }
        for &v in &verts {
            if let Some(t) = point_on_edge_t(ea, eb, v, eps) {
                ts.push(t);
            }
        }
        ts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mut prev = f64::NEG_INFINITY;
        let mut clean: Vec<f64> = Vec::new();
        for t in ts {
            if t - prev > eps {
                clean.push(t);
                prev = t;
            }
        }
        for w in clean.windows(2) {
            let p0 = Pt2::new(ea.x + w[0] * rx, ea.z + w[0] * rz);
            let p1 = Pt2::new(ea.x + w[1] * rx, ea.z + w[1] * rz);
            if (p0.x - p1.x).hypot(p0.z - p1.z) > eps {
                split.push((p0, p1));
            }
        }
    }
    split
}

fn boundary_edges(split: &[Edge2], eps: f64, filled: impl Fn(Pt2) -> bool) -> Vec<Edge2> {
    let mut boundary: Vec<Edge2> = Vec::new();
    for &(a, b) in split {
        let dx = b.x - a.x;
        let dz = b.z - a.z;
        let len = dx.hypot(dz);
        if len < eps {
            continue;
        }
        let nx = -dz / len;
        let nz = dx / len;
        let mx = (a.x + b.x) / 2.0;
        let mz = (a.z + b.z) / 2.0;
        let probe = probe_reach(Pt2::new(mx, mz), len, 1.0e-4, split, eps);
        let left = filled(Pt2::new(mx + nx * probe, mz + nz * probe));
        let right = filled(Pt2::new(mx - nx * probe, mz - nz * probe));
        if left == right {
            continue;
        }
        if left {
            boundary.push((a, b));
        } else {
            boundary.push((b, a));
        }
    }
    boundary
}

fn welded_boundary(mut edges: Vec<Edge2>, eps: f64) -> WeldedBoundary {
    let mut weld = VertexWeld::new(eps.max(f64::EPSILON));
    let mut seen: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
    let mut ends: Vec<(usize, usize)> = Vec::new();
    edges.retain(|&(a, b)| {
        let key = (weld.weld(a), weld.weld(b));
        let fresh = key.0 != key.1 && seen.insert(key);
        if fresh {
            ends.push(key);
        }
        fresh
    });
    WeldedBoundary {
        edges,
        ends,
        vertex_count: weld.vertices.len(),
    }
}

impl VertexWeld {
    fn new(reach: f64) -> Self {
        Self {
            reach,
            vertices: Vec::new(),
            cells: std::collections::HashMap::new(),
        }
    }

    fn weld(&mut self, point: Pt2) -> usize {
        let (cx, cz) = self.cell(point);
        let mut found: Option<usize> = None;
        for key in [-1, 0, 1]
            .into_iter()
            .flat_map(|dx| [-1, 0, 1].map(|dz| (cx.saturating_add(dx), cz.saturating_add(dz))))
        {
            for &id in self
                .cells
                .get(&key)
                .map(|ids| ids.as_slice())
                .unwrap_or(&[])
            {
                let vertex = self.vertices[id];
                if (vertex.x - point.x).hypot(vertex.z - point.z) <= self.reach
                    && found.is_none_or(|best| id < best)
                {
                    found = Some(id);
                }
            }
        }
        found.unwrap_or_else(|| {
            self.vertices.push(point);
            self.cells
                .entry((cx, cz))
                .or_default()
                .push(self.vertices.len() - 1);
            self.vertices.len() - 1
        })
    }

    fn cell(&self, point: Pt2) -> (i64, i64) {
        (
            (point.x / self.reach).floor() as i64,
            (point.z / self.reach).floor() as i64,
        )
    }
}

fn boundary_loops(welded: &WeldedBoundary, eps: f64) -> Result<Vec<Vec<Pt2>>, RegionBooleanError> {
    let boundary = &welded.edges;
    let mut out_edges: Vec<Vec<usize>> = vec![Vec::new(); welded.vertex_count];
    for (idx, &(start, _)) in welded.ends.iter().enumerate() {
        out_edges[start].push(idx);
    }
    let mut used = vec![false; boundary.len()];
    let mut raw_loops: Vec<Vec<Pt2>> = Vec::new();
    for start in 0..boundary.len() {
        if used[start] {
            continue;
        }
        let mut loop_pts: Vec<Pt2> = Vec::new();
        let mut cur = start;
        let mut last = start;
        let mut guard = 0;
        while !used[cur] && guard <= boundary.len() {
            guard += 1;
            used[cur] = true;
            last = cur;
            let (a, b) = boundary[cur];
            loop_pts.push(a);
            let back = Pt2::new(a.x - b.x, a.z - b.z);
            let mut best: Option<usize> = None;
            let mut best_cw = f64::INFINITY;
            for &c in &out_edges[welded.ends[cur].1] {
                if used[c] {
                    continue;
                }
                let (ca, cb) = boundary[c];
                let out_dir = Pt2::new(cb.x - ca.x, cb.z - ca.z);
                let mut cw = -cross2(back, out_dir).atan2(dot2(back, out_dir));
                if cw < eps {
                    cw += 2.0 * std::f64::consts::PI;
                }
                if cw < best_cw {
                    best_cw = cw;
                    best = Some(c);
                }
            }
            match best {
                Some(next) => cur = next,
                None => break,
            }
        }
        if welded.ends[last].1 != welded.ends[start].0 {
            return Err(RegionBooleanError::OpenBoundary);
        }
        if loop_pts.len() >= 3 {
            raw_loops.push(loop_pts);
        }
    }
    Ok(raw_loops)
}

fn nested_regions(
    raw_loops: Vec<Vec<Pt2>>,
    eps: f64,
) -> Result<Vec<RingRegion>, RegionBooleanError> {
    let mut regions: Vec<RingRegion> = Vec::new();
    let mut holes: Vec<Vec<Pt2>> = Vec::new();
    for raw in raw_loops {
        let ring = dedupe_ring(&raw, eps);
        if ring_is_degenerate(&ring, eps) {
            continue;
        }
        if signed_area2(&ring) > 0.0 {
            regions.push(RingRegion {
                outer: ring,
                holes: Vec::new(),
            });
        } else {
            holes.push(ring);
        }
    }
    for hole in holes {
        regions
            .iter_mut()
            .filter(|region| ring_inside2(&hole, &region.outer, eps))
            .min_by(|first, second| {
                signed_area2(&first.outer)
                    .abs()
                    .total_cmp(&signed_area2(&second.outer).abs())
            })
            .ok_or(RegionBooleanError::OrphanHole)?
            .holes
            .push(hole);
    }
    Ok(regions)
}
