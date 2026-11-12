use super::support::{
    assert_classifies, assert_seam_coverage_gap, cylinder, sphere, volume_at_deflection,
};
use crate::brep::{BrepEnvelope, Frame3};
use crate::math::{add, scale};
use crate::operations::modifying::boolean::dispatch::{boolean_brep, boolean_brep_with_handlers};
use crate::operations::modifying::boolean::types::BooleanOp;
use crate::primitives;
use crate::query::PointClassification::{Inside as In, Outside as Out};
use crate::tessellation::tessellate;

fn cone_and_sphere_with_winding_loop() -> (BrepEnvelope, BrepEnvelope) {
    let accuracy = cylinder("accuracy", 0.0, 1.0, false).accuracy;
    let cone = primitives::cone("cone".into(), Frame3::IDENTITY, 2.0, 2.0, accuracy).unwrap();
    let sphere = primitives::sphere(
        "sphere".into(),
        Frame3 {
            origin: [1.1, 0.0, 0.8],
            ..Frame3::IDENTITY
        },
        0.35,
        accuracy,
    )
    .unwrap();
    (cone, sphere)
}

fn cylinder_and_sphere_with_winding_loop() -> (BrepEnvelope, BrepEnvelope) {
    let accuracy = sphere("accuracy", [0.0; 3], 1.0).accuracy;
    let cylinder =
        primitives::cylinder("cylinder".into(), Frame3::IDENTITY, 1.0, 2.0, accuracy).unwrap();
    let sphere = primitives::sphere(
        "sphere".into(),
        Frame3::from_axis([0.95, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap(),
        0.3,
        accuracy,
    )
    .unwrap();
    (cylinder, sphere)
}

#[test]
fn generic_closed_loop_handles_periodic_winding() {
    let (cone, sphere) = cone_and_sphere_with_winding_loop();
    let result = boolean_brep(&cone, &sphere, BooleanOp::Intersection, "winding".into()).unwrap();
    result.brep.validate().unwrap();
    assert_classifies(
        &result.brep,
        &[
            ([1.0, 0.0, 0.8], In),
            ([0.8, 0.0, 0.8], In),
            ([0.6, 0.0, 0.8], Out),
            ([1.4, 0.0, 0.9], Out),
        ],
    );
    tessellate(&result.brep, 0.01, 2_000_000).unwrap();
    let sphere_volume = 4.0 / 3.0 * std::f64::consts::PI * 0.35_f64.powi(3);
    let inside = volume_at_deflection(&result.brep, 0.002);
    assert!(
        inside > 0.5 * sphere_volume && inside < sphere_volume,
        "{inside} of {sphere_volume}"
    );
}

#[test]
fn generic_closed_loop_subtracts_a_cone_from_a_sphere_with_winding_loop() {
    let (cone, sphere) = cone_and_sphere_with_winding_loop();
    let result = boolean_brep(&sphere, &cone, BooleanOp::Subtraction, "winding".into()).unwrap();
    result.brep.validate().unwrap();
    assert_classifies(
        &result.brep,
        &[
            ([1.4, 0.0, 0.9], In),
            ([1.0, 0.0, 0.8], Out),
            ([0.6, 0.0, 0.8], Out),
        ],
    );
    let intersection =
        boolean_brep(&cone, &sphere, BooleanOp::Intersection, "winding".into()).unwrap();
    let whole = volume_at_deflection(&sphere, 0.002);
    let outside = volume_at_deflection(&result.brep, 0.002);
    let inside = volume_at_deflection(&intersection.brep, 0.002);
    assert!(
        (inside + outside - whole).abs() < 0.01 * whole,
        "{inside} + {outside} != {whole}"
    );
}

#[test]
fn generic_closed_loop_intersects_a_torus_with_a_sphere_with_winding_loop() {
    let accuracy = sphere("accuracy", [0.0; 3], 1.0).accuracy;
    let torus = primitives::torus("torus".into(), Frame3::IDENTITY, 2.0, 0.5, accuracy).unwrap();
    for (centre, axis) in [
        ([2.0, 0.0, 0.45], [0.0, 0.0, 1.0]),
        ([2.45, 0.0, 0.0], [1.0, 0.0, 0.0]),
    ] {
        let sphere = primitives::sphere(
            "sphere".into(),
            Frame3::from_axis(centre, axis, [0.0, 1.0, 0.0]).unwrap(),
            0.2,
            accuracy,
        )
        .unwrap();
        let result =
            boolean_brep(&torus, &sphere, BooleanOp::Intersection, "winding".into()).unwrap();
        result.brep.validate().unwrap();
        assert_classifies(
            &result.brep,
            &[
                (add(centre, scale(axis, -0.1)), In),
                (add(centre, scale(axis, 0.17)), Out),
                (add(centre, scale(axis, -0.3)), Out),
            ],
        );
        let sphere_volume = 4.0 / 3.0 * std::f64::consts::PI * 0.2_f64.powi(3);
        let inside = volume_at_deflection(&result.brep, 0.001);
        assert!(
            inside > 0.5 * sphere_volume && inside < sphere_volume,
            "{centre:?}: {inside} of {sphere_volume}"
        );
    }
}

#[test]
fn sphere_cylinder_loop_winding_the_sphere_uses_the_generic_closed_loop() {
    let (cylinder, sphere) = cylinder_and_sphere_with_winding_loop();
    let in_both = [0.9, 0.0, 1.0];
    let sphere_only = [1.15, 0.0, 1.0];
    let cylinder_only = [0.5, 0.0, 1.0];
    let intersection =
        boolean_brep(&cylinder, &sphere, BooleanOp::Intersection, "i".into()).unwrap();
    assert_classifies(
        &intersection.brep,
        &[(in_both, In), (sphere_only, Out), (cylinder_only, Out)],
    );
    let difference = boolean_brep(&sphere, &cylinder, BooleanOp::Subtraction, "d".into()).unwrap();
    assert_classifies(
        &difference.brep,
        &[(in_both, Out), (sphere_only, In), (cylinder_only, Out)],
    );
    let whole = volume_at_deflection(&sphere, 0.002);
    let inside = volume_at_deflection(&intersection.brep, 0.002);
    let outside = volume_at_deflection(&difference.brep, 0.002);
    assert!(
        (inside + outside - whole).abs() < 0.01 * whole,
        "{inside} + {outside} != {whole}"
    );
}

#[test]
fn generic_closed_loop_lifts_the_host_hole_into_its_face_window() {
    let accuracy = cylinder("accuracy", 0.0, 1.0, false).accuracy;
    let cone = primitives::cone("cone".into(), Frame3::IDENTITY, 2.0, 2.0, accuracy).unwrap();
    let cone_volume = volume_at_deflection(&cone, 0.002);
    for (centre, in_both, sphere_only, cone_only, neither) in [
        (
            [0.0, 0.9, 0.9],
            [0.0, 0.8, 0.9],
            [0.0, 1.2, 0.95],
            [0.0, 0.4, 0.9],
            [0.0, 1.3, 1.1],
        ),
        (
            [0.0, 1.42, 1.0],
            [0.0, 1.196, 0.776],
            [0.0, 1.5, 1.0],
            [0.0, 0.5, 0.8],
            [0.0, 1.8, 1.0],
        ),
    ] {
        let sphere = primitives::sphere(
            "sphere".into(),
            Frame3 {
                origin: centre,
                ..Frame3::IDENTITY
            },
            0.35,
            accuracy,
        )
        .unwrap();
        let sphere_volume = volume_at_deflection(&sphere, 0.002);
        let union = boolean_brep(&cone, &sphere, BooleanOp::Union, "u".into()).unwrap();
        let intersection =
            boolean_brep(&cone, &sphere, BooleanOp::Intersection, "i".into()).unwrap();
        let difference = boolean_brep(&cone, &sphere, BooleanOp::Subtraction, "d".into()).unwrap();
        for (result, expected) in [
            (&union, [In, In, In, Out]),
            (&intersection, [In, Out, Out, Out]),
            (&difference, [Out, Out, In, Out]),
        ] {
            assert_classifies(
                &result.brep,
                &[
                    (in_both, expected[0]),
                    (sphere_only, expected[1]),
                    (cone_only, expected[2]),
                    (neither, expected[3]),
                ],
            );
        }
        let union = volume_at_deflection(&union.brep, 0.002);
        let intersection = volume_at_deflection(&intersection.brep, 0.002);
        let difference = volume_at_deflection(&difference.brep, 0.002);
        assert!(
            (union + intersection - cone_volume - sphere_volume).abs() < 2e-3 * cone_volume,
            "{centre:?}: {union} + {intersection} != {cone_volume} + {sphere_volume}"
        );
        assert!(
            (difference + intersection - cone_volume).abs() < 2e-3 * cone_volume,
            "{centre:?}: {difference} + {intersection} != {cone_volume}"
        );
    }
}

#[test]
fn winding_loop_across_the_host_seam_is_a_coverage_gap() {
    let (cone, sphere_on_cone) = cone_and_sphere_with_winding_loop();
    let (cylinder, sphere_on_cylinder) = cylinder_and_sphere_with_winding_loop();
    for (host, sphere, face) in [
        (&cone, &sphere_on_cone, "cone:lateral"),
        (&cylinder, &sphere_on_cylinder, "cylinder:lateral"),
    ] {
        for operation in [BooleanOp::Union, BooleanOp::Subtraction] {
            assert_seam_coverage_gap(
                boolean_brep(host, sphere, operation, format!("{operation:?}")),
                face,
            );
        }
    }
}

#[test]
fn generic_multiple_closed_loops_keep_a_ring_poking_out_of_a_torus_off_its_seams() {
    let accuracy = sphere("accuracy", [0.0; 3], 1.0).accuracy;
    let host = primitives::torus("host".into(), Frame3::IDENTITY, 3.0, 1.0, accuracy).unwrap();
    let ring = primitives::torus(
        "ring".into(),
        Frame3 {
            origin: [0.0, 3.0, 0.7],
            ..Frame3::IDENTITY
        },
        0.5,
        0.2,
        accuracy,
    )
    .unwrap();
    let ring_only = [0.0, 3.6047, 0.8465];
    let in_both = [0.0, 3.5, 0.7];
    let host_only = [0.0, 3.0, 0.0];
    let neither = [0.0, 3.5, 1.3];
    for (operation, expected) in [
        (BooleanOp::Union, [In, In, In, Out]),
        (BooleanOp::Intersection, [Out, In, Out, Out]),
        (BooleanOp::Subtraction, [Out, Out, In, Out]),
    ] {
        let (result, handlers) =
            boolean_brep_with_handlers(&host, &ring, operation, format!("{operation:?}")).unwrap();
        assert_eq!(handlers, ["generic_multiple_closed_loops_boolean"]);
        result.brep.validate().unwrap();
        assert_classifies(
            &result.brep,
            &[
                (ring_only, expected[0]),
                (in_both, expected[1]),
                (host_only, expected[2]),
                (neither, expected[3]),
            ],
        );
    }
}
