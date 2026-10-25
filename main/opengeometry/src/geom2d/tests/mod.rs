mod boolean2d;
mod region_probe;

use super::boolean2d::{
    boolean_oriented_regions, regions_from_edges_by, PlanarBooleanOp, RingRegion,
};
use super::poly2d::{signed_area2, winding_number2, Edge2, Pt2};

pub(super) const DEFAULT_EPS: f64 = 1.0e-6;

pub(super) fn p(x: f64, z: f64) -> Pt2 {
    Pt2::new(x, z)
}

pub(super) fn rectangle(x0: f64, z0: f64, x1: f64, z1: f64) -> Vec<Pt2> {
    vec![p(x0, z0), p(x1, z0), p(x1, z1), p(x0, z1)]
}

pub(super) fn hole(x0: f64, z0: f64, x1: f64, z1: f64) -> Vec<Pt2> {
    let mut ring = rectangle(x0, z0, x1, z1);
    ring.reverse();
    ring
}

pub(super) fn edges_of(ring: &[Pt2]) -> Vec<Edge2> {
    (0..ring.len())
        .map(|index| (ring[index], ring[(index + 1) % ring.len()]))
        .collect()
}

pub(super) fn regions(
    a: &[Vec<Pt2>],
    b: &[Vec<Pt2>],
    operation: PlanarBooleanOp,
) -> Vec<RingRegion> {
    boolean_oriented_regions(a, b, operation, DEFAULT_EPS).unwrap()
}

pub(super) fn union(a: &[Vec<Pt2>], b: &[Vec<Pt2>]) -> Vec<RingRegion> {
    regions(a, b, PlanarBooleanOp::Union)
}

pub(super) fn winding_regions(polys: &[Vec<Pt2>], filled: impl Fn(i32) -> bool) -> Vec<RingRegion> {
    let edges: Vec<Edge2> = polys
        .iter()
        .flat_map(|poly| edges_of(poly))
        .filter(|(a, b)| (a.x - b.x).hypot(a.z - b.z) > DEFAULT_EPS)
        .collect();
    regions_from_edges_by(&edges, DEFAULT_EPS, |point| {
        filled(polys.iter().map(|poly| winding_number2(point, poly)).sum())
    })
    .unwrap()
}

pub(super) fn region_area(region: &RingRegion) -> f64 {
    signed_area2(&region.outer).abs()
        - region
            .holes
            .iter()
            .map(|ring| signed_area2(ring).abs())
            .sum::<f64>()
}

pub(super) fn area(regions: &[RingRegion]) -> f64 {
    regions.iter().map(region_area).sum()
}
