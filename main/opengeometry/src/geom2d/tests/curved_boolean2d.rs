use super::{edges_of, hole};
use crate::brep::GeometryError;
use crate::geom2d::boolean2d::PlanarBooleanOp;
use crate::geom2d::curved_boolean2d::{
    boolean_curved_regions, nested_regions, winding, CurveEdge2, CurveRegion2,
};
use crate::geom2d::poly2d::Pt2;
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};

fn sector(start: f64, end: f64) -> CurveRegion2 {
    let at = |radius: f64, angle: f64| [radius * angle.cos(), radius * angle.sin()];
    CurveRegion2 {
        outer: vec![
            CurveEdge2::Arc {
                center: [0.0, 0.0],
                radius: 2.0,
                start_angle: end,
                sweep_angle: start - end,
            },
            CurveEdge2::Line {
                from: at(2.0, start),
                to: at(1.0, start),
            },
            CurveEdge2::Arc {
                center: [0.0, 0.0],
                radius: 1.0,
                start_angle: start,
                sweep_angle: end - start,
            },
            CurveEdge2::Line {
                from: at(1.0, end),
                to: at(2.0, end),
            },
        ],
        holes: Vec::new(),
    }
}

fn curve_rectangle(x0: f64, z0: f64, x1: f64, z1: f64) -> CurveRegion2 {
    let clockwise = hole(x0, z0, x1, z1);
    CurveRegion2 {
        outer: edges_of(&clockwise)
            .into_iter()
            .map(|(from, to)| CurveEdge2::Line {
                from: [from.x, from.z],
                to: [to.x, to.z],
            })
            .collect(),
        holes: Vec::new(),
    }
}

fn disk(center: [f64; 2], radius: f64, angle: f64) -> CurveRegion2 {
    CurveRegion2 {
        outer: vec![
            CurveEdge2::Arc {
                center,
                radius,
                start_angle: angle,
                sweep_angle: -PI,
            },
            CurveEdge2::Arc {
                center,
                radius,
                start_angle: angle - PI,
                sweep_angle: -PI,
            },
        ],
        holes: Vec::new(),
    }
}

fn ring_area(ring: &[CurveEdge2]) -> f64 {
    ring.iter().map(CurveEdge2::twice_area).sum::<f64>() * 0.5
}

fn area(regions: &[CurveRegion2]) -> f64 {
    regions
        .iter()
        .map(|region| {
            -ring_area(&region.outer) - region.holes.iter().map(|hole| ring_area(hole)).sum::<f64>()
        })
        .sum()
}

fn has_arc(ring: &[CurveEdge2]) -> bool {
    ring.iter()
        .any(|edge| matches!(edge, CurveEdge2::Arc { .. }))
}

#[test]
fn coincident_arc_boundaries_preserve_exact_overlap() {
    let a = sector(0.0, FRAC_PI_2);
    let b = sector(FRAC_PI_4, 3.0 * FRAC_PI_4);
    let expected = 1.5 * FRAC_PI_4;
    for (operation, target) in [
        (PlanarBooleanOp::Intersection, expected),
        (PlanarBooleanOp::Union, 1.5 * 3.0 * FRAC_PI_4),
        (PlanarBooleanOp::Subtraction, expected),
    ] {
        let out = boolean_curved_regions(&[a.clone()], &[b.clone()], operation, 1e-7).unwrap();
        assert_eq!(out.len(), 1);
        assert!(
            (area(&out) - target).abs() < 1e-6,
            "{operation:?}: expected {target}, got {}",
            area(&out)
        );
        assert!(has_arc(&out[0].outer));
    }
}

#[test]
fn line_arc_cut_splits_sector_without_chording() {
    let base = sector(0.0, FRAC_PI_2);
    let cutter = curve_rectangle(1.2, -0.2, 1.8, 2.2);
    let out = boolean_curved_regions(
        &[base.clone()],
        &[cutter],
        PlanarBooleanOp::Subtraction,
        1e-7,
    )
    .unwrap();
    assert_eq!(out.len(), 2);
    assert!(out.iter().all(|region| ring_area(&region.outer) < 0.0));
    assert!(
        out.iter()
            .flat_map(|region| &region.outer)
            .filter(|edge| matches!(edge, CurveEdge2::Arc { .. }))
            .count()
            >= 2
    );
    assert!(area(&out) > 0.0 && area(&out) < area(&[base]));
}

#[test]
fn identical_arc_region_union_and_subtraction_are_canonical() {
    let source = sector(-0.4, 1.7);
    let union = boolean_curved_regions(
        &[source.clone()],
        &[source.clone()],
        PlanarBooleanOp::Union,
        1e-7,
    )
    .unwrap();
    assert_eq!(union.len(), 1);
    assert!((area(&union) - area(&[source.clone()])).abs() < 1e-6);
    let difference = boolean_curved_regions(
        &[source.clone()],
        &[source],
        PlanarBooleanOp::Subtraction,
        1e-7,
    )
    .unwrap();
    assert!(difference.is_empty());
}

#[test]
fn random_concentric_sector_booleans_match_analytic_areas() {
    let mut seed = 0x8a53_71de_u64;
    let mut random = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        ((seed >> 32) as f64) / (u32::MAX as f64)
    };
    for case in 0..120 {
        let a0 = -0.7 + 1.4 * random();
        let a1 = a0 + 0.2 + 1.0 * random();
        let b0 = -0.7 + 1.4 * random();
        let b1 = b0 + 0.2 + 1.0 * random();
        let a = sector(a0, a1);
        let b = sector(b0, b1);
        let overlap = (a1.min(b1) - a0.max(b0)).max(0.0);
        for (operation, expected) in [
            (PlanarBooleanOp::Union, 1.5 * (a1 - a0 + b1 - b0 - overlap)),
            (PlanarBooleanOp::Intersection, 1.5 * overlap),
            (PlanarBooleanOp::Subtraction, 1.5 * (a1 - a0 - overlap)),
        ] {
            let out = boolean_curved_regions(&[a.clone()], &[b.clone()], operation, 1e-7).unwrap();
            assert!(
                (area(&out) - expected).abs() < 2e-6,
                "case {case} {operation:?}: expected {expected}, got {}",
                area(&out)
            );
            assert!(out.iter().any(|region| has_arc(&region.outer)) || out.is_empty());
        }
    }
}

#[test]
fn continuation_pair_subtracts_from_both_sides_of_a_third_host() {
    let base = curve_rectangle(-2.0, -0.1, 0.30000000000000027, 0.1);
    let north = curve_rectangle(-0.1, 0.0, 0.1, 2.0);
    let south_arc = CurveRegion2 {
        outer: vec![
            CurveEdge2::Line {
                from: [-0.10000000000000009, 1.3471114790620887e-16],
                to: [0.09999999999999998, 1.1021821192326179e-16],
            },
            CurveEdge2::Arc {
                center: [1.0, 0.0],
                radius: 0.9,
                start_angle: PI,
                sweep_angle: FRAC_PI_2,
            },
            CurveEdge2::Line {
                from: [0.9999999999999999, -0.9],
                to: [0.9999999999999998, -1.1],
            },
            CurveEdge2::Arc {
                center: [1.0, 0.0],
                radius: 1.1,
                start_angle: 3.0 * FRAC_PI_2,
                sweep_angle: -FRAC_PI_2,
            },
        ],
        holes: Vec::new(),
    };
    let out = boolean_curved_regions(
        &[base],
        &[north, south_arc],
        PlanarBooleanOp::Subtraction,
        1e-6,
    )
    .unwrap();
    assert_eq!(out.len(), 2);
    assert!(out
        .iter()
        .any(|region| region.outer.iter().any(|edge| edge.point(0.5).x < -1.0)));
    let original = curve_rectangle(-2.0, -0.1, 0.0, 0.1);
    let intersections: Vec<_> = out
        .iter()
        .map(|region| {
            boolean_curved_regions(
                &[region.clone()],
                &[original.clone()],
                PlanarBooleanOp::Intersection,
                1e-6,
            )
            .unwrap()
        })
        .collect();
    assert_eq!(
        intersections
            .iter()
            .filter(|regions| !regions.is_empty())
            .count(),
        1
    );
    for region in out {
        boolean_curved_regions(
            &[region],
            &[original.clone()],
            PlanarBooleanOp::Subtraction,
            1e-6,
        )
        .unwrap();
    }
}

#[test]
fn subtracting_concentric_disk_keeps_an_exact_circular_hole() {
    let out = boolean_curved_regions(
        &[disk([0.0, 0.0], 2.0, 0.0)],
        &[disk([0.0, 0.0], 1.0, 0.0)],
        PlanarBooleanOp::Subtraction,
        1e-7,
    )
    .unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].holes.len(), 1);
    assert!((area(&out) - 3.0 * PI).abs() < 1e-7);
    assert!(out[0].holes[0]
        .iter()
        .all(|edge| matches!(edge, CurveEdge2::Arc { .. })));
}

#[test]
fn intersecting_disks_keep_exact_circle_arcs_and_lens_area() {
    let out = boolean_curved_regions(
        &[disk([-1.0, 0.0], 2.0, 0.0)],
        &[disk([1.0, 0.0], 2.0, 0.0)],
        PlanarBooleanOp::Intersection,
        1e-7,
    )
    .unwrap();
    let expected = 8.0 * (0.5_f64).acos() - 2.0 * 3.0_f64.sqrt();
    assert_eq!(out.len(), 1);
    assert!(out[0]
        .outer
        .iter()
        .all(|edge| matches!(edge, CurveEdge2::Arc { .. })));
    assert!((area(&out) - expected).abs() < 1e-7);
}

#[test]
fn disjoint_output_has_canonical_region_and_ring_order() {
    let left = curve_rectangle(-3.0, -1.0, -2.0, 1.0);
    let right = curve_rectangle(2.0, -1.0, 3.0, 1.0);
    let first = boolean_curved_regions(
        &[right.clone(), left.clone()],
        &[],
        PlanarBooleanOp::Union,
        1e-7,
    )
    .unwrap();
    let second = boolean_curved_regions(&[left, right], &[], PlanarBooleanOp::Union, 1e-7).unwrap();
    assert_eq!(
        serde_json::to_value(&first).unwrap(),
        serde_json::to_value(&second).unwrap()
    );
    assert_eq!(first.len(), 2);
    assert_eq!(first[0].outer[0].point(0.0).x, -3.0);
    assert_eq!(first[1].outer[0].point(0.0).x, 2.0);
}

#[test]
fn translated_and_rotated_circle_lenses_match_analytic_area() {
    for index in 0..80 {
        let angle = index as f64 * 0.783_124;
        let distance = 0.25 + (index % 31) as f64 * 0.115;
        let centre = [
            3.0 + (index % 7) as f64 * 0.23,
            -4.0 + (index % 9) as f64 * 0.31,
        ];
        let other = [
            centre[0] + distance * angle.cos(),
            centre[1] + distance * angle.sin(),
        ];
        let actual = boolean_curved_regions(
            &[disk(centre, 2.0, angle / 3.0)],
            &[disk(other, 2.0, angle / 5.0)],
            PlanarBooleanOp::Intersection,
            1e-7,
        )
        .unwrap();
        let expected =
            8.0 * (distance / 4.0).acos() - 0.5 * distance * (16.0 - distance * distance).sqrt();
        assert_eq!(actual.len(), 1, "case {index}");
        assert!(
            (area(&actual) - expected).abs() < 1e-6,
            "case {index}: expected {expected}, got {}",
            area(&actual)
        );
    }
}

#[test]
fn an_arc_and_a_line_meet_whatever_grid_cell_their_shared_end_falls_in() {
    for left in [
        -0.044939, -0.1, 0.030023, -0.066356, 0.072859, 0.137112, -0.077065, 0.104986, 0.222783,
        0.233492, -0.000003, 0.000005,
    ] {
        let center = left + 1.0;
        let half_disk = CurveRegion2 {
            outer: vec![
                CurveEdge2::Arc {
                    center: [center, 0.0],
                    radius: 1.0,
                    start_angle: PI,
                    sweep_angle: -PI,
                },
                CurveEdge2::Line {
                    from: [center + 1.0, 0.0],
                    to: [left, 0.0],
                },
            ],
            holes: Vec::new(),
        };
        let united = boolean_curved_regions(&[half_disk], &[], PlanarBooleanOp::Union, 1e-6)
            .unwrap_or_else(|error| panic!("left end at {left}: {error:?}"));
        assert_eq!(united.len(), 1, "left end at {left}");
        assert!(
            (area(&united) - FRAC_PI_2).abs() < 1e-9,
            "left end at {left}: area {}",
            area(&united)
        );
    }
}

#[test]
fn two_rectangles_closer_than_the_old_probe_reach_stay_two_regions() {
    for gap in [2.0e-6, 1.0e-5, 9.0e-5] {
        let united = boolean_curved_regions(
            &[curve_rectangle(0.0, 0.0, 1.0, 1.0)],
            &[curve_rectangle(1.0 + gap, 0.0, 2.0 + gap, 1.0)],
            PlanarBooleanOp::Union,
            1e-7,
        )
        .unwrap_or_else(|error| panic!("gap {gap}: {error:?}"));
        assert_eq!(united.len(), 2, "gap {gap}");
        assert!((area(&united) - 2.0).abs() < 1e-9, "gap {gap}");
    }
}

fn circle(seam: f64, sweep: f64) -> [CurveEdge2; 2] {
    [
        CurveEdge2::Arc {
            center: [0.0, 0.0],
            radius: 1.0,
            start_angle: seam,
            sweep_angle: sweep,
        },
        CurveEdge2::Arc {
            center: [0.0, 0.0],
            radius: 1.0,
            start_angle: seam + sweep,
            sweep_angle: sweep,
        },
    ]
}

#[test]
fn a_point_on_the_seam_ray_of_a_circle_winds_once_in_either_direction() {
    for seam in [
        0.3_f64, -0.7, 1.1, 0.123456, -1.3, 0.9, 0.05, -0.2, 0.6, 1.4, 1.2, -1.2, -1.4,
    ] {
        let point = Pt2::new(0.0, seam.sin());
        assert_eq!(
            winding(point, &circle(seam, PI), 1e-7),
            1,
            "counter-clockwise, seam at {seam}"
        );
        assert_eq!(
            winding(point, &circle(seam, -PI), 1e-7),
            -1,
            "clockwise, seam at {seam}"
        );
        let left_seam = PI - seam;
        let outside = Pt2::new(-2.0, left_seam.sin());
        for sweep in [PI, -PI] {
            assert_eq!(
                winding(outside, &circle(left_seam, sweep), 1e-7),
                0,
                "sweep {sweep}, seam at {left_seam}"
            );
        }
    }
}

#[test]
fn a_ray_just_below_where_a_line_meets_an_arc_start_crosses_it_once() {
    let eps = 1e-6;
    let ring = [
        CurveEdge2::Line {
            from: [1.0, -1.0],
            to: [1.0, 0.0],
        },
        CurveEdge2::Arc {
            center: [0.0, 0.0],
            radius: 1.0,
            start_angle: 0.0,
            sweep_angle: FRAC_PI_2,
        },
        CurveEdge2::Line {
            from: [0.0, 1.0],
            to: [0.0, -1.0],
        },
        CurveEdge2::Line {
            from: [0.0, -1.0],
            to: [1.0, -1.0],
        },
    ];
    let below = -0.5 * eps;
    assert_eq!(winding(Pt2::new(0.5, below), &ring, eps), 1);
    assert_eq!(winding(Pt2::new(-0.5, below), &ring, eps), 0);
}

#[test]
fn a_ray_just_below_where_an_arc_end_meets_a_line_crosses_it_once() {
    let eps = 1e-6;
    let ring = [
        CurveEdge2::Arc {
            center: [0.0, 0.0],
            radius: 1.0,
            start_angle: FRAC_PI_2,
            sweep_angle: -FRAC_PI_2,
        },
        CurveEdge2::Line {
            from: [1.0, 0.0],
            to: [1.0, -1.0],
        },
        CurveEdge2::Line {
            from: [1.0, -1.0],
            to: [0.0, -1.0],
        },
        CurveEdge2::Line {
            from: [0.0, -1.0],
            to: [0.0, 1.0],
        },
    ];
    let below = -0.5 * eps;
    assert_eq!(winding(Pt2::new(0.5, below), &ring, eps), -1);
    assert_eq!(winding(Pt2::new(-0.5, below), &ring, eps), 0);
}

#[test]
fn coincident_lines_whose_ends_differ_in_the_last_bit_merge_into_one_boundary() {
    let left = 0.072859;
    let center = left + 1.0;
    let half_disk = |right: f64| CurveRegion2 {
        outer: vec![
            CurveEdge2::Arc {
                center: [center, 0.0],
                radius: 1.0,
                start_angle: PI,
                sweep_angle: -PI,
            },
            CurveEdge2::Line {
                from: [right, 0.0],
                to: [left, 0.0],
            },
        ],
        holes: Vec::new(),
    };
    assert_ne!(center + 1.0, left + 2.0);
    let united = boolean_curved_regions(
        &[half_disk(center + 1.0)],
        &[half_disk(left + 2.0)],
        PlanarBooleanOp::Union,
        1e-6,
    )
    .unwrap();
    assert_eq!(united.len(), 1);
    assert!((area(&united) - FRAC_PI_2).abs() < 1e-9);
}

#[test]
fn a_hole_that_no_region_holds_is_an_invalid_topology_error() {
    let hole = disk([0.0, 0.0], 1.0, 0.0).outer;
    assert!(matches!(
        nested_regions(&[hole], 1e-7),
        Err(GeometryError::InvalidTopology(message))
            if message == "Curved Boolean produced a hole outside every region"
    ));
}

#[test]
fn two_rectangles_touching_at_a_corner_stay_two_rings() {
    let unit = curve_rectangle(0.0, 0.0, 1.0, 1.0);
    for other in [
        curve_rectangle(1.0, 1.0, 2.0, 2.0),
        curve_rectangle(1.0, -1.0, 2.0, 0.0),
        curve_rectangle(-1.0, 1.0, 0.0, 2.0),
        curve_rectangle(-1.0, -1.0, 0.0, 0.0),
    ] {
        for (a, b) in [(&unit, &other), (&other, &unit)] {
            let united = boolean_curved_regions(
                std::slice::from_ref(a),
                std::slice::from_ref(b),
                PlanarBooleanOp::Union,
                1e-7,
            )
            .unwrap();
            assert_eq!(united.len(), 2);
            assert!(united.iter().all(|region| region.outer.len() == 4));
            assert!((area(&united) - 2.0).abs() < 1e-12);
        }
    }
}
