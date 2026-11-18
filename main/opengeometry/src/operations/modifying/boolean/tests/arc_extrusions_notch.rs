use super::arc_extrusions_half_space::{
    along_x, cut_ellipses, extrusion, half_round_edges, slanted_block, slanted_cuboid,
    slanted_prism, HANDLER,
};
use super::support::volume_at_deflection;
use crate::brep::{fine_accuracy, BrepEnvelope, SurfaceGeometry};
use crate::operations::modifying::boolean::dispatch::boolean_brep_with_handlers;
use crate::operations::modifying::boolean::types::BooleanOp;
use crate::primitives;
use crate::query::{classify_point, PointClassification};
use std::f64::consts::PI;

fn rod() -> BrepEnvelope {
    primitives::cylinder("rod".into(), along_x([0.0; 3]), 0.1, 4.0, fine_accuracy()).unwrap()
}

fn assert_notch(host: &BrepEnvelope, tool: &BrepEnvelope, radii: &[f64], degrees: f64) -> f64 {
    let (result, handlers) =
        boolean_brep_with_handlers(host, tool, BooleanOp::Subtraction, "notch".into())
            .unwrap_or_else(|error| panic!("{degrees}: {error:?}"));
    assert_eq!(handlers, [HANDLER]);
    let brep = &result.brep;
    brep.validate().unwrap();
    assert_eq!(brep.solids.len(), 1);
    assert!(brep.topology.shells.iter().all(|shell| shell.is_closed));
    assert!(brep.geometry.surfaces.iter().all(|surface| matches!(
        surface,
        SurfaceGeometry::Plane { .. } | SurfaceGeometry::Cylinder { .. }
    )));
    let ellipses = cut_ellipses(brep);
    let slant = degrees.to_radians().cos();
    for radius in radii {
        assert!(
            ellipses
                .iter()
                .any(|(_, major, minor)| (minor - radius).abs() <= 1e-12
                    && (major - radius / slant).abs() <= 1e-12),
            "no rim ellipse of minor radius {radius} in {ellipses:?}"
        );
    }
    volume_at_deflection(host, 1e-5).abs() - volume_at_deflection(brep, 1e-5).abs()
}

#[test]
fn slanted_cuboid_minus_a_rod_keeps_a_blind_round_hole_with_an_ellipse_rim() {
    for degrees in [30.0, 45.0, 60.0] {
        let cuboid = slanted_cuboid(degrees);
        let removed = assert_notch(&cuboid, &rod(), &[0.1], degrees);
        let expected = PI * 0.01;
        assert!((removed - expected).abs() <= 2e-3 * expected, "{removed}");
        let (result, _) =
            boolean_brep_with_handlers(&cuboid, &rod(), BooleanOp::Subtraction, "notch".into())
                .unwrap();
        assert_eq!(
            classify_point(&result.brep, [3.5, 0.0, 0.0]).unwrap(),
            PointClassification::Outside
        );
        assert_eq!(
            classify_point(&result.brep, [3.5, 0.5, 0.0]).unwrap(),
            PointClassification::Inside
        );
    }
}

#[test]
fn slanted_slab_minus_a_rod_keeps_a_through_hole_with_two_ellipse_rims() {
    for degrees in [30.0, 45.0, 60.0] {
        let removed = assert_notch(&slanted_block(degrees, 0.25), &rod(), &[0.1], degrees);
        let expected = PI * 0.01 * 0.25 / degrees.to_radians().cos();
        assert!((removed - expected).abs() <= 2e-3 * expected, "{removed}");
    }
}

#[test]
fn slanted_prism_minus_a_half_round_gutter_keeps_a_gutter_shaped_pocket() {
    let gutter = extrusion(along_x([0.0; 3]), half_round_edges(), 4.0);
    let area = PI * (0.1f64.powi(2) - 0.09f64.powi(2)) / 2.0;
    for degrees in [30.0, 45.0, 60.0] {
        let removed = assert_notch(&slanted_prism(degrees), &gutter, &[0.1, 0.09], degrees);
        assert!((removed - area).abs() <= 2e-3 * area, "{removed}");
    }
}
