use crate::brep::fine_accuracy;
use crate::brep::{CurveGeometry, Frame3, SurfaceGeometry};
use crate::primitives::internal::{
    arc_edged_extrusion, arc_edged_extrusion_with_holes, linear_extrusion,
};
use crate::primitives::profile::ProfileEdge;
use crate::query::{classify_point, PointClassification};
use crate::tessellation::tessellate;

#[test]
fn linear_extrusion_preserves_planar_holes_and_shared_edges() {
    let outer = vec![[0.0, 0.0], [5.0, 0.0], [5.0, 4.0], [0.0, 4.0]];
    let hole = vec![[1.0, 1.0], [1.0, 3.0], [3.0, 3.0], [3.0, 1.0]];
    let body = linear_extrusion(
        "profile".into(),
        Frame3::IDENTITY,
        outer,
        vec![hole],
        2.5,
        fine_accuracy(),
    )
    .unwrap();

    body.validate().unwrap();
    assert_eq!(body.topology.vertices.len(), 16);
    assert_eq!(body.topology.edges.len(), 24);
    assert_eq!(body.topology.faces.len(), 10);
    assert_eq!(body.topology.shells.len(), 1);
    assert_eq!(body.solids.len(), 1);
    assert_eq!(body.topology.faces[0].trim.holes.len(), 1);
    assert_eq!(body.topology.faces[1].trim.holes.len(), 1);
    assert!(body
        .topology
        .edges
        .iter()
        .all(|edge| edge.twin_halfedge.is_some()));
    let tessellation = tessellate(&body, 0.01, 100_000).unwrap();
    assert!(!tessellation.indices.is_empty());
    assert_eq!(
        tessellation.triangle_face_ids.len(),
        tessellation.indices.len() / 3
    );
}
#[test]
fn arc_edged_extrusion_preserves_cylindrical_sides_for_both_sweep_signs() {
    for sign in [1.0, -1.0] {
        let sweep = sign * std::f64::consts::FRAC_PI_2;
        let outer = 2.0;
        let inner = 1.5;
        let point = |radius: f64, angle: f64| [radius * angle.cos(), radius * angle.sin()];
        let edges = vec![
            ProfileEdge::Arc {
                center: [0.0, 0.0],
                radius: outer,
                start_angle: 0.0,
                sweep_angle: sweep,
            },
            ProfileEdge::Line {
                from: point(outer, sweep),
                to: point(inner, sweep),
            },
            ProfileEdge::Arc {
                center: [0.0, 0.0],
                radius: inner,
                start_angle: sweep,
                sweep_angle: -sweep,
            },
            ProfileEdge::Line {
                from: point(inner, 0.0),
                to: point(outer, 0.0),
            },
        ];
        let body = arc_edged_extrusion(
            format!("arc-strip-{sign}"),
            Frame3::IDENTITY,
            edges,
            3.0,
            fine_accuracy(),
        )
        .unwrap();
        body.validate().unwrap();
        assert_eq!(body.topology.faces.len(), 6);
        assert_eq!(body.topology.shells.len(), 1);
        assert_eq!(body.solids.len(), 1);
        assert_eq!(
            body.geometry
                .surfaces
                .iter()
                .filter(|surface| matches!(surface, SurfaceGeometry::Cylinder { .. }))
                .count(),
            2
        );
        assert_eq!(
            body.geometry
                .curves
                .iter()
                .filter(|curve| matches!(curve, CurveGeometry::Circle { .. }))
                .count(),
            4
        );
        let middle = sweep * 0.5;
        assert_eq!(
            classify_point(&body, [1.75 * middle.cos(), 1.75 * middle.sin(), 1.0],).unwrap(),
            PointClassification::Inside,
        );
        assert_eq!(
            classify_point(&body, [1.0 * middle.cos(), 1.0 * middle.sin(), 1.0],).unwrap(),
            PointClassification::Outside,
        );
        let mesh = tessellate(&body, 0.01, 100_000).unwrap();
        assert!(!mesh.indices.is_empty());
    }
}
#[test]
fn arc_edged_extrusion_preserves_an_analytic_profile_hole() {
    let quarter = std::f64::consts::FRAC_PI_2;
    let outer = vec![
        ProfileEdge::Arc {
            center: [0.0, 0.0],
            radius: 2.0,
            start_angle: 0.0,
            sweep_angle: quarter,
        },
        ProfileEdge::Line {
            from: [0.0, 2.0],
            to: [0.0, 1.0],
        },
        ProfileEdge::Arc {
            center: [0.0, 0.0],
            radius: 1.0,
            start_angle: quarter,
            sweep_angle: -quarter,
        },
        ProfileEdge::Line {
            from: [1.0, 0.0],
            to: [2.0, 0.0],
        },
    ];
    let hole = vec![
        ProfileEdge::Arc {
            center: [1.2, 1.2],
            radius: 0.1,
            start_angle: 0.0,
            sweep_angle: -std::f64::consts::PI,
        },
        ProfileEdge::Arc {
            center: [1.2, 1.2],
            radius: 0.1,
            start_angle: -std::f64::consts::PI,
            sweep_angle: -std::f64::consts::PI,
        },
    ];
    let body = arc_edged_extrusion_with_holes(
        "arc-profile-hole".into(),
        Frame3::IDENTITY,
        outer,
        vec![hole],
        3.0,
        fine_accuracy(),
    )
    .unwrap();
    body.validate().unwrap();
    assert_eq!(body.topology.faces[0].trim.holes.len(), 1);
    assert_eq!(body.topology.faces[1].trim.holes.len(), 1);
    assert_eq!(body.topology.shells.len(), 1);
    assert_eq!(body.solids.len(), 1);
    assert_eq!(
        classify_point(&body, [1.2, 1.2, 1.0]).unwrap(),
        PointClassification::Outside
    );
    assert_eq!(
        classify_point(&body, [1.4, 1.0, 1.0]).unwrap(),
        PointClassification::Inside
    );
    assert!(!tessellate(&body, 0.01, 100_000).unwrap().indices.is_empty());
}
#[test]
fn arc_edged_extrusion_rejects_holes_outside_or_crossing_its_profile() {
    let quarter = std::f64::consts::FRAC_PI_2;
    let outer = vec![
        ProfileEdge::Arc {
            center: [0.0, 0.0],
            radius: 2.0,
            start_angle: 0.0,
            sweep_angle: quarter,
        },
        ProfileEdge::Line {
            from: [0.0, 2.0],
            to: [0.0, 1.0],
        },
        ProfileEdge::Arc {
            center: [0.0, 0.0],
            radius: 1.0,
            start_angle: quarter,
            sweep_angle: -quarter,
        },
        ProfileEdge::Line {
            from: [1.0, 0.0],
            to: [2.0, 0.0],
        },
    ];
    let square = |x0, y0, x1, y1| {
        let points = [[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
        (0..4)
            .map(|index| ProfileEdge::Line {
                from: points[index],
                to: points[(index + 1) % 4],
            })
            .collect::<Vec<_>>()
    };
    for hole in [square(3.0, 3.0, 3.2, 3.2), square(1.9, 0.1, 2.1, 0.3)] {
        assert!(arc_edged_extrusion_with_holes(
            "invalid-arc-hole".into(),
            Frame3::IDENTITY,
            outer.clone(),
            vec![hole],
            3.0,
            fine_accuracy(),
        )
        .is_err());
    }
}
#[test]
fn arc_edged_extrusion_rejects_exact_line_arc_crossings_and_overlap() {
    let crossing = vec![
        ProfileEdge::Arc {
            center: [0.0, 0.0],
            radius: 1.0,
            start_angle: 0.0,
            sweep_angle: std::f64::consts::PI,
        },
        ProfileEdge::Line {
            from: [-1.0, 0.0],
            to: [0.0, 1.2],
        },
        ProfileEdge::Line {
            from: [0.0, 1.2],
            to: [1.0, 0.0],
        },
    ];
    assert!(arc_edged_extrusion(
        "crossing".into(),
        Frame3::IDENTITY,
        crossing,
        2.0,
        fine_accuracy(),
    )
    .is_err());
    let overlapping = vec![
        ProfileEdge::Arc {
            center: [0.0, 0.0],
            radius: 1.0,
            start_angle: 0.0,
            sweep_angle: std::f64::consts::PI,
        },
        ProfileEdge::Arc {
            center: [0.0, 0.0],
            radius: 1.0,
            start_angle: std::f64::consts::PI,
            sweep_angle: -std::f64::consts::FRAC_PI_2,
        },
        ProfileEdge::Line {
            from: [0.0, 1.0],
            to: [1.0, 0.0],
        },
    ];
    assert!(arc_edged_extrusion(
        "overlap".into(),
        Frame3::IDENTITY,
        overlapping,
        2.0,
        fine_accuracy(),
    )
    .is_err());
}
#[test]
fn arc_edged_extrusion_accepts_nonradial_join_caps() {
    let outer_end = std::f64::consts::FRAC_PI_2;
    let inner_start = outer_end - 0.1;
    let inner_end = 0.1;
    let point = |radius: f64, angle: f64| [radius * angle.cos(), radius * angle.sin()];
    let profile = vec![
        ProfileEdge::Arc {
            center: [0.0, 0.0],
            radius: 2.0,
            start_angle: 0.0,
            sweep_angle: outer_end,
        },
        ProfileEdge::Line {
            from: point(2.0, outer_end),
            to: point(1.5, inner_start),
        },
        ProfileEdge::Arc {
            center: [0.0, 0.0],
            radius: 1.5,
            start_angle: inner_start,
            sweep_angle: inner_end - inner_start,
        },
        ProfileEdge::Line {
            from: point(1.5, inner_end),
            to: point(2.0, 0.0),
        },
    ];
    let body = arc_edged_extrusion(
        "curved-join".into(),
        Frame3::IDENTITY,
        profile,
        2.4,
        fine_accuracy(),
    )
    .unwrap();
    body.validate().unwrap();
    assert_eq!(body.topology.faces.len(), 6);
    assert_eq!(body.solids.len(), 1);
}
#[test]
fn linear_extrusion_rejects_invalid_profile_relationships() {
    let square = vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]];
    let crossing = vec![[1.0, 1.0], [3.0, 3.0], [1.0, 3.0], [3.0, 1.0]];
    assert!(linear_extrusion(
        "crossing".into(),
        Frame3::IDENTITY,
        crossing,
        Vec::new(),
        1.0,
        fine_accuracy(),
    )
    .is_err());
    let outside = vec![[3.0, 1.0], [5.0, 1.0], [5.0, 2.0], [3.0, 2.0]];
    assert!(linear_extrusion(
        "outside".into(),
        Frame3::IDENTITY,
        square.clone(),
        vec![outside],
        1.0,
        fine_accuracy(),
    )
    .is_err());
    let first = vec![[0.5, 0.5], [2.5, 0.5], [2.5, 2.5], [0.5, 2.5]];
    let second = vec![[1.5, 1.5], [3.0, 1.5], [3.0, 3.0], [1.5, 3.0]];
    assert!(linear_extrusion(
        "overlap".into(),
        Frame3::IDENTITY,
        square,
        vec![first, second],
        1.0,
        fine_accuracy(),
    )
    .is_err());
}
