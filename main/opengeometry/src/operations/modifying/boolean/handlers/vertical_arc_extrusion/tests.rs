use super::cap_provenance::cap_regions_overlap;
use crate::geom2d::{reversed_ring, CurveEdge2, CurveRegion2};

fn rectangle(x0: f64, z0: f64, x1: f64, z1: f64) -> CurveRegion2 {
    let points = [[x0, z0], [x0, z1], [x1, z1], [x1, z0]];
    CurveRegion2 {
        outer: (0..4)
            .map(|index| CurveEdge2::Line {
                from: points[index],
                to: points[(index + 1) % 4],
            })
            .collect(),
        holes: Vec::new(),
    }
}

#[test]
fn cap_overlap_requires_positive_shared_area() {
    let host = rectangle(0.0, 0.0, 2.0, 2.0);
    assert!(cap_regions_overlap(
        &host,
        &rectangle(1.0, 1.0, 3.0, 3.0),
        1e-7
    ));
    assert!(cap_regions_overlap(
        &host,
        &rectangle(0.5, 0.5, 1.5, 1.5),
        1e-7
    ));
    assert!(!cap_regions_overlap(
        &host,
        &rectangle(2.0, 0.0, 3.0, 1.0),
        1e-7
    ));
    assert!(!cap_regions_overlap(
        &host,
        &rectangle(3.0, 0.0, 4.0, 1.0),
        1e-7
    ));
}

#[test]
fn cap_overlap_excludes_a_cutter_inside_a_hole() {
    let mut host = rectangle(0.0, 0.0, 3.0, 3.0);
    host.holes
        .push(reversed_ring(&rectangle(1.0, 1.0, 2.0, 2.0).outer));
    assert!(!cap_regions_overlap(
        &host,
        &rectangle(1.2, 1.2, 1.8, 1.8),
        1e-7
    ));
    assert!(cap_regions_overlap(
        &host,
        &rectangle(0.8, 1.2, 1.2, 1.8),
        1e-7
    ));
}
