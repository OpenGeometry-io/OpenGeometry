use super::curve_area;
use crate::geom2d::boolean2d::PlanarBooleanOp;
use crate::geom2d::curved_boolean2d::{boolean_curved_regions, winding, CurveEdge2, CurveRegion2};
use crate::geom2d::poly2d::Pt2;
use std::f64::consts::{FRAC_PI_2, PI};

const EPS: f64 = 1e-6;
const SNAPPED: [f64; 5] = [-2e-7, -1e-7, 0.0, 1e-7, 2e-7];

fn line(from: [f64; 2], to: [f64; 2]) -> CurveEdge2 {
    CurveEdge2::Line { from, to }
}

fn bump() -> CurveRegion2 {
    CurveRegion2 {
        outer: vec![
            line([0.0, 0.0], [0.0, 10.0]),
            line([0.0, 10.0], [10.0, 10.0]),
            line([10.0, 10.0], [10.0, 7.0]),
            CurveEdge2::Arc {
                center: [10.0, 5.0],
                radius: 2.0,
                start_angle: FRAC_PI_2,
                sweep_angle: -PI,
            },
            line([10.0, 3.0], [10.0, 0.0]),
            line([10.0, 0.0], [0.0, 0.0]),
        ],
        holes: Vec::new(),
    }
}

fn keel() -> Vec<CurveEdge2> {
    vec![
        line([-3.0, 2.0], [1.0, 1.0]),
        CurveEdge2::Arc {
            center: [0.0, 1.0],
            radius: 1.0,
            start_angle: 0.0,
            sweep_angle: -FRAC_PI_2,
        },
        line([0.0, 0.0], [-3.0, 2.0]),
    ]
}

fn disk(center: [f64; 2], radius: f64) -> CurveRegion2 {
    let half = |start_angle: f64| CurveEdge2::Arc {
        center,
        radius,
        start_angle,
        sweep_angle: -PI,
    };
    CurveRegion2 {
        outer: vec![half(FRAC_PI_2), half(-FRAC_PI_2)],
        holes: Vec::new(),
    }
}

#[test]
fn a_ray_at_the_height_where_an_arc_ends_at_its_lowest_point_still_crosses_it() {
    let ring = bump().outer;
    for delta in SNAPPED {
        assert_ne!(
            winding(Pt2::new(5.0, 3.0 + delta), &ring, EPS),
            0,
            "lowest end, {delta}"
        );
        assert_ne!(
            winding(Pt2::new(5.0, 7.0 + delta), &ring, EPS),
            0,
            "highest end, {delta}"
        );
    }
}

#[test]
fn a_point_outside_at_the_height_of_a_keel_corner_winds_zero() {
    let ring = keel();
    for delta in SNAPPED {
        assert_eq!(winding(Pt2::new(-5.0, delta), &ring, EPS), 0, "{delta}");
    }
}

#[test]
fn a_disk_at_the_height_of_the_bumps_lowest_corner_unites_intersects_and_subtracts_by_area() {
    let host = bump();
    let host_area = curve_area(std::slice::from_ref(&host));
    let disk_area = PI * 0.25;
    for delta in [0.0, 1e-7] {
        let cutter = disk([5.0, 3.0 + delta], 0.5);
        let run = |operation| {
            boolean_curved_regions(&[host.clone()], &[cutter.clone()], operation, EPS).unwrap()
        };
        let united = run(PlanarBooleanOp::Union);
        let common = run(PlanarBooleanOp::Intersection);
        let cut = run(PlanarBooleanOp::Subtraction);
        assert_eq!(
            (united.len(), common.len(), cut.len()),
            (1, 1, 1),
            "{delta}"
        );
        assert!((curve_area(&united) - host_area).abs() < 1e-6, "{delta}");
        assert!((curve_area(&common) - disk_area).abs() < 1e-6, "{delta}");
        assert!(
            (curve_area(&cut) - (host_area - disk_area)).abs() < 1e-6,
            "{delta}"
        );
        assert_eq!(cut[0].holes.len(), 1, "{delta}");
    }
}

#[test]
fn a_ray_on_the_chord_of_a_half_disk_crosses_neither_end_from_outside() {
    let half_disk = vec![
        CurveEdge2::Arc {
            center: [0.0, 0.0],
            radius: 1.0,
            start_angle: 0.0,
            sweep_angle: PI,
        },
        line([-1.0, 0.0], [1.0, 0.0]),
    ];
    for delta in SNAPPED {
        assert_eq!(
            winding(Pt2::new(-5.0, delta), &half_disk, EPS),
            0,
            "{delta}"
        );
        assert_ne!(
            winding(Pt2::new(0.0, 0.5 + delta), &half_disk, EPS),
            0,
            "{delta}"
        );
    }
}
