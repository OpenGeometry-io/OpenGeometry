use crate::geom2d::poly2d::Pt2;
use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum CurveEdge2 {
    Line {
        from: [f64; 2],
        to: [f64; 2],
    },
    Arc {
        center: [f64; 2],
        radius: f64,
        start_angle: f64,
        sweep_angle: f64,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CurveRegion2 {
    pub(crate) outer: Vec<CurveEdge2>,
    #[serde(default)]
    pub(crate) holes: Vec<Vec<CurveEdge2>>,
}

pub(super) fn canonicalize_ring(ring: &mut [CurveEdge2]) {
    if let Some((index, _)) = ring.iter().enumerate().min_by(|(_, first), (_, second)| {
        compare_points(first.point(0.0), second.point(0.0))
            .then_with(|| compare_points(first.point(0.5), second.point(0.5)))
            .then_with(|| compare_points(first.point(1.0), second.point(1.0)))
    }) {
        ring.rotate_left(index);
    }
}

pub(super) fn compare_points(a: Pt2, b: Pt2) -> std::cmp::Ordering {
    a.x.total_cmp(&b.x).then_with(|| a.z.total_cmp(&b.z))
}

impl CurveEdge2 {
    pub(crate) fn point(&self, t: f64) -> Pt2 {
        match self {
            Self::Line { from, to } => add(p(*from), scale(sub(p(*to), p(*from)), t)),
            Self::Arc {
                center,
                radius,
                start_angle,
                sweep_angle,
            } => {
                let angle = start_angle + sweep_angle * t;
                Pt2::new(
                    center[0] + radius * angle.cos(),
                    center[1] + radius * angle.sin(),
                )
            }
        }
    }

    pub(crate) fn tangent(&self, t: f64) -> Pt2 {
        match self {
            Self::Line { from, to } => sub(p(*to), p(*from)),
            Self::Arc {
                radius,
                start_angle,
                sweep_angle,
                ..
            } => {
                let angle = start_angle + sweep_angle * t;
                Pt2::new(
                    -radius * sweep_angle * angle.sin(),
                    radius * sweep_angle * angle.cos(),
                )
            }
        }
    }

    pub(crate) fn length(&self) -> f64 {
        match self {
            Self::Line { .. } => distance(self.point(0.0), self.point(1.0)),
            Self::Arc {
                radius,
                sweep_angle,
                ..
            } => radius * sweep_angle.abs(),
        }
    }

    pub(crate) fn reversed(&self) -> Self {
        match self {
            Self::Line { from, to } => Self::Line {
                from: *to,
                to: *from,
            },
            Self::Arc {
                center,
                radius,
                start_angle,
                sweep_angle,
            } => Self::Arc {
                center: *center,
                radius: *radius,
                start_angle: start_angle + sweep_angle,
                sweep_angle: -*sweep_angle,
            },
        }
    }

    pub(crate) fn slice(&self, a: f64, b: f64) -> Self {
        match self {
            Self::Line { .. } => Self::Line {
                from: arr(self.point(a)),
                to: arr(self.point(b)),
            },
            Self::Arc {
                center,
                radius,
                start_angle,
                sweep_angle,
            } => Self::Arc {
                center: *center,
                radius: *radius,
                start_angle: start_angle + sweep_angle * a,
                sweep_angle: sweep_angle * (b - a),
            },
        }
    }

    pub(crate) fn parameter(&self, point: Pt2, eps: f64) -> Option<f64> {
        match self {
            Self::Line { from, to } => {
                let d = sub(p(*to), p(*from));
                let len2 = dot(d, d);
                if len2 <= eps * eps {
                    return None;
                }
                let t = dot(sub(point, p(*from)), d) / len2;
                (t >= -eps / len2.sqrt()
                    && t <= 1.0 + eps / len2.sqrt()
                    && distance(self.point(t), point) <= eps)
                    .then_some(t.clamp(0.0, 1.0))
            }
            Self::Arc {
                center,
                radius,
                start_angle,
                sweep_angle,
            } => {
                let radial = sub(point, p(*center));
                if (radial.x.hypot(radial.z) - radius).abs() > eps {
                    return None;
                }
                let angle = radial.z.atan2(radial.x);
                let travel = if *sweep_angle > 0.0 {
                    (angle - start_angle).rem_euclid(TAU)
                } else {
                    (start_angle - angle).rem_euclid(TAU)
                };
                let delta = eps / radius;
                if travel <= sweep_angle.abs() + delta {
                    Some((travel / sweep_angle.abs()).clamp(0.0, 1.0))
                } else if TAU - travel <= delta {
                    Some(0.0)
                } else {
                    None
                }
            }
        }
    }

    pub(crate) fn twice_area(&self) -> f64 {
        let a = self.point(0.0);
        let b = self.point(1.0);
        match self {
            Self::Line { .. } => cross(a, b),
            Self::Arc {
                center,
                radius,
                sweep_angle,
                ..
            } => radius * radius * sweep_angle + center[0] * (b.z - a.z) - center[1] * (b.x - a.x),
        }
    }
}

pub(super) fn add(a: Pt2, b: Pt2) -> Pt2 {
    Pt2::new(a.x + b.x, a.z + b.z)
}

pub(super) fn p(a: [f64; 2]) -> Pt2 {
    Pt2::new(a[0], a[1])
}

pub(super) fn scale(a: Pt2, s: f64) -> Pt2 {
    Pt2::new(a.x * s, a.z * s)
}

fn sub(a: Pt2, b: Pt2) -> Pt2 {
    Pt2::new(a.x - b.x, a.z - b.z)
}

pub(super) fn distance(a: Pt2, b: Pt2) -> f64 {
    sub(a, b).x.hypot(sub(a, b).z)
}

fn arr(a: Pt2) -> [f64; 2] {
    [a.x, a.z]
}

pub(super) fn dot(a: Pt2, b: Pt2) -> f64 {
    a.x * b.x + a.z * b.z
}
pub(super) fn cross(a: Pt2, b: Pt2) -> f64 {
    a.x * b.z - a.z * b.x
}

pub(crate) fn reversed_ring(ring: &[CurveEdge2]) -> Vec<CurveEdge2> {
    ring.iter().rev().map(CurveEdge2::reversed).collect()
}

pub(crate) fn intersections(a: &CurveEdge2, b: &CurveEdge2, eps: f64) -> Vec<Pt2> {
    let mut candidates = Vec::new();
    let mut offer = |hit: Pt2| {
        if a.parameter(hit, eps).is_some()
            && b.parameter(hit, eps).is_some()
            && !candidates.iter().any(|prior| distance(*prior, hit) <= eps)
        {
            candidates.push(hit);
        }
    };
    match (a, b) {
        (CurveEdge2::Line { from, to }, CurveEdge2::Line { from: bf, to: bt }) => {
            let d = sub(p(*to), p(*from));
            let e = sub(p(*bt), p(*bf));
            let denom = cross(d, e);
            if denom.abs() > eps * d.x.hypot(d.z) * e.x.hypot(e.z) {
                offer(add(
                    p(*from),
                    scale(d, cross(sub(p(*bf), p(*from)), e) / denom),
                ));
            } else {
                for endpoint in [*from, *to, *bf, *bt] {
                    offer(p(endpoint));
                }
            }
        }
        (CurveEdge2::Line { from, to }, CurveEdge2::Arc { center, radius, .. })
        | (CurveEdge2::Arc { center, radius, .. }, CurveEdge2::Line { from, to }) => {
            let d = sub(p(*to), p(*from));
            let f = sub(p(*from), p(*center));
            let aa = dot(d, d);
            if aa <= eps * eps {
                return candidates;
            }
            let bb = 2.0 * dot(f, d);
            let cc = dot(f, f) - radius * radius;
            let disc = bb * bb - 4.0 * aa * cc;
            if disc >= -eps * eps * aa {
                let root = disc.max(0.0).sqrt();
                for t in [(-bb - root) / (2.0 * aa), (-bb + root) / (2.0 * aa)] {
                    offer(add(p(*from), scale(d, t)));
                }
            }
        }
        (
            CurveEdge2::Arc {
                center: ca,
                radius: ra,
                ..
            },
            CurveEdge2::Arc {
                center: cb,
                radius: rb,
                ..
            },
        ) => {
            let centre_delta = sub(p(*cb), p(*ca));
            let d = centre_delta.x.hypot(centre_delta.z);
            if d <= eps && (ra - rb).abs() <= eps {
                for endpoint in [a.point(0.0), a.point(1.0), b.point(0.0), b.point(1.0)] {
                    offer(endpoint);
                }
            } else if d > eps && d <= ra + rb + eps && d + ra.min(*rb) + eps >= ra.max(*rb) {
                let along = (ra * ra - rb * rb + d * d) / (2.0 * d);
                let height2 = ra * ra - along * along;
                if height2 >= -eps * eps {
                    let u = scale(centre_delta, 1.0 / d);
                    let base = add(p(*ca), scale(u, along));
                    let normal = Pt2::new(-u.z, u.x);
                    let h = height2.max(0.0).sqrt();
                    offer(add(base, scale(normal, h)));
                    offer(add(base, scale(normal, -h)));
                }
            }
        }
    }
    candidates
}
