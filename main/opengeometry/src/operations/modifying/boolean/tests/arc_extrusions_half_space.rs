use super::support::volume_at_deflection;
use crate::brep::{
    fine_accuracy, BrepEnvelope, CurveGeometry, EdgeGeometry, Frame3, SurfaceGeometry,
};
use crate::math::{dot, Point3};
use crate::operations::modifying::boolean::batch::subtract_planar_cutters_with_handlers;
use crate::operations::modifying::boolean::dispatch::boolean_brep_with_handlers;
use crate::operations::modifying::boolean::types::BooleanOp;
use crate::primitives::{self, ProfileEdge};
use crate::query::{classify_point, PointClassification};
use std::f64::consts::PI;

const HANDLER: &str = "subtract_half_space_from_arc_extrusion";

fn along_x(origin: Point3) -> Frame3 {
    Frame3::from_axis(origin, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap()
}

fn along_y(origin: Point3) -> Frame3 {
    Frame3::from_axis(origin, [0.0, 1.0, 0.0], [-1.0, 0.0, 0.0]).unwrap()
}

fn half_round_edges() -> Vec<ProfileEdge> {
    vec![
        ProfileEdge::Arc {
            center: [0.0, 0.0],
            radius: 0.1,
            start_angle: PI,
            sweep_angle: PI,
        },
        ProfileEdge::Line {
            from: [0.1, 0.0],
            to: [0.09, 0.0],
        },
        ProfileEdge::Arc {
            center: [0.0, 0.0],
            radius: 0.09,
            start_angle: 0.0,
            sweep_angle: -PI,
        },
        ProfileEdge::Line {
            from: [-0.09, 0.0],
            to: [-0.1, 0.0],
        },
    ]
}

fn full_round_edges() -> Vec<ProfileEdge> {
    vec![
        ProfileEdge::Arc {
            center: [0.0, 0.0],
            radius: 0.1,
            start_angle: 0.0,
            sweep_angle: PI,
        },
        ProfileEdge::Arc {
            center: [0.0, 0.0],
            radius: 0.1,
            start_angle: PI,
            sweep_angle: PI,
        },
    ]
}

fn ogee_edges() -> Vec<ProfileEdge> {
    vec![
        ProfileEdge::Line {
            from: [-0.1, 0.0],
            to: [0.0, 0.0],
        },
        ProfileEdge::Arc {
            center: [0.0, 0.05],
            radius: 0.05,
            start_angle: -PI / 2.0,
            sweep_angle: PI / 2.0,
        },
        ProfileEdge::Arc {
            center: [0.1, 0.05],
            radius: 0.05,
            start_angle: PI,
            sweep_angle: -PI / 2.0,
        },
        ProfileEdge::Line {
            from: [0.1, 0.1],
            to: [-0.1, 0.1],
        },
        ProfileEdge::Line {
            from: [-0.1, 0.1],
            to: [-0.1, 0.0],
        },
    ]
}

fn extrusion(frame: Frame3, edges: Vec<ProfileEdge>, length: f64) -> BrepEnvelope {
    primitives::arc_edged_extrusion("gutter".into(), frame, edges, length, fine_accuracy()).unwrap()
}

fn prism_cutter(id: &str, ring: Vec<[f64; 2]>) -> BrepEnvelope {
    primitives::linear_extrusion(
        id.into(),
        Frame3 {
            origin: [0.0, 0.0, -1.0],
            ..Frame3::IDENTITY
        },
        ring,
        vec![],
        2.0,
        fine_accuracy(),
    )
    .unwrap()
}

fn slanted_prism(degrees: f64) -> BrepEnvelope {
    let t = degrees.to_radians();
    let along = [-t.sin(), t.cos()];
    let at = |s: f64, away: f64| [3.0 + s * along[0] + away, s * along[1]];
    prism_cutter(
        "slanted-prism",
        vec![at(-2.0, 0.0), at(2.0, 0.0), at(2.0, 2.0), at(-2.0, 2.0)],
    )
}

fn slanted_cuboid(degrees: f64) -> BrepEnvelope {
    let t = degrees.to_radians();
    let x = [t.cos(), t.sin(), 0.0];
    let y = [-t.sin(), t.cos(), 0.0];
    primitives::cuboid(
        "slanted-cuboid".into(),
        Frame3 {
            origin: [3.0 - 5.0 * y[0], -5.0 * y[1], -1.0],
            x,
            y,
            z: [0.0, 0.0, 1.0],
        },
        [10.0, 10.0, 2.0],
        fine_accuracy(),
    )
    .unwrap()
}

fn cut_ellipses(brep: &BrepEnvelope) -> Vec<(Point3, f64, f64)> {
    brep.topology
        .edges
        .iter()
        .filter_map(|edge| match &edge.geometry {
            EdgeGeometry::Curve { curve, .. } => match &brep.geometry.curves[*curve as usize] {
                CurveGeometry::Ellipse {
                    frame,
                    major_radius,
                    minor_radius,
                } => Some((frame.origin, *major_radius, *minor_radius)),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

fn assert_exact_cut(brep: &BrepEnvelope, radii: &[f64], axis_to_normal: f64) {
    brep.validate().unwrap();
    assert_eq!(brep.solids.len(), 1);
    assert!(brep.topology.shells.iter().all(|shell| shell.is_closed));
    assert!(brep.geometry.surfaces.iter().all(|surface| matches!(
        surface,
        SurfaceGeometry::Plane { .. } | SurfaceGeometry::Cylinder { .. }
    )));
    let cylinders = brep
        .topology
        .faces
        .iter()
        .filter(|face| {
            matches!(
                brep.geometry.surfaces[face.surface as usize],
                SurfaceGeometry::Cylinder { .. }
            )
        })
        .count();
    assert!(cylinders >= radii.len(), "{cylinders} cylinder faces");
    let ellipses = cut_ellipses(brep);
    for radius in radii {
        assert!(
            ellipses
                .iter()
                .any(|(_, major, minor)| (minor - radius).abs() <= 1e-12
                    && (major - radius / axis_to_normal.cos()).abs() <= 1e-12),
            "no ellipse of minor radius {radius} in {ellipses:?}"
        );
    }
}

fn kept_volume(host: &BrepEnvelope, cutter: &BrepEnvelope) -> f64 {
    let (result, handlers) =
        boolean_brep_with_handlers(host, cutter, BooleanOp::Subtraction, "cut".into())
            .unwrap_or_else(|error| panic!("{error:?}"));
    assert_eq!(handlers, [HANDLER]);
    volume_at_deflection(&result.brep, 1e-4).abs()
}

#[test]
fn half_round_gutter_cut_by_a_slanted_plane_keeps_its_cylinders_and_gets_ellipse_trims() {
    let host = extrusion(along_x([0.0; 3]), half_round_edges(), 4.0);
    let area = PI * (0.1f64.powi(2) - 0.09f64.powi(2)) / 2.0;
    for degrees in [30.0, 45.0, 60.0] {
        for cutter in [slanted_prism(degrees), slanted_cuboid(degrees)] {
            let (result, handlers) =
                boolean_brep_with_handlers(&host, &cutter, BooleanOp::Subtraction, "cut".into())
                    .unwrap_or_else(|error| panic!("{degrees}: {error:?}"));
            assert_eq!(handlers, [HANDLER]);
            assert_exact_cut(&result.brep, &[0.1, 0.09], f64::to_radians(degrees));
            let volume = volume_at_deflection(&result.brep, 1e-4).abs();
            assert!((volume - 3.0 * area).abs() <= 1e-3 * 3.0 * area, "{volume}");
            assert_eq!(
                classify_point(&result.brep, [2.9, 0.0, -0.095]).unwrap(),
                PointClassification::Inside
            );
            assert_eq!(
                classify_point(&result.brep, [3.1, 0.0, -0.095]).unwrap(),
                PointClassification::Outside
            );
        }
    }
}

#[test]
fn full_round_extrusion_cut_by_a_slanted_plane_keeps_its_cylinders_and_gets_an_ellipse_trim() {
    let host = extrusion(along_x([0.0; 3]), full_round_edges(), 4.0);
    let area = PI * 0.01;
    for degrees in [30.0, 45.0, 60.0] {
        for cutter in [slanted_prism(degrees), slanted_cuboid(degrees)] {
            let (result, handlers) =
                boolean_brep_with_handlers(&host, &cutter, BooleanOp::Subtraction, "cut".into())
                    .unwrap_or_else(|error| panic!("{degrees}: {error:?}"));
            assert_eq!(handlers, [HANDLER]);
            assert_exact_cut(&result.brep, &[0.1], f64::to_radians(degrees));
            let volume = volume_at_deflection(&result.brep, 1e-4).abs();
            assert!((volume - 3.0 * area).abs() <= 1e-3 * 3.0 * area, "{volume}");
        }
    }
}

#[test]
fn primitive_cylinder_cut_by_a_slanted_prism_gets_an_ellipse_trim() {
    let host =
        primitives::cylinder("rod".into(), along_x([0.0; 3]), 0.1, 4.0, fine_accuracy()).unwrap();
    for degrees in [30.0, 45.0, 60.0] {
        let (result, handlers) = boolean_brep_with_handlers(
            &host,
            &slanted_prism(degrees),
            BooleanOp::Subtraction,
            "cut".into(),
        )
        .unwrap_or_else(|error| panic!("{degrees}: {error:?}"));
        assert_eq!(handlers, [HANDLER]);
        assert_exact_cut(&result.brep, &[0.1], f64::to_radians(degrees));
        let volume = volume_at_deflection(&result.brep, 1e-4).abs();
        assert!((volume - 0.03 * PI).abs() <= 3e-5 * PI, "{volume}");
    }
}

#[test]
fn ogee_cut_at_opposite_slants_keeps_the_volume_a_plane_through_its_middle_gives() {
    let host = extrusion(along_x([0.0; 3]), ogee_edges(), 4.0);
    let whole = volume_at_deflection(&host, 1e-4).abs();
    for degrees in [30.0, 45.0, 60.0] {
        let left = kept_volume(&host, &slanted_prism(degrees));
        let right = kept_volume(&host, &slanted_prism(-degrees));
        assert!(
            (left + right - 1.5 * whole).abs() <= 1e-3 * whole,
            "{left} {right}"
        );
    }
}

fn corner_gutters() -> [BrepEnvelope; 2] {
    [
        extrusion(along_x([-1.0, -0.15, 0.0]), half_round_edges(), 4.0),
        extrusion(along_y([-0.15, -1.0, 0.0]), half_round_edges(), 4.0),
    ]
}

#[test]
fn two_gutters_mitred_on_the_corner_bisector_share_one_ellipse_on_the_mitre() {
    let [along_x_gutter, along_y_gutter] = corner_gutters();
    let x_cutter = prism_cutter(
        "mitre-x",
        vec![[-2.0, -2.0], [2.0, 2.0], [-2.0, 2.0], [-6.0, -2.0]],
    );
    let y_cutter = prism_cutter(
        "mitre-y",
        vec![[-2.0, -2.0], [2.0, 2.0], [2.0, -2.0], [-2.0, -6.0]],
    );
    let mut mitres = Vec::new();
    for (host, cutter) in [(&along_x_gutter, &x_cutter), (&along_y_gutter, &y_cutter)] {
        let (result, handlers) =
            boolean_brep_with_handlers(host, cutter, BooleanOp::Subtraction, "mitre".into())
                .unwrap_or_else(|error| panic!("{error:?}"));
        assert_eq!(handlers, [HANDLER]);
        assert_exact_cut(&result.brep, &[0.1, 0.09], PI / 4.0);
        let area = PI * (0.1f64.powi(2) - 0.09f64.powi(2)) / 2.0;
        let volume = volume_at_deflection(&result.brep, 1e-4).abs();
        assert!(
            (volume - 3.15 * area).abs() <= 1e-3 * 3.15 * area,
            "{volume}"
        );
        mitres.push(cut_ellipses(&result.brep));
    }
    for (center, major, minor) in &mitres[0] {
        assert!((dot(*center, [1.0, -1.0, 0.0])).abs() <= 1e-12);
        assert!(mitres[1].iter().any(|(other, other_major, other_minor)| {
            (0..3).all(|axis| (center[axis] - other[axis]).abs() <= 1e-12)
                && (major - other_major).abs() <= 1e-12
                && (minor - other_minor).abs() <= 1e-12
        }));
    }
}

#[test]
fn a_gutter_mitred_at_both_ends_in_one_batch_keeps_its_cylinders() {
    let [host, _] = corner_gutters();
    let near = prism_cutter(
        "mitre-near",
        vec![[-2.0, -2.0], [2.0, 2.0], [-2.0, 2.0], [-6.0, -2.0]],
    );
    let far = prism_cutter(
        "mitre-far",
        vec![[0.0, 2.0], [4.0, -2.0], [8.0, -2.0], [4.0, 2.0]],
    );
    let (result, handlers) =
        subtract_planar_cutters_with_handlers(&host, &[near, far], "mitres".into())
            .unwrap_or_else(|error| panic!("{error:?}"));
    assert_eq!(handlers, ["subtract_planar_cutters", HANDLER, HANDLER]);
    assert_exact_cut(&result.brep, &[0.1, 0.09], PI / 4.0);
    assert_eq!(cut_ellipses(&result.brep).len(), 4);
    let area = PI * (0.1f64.powi(2) - 0.09f64.powi(2)) / 2.0;
    let volume = volume_at_deflection(&result.brep, 1e-4).abs();
    assert!((volume - 2.3 * area).abs() <= 1e-3 * 2.3 * area, "{volume}");
}
