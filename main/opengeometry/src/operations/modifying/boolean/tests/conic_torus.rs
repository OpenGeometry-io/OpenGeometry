use super::support::{sphere, volume};
use crate::brep::{FaceRole, Frame3, GeometryError, SurfaceGeometry};
use crate::operations::modifying::boolean::dispatch::boolean_brep;
use crate::operations::modifying::boolean::types::BooleanOp;
use crate::primitives;
use crate::tessellation::tessellate;

#[test]
fn concentric_torus_booleans_preserve_exact_cavity_supports() {
    let accuracy = sphere("accuracy", [0.0; 3], 1.0).accuracy;
    let host = primitives::torus("host".into(), Frame3::IDENTITY, 3.0, 1.0, accuracy).unwrap();
    let cutter = primitives::torus("cutter".into(), Frame3::IDENTITY, 3.0, 0.4, accuracy).unwrap();
    let subtraction =
        boolean_brep(&host, &cutter, BooleanOp::Subtraction, "torus-shell".into()).unwrap();
    subtraction.brep.validate().unwrap();
    assert_eq!(subtraction.brep.solids.len(), 1);
    assert_eq!(subtraction.brep.solids[0].cavity_shells.len(), 1);
    assert!(subtraction
        .brep
        .topology
        .faces
        .iter()
        .any(|face| { face.provenance.role == FaceRole::Cut && face.provenance.reversed }));
    assert!(subtraction
        .brep
        .geometry
        .surfaces
        .iter()
        .all(|surface| matches!(surface, SurfaceGeometry::Torus { .. })));
    tessellate(&subtraction.brep, 0.05, 2_000_000).unwrap();

    let intersection =
        boolean_brep(&host, &cutter, BooleanOp::Intersection, "torus-core".into()).unwrap();
    assert_eq!(intersection.brep.topology.faces.len(), 1);
    assert!(matches!(
        intersection.brep.geometry.surfaces[0],
        SurfaceGeometry::Torus {
            minor_radius: 0.4,
            ..
        }
    ));

    let coincident =
        boolean_brep(&host, &host, BooleanOp::Subtraction, "empty-torus".into()).unwrap();
    assert!(coincident.report.coincident);
    assert!(coincident.brep.solids.is_empty());
}

#[test]
fn sphere_torus_containment_is_exact_in_both_directions() {
    let accuracy = sphere("accuracy", [0.0; 3], 1.0).accuracy;
    let torus = primitives::torus("torus".into(), Frame3::IDENTITY, 3.0, 1.0, accuracy).unwrap();
    let small_sphere = sphere("small", [3.0, 0.0, 0.0], 0.25);
    let torus_cut = boolean_brep(
        &torus,
        &small_sphere,
        BooleanOp::Subtraction,
        "torus-cut".into(),
    )
    .unwrap();
    assert_eq!(torus_cut.brep.solids[0].cavity_shells.len(), 1);
    assert!(torus_cut.brep.geometry.surfaces.iter().any(
        |surface| matches!(surface, SurfaceGeometry::Sphere { radius, .. } if *radius == 0.25)
    ));

    let large_sphere = sphere("large", [0.0; 3], 5.0);
    let sphere_cut = boolean_brep(
        &large_sphere,
        &torus,
        BooleanOp::Subtraction,
        "sphere-cut".into(),
    )
    .unwrap();
    sphere_cut.brep.validate().unwrap();
    assert_eq!(sphere_cut.brep.solids[0].cavity_shells.len(), 1);
    assert!(sphere_cut
        .brep
        .topology
        .faces
        .iter()
        .any(|face| face.provenance.role == FaceRole::Cut));
}

#[test]
fn coaxial_conic_containment_builds_exact_cavities_and_ownership() {
    let accuracy = sphere("accuracy", [0.0; 3], 1.0).accuracy;
    let host =
        primitives::frustum("host".into(), Frame3::IDENTITY, 3.0, 4.0, 4.0, accuracy).unwrap();
    let cutter = primitives::frustum(
        "cutter".into(),
        Frame3 {
            origin: [0.0, 0.0, 1.0],
            ..Frame3::IDENTITY
        },
        1.0,
        1.5,
        1.0,
        accuracy,
    )
    .unwrap();
    let host_volume = std::f64::consts::PI * 4.0 * (9.0 + 12.0 + 16.0) / 3.0;
    let cutter_volume = std::f64::consts::PI * (1.0 + 1.5 + 2.25) / 3.0;
    for (operation, expected_volume, expected_faces) in [
        (BooleanOp::Union, host_volume, 3),
        (BooleanOp::Intersection, cutter_volume, 3),
        (BooleanOp::Subtraction, host_volume - cutter_volume, 6),
    ] {
        let result =
            boolean_brep(&host, &cutter, operation, format!("conic-{operation:?}")).unwrap();
        result.brep.validate().unwrap();
        assert_eq!(result.brep.topology.faces.len(), expected_faces);
        assert_eq!(result.report.face_mappings.len(), 6);
        let measured = volume(&result.brep).abs();
        assert!(
            (measured - expected_volume).abs() < 0.75,
            "{operation:?}: measured {measured}, expected {expected_volume}"
        );
        if operation == BooleanOp::Subtraction {
            assert_eq!(result.brep.solids[0].cavity_shells.len(), 1);
            assert_eq!(
                result
                    .brep
                    .topology
                    .faces
                    .iter()
                    .filter(|face| face.provenance.role == FaceRole::Cut
                        && face.provenance.reversed)
                    .count(),
                3
            );
        }
    }

    let touching =
        primitives::frustum("touching".into(), Frame3::IDENTITY, 1.0, 1.5, 1.0, accuracy).unwrap();
    assert!(matches!(
        boolean_brep(
            &host,
            &touching,
            BooleanOp::Subtraction,
            "touching-result".into()
        ),
        Err(GeometryError::UnresolvedIntersection(_))
    ));
}

#[test]
fn mixed_conic_containment_preserves_all_analytic_supports() {
    let accuracy = sphere("accuracy", [0.0; 3], 1.0).accuracy;
    let conic =
        primitives::frustum("frustum".into(), Frame3::IDENTITY, 3.0, 4.0, 4.0, accuracy).unwrap();
    let inner_sphere = primitives::sphere(
        "inner-sphere".into(),
        Frame3 {
            origin: [0.0, 0.0, 2.0],
            ..Frame3::IDENTITY
        },
        0.5,
        accuracy,
    )
    .unwrap();
    let outer_sphere = primitives::sphere(
        "outer-sphere".into(),
        Frame3 {
            origin: [0.0, 0.0, 2.0],
            ..Frame3::IDENTITY
        },
        6.0,
        accuracy,
    )
    .unwrap();
    let inner_cylinder = primitives::cylinder(
        "inner-cylinder".into(),
        Frame3 {
            origin: [0.0, 0.0, 1.0],
            ..Frame3::IDENTITY
        },
        0.5,
        1.0,
        accuracy,
    )
    .unwrap();
    let outer_cylinder = primitives::cylinder(
        "outer-cylinder".into(),
        Frame3 {
            origin: [0.0, 0.0, -1.0],
            ..Frame3::IDENTITY
        },
        5.0,
        6.0,
        accuracy,
    )
    .unwrap();
    let inner_box = primitives::cuboid(
        "inner-box".into(),
        Frame3 {
            origin: [-0.25, -0.25, 1.75],
            ..Frame3::IDENTITY
        },
        [0.5, 0.5, 0.5],
        accuracy,
    )
    .unwrap();
    let outer_box = primitives::cuboid(
        "outer-box".into(),
        Frame3 {
            origin: [-5.0, -5.0, -1.0],
            ..Frame3::IDENTITY
        },
        [10.0, 10.0, 6.0],
        accuracy,
    )
    .unwrap();

    for (host, cutter, expected_faces) in [
        (&conic, &inner_sphere, 4),
        (&outer_sphere, &conic, 4),
        (&conic, &inner_cylinder, 6),
        (&outer_cylinder, &conic, 6),
        (&conic, &inner_box, 9),
        (&outer_box, &conic, 9),
    ] {
        let result = boolean_brep(
            host,
            cutter,
            BooleanOp::Subtraction,
            format!("{}-minus-{}", host.id, cutter.id),
        )
        .unwrap();
        result.brep.validate().unwrap();
        assert_eq!(result.brep.solids.len(), 1);
        assert_eq!(result.brep.solids[0].cavity_shells.len(), 1);
        assert_eq!(result.brep.topology.faces.len(), expected_faces);
        assert_eq!(
            result
                .brep
                .topology
                .faces
                .iter()
                .filter(|face| face.provenance.role == FaceRole::Cut && face.provenance.reversed)
                .count(),
            cutter.topology.faces.len()
        );
        tessellate(&result.brep, 0.02, 2_000_000).unwrap();
    }
}

#[test]
fn ring_torus_containment_uses_exact_host_support_bounds() {
    let accuracy = sphere("accuracy", [0.0; 3], 1.0).accuracy;
    let torus = primitives::torus(
        "torus".into(),
        Frame3 {
            origin: [0.0, 0.0, 2.0],
            ..Frame3::IDENTITY
        },
        2.0,
        0.5,
        accuracy,
    )
    .unwrap();
    let cylinder =
        primitives::cylinder("cylinder".into(), Frame3::IDENTITY, 4.0, 4.0, accuracy).unwrap();
    let box_ = primitives::cuboid(
        "box".into(),
        Frame3 {
            origin: [-4.0, -4.0, -1.0],
            ..Frame3::IDENTITY
        },
        [8.0, 8.0, 6.0],
        accuracy,
    )
    .unwrap();
    let conic =
        primitives::frustum("frustum".into(), Frame3::IDENTITY, 4.0, 5.0, 4.0, accuracy).unwrap();
    for host in [&cylinder, &box_, &conic] {
        let result = boolean_brep(
            host,
            &torus,
            BooleanOp::Subtraction,
            format!("{}-minus-torus", host.id),
        )
        .unwrap();
        result.brep.validate().unwrap();
        assert_eq!(result.brep.solids.len(), 1);
        assert_eq!(result.brep.solids[0].cavity_shells.len(), 1);
        assert!(result.brep.topology.faces.iter().any(|face| {
            matches!(
                result.brep.geometry.surfaces[face.surface as usize],
                SurfaceGeometry::Torus { .. }
            ) && face.provenance.role == FaceRole::Cut
                && face.provenance.reversed
        }));
        tessellate(&result.brep, 0.02, 2_000_000).unwrap();
    }
}
