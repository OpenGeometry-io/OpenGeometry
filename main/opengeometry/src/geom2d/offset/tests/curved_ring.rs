use super::polyline::filled;
use crate::geom2d::boolean2d::distance_to_segment;
use crate::geom2d::curved_boolean2d::{CurveEdge2, CurveRegion2};
use crate::geom2d::offset::inset_curved_region;
use crate::geom2d::poly2d::Pt2;
use crate::geom2d::tests::{curve_ring_area, p, DEFAULT_EPS};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

fn line(from: [f64; 2], to: [f64; 2]) -> CurveEdge2 {
    CurveEdge2::Line { from, to }
}

fn arc(center: [f64; 2], radius: f64, angles: [f64; 2]) -> CurveEdge2 {
    CurveEdge2::Arc {
        center,
        radius,
        start_angle: angles[0],
        sweep_angle: angles[1],
    }
}

fn region(outer: Vec<CurveEdge2>, holes: Vec<Vec<CurveEdge2>>) -> CurveRegion2 {
    CurveRegion2 { outer, holes }
}

fn d_shape() -> CurveRegion2 {
    region(
        vec![
            line([-2.0, -2.0], [2.0, -2.0]),
            arc([2.0, 0.0], 2.0, [-FRAC_PI_2, PI]),
            line([2.0, 2.0], [-2.0, 2.0]),
            line([-2.0, 2.0], [-2.0, -2.0]),
        ],
        Vec::new(),
    )
}

fn circular_segment() -> CurveRegion2 {
    let angle = PI / 3.0;
    let corner = |sign: f64| [2.0 * angle.cos(), sign * 2.0 * angle.sin()];
    region(
        vec![
            arc([0.0, 0.0], 2.0, [-angle, 2.0 * angle]),
            line(corner(1.0), corner(-1.0)),
        ],
        Vec::new(),
    )
}

fn bumped_square() -> CurveRegion2 {
    region(
        vec![
            line([0.0, 0.0], [4.0, 0.0]),
            line([4.0, 0.0], [4.0, 4.0]),
            line([4.0, 4.0], [3.0, 4.0]),
            arc([2.0, 4.0], 1.0, [0.0, PI]),
            line([1.0, 4.0], [0.0, 4.0]),
            line([0.0, 4.0], [0.0, 0.0]),
        ],
        Vec::new(),
    )
}

fn filleted_square(fillet: f64) -> CurveRegion2 {
    let near = 4.0 - fillet;
    region(
        vec![
            line([0.0, 0.0], [4.0, 0.0]),
            line([4.0, 0.0], [4.0, near]),
            arc([near, near], fillet, [0.0, FRAC_PI_2]),
            line([near, 4.0], [0.0, 4.0]),
            line([0.0, 4.0], [0.0, 0.0]),
        ],
        Vec::new(),
    )
}

fn square_with_round_hole() -> CurveRegion2 {
    region(
        vec![
            line([0.0, 0.0], [4.0, 0.0]),
            line([4.0, 0.0], [4.0, 4.0]),
            line([4.0, 4.0], [0.0, 4.0]),
            line([0.0, 4.0], [0.0, 0.0]),
        ],
        vec![vec![
            arc([2.0, 2.0], 0.5, [0.0, -PI]),
            arc([2.0, 2.0], 0.5, [-PI, -PI]),
        ]],
    )
}

fn rings(region: &CurveRegion2) -> impl Iterator<Item = &Vec<CurveEdge2>> {
    std::iter::once(&region.outer).chain(&region.holes)
}

fn area(regions: &[CurveRegion2]) -> f64 {
    regions
        .iter()
        .map(|region| {
            curve_ring_area(&region.outer).abs()
                - region
                    .holes
                    .iter()
                    .map(|hole| curve_ring_area(hole).abs())
                    .sum::<f64>()
        })
        .sum()
}

fn edge_distance(edge: &CurveEdge2, point: Pt2) -> f64 {
    match edge {
        CurveEdge2::Line { from, to } => {
            distance_to_segment(point, p(from[0], from[1]), p(to[0], to[1]))
        }
        CurveEdge2::Arc {
            center,
            radius,
            start_angle,
            sweep_angle,
        } => {
            let angle = (point.z - center[1]).atan2(point.x - center[0]);
            let along = ((angle - start_angle) * sweep_angle.signum()).rem_euclid(TAU);
            if along <= sweep_angle.abs() {
                ((point.x - center[0]).hypot(point.z - center[1]) - radius).abs()
            } else {
                [edge.point(0.0), edge.point(1.0)]
                    .iter()
                    .map(|end| (end.x - point.x).hypot(end.z - point.z))
                    .fold(f64::INFINITY, f64::min)
            }
        }
    }
}

fn boundary_distance(source: &CurveRegion2, point: Pt2) -> f64 {
    rings(source)
        .flatten()
        .map(|edge| edge_distance(edge, point))
        .fold(f64::INFINITY, f64::min)
}

fn assert_at_distance(result: &[CurveRegion2], source: &CurveRegion2, distance: f64) {
    for edge in result.iter().flat_map(rings).flatten() {
        for t in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let point = edge.point(t);
            let found = boundary_distance(source, point);
            assert!(
                (found - distance).abs() <= 1e-9,
                "({}, {}) on {edge:?} is {found} from the boundary, not {distance}",
                point.x,
                point.z
            );
        }
    }
}

fn assert_classified(result: &[CurveRegion2], source: &CurveRegion2, distance: f64) {
    let steps = 60;
    for i in 0..=steps {
        for j in 0..=steps {
            let point = p(
                -2.5 + 7.0 * i as f64 / steps as f64,
                -2.5 + 7.5 * j as f64 / steps as f64,
            );
            let clearance = boundary_distance(source, point);
            if !filled(std::slice::from_ref(source), point) || (clearance - distance).abs() <= 1e-6
            {
                continue;
            }
            assert_eq!(
                filled(result, point),
                clearance > distance,
                "({}, {}) is {clearance} from the boundary",
                point.x,
                point.z
            );
        }
    }
}

fn assert_exact(source: &CurveRegion2, distance: f64) -> Vec<CurveRegion2> {
    let result = inset_curved_region(source, distance, DEFAULT_EPS).unwrap();
    assert_at_distance(&result, source, distance);
    assert_classified(&result, source, distance);
    result
}

fn arcs(regions: &[CurveRegion2]) -> Vec<([f64; 2], f64)> {
    regions
        .iter()
        .flat_map(rings)
        .flatten()
        .filter_map(|edge| match edge {
            CurveEdge2::Arc { center, radius, .. } => Some((*center, *radius)),
            CurveEdge2::Line { .. } => None,
        })
        .collect()
}

fn has_arc(regions: &[CurveRegion2], center: [f64; 2], radius: f64) -> bool {
    arcs(regions).iter().any(|(at, size)| {
        (at[0] - center[0]).abs() <= 1e-9
            && (at[1] - center[1]).abs() <= 1e-9
            && (size - radius).abs() <= 1e-9
    })
}

#[test]
fn a_d_shape_insets_to_the_smaller_d_with_one_concentric_arc() {
    let result = assert_exact(&d_shape(), 0.2);
    assert_eq!(result.len(), 1);
    let expected = 3.8 * 3.6 + PI * 1.8 * 1.8 / 2.0;
    assert!(
        (area(&result) - expected).abs() <= 1e-9,
        "{}",
        area(&result)
    );
    assert!(arcs(&result)
        .iter()
        .all(|(center, radius)| *center == [2.0, 0.0] && (radius - 1.8).abs() <= 1e-12));
    assert!(has_arc(&result, [2.0, 0.0], 1.8));
}

#[test]
fn a_circular_segment_insets_to_the_segment_of_the_smaller_circle() {
    let result = assert_exact(&circular_segment(), 0.1);
    let (radius, chord) = (1.9_f64, 1.1_f64);
    let expected = radius * radius * (chord / radius).acos()
        - chord * (radius * radius - chord * chord).sqrt();
    assert_eq!(result.len(), 1);
    assert!(
        (area(&result) - expected).abs() <= 1e-9,
        "{}",
        area(&result)
    );
    assert_eq!(arcs(&result).len(), 1);
    assert!(has_arc(&result, [0.0, 0.0], radius));
}

#[test]
fn a_reflex_line_arc_join_insets_to_a_round_of_the_thickness() {
    let result = assert_exact(&bumped_square(), 0.25);
    assert_eq!(result.len(), 1);
    assert!(has_arc(&result, [2.0, 4.0], 0.75));
    assert!(has_arc(&result, [3.0, 4.0], 0.25));
    assert!(has_arc(&result, [1.0, 4.0], 0.25));
}

#[test]
fn an_arc_with_radius_below_the_thickness_leaves_a_sharp_corner() {
    let result = assert_exact(&filleted_square(0.1), 0.3);
    assert_eq!(result.len(), 1);
    assert!(arcs(&result).is_empty(), "{result:?}");
    assert!(
        (area(&result) - 3.4 * 3.4).abs() <= 1e-9,
        "{}",
        area(&result)
    );
}

#[test]
fn a_round_hole_grows_by_the_thickness() {
    let result = assert_exact(&square_with_round_hole(), 0.2);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].holes.len(), 1);
    assert!(arcs(&result)
        .iter()
        .all(|(center, radius)| *center == [2.0, 2.0] && (radius - 0.7).abs() <= 1e-12));
    let expected = 3.6 * 3.6 - PI * 0.7 * 0.7;
    assert!(
        (area(&result) - expected).abs() <= 1e-9,
        "{}",
        area(&result)
    );
}

#[test]
fn a_thickness_past_the_half_width_collapses_to_nothing() {
    assert!(inset_curved_region(&d_shape(), 2.5, DEFAULT_EPS)
        .unwrap()
        .is_empty());
}
