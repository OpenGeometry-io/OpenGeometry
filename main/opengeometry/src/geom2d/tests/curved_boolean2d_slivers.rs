use crate::geom2d::boolean2d::PlanarBooleanOp;
use crate::geom2d::curved_boolean2d::{boolean_curved_regions, winding, CurveEdge2, CurveRegion2};
use crate::geom2d::poly2d::Pt2;

const EPS: f64 = 1e-6;

fn line(from: [f64; 2], to: [f64; 2]) -> CurveEdge2 {
    CurveEdge2::Line { from, to }
}

fn region(outer: Vec<CurveEdge2>) -> CurveRegion2 {
    CurveRegion2 {
        outer,
        holes: Vec::new(),
    }
}

fn ends(edge: &CurveEdge2) -> ([f64; 2], [f64; 2]) {
    match *edge {
        CurveEdge2::Line { from, to } => (from, to),
        CurveEdge2::Arc {
            center,
            radius,
            start_angle,
            sweep_angle,
        } => {
            let at = |angle: f64| {
                [
                    center[0] + radius * angle.cos(),
                    center[1] + radius * angle.sin(),
                ]
            };
            (at(start_angle), at(start_angle + sweep_angle))
        }
    }
}

fn cross(from: [f64; 2], to: [f64; 2]) -> f64 {
    from[0] * to[1] - to[0] * from[1]
}

fn green_area(ring: &[CurveEdge2]) -> f64 {
    let twice: f64 = ring
        .iter()
        .enumerate()
        .map(|(index, edge)| {
            let (start, end) = ends(edge);
            let closing = cross(end, ends(&ring[(index + 1) % ring.len()]).0);
            closing
                + match *edge {
                    CurveEdge2::Line { .. } => cross(start, end),
                    CurveEdge2::Arc {
                        center,
                        radius,
                        sweep_angle,
                        ..
                    } => {
                        radius * radius * sweep_angle + center[0] * (end[1] - start[1])
                            - center[1] * (end[0] - start[0])
                    }
                }
        })
        .sum();
    (0.5 * twice).abs()
}

fn assert_closed(ring: &[CurveEdge2]) {
    for (index, edge) in ring.iter().enumerate() {
        let end = ends(edge).1;
        let next = ends(&ring[(index + 1) % ring.len()]).0;
        assert!(
            (end[0] - next[0]).hypot(end[1] - next[1]) <= EPS,
            "edge {index} ends at {end:?}, the next starts at {next:?}"
        );
    }
}

fn closed_area(regions: &[CurveRegion2]) -> f64 {
    regions
        .iter()
        .map(|region| {
            assert_closed(&region.outer);
            green_area(&region.outer)
        })
        .sum()
}

fn line_meets_circle(from: [f64; 2], to: [f64; 2], center: [f64; 2], radius: f64) -> [f64; 2] {
    let (dx, dz) = (to[0] - from[0], to[1] - from[1]);
    let (fx, fz) = (from[0] - center[0], from[1] - center[1]);
    let a = dx * dx + dz * dz;
    let b = 2.0 * (fx * dx + fz * dz);
    let c = fx * fx + fz * fz - radius * radius;
    let t = (-b + (b * b - 4.0 * a * c).sqrt()) / (2.0 * a);
    [from[0] + t * dx, from[1] + t * dz]
}

fn polar_angle(point: [f64; 2], center: [f64; 2]) -> f64 {
    (point[1] - center[1]).atan2(point[0] - center[0])
}

fn on_ring(point: Pt2, ring: &[CurveEdge2]) -> bool {
    ring.iter().any(|edge| {
        let CurveEdge2::Line { from, to } = *edge else {
            return false;
        };
        let (dx, dz) = (to[0] - from[0], to[1] - from[1]);
        let t = (((point.x - from[0]) * dx + (point.z - from[1]) * dz) / (dx * dx + dz * dz))
            .clamp(0.0, 1.0);
        (point.x - from[0] - t * dx).hypot(point.z - from[1] - t * dz) <= EPS
    })
}

fn inside_or_on(point: Pt2, ring: &[CurveEdge2]) -> bool {
    winding(point, ring, EPS) != 0 || on_ring(point, ring)
}

#[test]
fn a_union_with_a_crescent_thinner_than_the_old_probe_step_inside_its_neighbour_is_one_region() {
    let center = [3.0, 0.0];
    let sweep = 0.8558475468495267;
    let shared = CurveEdge2::Arc {
        center,
        radius: 6.0,
        start_angle: std::f64::consts::PI,
        sweep_angle: sweep,
    };
    let near = [-0.9334724395205245, -4.530760925885679];
    let crescent_end = [-0.9278089883631674, -4.53567776643508];
    let neighbour_end = [-1.0410780115103093, -4.437340955447067];
    let crescent_radius = 6.000004687498169;
    let crescent = region(vec![
        line([-3.0, -0.007500000000000007], [-3.0, 0.0]),
        shared.clone(),
        line(near, crescent_end),
        CurveEdge2::Arc {
            center,
            radius: crescent_radius,
            start_angle: -2.2844951073913076,
            sweep_angle: -sweep,
        },
    ]);
    let neighbour = region(vec![
        shared,
        line(near, neighbour_end),
        CurveEdge2::Arc {
            center,
            radius: 6.0016919489423985,
            start_angle: 3.9736946644234292,
            sweep_angle: -sweep,
        },
        line([-3.0, 0.14250000000000002], [-3.0, 0.0]),
    ]);
    let cut = line_meets_circle(near, neighbour_end, center, crescent_radius);
    let from = polar_angle(cut, center);
    let outside_part = [
        line(near, cut),
        CurveEdge2::Arc {
            center,
            radius: crescent_radius,
            start_angle: from,
            sweep_angle: polar_angle(crescent_end, center) - from,
        },
        line(crescent_end, near),
    ];
    let expected = green_area(&neighbour.outer) + green_area(&outside_part);
    let united =
        boolean_curved_regions(&[crescent, neighbour], &[], PlanarBooleanOp::Union, EPS).unwrap();
    assert_eq!(united.len(), 1);
    assert!(
        (closed_area(&united) - expected).abs() < 1e-9,
        "expected {expected}, got {}",
        closed_area(&united)
    );
}

#[test]
fn an_intersection_where_a_strip_ends_just_past_a_mitre_is_cut_along_the_mitre() {
    let strip = region(vec![
        line([-3.0, 0.14250000000000002], [-3.0, 0.15750000000000003]),
        line(
            [-3.0, 0.15750000000000003],
            [-0.021019785944165648, 0.15750000000000003],
        ),
        line(
            [-0.021019785944165086, 0.15750000000000003],
            [-0.018992770174007464, 0.14250000000000002],
        ),
        line(
            [-0.018992770174007356, 0.14250000000000002],
            [-3.0, 0.14250000000000002],
        ),
    ]);
    let mitre_top = [-0.021019785944165648, 0.15750000000000003];
    let mitre_bottom = [0.02046555675664008, -0.15750000000000003];
    let footprint = region(vec![
        line([-3.0, 0.15750000000000003], mitre_top),
        line(mitre_top, mitre_bottom),
        line(mitre_bottom, [-3.0, -0.15750000000000003]),
        line([-3.0, -0.15750000000000003], [-3.0, 0.15750000000000003]),
    ]);
    let low = 0.14250000000000002;
    let along = (mitre_top[1] - low) / (mitre_top[1] - mitre_bottom[1]);
    let cut = [mitre_top[0] + along * (mitre_bottom[0] - mitre_top[0]), low];
    let expected = green_area(&[
        line([-3.0, low], [-3.0, mitre_top[1]]),
        line([-3.0, mitre_top[1]], mitre_top),
        line(mitre_top, cut),
        line(cut, [-3.0, low]),
    ]);
    let kept = boolean_curved_regions(
        &[strip.clone()],
        &[footprint.clone()],
        PlanarBooleanOp::Intersection,
        EPS,
    )
    .unwrap();
    assert_eq!(kept.len(), 1);
    assert!(
        (closed_area(&kept) - expected).abs() < 1e-9,
        "expected {expected}, got {}",
        closed_area(&kept)
    );
    for edge in &kept[0].outer {
        for t in [0.0, 0.25, 0.5, 0.75] {
            let point = edge.point(t);
            assert!(
                inside_or_on(point, &strip.outer),
                "{point:?} outside the strip"
            );
            assert!(
                inside_or_on(point, &footprint.outer),
                "{point:?} outside the footprint"
            );
        }
    }
}

#[test]
fn a_five_micrometre_crescent_on_the_inner_arc_of_a_band_unites_into_the_band() {
    let band = region(vec![
        CurveEdge2::Arc {
            center: [0.0, 0.0],
            radius: 1.0,
            start_angle: 0.0,
            sweep_angle: 1.0,
        },
        line(
            [1.0_f64.cos(), 1.0_f64.sin()],
            [1.01 * 1.0_f64.cos(), 1.01 * 1.0_f64.sin()],
        ),
        CurveEdge2::Arc {
            center: [0.0, 0.0],
            radius: 1.01,
            start_angle: 1.0,
            sweep_angle: -1.0,
        },
        line([1.01, 0.0], [1.0, 0.0]),
    ]);
    let thin = 1.0 + 5e-6;
    let crescent = region(vec![
        CurveEdge2::Arc {
            center: [0.0, 0.0],
            radius: 1.0,
            start_angle: 0.2,
            sweep_angle: 0.6,
        },
        line(
            [0.8_f64.cos(), 0.8_f64.sin()],
            [thin * 0.8_f64.cos(), thin * 0.8_f64.sin()],
        ),
        CurveEdge2::Arc {
            center: [0.0, 0.0],
            radius: thin,
            start_angle: 0.8,
            sweep_angle: -0.6,
        },
        line(
            [thin * 0.2_f64.cos(), thin * 0.2_f64.sin()],
            [0.2_f64.cos(), 0.2_f64.sin()],
        ),
    ]);
    let united =
        boolean_curved_regions(&[band, crescent], &[], PlanarBooleanOp::Union, EPS).unwrap();
    assert_eq!(united.len(), 1);
    let expected = 0.5 * (1.01 * 1.01 - 1.0);
    assert!(
        (closed_area(&united) - expected).abs() < 1e-9,
        "expected {expected}, got {}",
        closed_area(&united)
    );
}

#[test]
fn an_intersection_where_a_strip_ends_fifty_micrometres_past_an_edge_keeps_the_part_inside() {
    let strip = region(vec![
        line([0.0, 0.0], [0.0, 0.015]),
        line([0.0, 0.015], [1.0, 0.015]),
        line([1.0, 0.015], [1.0 + 5e-5, 0.0]),
        line([1.0 + 5e-5, 0.0], [0.0, 0.0]),
    ]);
    let footprint = region(vec![
        line([-1.0, 0.015], [1.0, 0.015]),
        line([1.0, 0.015], [1.0, -1.0]),
        line([1.0, -1.0], [-1.0, -1.0]),
        line([-1.0, -1.0], [-1.0, 0.015]),
    ]);
    let kept =
        boolean_curved_regions(&[strip], &[footprint], PlanarBooleanOp::Intersection, EPS).unwrap();
    assert_eq!(kept.len(), 1);
    assert!(
        (closed_area(&kept) - 0.015).abs() < 1e-9,
        "expected 0.015, got {}",
        closed_area(&kept)
    );
}
