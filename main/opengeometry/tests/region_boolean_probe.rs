use opengeometry::geometry::boolean2d::{boolean_oriented_regions, PlanarBooleanOp, RingRegion};
use opengeometry::geometry::poly2d::{signed_area2, Pt2, DEFAULT_EPS};

const GAPS_INSIDE_THE_OLD_REACH: [f64; 3] = [2.0e-6, 1.0e-5, 9.0e-5];

fn rectangle(x0: f64, z0: f64, x1: f64, z1: f64) -> Vec<Pt2> {
    vec![
        Pt2::new(x0, z0),
        Pt2::new(x1, z0),
        Pt2::new(x1, z1),
        Pt2::new(x0, z1),
    ]
}

fn hole(x0: f64, z0: f64, x1: f64, z1: f64) -> Vec<Pt2> {
    let mut ring = rectangle(x0, z0, x1, z1);
    ring.reverse();
    ring
}

fn framed(low: f64, high: f64) -> Vec<Vec<Pt2>> {
    vec![
        rectangle(low, low, high, high),
        hole(low + 1.0, low + 1.0, high - 1.0, high - 1.0),
    ]
}

fn regions(a: &[Vec<Pt2>], b: &[Vec<Pt2>], operation: PlanarBooleanOp) -> Vec<RingRegion> {
    boolean_oriented_regions(a, b, operation, DEFAULT_EPS).unwrap()
}

fn union(a: &[Vec<Pt2>], b: &[Vec<Pt2>]) -> Vec<RingRegion> {
    regions(a, b, PlanarBooleanOp::Union)
}

fn region_area(region: &RingRegion) -> f64 {
    signed_area2(&region.outer).abs()
        - region
            .holes
            .iter()
            .map(|ring| signed_area2(ring).abs())
            .sum::<f64>()
}

fn area(regions: &[RingRegion]) -> f64 {
    regions.iter().map(region_area).sum()
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

type Nest = Vec<((f64, f64), Vec<(f64, f64)>)>;

fn nest(regions: &[RingRegion]) -> Nest {
    let mut spans: Nest = regions
        .iter()
        .map(|region| {
            let mut holes: Vec<_> = region.holes.iter().map(|ring| span(ring)).collect();
            holes.sort_by(|first, second| first.0.total_cmp(&second.0));
            (span(&region.outer), holes)
        })
        .collect();
    spans.sort_by(|first, second| first.0 .0.total_cmp(&second.0 .0));
    spans
}

fn assert_area(regions: &[RingRegion], expected: f64, case: &str) {
    let got = area(regions);
    assert!(
        (got - expected).abs() <= 1.0e-9 * expected.max(1.0e-6),
        "{case}: area {got}, expected {expected}"
    );
}

#[test]
fn two_squares_with_a_small_gap_stay_two_regions_at_every_size_and_distance() {
    for side in [1.0e-2, 1.0, 1.0e3] {
        for far in [0.0, 100.0, 1000.0] {
            for gap in GAPS_INSIDE_THE_OLD_REACH {
                let case = format!("side {side}, at {far}, gap {gap}");
                let a = [rectangle(far, far, far + side, far + side)];
                let b = [rectangle(
                    far + side + gap,
                    far,
                    far + 2.0 * side + gap,
                    far + side,
                )];
                let united = union(&a, &b);
                assert_eq!(united.len(), 2, "{case}");
                assert!(
                    (area(&united) / (2.0 * side * side) - 1.0).abs() < 1.0e-6,
                    "{case}: area {}",
                    area(&united)
                );
                assert!(
                    regions(&a, &b, PlanarBooleanOp::Intersection).is_empty(),
                    "{case}"
                );
                let kept = regions(&a, &b, PlanarBooleanOp::Subtraction);
                assert_eq!(kept.len(), 1, "{case}");
                assert_eq!(kept[0].outer.len(), 4, "{case}");
            }
        }
    }
}

#[test]
fn squares_smaller_than_a_millimetre_are_kept_at_every_gap() {
    for side in [3.0e-4, 1.0e-4] {
        for gap in [2.0e-6, 1.0e-5, 9.0e-5, 1.0e-3, 1.0e-2] {
            let united = union(
                &[rectangle(0.0, 0.0, side, side)],
                &[rectangle(side + gap, 0.0, 2.0 * side + gap, side)],
            );
            assert_eq!(united.len(), 2, "side {side}, gap {gap}");
            assert!(
                (area(&united) / (2.0 * side * side) - 1.0).abs() < 1.0e-6,
                "side {side}, gap {gap}: area {}",
                area(&united)
            );
        }
    }
}

#[test]
fn a_strip_thinner_than_the_old_reach_is_a_region_of_its_own() {
    for width in [3.0e-6, 1.0e-5, 9.0e-5] {
        let strip = [rectangle(0.0, 0.0, 1.0, width)];
        let alone = union(&strip, &[]);
        assert_eq!(alone.len(), 1, "width {width}");
        assert_area(&alone, width, "strip alone");
        let kept = regions(&strip, &[], PlanarBooleanOp::Subtraction);
        assert_eq!(kept.len(), 1, "width {width}");
        assert_area(&kept, width, "strip minus nothing");
    }
}

#[test]
fn a_thin_strip_and_a_square_combine_rightly_in_every_position() {
    let width = 1.0e-5;
    let unit = [rectangle(0.0, 0.0, 1.0, 1.0)];
    let on_the_top_edge = [rectangle(0.0, 1.0, 1.0, 1.0 + width)];
    let across = [rectangle(-0.5, 0.5, 1.5, 0.5 + width)];
    let above = [rectangle(0.0, 1.5, 1.0, 1.5 + width)];
    let inside = [rectangle(0.2, 0.5, 0.8, 0.5 + width)];
    type Expected = (usize, f64);
    let cases: [(&str, &[Vec<Pt2>], Expected, Expected, Expected); 4] = [
        (
            "on the top edge",
            &on_the_top_edge,
            (1, 1.0 + width),
            (0, 0.0),
            (1, 1.0),
        ),
        (
            "across",
            &across,
            (1, 1.0 + width),
            (1, width),
            (2, 1.0 - width),
        ),
        ("above", &above, (2, 1.0 + width), (0, 0.0), (1, 1.0)),
        (
            "inside",
            &inside,
            (1, 1.0),
            (1, 0.6 * width),
            (1, 1.0 - 0.6 * width),
        ),
    ];
    for (name, strip, united, common, cut) in cases {
        for (operation, (count, expected)) in [
            (PlanarBooleanOp::Union, united),
            (PlanarBooleanOp::Intersection, common),
            (PlanarBooleanOp::Subtraction, cut),
        ] {
            let case = format!("strip {name}, {operation:?}");
            let result = regions(&unit, strip, operation);
            assert_eq!(result.len(), count, "{case}");
            if count > 0 {
                assert_area(&result, expected, &case);
            }
        }
    }
    let slot = regions(&unit, &inside, PlanarBooleanOp::Subtraction);
    assert_eq!(slot[0].holes.len(), 1);
}

#[test]
fn no_result_ring_is_an_open_chain_closed_by_a_straight_line() {
    for gap in GAPS_INSIDE_THE_OLD_REACH {
        let united = union(
            &[rectangle(0.0, 0.0, 1.0, 1.0)],
            &[rectangle(1.0 + gap, 0.0, 2.0 + gap, 1.0)],
        );
        for region in &united {
            assert_eq!(region.outer.len(), 4, "gap {gap}");
            assert!(
                (signed_area2(&region.outer) - 1.0).abs() < 1.0e-9,
                "gap {gap}"
            );
        }
    }
}

#[test]
fn a_hole_goes_to_the_innermost_region_that_holds_it_in_both_operand_orders() {
    let frame = framed(0.0, 10.0);
    let island = framed(3.0, 7.0);
    let expected: Nest = vec![
        ((0.0, 10.0), vec![(1.0, 9.0)]),
        ((3.0, 7.0), vec![(4.0, 6.0)]),
    ];
    assert_eq!(nest(&union(&frame, &island)), expected);
    assert_eq!(nest(&union(&island, &frame)), expected);
    let both: Vec<Vec<Pt2>> = frame.iter().chain(&island).cloned().collect();
    assert_eq!(nest(&union(&both, &[])), expected);
    for region in union(&frame, &island) {
        assert!(region_area(&region) > 0.0);
    }
}

#[test]
fn three_nested_rings_keep_their_own_holes_in_every_operation() {
    let outer = framed(0.0, 20.0);
    let middle = framed(3.0, 17.0);
    let inner = framed(6.0, 14.0);
    let core = rectangle(9.0, 9.0, 11.0, 11.0);
    let expected: Nest = vec![
        ((0.0, 20.0), vec![(1.0, 19.0)]),
        ((3.0, 17.0), vec![(4.0, 16.0)]),
        ((6.0, 14.0), vec![(7.0, 13.0)]),
        ((9.0, 11.0), vec![]),
    ];
    let inside: Vec<Vec<Pt2>> = middle
        .iter()
        .chain(&inner)
        .cloned()
        .chain(std::iter::once(core.clone()))
        .collect();
    assert_eq!(nest(&union(&outer, &inside)), expected);
    assert_eq!(nest(&union(&inside, &outer)), expected);
    let all: Vec<Vec<Pt2>> = outer.iter().chain(&inside).cloned().collect();
    let cover = [rectangle(0.0, 0.0, 20.0, 20.0)];
    let kept = regions(&all, &cover, PlanarBooleanOp::Intersection);
    assert_eq!(nest(&kept), expected);
    for region in &kept {
        assert!(region_area(region) > 0.0);
    }
    let cut = regions(&cover, &middle, PlanarBooleanOp::Subtraction);
    let expected_cut: Nest = vec![((0.0, 20.0), vec![(3.0, 17.0)]), ((4.0, 16.0), vec![])];
    assert_eq!(nest(&cut), expected_cut);
}

#[test]
fn an_island_a_hundredth_of_a_millimetre_inside_a_hole_stays_a_region() {
    let gap = 1.0e-5;
    let frame = framed(0.0, 10.0);
    let island = vec![
        rectangle(1.0 + gap, 1.0 + gap, 9.0 - gap, 9.0 - gap),
        hole(4.0, 4.0, 6.0, 6.0),
    ];
    let expected = 36.0 + (8.0 - 2.0 * gap) * (8.0 - 2.0 * gap) - 4.0;
    for united in [union(&frame, &island), union(&island, &frame)] {
        assert_eq!(united.len(), 2);
        assert_area(&united, expected, "island near the hole boundary");
        assert!(united.iter().all(|region| region.holes.len() == 1));
    }
}

#[test]
fn an_island_that_the_operation_clips_keeps_its_hole() {
    let frame = framed(0.0, 10.0);
    let island = framed(3.0, 7.0);
    let nested: Vec<Vec<Pt2>> = frame.iter().chain(&island).cloned().collect();
    let window = [rectangle(-1.0, -1.0, 6.5, 11.0)];
    let clipped = regions(&nested, &window, PlanarBooleanOp::Intersection);
    let expected: Nest = vec![((0.0, 6.5), vec![]), ((3.0, 6.5), vec![(4.0, 6.0)])];
    assert_eq!(nest(&clipped), expected);
    assert_area(&clipped, 21.0 + 10.0, "the clipped nest");
    let notch = [rectangle(6.5, 6.5, 7.5, 7.5)];
    let notched = regions(&nested, &notch, PlanarBooleanOp::Subtraction);
    let expected_notched: Nest = vec![
        ((0.0, 10.0), vec![(1.0, 9.0)]),
        ((3.0, 7.0), vec![(4.0, 6.0)]),
    ];
    assert_eq!(nest(&notched), expected_notched);
    assert_area(&notched, 36.0 + 12.0 - 0.25, "the notched island");
}
