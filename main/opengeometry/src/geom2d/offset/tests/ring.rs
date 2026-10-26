use super::{
    arcs, assert_clearance, assert_exact, corners, has_corner, single, variable, with_hole,
};
use crate::geom2d::boolean2d::distance_to_segment;
use crate::geom2d::curved_boolean2d::winding;
use crate::geom2d::offset::OffsetError;
use crate::geom2d::poly2d::Pt2;
use crate::geom2d::tests::{curve_area, curve_ring_area, p, DEFAULT_EPS};
use std::f64::consts::FRAC_PI_4;

fn rectangle_ring() -> Vec<Pt2> {
    vec![p(0.0, 0.0), p(20.0, 0.0), p(20.0, 30.0), p(0.0, 30.0)]
}

fn notched() -> Vec<Pt2> {
    vec![
        p(0.0, 0.0),
        p(40.0, 0.0),
        p(40.0, 30.0),
        p(22.0, 30.0),
        p(20.0, 12.0),
        p(18.0, 30.0),
        p(0.0, 30.0),
    ]
}

fn l_shape() -> Vec<Pt2> {
    vec![
        p(0.0, 0.0),
        p(20.0, 0.0),
        p(20.0, 10.0),
        p(10.0, 10.0),
        p(10.0, 20.0),
        p(0.0, 20.0),
    ]
}

#[test]
fn a_uniform_inset_of_a_square_is_the_shrunken_square() {
    let square = vec![p(0.0, 0.0), p(20.0, 0.0), p(20.0, 20.0), p(0.0, 20.0)];
    let region = single(variable(&square, &[2.0; 4]).unwrap());
    assert!(region.holes.is_empty());
    assert_eq!(region.outer.len(), 4);
    assert!(arcs(&region.outer).is_empty());
    for (x, z) in [(2.0, 2.0), (18.0, 2.0), (18.0, 18.0), (2.0, 18.0)] {
        assert!(has_corner(&region.outer, x, z), "missing corner ({x}, {z})");
    }
}

#[test]
fn each_edge_lands_at_its_own_distance() {
    let region = single(variable(&rectangle_ring(), &[6.0, 1.5, 3.0, 1.5]).unwrap());
    assert_eq!(region.outer.len(), 4);
    for (x, z) in [(1.5, 6.0), (18.5, 6.0), (18.5, 27.0), (1.5, 27.0)] {
        assert!(has_corner(&region.outer, x, z), "missing corner ({x}, {z})");
    }
    assert!((curve_area(std::slice::from_ref(&region)) - 17.0 * 21.0).abs() < 1e-9);
}

#[test]
fn a_zero_distance_edge_stays_in_place() {
    let region = single(variable(&rectangle_ring(), &[0.0, 1.5, 3.0, 1.5]).unwrap());
    assert!(has_corner(&region.outer, 1.5, 0.0));
    assert!(has_corner(&region.outer, 18.5, 0.0));
}

#[test]
fn all_zero_distances_return_the_ring_itself() {
    let region = single(variable(&rectangle_ring(), &[0.0; 4]).unwrap());
    assert!(region.holes.is_empty());
    assert_eq!(region.outer.len(), 4);
    for (x, z) in [(0.0, 0.0), (20.0, 0.0), (20.0, 30.0), (0.0, 30.0)] {
        assert!(has_corner(&region.outer, x, z));
    }
}

#[test]
fn a_distance_count_that_is_not_one_per_edge_is_invalid_parameter() {
    let Err(OffsetError::InvalidParameter(message)) = variable(&rectangle_ring(), &[6.0, 1.5, 3.0])
    else {
        panic!("a count mismatch must be InvalidParameter");
    };
    assert!(
        message.contains("4 edges") && message.contains("3 distances"),
        "{message}"
    );
    assert!(matches!(
        variable(&rectangle_ring(), &[6.0, 1.5, 3.0, 1.5, 2.0]),
        Err(OffsetError::InvalidParameter(_))
    ));
}

#[test]
fn a_negative_or_non_finite_distance_is_invalid_parameter() {
    for bad in [-1.5, f64::NAN, f64::INFINITY] {
        let Err(OffsetError::InvalidParameter(message)) =
            variable(&rectangle_ring(), &[6.0, bad, 3.0, 1.5])
        else {
            panic!("distance {bad} must be InvalidParameter");
        };
        assert!(message.contains("distances[1]"), "{message}");
    }
}

#[test]
fn a_closing_duplicate_point_takes_the_same_distances_and_gives_the_same_result() {
    let mut closed = rectangle_ring();
    closed.push(closed[0]);
    let distances = [6.0, 1.5, 3.0, 1.5];
    let open = variable(&rectangle_ring(), &distances).unwrap();
    let again = variable(&closed, &distances).unwrap();
    assert_eq!(
        serde_json::to_value(&open).unwrap(),
        serde_json::to_value(&again).unwrap()
    );
}

#[test]
fn a_clockwise_ring_with_remapped_distances_matches_the_counter_clockwise_one() {
    let ccw = rectangle_ring();
    let ccw_distances = [6.0, 1.5, 3.0, 1.5];
    let cw: Vec<Pt2> = ccw.iter().rev().copied().collect();
    let count = ccw.len();
    let cw_distances: Vec<f64> = (0..count)
        .map(|index| ccw_distances[(2 * count - 2 - index) % count])
        .collect();
    assert_eq!(
        serde_json::to_value(variable(&ccw, &ccw_distances).unwrap()).unwrap(),
        serde_json::to_value(variable(&cw, &cw_distances).unwrap()).unwrap()
    );
}

#[test]
fn a_split_edge_steps_between_its_distances_along_the_deeper_edges_exact_arc() {
    let ring = vec![
        p(0.0, 0.0),
        p(10.0, 0.0),
        p(20.0, 0.0),
        p(20.0, 30.0),
        p(0.0, 30.0),
    ];
    let distances = [2.0, 5.0, 1.5, 3.0, 1.5];
    let region = single(variable(&ring, &distances).unwrap());
    assert!(has_corner(&region.outer, 10.0, 5.0));
    assert!(has_corner(&region.outer, 10.0 - 21.0_f64.sqrt(), 2.0));
    assert!(arcs(&region.outer).contains(&([10.0, 0.0], 5.0)));
    for point in corners(&region.outer) {
        assert!(distance_to_segment(point, p(10.0, 0.0), p(20.0, 0.0)) >= 5.0 - 1e-9);
    }
    assert_exact(&[region], &[(&ring, &distances)]);
}

#[test]
fn every_output_point_keeps_its_clearance_from_every_edge() {
    for (ring, distances) in [
        (rectangle_ring(), vec![6.0, 1.5, 3.0, 1.5]),
        (notched(), vec![3.0, 2.0, 1.0, 2.5, 2.5, 1.0, 4.0]),
    ] {
        let regions = variable(&ring, &distances).unwrap();
        assert!(!regions.is_empty());
        assert_clearance(&regions, &[(&ring, &distances)]);
    }
}

#[test]
fn every_output_line_and_arc_lies_at_exactly_its_source_distance() {
    for (ring, distances) in [
        (rectangle_ring(), vec![6.0, 1.5, 3.0, 1.5]),
        (notched(), vec![3.0, 2.0, 1.0, 2.5, 2.5, 1.0, 4.0]),
        (l_shape(), vec![1.0; 6]),
    ] {
        let regions = variable(&ring, &distances).unwrap();
        assert!(!regions.is_empty());
        assert_exact(&regions, &[(&ring, &distances)]);
    }
}

#[test]
fn a_reflex_corner_becomes_an_exact_quarter_circle_and_the_area_is_analytic() {
    let region = single(variable(&l_shape(), &[1.0; 6]).unwrap());
    assert_eq!(arcs(&region.outer), vec![([10.0, 10.0], 1.0)]);
    assert!(has_corner(&region.outer, 10.0, 9.0) || has_corner(&region.outer, 9.0, 10.0));
    let expected = 18.0 * 8.0 + 8.0 * 10.0 + 1.0 - FRAC_PI_4;
    assert!((curve_area(std::slice::from_ref(&region)) - expected).abs() < 1e-9);
}

#[test]
fn an_edge_across_a_slot_keeps_its_clearance_on_the_far_side() {
    let ring = vec![
        p(0.0, 0.0),
        p(30.0, 0.0),
        p(30.0, 20.0),
        p(12.0, 20.0),
        p(12.0, 8.0),
        p(10.0, 8.0),
        p(10.0, 20.0),
        p(0.0, 20.0),
    ];
    let distances = [1.0, 1.0, 1.0, 1.0, 1.0, 5.0, 1.0, 1.0];
    let regions = variable(&ring, &distances).unwrap();
    assert!(!regions.is_empty());
    assert_clearance(&regions, &[(&ring, &distances)]);
    assert!(regions
        .iter()
        .all(|region| winding(p(13.0, 14.0), &region.outer, DEFAULT_EPS) == 0));
}

#[test]
fn distances_that_consume_the_ring_give_no_region() {
    let square = vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)];
    assert!(variable(&square, &[3.0; 4]).unwrap().is_empty());
}

#[test]
fn a_waist_narrower_than_the_inset_splits_the_ring_in_two() {
    let dumbbell = vec![
        p(0.0, 0.0),
        p(20.0, 0.0),
        p(20.0, 14.0),
        p(12.0, 14.0),
        p(12.0, 26.0),
        p(20.0, 26.0),
        p(20.0, 40.0),
        p(0.0, 40.0),
        p(0.0, 26.0),
        p(8.0, 26.0),
        p(8.0, 14.0),
        p(0.0, 14.0),
    ];
    let regions = variable(&dumbbell, &[3.0; 12]).unwrap();
    assert_eq!(regions.len(), 2);
    for region in &regions {
        assert!(-curve_ring_area(&region.outer) > 1.0);
    }
    assert_clearance(&regions, &[(&dumbbell, &[3.0; 12])]);
}

#[test]
fn a_degenerate_outer_ring_gives_no_region() {
    let two = vec![p(0.0, 0.0), p(10.0, 0.0)];
    assert!(variable(&two, &[1.0, 1.0]).unwrap().is_empty());
    let collinear = vec![p(0.0, 0.0), p(10.0, 0.0), p(20.0, 0.0)];
    assert!(variable(&collinear, &[1.0; 3]).unwrap().is_empty());
}

#[test]
fn a_self_crossing_outer_ring_is_invalid_topology() {
    let bowtie = vec![p(0.0, 0.0), p(10.0, 10.0), p(10.0, 0.0), p(0.0, 10.0)];
    assert!(matches!(
        variable(&bowtie, &[1.0; 4]),
        Err(OffsetError::InvalidTopology(_))
    ));
}

#[test]
fn the_same_input_gives_the_same_output() {
    let distances = [3.0, 2.0, 1.0, 2.5, 2.5, 1.0, 4.0];
    let baseline = serde_json::to_value(variable(&notched(), &distances).unwrap()).unwrap();
    for _ in 0..4 {
        assert_eq!(
            serde_json::to_value(variable(&notched(), &distances).unwrap()).unwrap(),
            baseline
        );
    }
}

#[test]
fn a_hole_grows_by_its_own_distances_with_exact_rounded_corners() {
    let square = vec![p(0.0, 0.0), p(20.0, 0.0), p(20.0, 20.0), p(0.0, 20.0)];
    let hole = vec![p(8.0, 8.0), p(12.0, 8.0), p(12.0, 12.0), p(8.0, 12.0)];
    let region = single(with_hole(&square, &[1.0; 4], hole.clone(), vec![1.0; 4]).unwrap());
    for (x, z) in [(1.0, 1.0), (19.0, 1.0), (19.0, 19.0), (1.0, 19.0)] {
        assert!(has_corner(&region.outer, x, z), "outer corner ({x}, {z})");
    }
    assert_eq!(region.holes.len(), 1);
    let mut centres: Vec<[f64; 2]> = arcs(&region.holes[0]).iter().map(|arc| arc.0).collect();
    centres.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    assert_eq!(
        centres,
        vec![[8.0, 8.0], [8.0, 12.0], [12.0, 8.0], [12.0, 12.0]]
    );
    assert!(arcs(&region.holes[0]).iter().all(|arc| arc.1 == 1.0));
    let expected = 18.0 * 18.0 - (16.0 + 4.0 * 4.0 + std::f64::consts::PI);
    assert!((curve_area(std::slice::from_ref(&region)) - expected).abs() < 1e-9);
    assert_exact(&[region], &[(&square, &[1.0; 4]), (&hole, &[1.0; 4])]);
}

#[test]
fn a_hole_whose_clearance_reaches_both_sides_splits_the_ring() {
    let ring = vec![p(0.0, 0.0), p(20.0, 0.0), p(20.0, 10.0), p(0.0, 10.0)];
    let hole = vec![p(9.0, 2.0), p(11.0, 2.0), p(11.0, 8.0), p(9.0, 8.0)];
    let regions = with_hole(&ring, &[0.0; 4], hole, vec![3.0; 4]).unwrap();
    assert_eq!(regions.len(), 2);
    let sides: Vec<bool> = regions
        .iter()
        .map(|region| {
            corners(&region.outer)
                .iter()
                .all(|point| point.x <= 9.0 + 1e-9)
        })
        .collect();
    assert!(sides.contains(&true) && sides.contains(&false));
    assert!(regions.iter().all(|region| region.holes.is_empty()));
}

#[test]
fn a_degenerate_hole_is_invalid_topology() {
    let square = vec![p(0.0, 0.0), p(20.0, 0.0), p(20.0, 20.0), p(0.0, 20.0)];
    let hole = vec![p(5.0, 5.0), p(6.0, 5.0), p(7.0, 5.0)];
    assert!(matches!(
        with_hole(&square, &[1.0; 4], hole, vec![1.0; 3]),
        Err(OffsetError::InvalidTopology(_))
    ));
}

fn regular(sides: usize, radius: f64) -> Vec<Pt2> {
    (0..sides)
        .map(|index| {
            let angle = std::f64::consts::TAU * index as f64 / sides as f64;
            p(radius * angle.cos(), radius * angle.sin())
        })
        .collect()
}

#[test]
fn a_uniform_setback_of_regular_and_round_lots_is_exact() {
    for sides in [3, 5, 6, 8, 12, 48, 96] {
        let lot = regular(sides, 20.0);
        let distances = vec![3.0; sides];
        let region = single(
            variable(&lot, &distances).unwrap_or_else(|error| panic!("{sides} sides: {error:?}")),
        );
        assert_exact(std::slice::from_ref(&region), &[(&lot, &distances)]);
        assert_clearance(std::slice::from_ref(&region), &[(&lot, &distances)]);
    }
}

#[test]
fn a_parcel_with_setbacks_on_some_edges_and_a_sub_millimetre_setback_is_exact() {
    let parcel = vec![
        p(0.0, 0.0),
        p(32.5, 0.0),
        p(32.5, 0.0004),
        p(30.0, 18.0),
        p(2.0, 21.0),
        p(0.0, 12.0),
    ];
    for distances in [
        vec![6.0, 0.0, 1.5, 3.0, 0.0, 1.5],
        vec![6.0, 0.0, 1.5, 3.0, 0.0004, 1.5],
        vec![0.0005; 6],
    ] {
        let regions = variable(&parcel, &distances)
            .unwrap_or_else(|error| panic!("{distances:?}: {error:?}"));
        assert!(!regions.is_empty());
        assert_exact(&regions, &[(&parcel, &distances)]);
    }
}

fn zigzag(amplitude: f64, pitch: f64, count: usize) -> Vec<Pt2> {
    let mut ring: Vec<Pt2> = (0..=count)
        .map(|index| p(index as f64 * pitch, amplitude * (index % 2) as f64))
        .collect();
    ring.push(p(count as f64 * pitch, 10.0));
    ring.push(p(0.0, 10.0));
    ring
}

#[test]
fn a_near_flat_convex_vertex_or_a_surveyed_zigzag_insets_exactly() {
    let kink = vec![
        p(0.0, 0.0),
        p(10.0, -1e-4),
        p(20.0, 0.0),
        p(20.0, 10.0),
        p(0.0, 10.0),
    ];
    for ring in [
        kink,
        zigzag(1e-3, 2.0, 10),
        zigzag(3e-5, 2.0, 10),
        zigzag(3e-6, 5.0, 4),
    ] {
        let distances = vec![1.0; ring.len()];
        let regions = variable(&ring, &distances).unwrap_or_else(|error| panic!("{error:?}"));
        assert_eq!(regions.len(), 1);
        assert_exact(&regions, &[(&ring, &distances)]);
        assert_clearance(&regions, &[(&ring, &distances)]);
    }
}

#[test]
fn a_micro_spike_in_the_ring_still_insets() {
    let spike = vec![
        p(0.0, 0.0),
        p(5e-6, 0.0),
        p(2.5e-6, 2e-6),
        p(0.0, 0.0),
        p(0.0, -0.0),
        p(10.0, 10.0),
        p(0.0, 10.0),
    ];
    let regions = variable(&spike, &[1.0; 7]).unwrap_or_else(|error| panic!("{error:?}"));
    assert_eq!(regions.len(), 1);
}

#[test]
fn an_edge_shorter_than_the_tolerance_still_keeps_its_clearance() {
    let ring = vec![
        p(0.0, 0.0),
        p(20.0, 0.0),
        p(20.0, 10.0),
        p(20.0, 10.000003),
        p(0.0, 10.0),
    ];
    let distances = [1.0, 1.0, 6.0, 1.0, 1.0];
    let regions = variable(&ring, &distances).unwrap();
    assert!(!regions.is_empty());
    assert_clearance(&regions, &[(&ring, &distances)]);
    assert!(regions
        .iter()
        .all(|region| winding(p(16.0, 8.0), &region.outer, DEFAULT_EPS) == 0));
}

#[test]
fn a_uniform_setback_of_a_regular_lot_has_the_analytic_area() {
    for sides in [3, 5, 6, 8, 12, 48, 96] {
        let distances = vec![3.0; sides];
        let regions = variable(&regular(sides, 20.0), &distances).unwrap();
        let half = std::f64::consts::PI / sides as f64;
        let apothem = 20.0 * half.cos() - 3.0;
        let expected = sides as f64 * apothem * apothem * half.tan();
        assert!(
            (curve_area(&regions) - expected).abs() < 1e-9 * expected,
            "{sides} sides: {} against {expected}",
            curve_area(&regions)
        );
    }
}

#[test]
fn a_tongue_tip_facing_a_wall_keeps_its_clearance_across_the_gap() {
    let ring = vec![
        p(0.0, 0.0),
        p(20.0, 0.0),
        p(20.0, 20.0),
        p(12.0, 20.0),
        p(12.0, 5.0),
        p(8.0, 5.0),
        p(8.0, 8.0),
        p(11.5, 8.0),
        p(11.5, 12.0),
        p(8.0, 12.0),
        p(8.0, 20.0),
        p(0.0, 20.0),
    ];
    let distances = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 3.0, 3.0, 3.0, 0.0, 0.0, 0.0];
    let regions = variable(&ring, &distances).unwrap();
    assert!(!regions.is_empty());
    assert_clearance(&regions, &[(&ring, &distances)]);
    assert!(regions
        .iter()
        .all(|region| winding(p(12.5, 13.0), &region.outer, DEFAULT_EPS) == 0));
}

#[test]
fn a_near_flat_zigzag_finer_than_its_setback_insets_exactly() {
    for amplitude in [3e-4, 1e-4, 3e-5, 1e-5, 3e-6] {
        let ring = zigzag(amplitude, 0.5, 40);
        let distances = vec![1.0; ring.len()];
        let regions =
            variable(&ring, &distances).unwrap_or_else(|error| panic!("{amplitude}: {error:?}"));
        assert_eq!(regions.len(), 1, "amplitude {amplitude}");
        assert_exact(&regions, &[(&ring, &distances)]);
        assert_clearance(&regions, &[(&ring, &distances)]);
    }
}

#[test]
fn an_edge_shorter_than_the_tolerance_clears_by_a_disk_and_leaves_its_neighbour_alone() {
    let ring = vec![
        p(0.0, 0.0),
        p(20.0, 0.0),
        p(20.0, 10.0),
        p(20.0, 10.000003),
        p(0.0, 10.0),
    ];
    let distances = [1.0, 1.0, 6.0, 0.0, 1.0];
    let regions = variable(&ring, &distances).unwrap();
    assert_clearance(&regions, &[(&ring, &distances)]);
    let kept = vec![p(0.0, 0.0), p(20.0, 0.0), p(20.0, 10.0), p(0.0, 10.0)];
    assert_exact(
        &regions,
        &[(&ring, &distances), (&kept, &[1.0, 1.0, 0.0, 1.0])],
    );
    assert!(regions
        .iter()
        .any(|region| winding(p(10.0, 9.5), &region.outer, DEFAULT_EPS) != 0));
    assert!(regions
        .iter()
        .all(|region| winding(p(16.0, 8.0), &region.outer, DEFAULT_EPS) == 0));
}

fn assert_single_exact_inset(ring: &[Pt2], distance: f64) -> f64 {
    let distances = vec![distance; ring.len()];
    let regions = variable(ring, &distances).unwrap_or_else(|error| panic!("{error:?}"));
    assert_eq!(regions.len(), 1);
    assert_exact(&regions, &[(ring, &distances)]);
    assert_clearance(&regions, &[(ring, &distances)]);
    curve_area(&regions)
}

#[test]
fn a_near_flat_zigzag_between_half_and_one_tolerance_of_turn_insets_exactly() {
    assert_single_exact_inset(&zigzag(4e-4, 0.5, 40), 1.0);
    assert_single_exact_inset(&zigzag(3e-4, 0.37, 50), 1.0);
}

#[test]
fn a_zigzag_whose_tangent_points_are_about_one_tolerance_apart_insets_exactly() {
    let area = assert_single_exact_inset(&zigzag(1e-6, 1.7, 20), 2.0);
    assert!((area - 180.0).abs() < 1e-4, "area {area}");
    let theta: f64 = 5.32e-7;
    let area = assert_single_exact_inset(&zigzag(theta * 1.7, 1.7, 20), 2.0);
    assert!((area - 180.0).abs() < 1e-4, "area {area}");
}

#[test]
fn a_noisy_near_flat_ring_with_micrometre_noise_insets_exactly() {
    let ring = vec![
        p(0.0, 0.0),
        p(0.7602052579050635, 1.5892193772219576e-7),
        p(1.520410515810127, 0.0),
        p(2.2806157737151906, -8.335971389935487e-7),
        p(3.040821031620254, 0.0),
        p(3.8010262895253177, -7.634802427735399e-8),
        p(4.561231547430381, 0.0),
        p(5.321436805335445, 1.506108105148871e-7),
        p(6.081642063240508, 0.0),
        p(6.841847321145572, 1.0278760134429032e-7),
        p(7.6020525790506355, 0.0),
        p(8.362257836955699, -4.772987971853007e-7),
        p(9.122463094860763, 0.0),
        p(9.882668352765826, -1.8132917549579485e-7),
        p(10.64287361067089, 0.0),
        p(11.403078868575953, 1.4784008090445777e-8),
        p(12.163284126481017, 0.0),
        p(12.92348938438608, -2.755551512133567e-7),
        p(13.683694642291144, 0.0),
        p(14.443899900196207, 8.836036112438148e-7),
        p(15.204105158101271, 0.0),
        p(15.964310416006334, 1.1149338053524182e-7),
        p(16.724515673911398, 0.0),
        p(17.48472093181646, -5.02350056402294e-7),
        p(18.244926189721525, 0.0),
        p(18.244926189721525, 10.0),
        p(0.0, 10.0),
    ];
    let distances = vec![1.0; ring.len()];
    let regions = variable(&ring, &distances).unwrap();
    assert_eq!(regions.len(), 1);
    assert_exact(&regions, &[(&ring, &distances)]);
    assert_clearance(&regions, &[(&ring, &distances)]);
}
