use openmaths::Vector3;

pub const DEFAULT_EPS: f64 = 1.0e-6;

pub const DEFAULT_MITER_LIMIT: f64 = 4.0;

#[derive(Clone, Copy, Debug)]
pub struct Pt2 {
    pub x: f64,
    pub z: f64,
}

impl Pt2 {
    pub fn new(x: f64, z: f64) -> Self {
        Self { x, z }
    }
}

#[inline]
pub fn cross2(a: Pt2, b: Pt2) -> f64 {
    a.x * b.z - a.z * b.x
}

#[inline]
pub fn dot2(a: Pt2, b: Pt2) -> f64 {
    a.x * b.x + a.z * b.z
}

pub fn intersect2(a: Pt2, da: Pt2, b: Pt2, db: Pt2, eps: f64) -> Option<Pt2> {
    let denom = cross2(da, db);
    if denom.abs() < eps {
        return None;
    }
    let t = ((b.x - a.x) * db.z - (b.z - a.z) * db.x) / denom;
    Some(Pt2::new(a.x + t * da.x, a.z + t * da.z))
}

pub fn signed_area2(ring: &[Pt2]) -> f64 {
    let mut area = 0.0;
    let count = ring.len();
    for i in 0..count {
        let c = ring[i];
        let n = ring[(i + 1) % count];
        area += c.x * n.z - n.x * c.z;
    }
    area / 2.0
}

pub fn segments_cross2(a1: Pt2, a2: Pt2, b1: Pt2, b2: Pt2, eps: f64) -> bool {
    let d1 = Pt2::new(a2.x - a1.x, a2.z - a1.z);
    let d2 = Pt2::new(b2.x - b1.x, b2.z - b1.z);
    let denom = cross2(d1, d2);
    if denom.abs() < eps {
        return false;
    }
    let diff = Pt2::new(b1.x - a1.x, b1.z - a1.z);
    let t = cross2(diff, d2) / denom;
    let s = cross2(diff, d1) / denom;
    t > eps && t < 1.0 - eps && s > eps && s < 1.0 - eps
}

pub fn self_intersects2(ring: &[Pt2], eps: f64) -> bool {
    let m = ring.len();
    for i in 0..m {
        for j in (i + 1)..m {
            if j == i + 1 || (i == 0 && j == m - 1) {
                continue;
            }
            if segments_cross2(ring[i], ring[(i + 1) % m], ring[j], ring[(j + 1) % m], eps) {
                return true;
            }
        }
    }
    false
}

#[inline]
pub fn xz(v: &Vector3) -> Pt2 {
    Pt2::new(v.x, v.z)
}

pub fn signed_area_xz(ring: &[Vector3]) -> f64 {
    let pts: Vec<Pt2> = ring.iter().map(xz).collect();
    signed_area2(&pts)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Winding {
    Cw,
    Ccw,
}

pub fn normalize_winding(ring: &[Vector3], want: Winding) -> Vec<Vector3> {
    let is_cw = signed_area_xz(ring) < 0.0;
    if is_cw == (want == Winding::Cw) {
        ring.to_vec()
    } else {
        ring.iter().rev().copied().collect()
    }
}

pub fn canonical_ring(ring: &[Vector3]) -> Vec<Vector3> {
    if ring.len() < 2 {
        return ring.to_vec();
    }
    let mut min_index = 0;
    for i in 1..ring.len() {
        let dx = ring[i].x - ring[min_index].x;
        if dx < -1.0e-9 || (dx.abs() <= 1.0e-9 && ring[i].z < ring[min_index].z - 1.0e-9) {
            min_index = i;
        }
    }
    let mut out = Vec::with_capacity(ring.len());
    out.extend_from_slice(&ring[min_index..]);
    out.extend_from_slice(&ring[..min_index]);
    out
}

pub fn dedupe_ring(ring: &[Pt2], eps: f64) -> Vec<Pt2> {
    let mut out: Vec<Pt2> = Vec::with_capacity(ring.len());
    for p in ring {
        match out.last() {
            Some(last) if (last.x - p.x).hypot(last.z - p.z) <= eps => {}
            _ => out.push(*p),
        }
    }
    if out.len() > 1 {
        let first = out[0];
        let last = out[out.len() - 1];
        if (first.x - last.x).hypot(first.z - last.z) <= eps {
            out.pop();
        }
    }
    out
}

pub fn point_in_ring2(p: Pt2, ring: &[Pt2]) -> bool {
    let mut inside = false;
    let n = ring.len();
    if n == 0 {
        return false;
    }
    let mut j = n - 1;
    for i in 0..n {
        let a = ring[i];
        let b = ring[j];
        if (a.z > p.z) != (b.z > p.z) && p.x < (b.x - a.x) * (p.z - a.z) / (b.z - a.z) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

pub fn ring_inside2(inner: &[Pt2], outer: &[Pt2], eps: f64) -> bool {
    for p in inner {
        if !point_in_ring2(*p, outer) {
            return false;
        }
    }
    for i in 0..inner.len() {
        for j in 0..outer.len() {
            if segments_cross2(
                inner[i],
                inner[(i + 1) % inner.len()],
                outer[j],
                outer[(j + 1) % outer.len()],
                eps,
            ) {
                return false;
            }
        }
    }
    true
}

pub fn segment_cross_t2(a1: Pt2, a2: Pt2, b1: Pt2, b2: Pt2, eps: f64) -> Option<(Pt2, f64, f64)> {
    let r = Pt2::new(a2.x - a1.x, a2.z - a1.z);
    let s = Pt2::new(b2.x - b1.x, b2.z - b1.z);
    let denom = cross2(r, s);
    if denom.abs() < eps {
        return None;
    }
    let qp = Pt2::new(b1.x - a1.x, b1.z - a1.z);
    let ta = cross2(qp, s) / denom;
    let tb = cross2(qp, r) / denom;
    if ta > eps && ta < 1.0 - eps && tb > eps && tb < 1.0 - eps {
        Some((Pt2::new(a1.x + ta * r.x, a1.z + ta * r.z), ta, tb))
    } else {
        None
    }
}

pub fn winding_number2(p: Pt2, ring: &[Pt2]) -> i32 {
    let mut wn = 0;
    let count = ring.len();
    for i in 0..count {
        let a = ring[i];
        let b = ring[(i + 1) % count];
        let on_left = cross2(
            Pt2::new(b.x - a.x, b.z - a.z),
            Pt2::new(p.x - a.x, p.z - a.z),
        );
        if a.z <= p.z {
            if b.z > p.z && on_left > 0.0 {
                wn += 1;
            }
        } else if b.z <= p.z && on_left < 0.0 {
            wn -= 1;
        }
    }
    wn
}

pub type Edge2 = (Pt2, Pt2);

pub fn qkey(p: Pt2) -> (i64, i64) {
    ((p.x * 1.0e6).round() as i64, (p.z * 1.0e6).round() as i64)
}

pub fn point_on_edge_t(a: Pt2, b: Pt2, p: Pt2, eps: f64) -> Option<f64> {
    let rx = b.x - a.x;
    let rz = b.z - a.z;
    let len2 = rx * rx + rz * rz;
    if len2 < eps * eps {
        return None;
    }
    let t = ((p.x - a.x) * rx + (p.z - a.z) * rz) / len2;
    if t <= eps || t >= 1.0 - eps {
        return None;
    }
    if (a.x + t * rx - p.x).hypot(a.z + t * rz - p.z) < eps {
        Some(t)
    } else {
        None
    }
}
