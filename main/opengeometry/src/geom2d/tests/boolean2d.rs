use super::{area, edges_of, hole, p, rectangle, regions, union, winding_regions, DEFAULT_EPS};
use crate::geom2d::boolean2d::{
    boolean_oriented_regions, regions_from_edges_by, PlanarBooleanOp, RegionBooleanError,
};
use crate::geom2d::poly2d::{self_intersects2, signed_area2, Pt2};

#[test]
fn open_section_segment_partitions_a_classified_face() {
    let mut edges = edges_of(&rectangle(0.0, 0.0, 2.0, 2.0));
    edges.push((p(1.0, 0.0), p(1.0, 2.0)));
    let regions = regions_from_edges_by(&edges, DEFAULT_EPS, |point| {
        point.x > 0.0 && point.x < 1.0 && point.z > 0.0 && point.z < 2.0
    })
    .unwrap();
    assert_eq!(regions.len(), 1);
    assert!((signed_area2(&regions[0].outer) - 2.0).abs() < 1.0e-9);
}

#[test]
fn positive_clip_inner_cutter_makes_a_hole() {
    let base = rectangle(0.0, 0.0, 10.0, 10.0);
    let cutter = hole(4.0, 4.0, 6.0, 6.0);
    let regions = winding_regions(&[base, cutter], |winding| winding > 0);
    assert_eq!(regions.len(), 1, "one annular region");
    assert_eq!(regions[0].holes.len(), 1, "the cutter is its hole");
    assert!((signed_area2(&regions[0].outer) - 100.0).abs() < 1e-6);
    assert!(
        (signed_area2(&regions[0].holes[0]) + 4.0).abs() < 1e-6,
        "hole is CW, area -4"
    );
}

#[test]
fn positive_clip_keeps_every_disjoint_remainder() {
    let base = rectangle(0.0, 0.0, 10.0, 10.0);
    let cutter = hole(-1.0, 4.0, 11.0, 6.0);
    let regions = winding_regions(&[base, cutter], |winding| winding > 0);
    assert_eq!(regions.len(), 2, "the band splits the base in two");
    for region in &regions {
        assert!(
            (signed_area2(&region.outer) - 40.0).abs() < 1e-6,
            "each half is 10x4"
        );
        assert!(region.holes.is_empty());
    }
}

#[test]
fn positive_clip_total_cover_and_lone_cutter_are_empty() {
    let base = rectangle(0.0, 0.0, 4.0, 4.0);
    let cutter = hole(-1.0, -1.0, 5.0, 5.0);
    assert!(winding_regions(&[base, cutter], |winding| winding > 0).is_empty());

    let lone = hole(0.0, 0.0, 1.0, 1.0);
    assert!(winding_regions(&[lone.clone()], |winding| winding > 0).is_empty());
    assert_eq!(
        winding_regions(&[lone], |winding| winding != 0).len(),
        1,
        "contrast: the nonzero rule fills a lone CW ring"
    );
}

#[test]
fn positive_clip_overlapping_cutters_merge() {
    let base = rectangle(0.0, 0.0, 10.0, 10.0);
    let cutter_a = hole(-1.0, -1.0, 3.0, 3.0);
    let cutter_b = hole(1.0, 1.0, 4.0, 2.0);
    let regions = winding_regions(&[base, cutter_a, cutter_b], |winding| winding > 0);
    assert_eq!(regions.len(), 1);
    let region = &regions[0];
    assert!(region.holes.is_empty());
    assert!(
        !self_intersects2(&region.outer, DEFAULT_EPS),
        "merged outline is simple"
    );
    assert!((signed_area2(&region.outer) - 90.0).abs() < 1e-6);
}

#[test]
fn grouped_boolean_preserves_holes_and_all_operations() {
    let a = vec![rectangle(0.0, 0.0, 10.0, 10.0), hole(4.0, 4.0, 6.0, 6.0)];
    let b = vec![rectangle(5.0, -1.0, 12.0, 5.0)];

    let union = regions(&a, &b, PlanarBooleanOp::Union);
    assert_eq!(union.len(), 1);
    assert_eq!(union[0].holes.len(), 1);
    assert!((signed_area2(&union[0].holes[0]) + 3.0).abs() < 1e-6);

    let intersection = regions(&a, &b, PlanarBooleanOp::Intersection);
    assert_eq!(intersection.len(), 1);
    assert!(intersection[0].holes.is_empty());
    assert!((signed_area2(&intersection[0].outer) - 24.0).abs() < 1e-6);

    let subtraction = regions(&a, &b, PlanarBooleanOp::Subtraction);
    assert_eq!(subtraction.len(), 1);
    assert!(subtraction[0].holes.is_empty());
    assert!(
        (signed_area2(&subtraction[0].outer) - 72.0).abs() < 1e-6,
        "the cutter opens the original void to the exterior"
    );
}

#[test]
fn arrangement_stitching_uses_the_requested_scale() {
    let scale = 1.0e-4;
    let a = vec![rectangle(0.0, 0.0, scale, scale)];
    let b = vec![rectangle(0.5 * scale, 0.0, 1.5 * scale, scale)];
    let result = boolean_oriented_regions(&a, &b, PlanarBooleanOp::Union, 1.0e-10).unwrap();
    assert_eq!(result.len(), 1);
    assert!((signed_area2(&result[0].outer) - 1.5 * scale * scale).abs() < 1.0e-14);
}

fn mitered_room_walls(left_face: f64) -> Vec<Vec<Pt2>> {
    let inner_face = left_face + 0.2;
    let door_start = 1.5 + left_face + 0.1;
    vec![
        vec![
            p(door_start + 1.0, -0.1),
            p(door_start + 1.0, 0.1),
            p(door_start + 2.35, 0.1),
            p(door_start + 2.35, -0.1),
        ],
        vec![
            p(door_start + 3.65, -0.1),
            p(door_start + 3.65, 0.1),
            p(5.9, 0.1),
            p(6.1, -0.1),
        ],
        vec![
            p(left_face, -0.1),
            p(inner_face, 0.1),
            p(door_start, 0.1),
            p(door_start, -0.1),
        ],
        vec![p(5.9, 0.1), p(5.9, 3.9), p(6.1, 4.1), p(6.1, -0.1)],
        vec![
            p(left_face, 4.1),
            p(6.1, 4.1),
            p(5.9, 3.9),
            p(inner_face, 3.9),
        ],
        vec![
            p(left_face, -0.1),
            p(left_face, 4.1),
            p(inner_face, 3.9),
            p(inner_face, 0.1),
        ],
    ]
}

#[test]
fn union_of_mitered_walls_is_stable_at_any_coordinate() {
    for left_face in [
        -0.1, -0.044939, 0.030023, -0.066356, 0.072859, 0.137112, -0.077065, 0.104986, 0.222783,
        0.233492,
    ] {
        let walls = mitered_room_walls(left_face);
        let expected_area: f64 = walls.iter().map(|wall| signed_area2(wall).abs()).sum();
        let mut merged: Vec<Vec<Pt2>> = Vec::new();
        let mut united = Vec::new();
        for wall in &walls {
            united = union(&merged, std::slice::from_ref(wall));
            merged = united
                .iter()
                .flat_map(|region| {
                    std::iter::once(region.outer.clone()).chain(region.holes.iter().cloned())
                })
                .collect();
        }
        let mut ring_sizes: Vec<usize> = united.iter().map(|region| region.outer.len()).collect();
        ring_sizes.sort_unstable();
        assert_eq!(ring_sizes, vec![4, 12], "left face at {left_face}");
        assert!(united.iter().all(|region| region.holes.is_empty()));
        let total: f64 = united
            .iter()
            .map(|region| signed_area2(&region.outer))
            .sum();
        assert!(
            (total - expected_area).abs() < 1.0e-9,
            "left face at {left_face}: area {total}, expected {expected_area}"
        );
    }
}

#[test]
fn two_squares_touching_at_a_corner_stay_two_rings() {
    let united = union(
        &[rectangle(0.0, 0.0, 1.0, 1.0)],
        &[rectangle(1.0, 1.0, 2.0, 2.0)],
    );
    assert_eq!(united.len(), 2);
    assert!(united.iter().all(|region| region.outer.len() == 4));
    assert!((area(&united) - 2.0).abs() < 1.0e-12);
}

fn areas_of_all_operations(a: &[Vec<Pt2>], b: &[Vec<Pt2>]) -> [(usize, f64); 3] {
    [
        PlanarBooleanOp::Union,
        PlanarBooleanOp::Intersection,
        PlanarBooleanOp::Subtraction,
    ]
    .map(|operation| {
        let result = regions(a, b, operation);
        (result.len(), area(&result))
    })
}

fn assert_counts_and_areas(got: [(usize, f64); 3], expected: [(usize, f64); 3]) {
    for (index, (got, expected)) in got.into_iter().zip(expected).enumerate() {
        assert_eq!(got.0, expected.0, "operation {index}");
        assert!(
            (got.1 - expected.1).abs() < 1.0e-9,
            "operation {index}: area {}, expected {}",
            got.1,
            expected.1
        );
    }
}

#[test]
fn coincident_edges_of_two_operands_class_by_both_windings() {
    let unit = vec![rectangle(0.0, 0.0, 1.0, 1.0)];
    let beside = vec![rectangle(1.0, 0.0, 2.0, 1.0)];
    assert_counts_and_areas(
        areas_of_all_operations(&unit, &beside),
        [(1, 2.0), (0, 0.0), (1, 1.0)],
    );
    let within = vec![rectangle(0.0, 0.0, 0.5, 1.0)];
    assert_counts_and_areas(
        areas_of_all_operations(&unit, &within),
        [(1, 1.0), (1, 0.5), (1, 0.5)],
    );
    assert_counts_and_areas(
        areas_of_all_operations(&unit, &unit),
        [(1, 1.0), (1, 1.0), (0, 0.0)],
    );
}

#[test]
fn a_zero_area_sliver_adds_nothing_inside_or_outside_a_region() {
    let unit = vec![rectangle(0.0, 0.0, 1.0, 1.0)];
    for sliver in [
        vec![p(0.2, 0.5), p(0.8, 0.5), p(0.5, 0.5)],
        vec![p(1.5, 0.5), p(2.5, 0.5), p(2.0, 0.5)],
    ] {
        let united = union(&unit, &[sliver]);
        assert_eq!(united.len(), 1);
        assert_eq!(united[0].outer.len(), 4);
        assert!(united[0].holes.is_empty());
        assert!((area(&united) - 1.0).abs() < 1.0e-12);
    }
}

#[test]
fn a_t_junction_splits_the_edge_it_meets() {
    let unit = vec![rectangle(0.0, 0.0, 1.0, 1.0)];
    let stem = vec![rectangle(0.25, 1.0, 0.75, 2.0)];
    let united = union(&unit, &stem);
    assert_eq!(united.len(), 1);
    assert_eq!(united[0].outer.len(), 8);
    assert!((area(&united) - 1.5).abs() < 1.0e-12);
}

#[test]
fn a_clearance_below_twice_the_tolerance_still_classes_both_edges() {
    let gap = 1.5 * DEFAULT_EPS;
    let unit = vec![rectangle(0.0, 0.0, 1.0, 1.0)];
    let beside = vec![rectangle(1.0 + gap, 0.0, 2.0 + gap, 1.0)];
    assert_counts_and_areas(
        areas_of_all_operations(&unit, &beside),
        [(2, 2.0), (0, 0.0), (1, 1.0)],
    );
}

#[test]
fn a_ring_is_degenerate_when_it_is_thinner_than_the_tolerance() {
    let long = |width: f64| vec![rectangle(0.0, 0.0, 10.0, width)];
    assert!(union(&long(0.4 * DEFAULT_EPS), &[]).is_empty());
    let wider = union(&long(4.0 * DEFAULT_EPS), &[]);
    assert_eq!(wider.len(), 1);
    assert!((area(&wider) - 40.0 * DEFAULT_EPS).abs() < 1.0e-12);
    let small = union(&[rectangle(0.0, 0.0, 3.0e-4, 3.0e-4)], &[]);
    assert_eq!(small.len(), 1);
}

fn inside_the_unit_square(point: Pt2) -> bool {
    point.x > 0.0 && point.x < 1.0 && point.z > 0.0 && point.z < 1.0
}

#[test]
fn an_open_chain_of_boundary_edges_is_an_error_and_not_a_ring() {
    let mut edges = edges_of(&rectangle(0.0, 0.0, 1.0, 1.0));
    edges.pop();
    assert_eq!(
        regions_from_edges_by(&edges, DEFAULT_EPS, inside_the_unit_square).err(),
        Some(RegionBooleanError::OpenBoundary)
    );
    assert_eq!(
        RegionBooleanError::OpenBoundary.to_string(),
        "Boolean regions produced an open boundary"
    );
}

#[test]
fn a_hole_that_no_region_contains_is_an_error() {
    let edges = edges_of(&rectangle(0.0, 0.0, 1.0, 1.0));
    assert_eq!(
        regions_from_edges_by(&edges, DEFAULT_EPS, |point| !inside_the_unit_square(point)).err(),
        Some(RegionBooleanError::OrphanHole)
    );
}
