use crate::primitives::ProfileEdge;
use std::f64::consts::{FRAC_PI_2, PI};

type ArcSpec = ([f64; 2], f64, f64, f64);

pub fn slot_outline(length: f64, width: f64) -> Vec<ProfileEdge> {
    let half = length / 2.0;
    let radius = width / 2.0;
    arcs_joined_by_lines(&[
        ([half, 0.0], radius, -FRAC_PI_2, PI),
        ([-half, 0.0], radius, FRAC_PI_2, PI),
    ])
}

pub fn rounded_rectangle_outline(width: f64, depth: f64, radius: f64) -> Vec<ProfileEdge> {
    let u = width / 2.0 - radius;
    let v = depth / 2.0 - radius;
    arcs_joined_by_lines(&[
        ([u, -v], radius, -FRAC_PI_2, FRAC_PI_2),
        ([u, v], radius, 0.0, FRAC_PI_2),
        ([-u, v], radius, FRAC_PI_2, FRAC_PI_2),
        ([-u, -v], radius, PI, FRAC_PI_2),
    ])
}

pub fn ring_sector_outline([outer, inner]: [f64; 2], [start, sweep]: [f64; 2]) -> Vec<ProfileEdge> {
    arcs_joined_by_lines(&[
        ([0.0; 2], outer, start, sweep),
        ([0.0; 2], inner, start + sweep, -sweep),
    ])
}

fn arcs_joined_by_lines(specs: &[ArcSpec]) -> Vec<ProfileEdge> {
    let arcs = specs
        .iter()
        .map(
            |&(center, radius, start_angle, sweep_angle)| ProfileEdge::Arc {
                center,
                radius,
                start_angle,
                sweep_angle,
            },
        )
        .collect::<Vec<_>>();
    let mut edges = Vec::with_capacity(2 * arcs.len());
    for (index, current) in arcs.iter().enumerate() {
        let next = &arcs[(index + 1) % arcs.len()];
        edges.push(current.clone());
        edges.push(ProfileEdge::Line {
            from: current.endpoints().1,
            to: next.endpoints().0,
        });
    }
    edges
}
