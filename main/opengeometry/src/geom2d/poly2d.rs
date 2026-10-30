#[derive(Clone, Copy, Debug)]
pub(crate) struct Pt2 {
    pub(crate) x: f64,
    pub(crate) z: f64,
}

impl Pt2 {
    pub(crate) fn new(x: f64, z: f64) -> Self {
        Self { x, z }
    }
}

#[inline]
pub(super) fn cross2(a: Pt2, b: Pt2) -> f64 {
    a.x * b.z - a.z * b.x
}

#[inline]
pub(super) fn dot2(a: Pt2, b: Pt2) -> f64 {
    a.x * b.x + a.z * b.z
}

pub(crate) fn signed_area2(ring: &[Pt2]) -> f64 {
    let mut area = 0.0;
    let count = ring.len();
    for i in 0..count {
        let c = ring[i];
        let n = ring[(i + 1) % count];
        area += c.x * n.z - n.x * c.z;
    }
    area / 2.0
}

pub(crate) fn segments_cross2(a1: Pt2, a2: Pt2, b1: Pt2, b2: Pt2, eps: f64) -> bool {
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

pub(crate) fn self_intersects2(ring: &[Pt2], eps: f64) -> bool {
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

pub(super) fn dedupe_ring(ring: &[Pt2], eps: f64) -> Vec<Pt2> {
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

pub(crate) fn point_in_ring2(p: Pt2, ring: &[Pt2]) -> bool {
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

pub(super) fn ring_inside2(inner: &[Pt2], outer: &[Pt2], eps: f64) -> bool {
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

pub(super) fn segment_cross_t2(
    a1: Pt2,
    a2: Pt2,
    b1: Pt2,
    b2: Pt2,
    eps: f64,
) -> Option<(Pt2, f64, f64)> {
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

pub(crate) fn winding_number2(p: Pt2, ring: &[Pt2]) -> i32 {
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

pub(super) type Edge2 = (Pt2, Pt2);

pub(super) fn point_on_edge_t(a: Pt2, b: Pt2, p: Pt2, eps: f64) -> Option<f64> {
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
