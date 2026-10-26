mod ring;

use crate::geom2d::boolean2d::distance_to_segment;
use crate::geom2d::curved_boolean2d::{CurveEdge2, CurveRegion2};
use crate::geom2d::offset::{offset_ring_variable, OffsetError, OffsetHole};
use crate::geom2d::poly2d::Pt2;
use crate::geom2d::tests::DEFAULT_EPS;

pub(super) fn variable(ring: &[Pt2], distances: &[f64]) -> Result<Vec<CurveRegion2>, OffsetError> {
    offset_ring_variable(ring, distances, &[], DEFAULT_EPS)
}

pub(super) fn with_hole(
    ring: &[Pt2],
    distances: &[f64],
    hole: Vec<Pt2>,
    hole_distances: Vec<f64>,
) -> Result<Vec<CurveRegion2>, OffsetError> {
    let hole = OffsetHole {
        ring: hole,
        distances: hole_distances,
    };
    offset_ring_variable(ring, distances, &[hole], DEFAULT_EPS)
}

pub(super) fn single(regions: Vec<CurveRegion2>) -> CurveRegion2 {
    assert_eq!(regions.len(), 1, "one region");
    regions.into_iter().next().unwrap()
}

pub(super) fn corners(ring: &[CurveEdge2]) -> Vec<Pt2> {
    ring.iter().map(|edge| edge.point(0.0)).collect()
}

pub(super) fn has_corner(ring: &[CurveEdge2], x: f64, z: f64) -> bool {
    corners(ring)
        .iter()
        .any(|point| (point.x - x).abs() < 1e-9 && (point.z - z).abs() < 1e-9)
}

pub(super) fn arcs(ring: &[CurveEdge2]) -> Vec<([f64; 2], f64)> {
    ring.iter()
        .filter_map(|edge| match edge {
            CurveEdge2::Arc { center, radius, .. } => Some((*center, *radius)),
            CurveEdge2::Line { .. } => None,
        })
        .collect()
}

fn samples(edge: &CurveEdge2) -> [Pt2; 3] {
    [edge.point(0.0), edge.point(0.5), edge.point(1.0)]
}

fn rings(regions: &[CurveRegion2]) -> Vec<&Vec<CurveEdge2>> {
    regions
        .iter()
        .flat_map(|region| std::iter::once(&region.outer).chain(&region.holes))
        .collect()
}

fn source_edges<'a>(sources: &'a [(&'a [Pt2], &'a [f64])]) -> Vec<(Pt2, Pt2, f64)> {
    sources
        .iter()
        .flat_map(|(ring, distances)| {
            (0..ring.len()).map(|index| {
                (
                    ring[index],
                    ring[(index + 1) % ring.len()],
                    distances[index],
                )
            })
        })
        .collect()
}

fn near(value: f64, target: f64) -> bool {
    (value - target).abs() <= 1e-9 * target.max(1.0)
}

pub(super) fn assert_clearance(regions: &[CurveRegion2], sources: &[(&[Pt2], &[f64])]) {
    for edge in rings(regions).into_iter().flatten() {
        for point in samples(edge) {
            for (a, b, distance) in source_edges(sources) {
                let clearance = distance_to_segment(point, a, b);
                assert!(
                    clearance >= distance - DEFAULT_EPS,
                    "({}, {}) is {clearance} from an edge that needs {distance}",
                    point.x,
                    point.z
                );
            }
        }
    }
}

pub(super) fn assert_exact(regions: &[CurveRegion2], sources: &[(&[Pt2], &[f64])]) {
    let edges = source_edges(sources);
    for edge in rings(regions).into_iter().flatten() {
        let exact = match edge {
            CurveEdge2::Line { .. } => edges.iter().any(|&(a, b, distance)| {
                samples(edge)
                    .iter()
                    .all(|point| near(distance_to_segment(*point, a, b), distance))
            }),
            CurveEdge2::Arc { center, radius, .. } => edges.iter().any(|&(a, b, distance)| {
                let centre = Pt2::new(center[0], center[1]);
                let on_corner = [a, b]
                    .iter()
                    .any(|corner| near(corner.x, centre.x) && near(corner.z, centre.z));
                let span = (b.x - a.x).hypot(b.z - a.z);
                let on_point_edge = span <= 4.0 * DEFAULT_EPS
                    && distance_to_segment(centre, a, b) <= 1e-9
                    && near(*radius, distance + span * 0.5);
                (on_corner && near(*radius, distance)) || on_point_edge
            }),
        };
        assert!(exact, "{edge:?} is not at its source distance");
    }
}
