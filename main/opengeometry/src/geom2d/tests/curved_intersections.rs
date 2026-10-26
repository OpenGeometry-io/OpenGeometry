use crate::geom2d::curved_boolean2d::{intersections, CurveEdge2};
use crate::geom2d::poly2d::Pt2;
use std::f64::consts::{FRAC_PI_2, PI};

fn reports(hits: &[Pt2], point: Pt2) -> bool {
    hits.iter()
        .any(|hit| (hit.x - point.x).hypot(hit.z - point.z) <= 1e-9)
}

#[test]
fn a_line_that_ends_on_an_arc_near_tangency_reports_its_end() {
    let arc = CurveEdge2::Arc {
        center: [0.0, 0.0],
        radius: 1.0,
        start_angle: 0.0,
        sweep_angle: PI,
    };
    for gap in [1e-12, 1e-9, 1e-7] {
        let line = CurveEdge2::Line {
            from: [-1.0, 1.0 - gap],
            to: [0.0, 1.0 - gap],
        };
        for (a, b) in [(&line, &arc), (&arc, &line)] {
            let hits = intersections(a, b, 1e-6);
            assert!(
                reports(&hits, Pt2::new(0.0, 1.0 - gap)),
                "gap {gap}: {hits:?}"
            );
            for hit in &hits {
                assert!(
                    (hit.x.hypot(hit.z) - 1.0).abs() <= 1e-6,
                    "{hit:?} is off the circle"
                );
                assert!(
                    (hit.z - (1.0 - gap)).abs() <= 1e-6 && hit.x <= 1e-6,
                    "{hit:?} is off the line"
                );
            }
        }
    }
}

#[test]
fn an_arc_that_ends_on_a_nearly_tangent_arc_reports_both_its_ends() {
    let outer = CurveEdge2::Arc {
        center: [0.0, 0.0],
        radius: 1.0,
        start_angle: 0.0,
        sweep_angle: PI,
    };
    let inner = CurveEdge2::Arc {
        center: [0.0, -1e-3],
        radius: 1.0 + 1e-3 - 1e-9,
        start_angle: FRAC_PI_2,
        sweep_angle: -1e-3,
    };
    for (a, b) in [(&outer, &inner), (&inner, &outer)] {
        let hits = intersections(a, b, 1e-6);
        assert!(reports(&hits, inner.point(0.0)), "{hits:?}");
        assert!(reports(&hits, inner.point(1.0)), "{hits:?}");
    }
}
