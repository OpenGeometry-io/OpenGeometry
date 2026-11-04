use super::{area, edges_of, hole, p, rectangle, regions, union, winding_regions, DEFAULT_EPS};
use crate::geom2d::boolean2d::{
    boolean_oriented_regions, regions_from_edges_by, PlanarBooleanOp, RegionBooleanError,
};
use crate::geom2d::poly2d::{crossing_margin, nearby_boxes, self_intersects2, signed_area2, Pt2};

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

#[test]
fn nearby_boxes_lists_exactly_the_pairs_within_the_margin() {
    let mut boxes = (0..30)
        .flat_map(|row| {
            (0..40).map(move |column| {
                let (x, z) = (f64::from(column) * 0.25, f64::from(row) * 0.25);
                [x, x + 0.125, z, z + 0.125]
            })
        })
        .collect::<Vec<_>>();
    boxes.push([0.0, 10.0, 0.0, 0.0625]);
    boxes.push([3.0, 3.0625, -2.0, 8.0]);
    let margin = 0.125;
    let near = nearby_boxes(&boxes, margin);
    let gap = |a: [f64; 4], b: [f64; 4]| {
        (b[0] - a[1])
            .max(a[0] - b[1])
            .max(b[2] - a[3])
            .max(a[2] - b[3])
    };
    for index in 0..boxes.len() {
        let expected = (0..boxes.len())
            .filter(|&other| other != index && gap(boxes[index], boxes[other]) <= margin)
            .collect::<Vec<_>>();
        assert_eq!(near.of(index).collect::<Vec<_>>(), expected, "box {index}");
    }
    assert_eq!(
        (0..boxes.len())
            .map(|index| near.of(index).count())
            .sum::<usize>(),
        9_386
    );
    assert!(near.lists.is_some());
}

#[test]
fn a_cutter_a_thousand_units_wide_visits_every_pair_without_listing_them() {
    let host = vec![rectangle(0.0, 0.0, 2_000.0, 2_000.0)];
    let cutter = vec![rectangle(1_000.0, -500.0, 3_000.0, 1_000.0)];
    let edges = edges_of(&host[0])
        .into_iter()
        .chain(edges_of(&cutter[0]))
        .collect::<Vec<_>>();
    let margin = crossing_margin(edges.iter().copied(), 1.0e-9);
    assert_eq!(margin, f64::INFINITY);
    assert!(nearby_boxes(&[[0.0, 1.0, 0.0, 1.0]; 3], margin)
        .lists
        .is_none());
    assert!(nearby_boxes(&[[0.0, 1.0, 0.0, 1.0]; 200], 0.0)
        .lists
        .is_none());
    let regions =
        boolean_oriented_regions(&host, &cutter, PlanarBooleanOp::Subtraction, 1.0e-9).unwrap();
    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0].outer.len(), 6);
    assert_eq!(area(&regions), 3_000_000.0);
}

#[test]
fn two_near_collinear_wall_pieces_unite_across_their_mitre() {
    let first = vec![
        p(-7.915506291175921, -2.642064756497319),
        p(-7.840493708824079, -2.575935243502681),
        p(-5.360466317231342, -5.389100659702814),
        p(-5.435482107488363, -5.455226533879618),
        p(-7.915504659116948, -2.642066607788105),
    ];
    let second = vec![
        p(-8.973504659071258, -1.442066607747827),
        p(-8.898495340928742, -1.375933392252173),
        p(-7.840495340928741, -2.575933392252173),
        p(-7.915504659071259, -2.642066607747827),
        p(-7.915506291221608, -2.642064756537601),
    ];
    let (a, b) = (vec![first], vec![second]);
    let [(united, union_area), (_, overlap), (_, remainder)] = areas_of_all_operations(&a, &b);
    let (first_area, second_area) = (signed_area2(&a[0]).abs(), signed_area2(&b[0]).abs());
    assert_eq!(united, 1);
    assert!(overlap > 0.0 && overlap < 1.0e-7, "overlap {overlap}");
    assert!((union_area + overlap - first_area - second_area).abs() < 1.0e-9);
    assert!((remainder + overlap - first_area).abs() < 1.0e-9);
}
