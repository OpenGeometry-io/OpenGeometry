use opengeometry::geometry::boolean2d::{
    boolean_oriented_regions, regions_from_edges_by, PlanarBooleanOp, RingRegion,
};
use opengeometry::geometry::poly2d::{
    ring_inside2, signed_area2, winding_number2, Edge2, Pt2, DEFAULT_EPS,
};

fn square(x0: f64, z0: f64, x1: f64, z1: f64) -> Vec<Pt2> {
    vec![
        Pt2::new(x0, z0),
        Pt2::new(x1, z0),
        Pt2::new(x1, z1),
        Pt2::new(x0, z1),
    ]
}

fn hole(x0: f64, z0: f64, x1: f64, z1: f64) -> Vec<Pt2> {
    let mut ring = square(x0, z0, x1, z1);
    ring.reverse();
    ring
}

fn area(regions: &[RingRegion]) -> f64 {
    regions
        .iter()
        .map(|region| {
            signed_area2(&region.outer).abs()
                - region
                    .holes
                    .iter()
                    .map(|ring| signed_area2(ring).abs())
                    .sum::<f64>()
        })
        .sum()
}

fn span(ring: &[Pt2]) -> (f64, f64) {
    let low = ring
        .iter()
        .map(|point| point.x)
        .fold(f64::INFINITY, f64::min);
    let high = ring
        .iter()
        .map(|point| point.x)
        .fold(f64::NEG_INFINITY, f64::max);
    (low, high)
}

fn hole_spans(regions: &[RingRegion]) -> Vec<((f64, f64), Vec<(f64, f64)>)> {
    let mut spans: Vec<_> = regions
        .iter()
        .map(|region| {
            (
                span(&region.outer),
                region.holes.iter().map(|ring| span(ring)).collect(),
            )
        })
        .collect();
    spans.sort_by(|first, second| first.0 .0.total_cmp(&second.0 .0));
    spans
}

fn union(a: &[Vec<Pt2>], b: &[Vec<Pt2>]) -> Vec<RingRegion> {
    boolean_oriented_regions(a, b, PlanarBooleanOp::Union, DEFAULT_EPS)
}

#[test]
#[ignore = "defect A: fails until the side of an edge no longer comes from a probe at a fixed reach"]
fn union_keeps_two_squares_whose_gap_is_above_the_tolerance_and_below_the_probe_reach() {
    for gap in [2.0e-6, 1.0e-5, 9.0e-5] {
        let regions = union(
            &[square(0.0, 0.0, 1.0, 1.0)],
            &[square(1.0 + gap, 0.0, 2.0 + gap, 1.0)],
        );
        assert_eq!(regions.len(), 2, "gap {gap}");
        assert!((area(&regions) - 2.0).abs() < 1.0e-9, "gap {gap}");
    }
}

#[test]
#[ignore = "defect A: fails until the side of an edge no longer comes from a probe at a fixed reach"]
fn union_keeps_a_strip_that_is_thinner_than_the_probe_reach() {
    for width in [3.0e-6, 1.0e-5, 9.0e-5] {
        let alone = union(&[square(0.0, 0.0, 1.0, width)], &[]);
        assert_eq!(alone.len(), 1, "width {width}");
        assert!((area(&alone) - width).abs() < 1.0e-12, "width {width}");
    }
}

#[test]
#[ignore = "defect A: fails until the side of an edge no longer comes from a probe at a fixed reach"]
fn a_thin_strip_across_a_square_does_not_erase_the_square() {
    for width in [3.0e-6, 1.0e-5, 9.0e-5] {
        let unit = [square(0.0, 0.0, 1.0, 1.0)];
        let strip = [square(-0.5, 0.5, 1.5, 0.5 + width)];
        let united = union(&unit, &strip);
        assert_eq!(united.len(), 1, "width {width}");
        assert!(
            (area(&united) - (1.0 + width)).abs() < 1.0e-9,
            "width {width}"
        );
        let cut =
            boolean_oriented_regions(&unit, &strip, PlanarBooleanOp::Subtraction, DEFAULT_EPS);
        assert_eq!(cut.len(), 2, "width {width}");
        assert!((area(&cut) - (1.0 - width)).abs() < 1.0e-9, "width {width}");
    }
}

#[test]
#[ignore = "defect A: fails until an open chain of boundary edges is no longer returned as a ring"]
fn a_result_is_never_an_open_chain_closed_by_a_straight_line() {
    let regions = union(
        &[square(0.0, 0.0, 1.0, 1.0)],
        &[square(1.0 + 1.0e-5, 0.0, 2.0 + 1.0e-5, 1.0)],
    );
    for region in &regions {
        assert!(
            region.outer.len() >= 4,
            "a ring of {} points with area {}",
            region.outer.len(),
            signed_area2(&region.outer)
        );
    }
}

#[test]
#[ignore = "defect B: fails until a hole goes to the innermost region that holds it"]
fn union_gives_each_hole_to_the_innermost_region_that_holds_it() {
    let frame = [square(0.0, 0.0, 10.0, 10.0), hole(1.0, 1.0, 9.0, 9.0)];
    let island = [square(3.0, 3.0, 7.0, 7.0), hole(4.0, 4.0, 6.0, 6.0)];
    let expected = vec![
        ((0.0, 10.0), vec![(1.0, 9.0)]),
        ((3.0, 7.0), vec![(4.0, 6.0)]),
    ];
    assert_eq!(hole_spans(&union(&frame, &island)), expected);
    assert_eq!(hole_spans(&union(&island, &frame)), expected);
}

#[test]
#[ignore = "defect B: fails until a hole goes to the innermost region that holds it"]
fn three_nested_rings_keep_their_own_holes_in_every_operation() {
    let level = |low: f64, high: f64| {
        vec![
            square(low, low, high, high),
            hole(low + 1.0, low + 1.0, high - 1.0, high - 1.0),
        ]
    };
    let outer = level(0.0, 20.0);
    let inner: Vec<Vec<Pt2>> = level(3.0, 17.0)
        .into_iter()
        .chain(level(6.0, 14.0))
        .chain(std::iter::once(square(9.0, 9.0, 11.0, 11.0)))
        .collect();
    let expected = vec![
        ((0.0, 20.0), vec![(1.0, 19.0)]),
        ((3.0, 17.0), vec![(4.0, 16.0)]),
        ((6.0, 14.0), vec![(7.0, 13.0)]),
        ((9.0, 11.0), vec![]),
    ];
    assert_eq!(hole_spans(&union(&outer, &inner)), expected);
    let all: Vec<Vec<Pt2>> = outer.iter().chain(&inner).cloned().collect();
    let cover = [square(0.0, 0.0, 20.0, 20.0)];
    let kept = boolean_oriented_regions(&all, &cover, PlanarBooleanOp::Intersection, DEFAULT_EPS);
    assert_eq!(hole_spans(&kept), expected);
    for region in &kept {
        assert!(area(std::slice::from_ref(region)) > 0.0);
    }
}

#[test]
#[ignore = "probe: prints how each edge of two squares 1e-5 apart is classed"]
fn probe_defect_a_the_two_facing_edges_are_classed_as_interior() {
    let gap = 1.0e-5;
    let a = square(0.0, 0.0, 1.0, 1.0);
    let b = square(1.0 + gap, 0.0, 2.0 + gap, 1.0);
    let edges: Vec<Edge2> = [&a, &b]
        .into_iter()
        .flat_map(|ring| (0..4).map(move |index| (ring[index], ring[(index + 1) % 4])))
        .collect();
    let calls = std::cell::RefCell::new(Vec::new());
    let regions = regions_from_edges_by(&edges, DEFAULT_EPS, |point| {
        let filled = winding_number2(point, &a) != 0 || winding_number2(point, &b) != 0;
        calls.borrow_mut().push((point, filled));
        filled
    });
    for (edge, pair) in edges.iter().zip(calls.borrow().chunks(2)) {
        println!(
            "edge ({}, {}) -> ({}, {}): left probe ({:.6}, {:.6}) filled {}, right probe ({:.6}, {:.6}) filled {} => {}",
            edge.0.x, edge.0.z, edge.1.x, edge.1.z,
            pair[0].0.x, pair[0].0.z, pair[0].1,
            pair[1].0.x, pair[1].0.z, pair[1].1,
            if pair[0].1 == pair[1].1 { "DROPPED as interior or exterior" } else { "kept as boundary" }
        );
    }
    for region in &regions {
        println!(
            "result ring: {:?}, area {}",
            region
                .outer
                .iter()
                .map(|point| (point.x, point.z))
                .collect::<Vec<_>>(),
            signed_area2(&region.outer)
        );
    }
}

#[test]
#[ignore = "probe: prints which outer rings hold each hole and what the union returns in both operand orders"]
fn probe_defect_b_the_first_outer_ring_that_holds_a_hole_takes_it() {
    let frame = [square(0.0, 0.0, 10.0, 10.0), hole(1.0, 1.0, 9.0, 9.0)];
    let island = [square(3.0, 3.0, 7.0, 7.0), hole(4.0, 4.0, 6.0, 6.0)];
    for (name, ring) in [
        ("frame hole 1..9", &frame[1]),
        ("island hole 4..6", &island[1]),
    ] {
        println!(
            "{name}: inside the frame outer {}, inside the island outer {}",
            ring_inside2(ring, &frame[0], DEFAULT_EPS),
            ring_inside2(ring, &island[0], DEFAULT_EPS)
        );
    }
    for (name, regions) in [
        ("union(frame, island)", union(&frame, &island)),
        ("union(island, frame)", union(&island, &frame)),
    ] {
        println!(
            "{name}: regions in output order {:?}",
            regions
                .iter()
                .map(|region| (
                    span(&region.outer),
                    region
                        .holes
                        .iter()
                        .map(|ring| span(ring))
                        .collect::<Vec<_>>()
                ))
                .collect::<Vec<_>>()
        );
    }
}

#[test]
#[ignore = "probe: prints the map of the union of two unit squares by gap"]
fn probe_map_of_the_union_of_two_squares_by_gap() {
    let mut gap = 1.0e-9;
    while gap < 2.0e-2 {
        let regions = union(
            &[square(0.0, 0.0, 1.0, 1.0)],
            &[square(1.0 + gap, 0.0, 2.0 + gap, 1.0)],
        );
        println!(
            "gap {gap:.2e}: {} regions, points {:?}, area {}",
            regions.len(),
            regions
                .iter()
                .map(|region| region.outer.len())
                .collect::<Vec<_>>(),
            area(&regions)
        );
        gap *= 3.0;
    }
}
