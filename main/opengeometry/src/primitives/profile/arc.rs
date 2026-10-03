use crate::brep::GeometryError;
use crate::geom2d::{winding, CurveEdge2, Pt2};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProfileEdge {
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

impl ProfileEdge {
    pub(crate) fn endpoints(&self) -> ([f64; 2], [f64; 2]) {
        match self {
            Self::Line { from, to } => (*from, *to),
            Self::Arc {
                center,
                radius,
                start_angle,
                sweep_angle,
            } => {
                let point = |angle: f64| {
                    [
                        center[0] + radius * angle.cos(),
                        center[1] + radius * angle.sin(),
                    ]
                };
                (point(*start_angle), point(start_angle + sweep_angle))
            }
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
                sweep_angle: -sweep_angle,
            },
        }
    }

    fn signed_area_twice(&self) -> f64 {
        let (from, to) = self.endpoints();
        match self {
            Self::Line { .. } => from[0] * to[1] - to[0] * from[1],
            Self::Arc {
                center,
                radius,
                sweep_angle,
                ..
            } => {
                radius * radius * sweep_angle + center[0] * (to[1] - from[1])
                    - center[1] * (to[0] - from[0])
            }
        }
    }
}

pub(crate) fn validate_arc_profile_loop(
    edges: &[ProfileEdge],
    g: f64,
) -> Result<f64, GeometryError> {
    if edges.len() < 2 {
        return Err(GeometryError::InvalidGeometry(
            "arc-edged profile needs at least two edges".into(),
        ));
    }
    for (index, edge) in edges.iter().enumerate() {
        let (from, to) = edge.endpoints();
        if !from.into_iter().chain(to).all(f64::is_finite) {
            return Err(GeometryError::InvalidGeometry(
                "profile coordinates must be finite".into(),
            ));
        }
        match edge {
            ProfileEdge::Line { .. } => {
                if (to[0] - from[0]).hypot(to[1] - from[1]) <= 4.0 * g {
                    return Err(GeometryError::UnresolvedIntersection(
                        "profile line is below resolution".into(),
                    ));
                }
            }
            ProfileEdge::Arc {
                center,
                radius,
                start_angle,
                sweep_angle,
            } => {
                if !center.iter().all(|value| value.is_finite())
                    || !radius.is_finite()
                    || !start_angle.is_finite()
                    || !sweep_angle.is_finite()
                    || *radius <= 4.0 * g
                    || sweep_angle.abs() >= std::f64::consts::TAU
                    || radius * sweep_angle.abs() <= 4.0 * g
                {
                    return Err(GeometryError::InvalidGeometry(
                        "profile arc is unresolved".into(),
                    ));
                }
            }
        }
        let (next, _) = edges[(index + 1) % edges.len()].endpoints();
        if (to[0] - next[0]).hypot(to[1] - next[1]) > g {
            return Err(GeometryError::InvalidGeometry(
                "profile edges do not close".into(),
            ));
        }
    }
    check_self_intersections(edges, g)?;
    let signed_area_twice: f64 = edges.iter().map(ProfileEdge::signed_area_twice).sum();
    if signed_area_twice.abs() <= 16.0 * g * g {
        return Err(GeometryError::UnresolvedIntersection(
            "profile area is below resolution".into(),
        ));
    }
    Ok(signed_area_twice)
}

fn check_self_intersections(edges: &[ProfileEdge], g: f64) -> Result<(), GeometryError> {
    for first in 0..edges.len() {
        for second in (first + 1)..edges.len() {
            let adjacent = second == first + 1 || first == 0 && second == edges.len() - 1;
            let expected = if second == first + 1 {
                Some(edges[first].endpoints().1)
            } else if first == 0 && second == edges.len() - 1 {
                Some(edges[first].endpoints().0)
            } else {
                None
            };
            for point in profile_edge_intersections(&edges[first], &edges[second], g) {
                let at_expected = expected
                    .is_some_and(|shared| (point[0] - shared[0]).hypot(point[1] - shared[1]) <= g);
                let at_second_join = edges.len() == 2
                    && (point[0] - edges[first].endpoints().0[0])
                        .hypot(point[1] - edges[first].endpoints().0[1])
                        <= g;
                if !adjacent || !(at_expected || at_second_join) {
                    return Err(GeometryError::InvalidGeometry(
                        "arc-edged profile self-intersects".into(),
                    ));
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn profile_edge_intersections(
    a: &ProfileEdge,
    b: &ProfileEdge,
    tolerance: f64,
) -> Vec<[f64; 2]> {
    let mut candidates = Vec::new();
    let mut offer = |point: [f64; 2]| {
        if point.iter().all(|value| value.is_finite())
            && profile_edge_contains(a, point, tolerance)
            && profile_edge_contains(b, point, tolerance)
            && !candidates.iter().any(|other: &[f64; 2]| {
                (other[0] - point[0]).hypot(other[1] - point[1]) <= tolerance
            })
        {
            candidates.push(point);
        }
    };
    match (a, b) {
        (ProfileEdge::Line { from: p, to: q }, ProfileEdge::Line { from: r, to: s }) => {
            offer_line_crossing(*p, *q, *r, *s, tolerance, &mut offer);
        }
        (ProfileEdge::Line { from, to }, ProfileEdge::Arc { center, radius, .. })
        | (ProfileEdge::Arc { center, radius, .. }, ProfileEdge::Line { from, to }) => {
            offer_line_circle_points(*from, *to, *center, *radius, tolerance, &mut offer);
        }
        (
            ProfileEdge::Arc {
                center: ca,
                radius: ra,
                start_angle,
                sweep_angle,
            },
            ProfileEdge::Arc {
                center: cb,
                radius: rb,
                ..
            },
        ) => {
            let d = (cb[0] - ca[0]).hypot(cb[1] - ca[1]);
            if d <= tolerance && (ra - rb).abs() <= tolerance {
                let (a0, a1) = a.endpoints();
                let (b0, b1) = b.endpoints();
                for point in [a0, a1, b0, b1] {
                    offer(point);
                }
                let midpoint_angle = start_angle + sweep_angle * 0.5;
                offer([
                    ca[0] + ra * midpoint_angle.cos(),
                    ca[1] + ra * midpoint_angle.sin(),
                ]);
            } else if d > tolerance
                && d <= ra + rb + tolerance
                && d + ra.min(*rb) + tolerance >= ra.max(*rb)
            {
                let along = (ra * ra - rb * rb + d * d) / (2.0 * d);
                let perpendicular_sq = ra * ra - along * along;
                if perpendicular_sq >= -tolerance * tolerance {
                    let ux = (cb[0] - ca[0]) / d;
                    let uy = (cb[1] - ca[1]) / d;
                    let base = [ca[0] + along * ux, ca[1] + along * uy];
                    let off = perpendicular_sq.max(0.0).sqrt();
                    offer([base[0] - off * uy, base[1] + off * ux]);
                    offer([base[0] + off * uy, base[1] - off * ux]);
                }
            }
        }
    }
    candidates
}

fn profile_edge_contains(edge: &ProfileEdge, point: [f64; 2], tolerance: f64) -> bool {
    match edge {
        ProfileEdge::Line { from, to } => {
            let dx = to[0] - from[0];
            let dy = to[1] - from[1];
            let length = dx.hypot(dy);
            let px = point[0] - from[0];
            let py = point[1] - from[1];
            (dx * py - dy * px).abs() <= tolerance * length
                && px * dx + py * dy >= -tolerance * length
                && px * dx + py * dy <= length * length + tolerance * length
        }
        ProfileEdge::Arc {
            center,
            radius,
            start_angle,
            sweep_angle,
        } => {
            let dx = point[0] - center[0];
            let dy = point[1] - center[1];
            if (dx.hypot(dy) - radius).abs() > tolerance {
                return false;
            }
            let angle = dy.atan2(dx);
            let travel = if *sweep_angle > 0.0 {
                (angle - start_angle).rem_euclid(std::f64::consts::TAU)
            } else {
                (start_angle - angle).rem_euclid(std::f64::consts::TAU)
            };
            travel <= sweep_angle.abs() + tolerance / radius
                || std::f64::consts::TAU - travel <= tolerance / radius
        }
    }
}

fn offer_line_crossing(
    p: [f64; 2],
    q: [f64; 2],
    r: [f64; 2],
    s: [f64; 2],
    tolerance: f64,
    offer: &mut impl FnMut([f64; 2]),
) {
    let d = [q[0] - p[0], q[1] - p[1]];
    let e = [s[0] - r[0], s[1] - r[1]];
    let divisor = d[0] * e[1] - d[1] * e[0];
    if divisor.abs() > tolerance * (d[0].hypot(d[1]) + e[0].hypot(e[1])) {
        let relative = [r[0] - p[0], r[1] - p[1]];
        let t = (relative[0] * e[1] - relative[1] * e[0]) / divisor;
        offer([p[0] + t * d[0], p[1] + t * d[1]]);
    } else {
        for point in [p, q, r, s] {
            offer(point);
        }
    }
}

fn offer_line_circle_points(
    from: [f64; 2],
    to: [f64; 2],
    center: [f64; 2],
    radius: f64,
    tolerance: f64,
    offer: &mut impl FnMut([f64; 2]),
) {
    let d = [to[0] - from[0], to[1] - from[1]];
    let f = [from[0] - center[0], from[1] - center[1]];
    let aa = d[0] * d[0] + d[1] * d[1];
    let bb = 2.0 * (f[0] * d[0] + f[1] * d[1]);
    let cc = f[0] * f[0] + f[1] * f[1] - radius * radius;
    let discriminant = bb * bb - 4.0 * aa * cc;
    if discriminant >= -tolerance * tolerance * aa {
        let root = discriminant.max(0.0).sqrt();
        for t in [(-bb - root) / (2.0 * aa), (-bb + root) / (2.0 * aa)] {
            offer([from[0] + t * d[0], from[1] + t * d[1]]);
        }
    }
}

pub(crate) fn arc_profile_contains(edges: &[ProfileEdge], point: [f64; 2], g: f64) -> bool {
    let ring = edges
        .iter()
        .map(|edge| match edge {
            ProfileEdge::Line { from, to } => CurveEdge2::Line {
                from: *from,
                to: *to,
            },
            ProfileEdge::Arc {
                center,
                radius,
                start_angle,
                sweep_angle,
            } => CurveEdge2::Arc {
                center: *center,
                radius: *radius,
                start_angle: *start_angle,
                sweep_angle: *sweep_angle,
            },
        })
        .collect::<Vec<_>>();
    winding(Pt2::new(point[0], point[1]), &ring, g) != 0
}
