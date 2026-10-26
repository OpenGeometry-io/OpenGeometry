use crate::geom2d::boolean2d::distance_to_segment;
use crate::geom2d::curved_boolean2d::{winding, CurveEdge2, CurveRegion2};
use crate::geom2d::offset::{offset_path, offset_polyline_regions, OffsetError, OffsetOptions};
use crate::geom2d::poly2d::Pt2;
use crate::geom2d::tests::{curve_area, p, DEFAULT_EPS};

const MITER_LIMIT: f64 = 4.0;
const PATH_EPS: f64 = 1e-9;
const DEFAULT_OPTIONS: OffsetOptions = OffsetOptions {
    bevel: true,
    acute_threshold_degrees: 35.0,
};

fn stroke(path: &[Pt2], width: f64, closed: bool) -> Vec<CurveRegion2> {
    offset_polyline_regions(path, width, closed, MITER_LIMIT, DEFAULT_EPS)
        .unwrap_or_else(|error| panic!("{error:?}"))
}

fn single(regions: Vec<CurveRegion2>) -> CurveRegion2 {
    assert_eq!(regions.len(), 1, "one region");
    regions.into_iter().next().unwrap()
}

fn line_distance(point: Pt2, a: Pt2, b: Pt2) -> f64 {
    let (dx, dz) = (b.x - a.x, b.z - a.z);
    ((point.x - a.x) * dz - (point.z - a.z) * dx).abs() / dx.hypot(dz)
}

fn near(value: f64, target: f64) -> bool {
    (value - target).abs() <= 1e-9 * target.max(1.0)
}

fn segments(path: &[Pt2], closed: bool) -> Vec<(Pt2, Pt2)> {
    let count = if closed { path.len() } else { path.len() - 1 };
    (0..count)
        .map(|index| (path[index], path[(index + 1) % path.len()]))
        .collect()
}

fn anchor(point: Pt2, path: &[Pt2], half: f64) -> bool {
    path.iter()
        .any(|vertex| near((point.x - vertex.x).hypot(point.z - vertex.z), half))
}

fn on_offset_lines(point: Pt2, path: &[Pt2], closed: bool, half: f64) -> usize {
    segments(path, closed)
        .iter()
        .filter(|(a, b)| near(line_distance(point, *a, *b), half))
        .count()
}

fn assert_edges_exact(edges: &[CurveEdge2], path: &[Pt2], closed: bool, half: f64) {
    for edge in edges {
        let CurveEdge2::Line { from, to } = edge else {
            panic!("{edge:?} is not a line");
        };
        let (from, to) = (p(from[0], from[1]), p(to[0], to[1]));
        let middle = p((from.x + to.x) * 0.5, (from.z + to.z) * 0.5);
        let side = segments(path, closed).iter().any(|(a, b)| {
            [from, middle, to]
                .iter()
                .all(|point| near(line_distance(*point, *a, *b), half))
        });
        let joint = path.iter().any(|vertex| {
            [from, to]
                .iter()
                .all(|point| near((point.x - vertex.x).hypot(point.z - vertex.z), half))
        });
        assert!(
            side || joint,
            "{edge:?} is neither an offset line nor a joint"
        );
        for point in [from, to] {
            assert!(
                anchor(point, path, half) || on_offset_lines(point, path, closed, half) >= 2,
                "({}, {}) is neither an anchor nor on two offset lines",
                point.x,
                point.z
            );
        }
    }
}

fn assert_stroke_exact(regions: &[CurveRegion2], path: &[Pt2], closed: bool, width: f64) {
    for region in regions {
        for ring in std::iter::once(&region.outer).chain(&region.holes) {
            assert_edges_exact(ring, path, closed, width * 0.5);
        }
    }
}

fn filled(regions: &[CurveRegion2], point: Pt2) -> bool {
    regions.iter().any(|region| {
        winding(point, &region.outer, DEFAULT_EPS) != 0
            && region
                .holes
                .iter()
                .all(|hole| winding(point, hole, DEFAULT_EPS) == 0)
    })
}

fn star(points: usize, outer: f64, inner: f64) -> Vec<Pt2> {
    (0..points * 2)
        .map(|index| {
            let radius = if index % 2 == 0 { outer } else { inner };
            let angle = index as f64 / (points * 2) as f64 * std::f64::consts::TAU;
            p(angle.cos() * radius, angle.sin() * radius)
        })
        .collect()
}

#[test]
fn a_straight_stroke_is_a_rectangle_offset_by_half_its_width() {
    let path = vec![p(0.0, 0.0), p(2.0, 0.0)];
    let region = single(stroke(&path, 0.3, false));
    assert_eq!(region.outer.len(), 4);
    assert!(region.holes.is_empty());
    assert!((curve_area(std::slice::from_ref(&region)) - 0.6).abs() < 1e-12);
    assert_stroke_exact(&[region], &path, false, 0.3);
}

#[test]
fn an_open_l_is_one_mitred_ring_whose_area_is_width_times_length() {
    let path = vec![p(0.0, 0.0), p(2.0, 0.0), p(2.0, 2.0), p(4.0, 2.0)];
    let region = single(stroke(&path, 0.3, false));
    assert!(region.holes.is_empty());
    assert!((curve_area(std::slice::from_ref(&region)) - 1.8).abs() < 1e-12);
    assert_stroke_exact(&[region], &path, false, 0.3);
}

#[test]
fn a_corner_past_the_miter_limit_is_bevelled() {
    let path = vec![p(0.0, 0.0), p(2.0, 0.0), p(2.0, 2.0), p(4.0, 2.0)];
    let regions = offset_polyline_regions(&path, 0.3, false, 1.2, DEFAULT_EPS).unwrap();
    assert!((curve_area(&regions) - (1.8 - 0.15 * 0.15)).abs() < 1e-12);
    assert_stroke_exact(&regions, &path, false, 0.3);
}

#[test]
fn a_closed_square_loop_has_its_outer_ring_and_one_hole() {
    let path = vec![
        p(0.0, 0.0),
        p(4.0, 0.0),
        p(4.0, 4.0),
        p(0.0, 4.0),
        p(0.0, 0.0),
    ];
    let region = single(stroke(&path, 0.4, true));
    assert_eq!(region.holes.len(), 1);
    assert!((curve_area(std::slice::from_ref(&region)) - (4.4 * 4.4 - 3.6 * 3.6)).abs() < 1e-12);
    let open_loop = &path[..4];
    assert_stroke_exact(&[region], open_loop, true, 0.4);
}

#[test]
fn a_stroke_as_wide_as_its_loop_is_solid() {
    let path = vec![
        p(0.0, 0.0),
        p(1.0, 0.0),
        p(1.0, 1.0),
        p(0.0, 1.0),
        p(0.0, 0.0),
    ];
    let region = single(stroke(&path, 1.0, true));
    assert!(region.holes.is_empty());
    assert!((curve_area(std::slice::from_ref(&region)) - 4.0).abs() < 1e-12);
}

#[test]
fn the_same_path_gives_the_same_stroke() {
    let path = star(24, 6.0, 3.3);
    let baseline = serde_json::to_value(stroke(&path, 0.3, true)).unwrap();
    for _ in 0..4 {
        assert_eq!(
            serde_json::to_value(stroke(&path, 0.3, true)).unwrap(),
            baseline
        );
    }
    assert_stroke_exact(&stroke(&path, 0.3, true), &path, true, 0.3);
}

#[test]
fn a_self_crossing_loop_merges_its_bands_at_the_crossing() {
    let bowtie = vec![
        p(0.0, 0.0),
        p(3.0, 3.0),
        p(3.0, 0.0),
        p(0.0, 3.0),
        p(0.0, 0.0),
    ];
    let regions = stroke(&bowtie, 0.3, true);
    assert!(!regions.is_empty());
    assert!(filled(&regions, p(1.5, 1.5)));
    assert!(!filled(&regions, p(0.6, 1.5)));
    assert_stroke_exact(&regions, &bowtie[..4], true, 0.3);
}

#[test]
fn an_acute_open_corner_is_bevelled_and_stays_near_the_path() {
    let path = vec![p(0.0, 0.0), p(5.0, 0.0), p(2.0, 1.0)];
    let region = single(stroke(&path, 0.5, false));
    assert!(region.holes.is_empty());
    assert!(region.outer.iter().all(|edge| edge.point(0.0).x < 6.0));
    assert_stroke_exact(&[region], &path, false, 0.5);
}

#[test]
fn an_open_path_that_doubles_back_is_one_region() {
    let path = vec![p(0.0, 0.0), p(5.0, 0.0), p(1.0, 0.4)];
    let region = single(stroke(&path, 0.3, false));
    assert!(curve_area(std::slice::from_ref(&region)) > 0.0);
    assert_stroke_exact(&[region], &path, false, 0.3);
}

#[test]
fn a_jog_shorter_than_half_the_width_is_dropped_and_the_end_kept() {
    let path = vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 0.001), p(3.0, 0.0)];
    let region = single(stroke(&path, 0.2, false));
    assert!(region
        .outer
        .iter()
        .all(|edge| (edge.point(0.0).z.abs() - 0.1).abs() < 1e-12));
    assert!((curve_area(std::slice::from_ref(&region)) - 0.6).abs() < 1e-12);
}

#[test]
fn a_routed_duct_run_strokes_with_mitred_bends_and_flat_ends() {
    let run = vec![
        p(0.0, 0.0),
        p(4.0, 0.0),
        p(4.0, 3.0),
        p(6.0, 5.0),
        p(9.0, 5.0),
    ];
    let width = 0.4;
    let regions = offset_polyline_regions(&run, width, false, 4.0, DEFAULT_EPS).unwrap();
    let length: f64 = segments(&run, false)
        .iter()
        .map(|(a, b)| (b.x - a.x).hypot(b.z - a.z))
        .sum();
    let region = single(regions);
    assert!((curve_area(std::slice::from_ref(&region)) - width * length).abs() < 1e-9);
    assert_stroke_exact(&[region], &run, false, width);
}

#[test]
fn a_zero_or_negative_width_or_a_single_point_gives_no_region() {
    let path = vec![p(0.0, 0.0), p(2.0, 0.0)];
    for width in [0.0, -1.0, 3e-6] {
        assert!(
            offset_polyline_regions(&path, width, false, MITER_LIMIT, DEFAULT_EPS)
                .unwrap()
                .is_empty()
        );
    }
    assert!(stroke(&[p(0.0, 0.0)], 0.3, false).is_empty());
}

#[test]
fn a_width_that_is_not_a_finite_number_is_invalid_parameter() {
    let path = vec![p(0.0, 0.0), p(2.0, 0.0)];
    for width in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(matches!(
            offset_polyline_regions(&path, width, false, MITER_LIMIT, DEFAULT_EPS),
            Err(OffsetError::InvalidParameter(_))
        ));
    }
}

#[test]
fn a_path_that_turns_straight_back_strokes_its_overlap_once() {
    let reverse = vec![p(0.0, 0.0), p(2.0, 0.0), p(1.0, 0.0)];
    let region = single(stroke(&reverse, 0.3, false));
    assert!((curve_area(std::slice::from_ref(&region)) - 0.6).abs() < 1e-12);
    assert_stroke_exact(&[region], &reverse, false, 0.3);
    for (path, closed) in [
        (
            vec![
                p(0.0, 0.0),
                p(4.0, 0.0),
                p(4.0, 3.0),
                p(4.0, 1.0),
                p(6.0, 1.0),
            ],
            false,
        ),
        (
            vec![
                p(0.0, 0.0),
                p(4.0, 0.0),
                p(4.0, 4.0),
                p(4.0, 2.0),
                p(0.0, 4.0),
            ],
            true,
        ),
        (
            vec![p(0.0, 0.0), p(3.0, 0.0), p(1.0, 0.0), p(1.0, 2.0)],
            false,
        ),
    ] {
        let regions = stroke(&path, 0.3, closed);
        assert!(!regions.is_empty());
        assert_stroke_exact(&regions, &path, closed, 0.3);
    }
}

#[test]
fn a_loop_wider_than_its_side_a_short_legged_u_and_a_self_crossing_open_path_have_their_true_areas()
{
    let tight = vec![
        p(0.0, 0.0),
        p(1.0, 0.0),
        p(1.0, 1.0),
        p(0.0, 1.0),
        p(0.0, 0.0),
    ];
    let solid = single(stroke(&tight, 1.2, true));
    assert!(solid.holes.is_empty());
    assert!((curve_area(std::slice::from_ref(&solid)) - 4.84).abs() < 1e-9);
    for (path, width, expected) in [
        (
            vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 1.0), p(0.0, 1.0)],
            2.0,
            33.0,
        ),
        (
            vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 0.6), p(0.0, 0.6)],
            1.0,
            16.8,
        ),
    ] {
        let region = single(stroke(&path, width, false));
        assert!((curve_area(std::slice::from_ref(&region)) - expected).abs() < 1e-9);
    }
    let crossing = vec![
        p(0.0, 0.0),
        p(4.0, 0.0),
        p(4.0, 2.0),
        p(2.0, 2.0),
        p(2.0, -2.0),
    ];
    let region = single(stroke(&crossing, 0.4, false));
    assert_eq!(region.holes.len(), 1);
    assert!((curve_area(std::slice::from_ref(&region)) - (0.4 * 12.0 - 0.16)).abs() < 1e-9);
    assert_stroke_exact(&[region], &crossing, false, 0.4);
}

fn path_points(edges: &[CurveEdge2]) -> Vec<Pt2> {
    let mut points: Vec<Pt2> = edges.iter().map(|edge| edge.point(0.0)).collect();
    if let Some(last) = edges.last() {
        points.push(last.point(1.0));
    }
    points
}

fn approx(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn a_line_offsets_to_a_parallel_line() {
    let path = vec![p(0.0, 0.0), p(5.0, 0.0)];
    let offset = offset_path(&path, 1.0, Some(false), DEFAULT_OPTIONS, PATH_EPS);
    let points = path_points(&offset.edges);
    assert_eq!(points.len(), 2);
    assert!(points.iter().all(|point| approx(point.z, 1.0)));
    assert_edges_exact(&offset.edges, &path, false, 1.0);
}

#[test]
fn an_acute_outer_corner_can_be_bevelled() {
    let path = vec![p(0.0, 0.0), p(2.0, 0.0), p(1.0, 0.3)];
    let options = OffsetOptions {
        bevel: true,
        acute_threshold_degrees: 45.0,
    };
    let offset = offset_path(&path, 0.3, Some(false), options, PATH_EPS);
    assert!(path_points(&offset.edges).len() >= 4);
    assert!(!offset.bevelled.is_empty());
    assert_edges_exact(&offset.edges, &path, false, 0.3);
}

#[test]
fn a_closed_offset_stays_closed() {
    let path = vec![
        p(0.0, 0.0),
        p(2.0, 0.0),
        p(2.0, 2.0),
        p(0.0, 2.0),
        p(0.0, 0.0),
    ];
    let offset = offset_path(&path, 0.2, None, DEFAULT_OPTIONS, PATH_EPS);
    assert!(offset.closed);
    let points = path_points(&offset.edges);
    assert!(points.len() >= 5);
    assert!(approx(points[0].x, points[points.len() - 1].x));
    assert!(approx(points[0].z, points[points.len() - 1].z));
    assert_edges_exact(&offset.edges, &path[..4], true, 0.2);
}

#[test]
fn an_acute_inner_corner_is_not_bevelled() {
    let path = vec![
        p(-1.2, -2.4),
        p(0.2, -1.7),
        p(0.8, -0.6),
        p(0.0, -0.3),
        p(2.4, 1.2),
    ];
    let options = OffsetOptions {
        bevel: true,
        acute_threshold_degrees: 90.0,
    };
    let offset = offset_path(&path, 0.45, Some(false), options, PATH_EPS);
    assert!(offset.bevelled.contains(&2));
    assert!(!offset.bevelled.contains(&3));
}

#[test]
fn an_open_inner_corner_above_the_threshold_is_mitred() {
    let path = vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0)];
    let options = OffsetOptions {
        bevel: false,
        acute_threshold_degrees: 35.0,
    };
    let offset = offset_path(&path, -0.2, Some(false), options, PATH_EPS);
    let points = path_points(&offset.edges);
    assert_eq!(points.len(), 3);
    assert!(offset.bevelled.is_empty());
    assert!(approx(points[1].x, 1.2) && approx(points[1].z, -0.2));
    assert_edges_exact(&offset.edges, &path, false, 0.2);
}

#[test]
fn an_open_inner_corner_at_or_below_the_threshold_is_clipped() {
    let path = vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0)];
    let options = OffsetOptions {
        bevel: false,
        acute_threshold_degrees: 90.0,
    };
    let offset = offset_path(&path, -0.2, Some(false), options, PATH_EPS);
    let points = path_points(&offset.edges);
    assert_eq!(points.len(), 4);
    assert!(offset.bevelled.is_empty());
    assert!(approx(points[1].x, 1.0) && approx(points[1].z, -0.2));
    assert!(approx(points[2].x, 1.2) && approx(points[2].z, 0.0));
}

#[test]
fn an_acute_open_inner_corner_is_clipped_at_its_two_anchors() {
    let path = vec![p(0.0, 0.0), p(2.0, 0.0), p(1.0, 1.0), p(3.0, 1.0)];
    let options = OffsetOptions {
        bevel: false,
        acute_threshold_degrees: 90.0,
    };
    let offset = offset_path(&path, 0.5, Some(false), options, PATH_EPS);
    let points = path_points(&offset.edges);
    assert_eq!(points.len(), 5);
    let shift = 0.5 / 2.0_f64.sqrt();
    assert!(approx(points[2].x, 1.0 - shift) && approx(points[2].z, 1.0 - shift));
    assert!(approx(points[3].x, 1.0) && approx(points[3].z, 1.5));
    assert_edges_exact(&offset.edges, &path, false, 0.5);
}

#[test]
fn a_zero_distance_returns_the_path_itself() {
    let path = vec![p(0.0, 0.0), p(2.0, 0.0), p(2.0, 2.0)];
    let offset = offset_path(&path, 0.0, Some(false), DEFAULT_OPTIONS, PATH_EPS);
    let points = path_points(&offset.edges);
    assert_eq!(points.len(), 3);
    assert!(points
        .iter()
        .zip(&path)
        .all(|(a, b)| distance_to_segment(*a, *b, *b) < 1e-12));
}

#[test]
fn a_near_reversal_is_bevelled_and_leaves_no_far_vertex() {
    for (side, bevelled) in [(1e-7, true), (-1e-7, false), (1e-10, false)] {
        let path = vec![p(0.0, 0.0), p(1.0, 0.0), p(0.0, side)];
        let offset = offset_path(&path, 0.2, Some(false), DEFAULT_OPTIONS, PATH_EPS);
        assert_eq!(offset.bevelled.contains(&1), bevelled, "{side}");
        assert!(
            path_points(&offset.edges)
                .iter()
                .all(|point| point.x.abs() < 2.0 && point.z.abs() < 2.0),
            "{side}"
        );
    }
}
