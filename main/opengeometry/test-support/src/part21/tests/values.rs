use super::base::{base, real};
use super::{appended, assert_close, edited, parsed};
use crate::part21::{CurveRadii, EdgeSupport, ProductCounts, SolidEntry};

#[test]
fn curve_point_follows_circle_and_ellipse_parameterisation() {
    let base = base();
    let document = parsed(&base.text);
    let circle = document.curve_point(base.cylinder.lower.circle, 0.25);
    assert_close(circle.unwrap(), [0.0, 0.0, -1.0]);
    let ellipse = |parameter: f64| document.curve_point(base.lens.ellipse, parameter).unwrap();
    assert_close(ellipse(0.25), [10.0, 1.0, 0.0]);
    assert_close(ellipse(0.5), [8.0, 0.0, 0.0]);
}

#[test]
fn void_map_counts_each_solids_cavity_shells() {
    let base = base();
    let solids = parsed(&base.text).solids().unwrap();
    let with = |voids| SolidEntry {
        voids,
        with_voids: true,
    };
    let manifold = SolidEntry {
        voids: 0,
        with_voids: false,
    };
    assert_eq!(solids, [with(1), manifold]);
    let second = base.entities + 1;
    let from = format!("(#{})", base.oriented_shell);
    let to = format!("(#{},#{second})", base.oriented_shell);
    let text = edited(&base.text, base.solids[0], &from, &to);
    let expression = format!("ORIENTED_CLOSED_SHELL('',*,#{},.T.)", base.lens.shell);
    let solids = parsed(&appended(&text, &expression)).solids().unwrap();
    assert_eq!(solids, [with(2), manifold]);
}

#[test]
fn kind_counts_agree_with_count_for_every_kind() {
    let document = parsed(&base().text);
    let counts = document.kind_counts();
    for (kind, count) in &counts {
        assert_eq!(document.count(kind), *count, "{kind}");
    }
    assert_eq!(counts["EDGE_CURVE"], 5);
    assert_eq!(counts["ADVANCED_FACE"], 7);
    assert_eq!(counts["LENGTH_MEASURE"], 1);
    assert_eq!(counts["SI_UNIT"], 3);
}

#[test]
fn typed_accessors_read_points_and_directions_by_dimension() {
    let base = base();
    let document = parsed(&base.text);
    let points3 = document.points3().unwrap();
    let points2 = document.points2().unwrap();
    let directions3 = document.directions3().unwrap();
    let directions2 = document.directions2().unwrap();
    assert_eq!(points3[&base.lens.vertex_point], [12.0, 0.0, 0.0]);
    assert_eq!(
        points2.len() + points3.len(),
        document.count("CARTESIAN_POINT")
    );
    assert_eq!(
        directions3[&base.cylinder.lower_cap_normal],
        [0.0, -1.0, 0.0]
    );
    assert!(directions2
        .values()
        .all(|value| value[0].hypot(value[1]) == 1.0));
    assert_eq!(
        directions2.len() + directions3.len(),
        document.count("DIRECTION")
    );
    let controls = document.spline_control_points().unwrap();
    assert_eq!(controls.len(), 4);
    assert!(controls.iter().all(|id| points3.contains_key(id)));
}

#[test]
fn edge_supports_name_each_edges_curve_radii_and_pcurves() {
    let base = base();
    let (cylinder, lens) = (&base.cylinder, &base.lens);
    let expected = [
        (
            cylinder.lower.edge,
            cylinder.lower.circle,
            "CIRCLE",
            vec![1.0],
            true,
        ),
        (
            cylinder.upper.edge,
            cylinder.upper.circle,
            "CIRCLE",
            vec![1.0],
            true,
        ),
        (cylinder.seam, cylinder.seam_line, "LINE", vec![], true),
        (lens.edge, lens.ellipse, "ELLIPSE", vec![2.0, 1.0], false),
        (
            base.spline.edge,
            base.spline.curve,
            "B_SPLINE_CURVE_WITH_KNOTS",
            vec![],
            false,
        ),
    ]
    .map(|(edge, curve, kind, radii, with_pcurves)| EdgeSupport {
        edge,
        curve,
        kind: kind.into(),
        radii,
        with_pcurves,
    });
    assert_eq!(parsed(&base.text).edge_supports().unwrap(), expected);
}

#[test]
fn curve_radii_cover_3d_and_2d_conics() {
    let base = base();
    let cylinder = &base.cylinder;
    let expected = [
        (cylinder.lower.circle, "CIRCLE", 3, vec![1.0]),
        (cylinder.lower.conic, "CIRCLE", 2, vec![1.0]),
        (cylinder.upper.circle, "CIRCLE", 3, vec![1.0]),
        (cylinder.upper.conic, "CIRCLE", 2, vec![1.0]),
        (base.lens.ellipse, "ELLIPSE", 3, vec![2.0, 1.0]),
    ]
    .map(|(curve, kind, dimension, radii)| CurveRadii {
        curve,
        kind: kind.into(),
        dimension,
        radii,
    });
    assert_eq!(parsed(&base.text).curve_radii().unwrap(), expected);
}

#[test]
fn product_counts_follow_each_products_shells() {
    let base = base();
    let expected = [
        ProductCounts {
            product: base.products.products[0],
            solids: 1,
            faces: 5,
            edges: 4,
            voids: 1,
            pcurveless_edges: 1,
            fitted_curves: 0,
        },
        ProductCounts {
            product: base.products.products[1],
            solids: 1,
            faces: 2,
            edges: 1,
            voids: 0,
            pcurveless_edges: 1,
            fitted_curves: 1,
        },
    ];
    assert_eq!(parsed(&base.text).product_counts().unwrap(), expected);
}

#[test]
fn reversed_conic_pcurve_is_reported_reversed() {
    let base = base();
    let figures = |text: &str| {
        let report = parsed(text).parameter_direction_report().unwrap();
        [
            report.assessed,
            report.aligned,
            report.reversed,
            report.conic_assessed,
            report.conic_aligned,
            report.conic_reversed,
        ]
    };
    assert_eq!(figures(&base.text), [2, 2, 0, 4, 3, 1]);
    let normal = base.cylinder.lower_cap_normal;
    let flipped = edited(&base.text, normal, &real(-1.0), &real(1.0));
    assert_eq!(figures(&flipped), [2, 2, 0, 4, 4, 0]);
}
