use super::support::{cylinder, volume};
use crate::brep::{CurveGeometry, FaceRole, Frame3, GeometryError, GeometryQuality};
use crate::exchange::export_step;
use crate::operations::modifying::boolean::dispatch::boolean_brep;
use crate::operations::modifying::boolean::types::BooleanOp;
use crate::primitives;
use crate::tessellation::tessellate;

#[test]
fn mixed_sphere_cylinder_containment_preserves_exact_cavity_supports() {
    let accuracy = cylinder("accuracy", 0.0, 1.0, false).accuracy;
    let cylinder_host =
        primitives::cylinder("cylinder".into(), Frame3::IDENTITY, 2.0, 4.0, accuracy).unwrap();
    let sphere_cutter = primitives::sphere(
        "sphere".into(),
        Frame3 {
            origin: [0.0, 0.0, 2.0],
            ..Frame3::IDENTITY
        },
        0.5,
        accuracy,
    )
    .unwrap();
    let cylindrical_cavity = boolean_brep(
        &cylinder_host,
        &sphere_cutter,
        BooleanOp::Subtraction,
        "cylindrical-cavity".into(),
    )
    .unwrap();
    assert_eq!(cylindrical_cavity.brep.solids.len(), 1);
    assert_eq!(cylindrical_cavity.brep.topology.shells.len(), 2);
    assert_eq!(cylindrical_cavity.brep.solids[0].cavity_shells, vec![1]);
    assert_eq!(cylindrical_cavity.brep.topology.faces.len(), 4);
    assert_eq!(
        cylindrical_cavity.brep.topology.faces[3].provenance.role,
        FaceRole::Cut
    );
    assert!(
        cylindrical_cavity.brep.topology.faces[3]
            .provenance
            .reversed
    );
    cylindrical_cavity.brep.validate().unwrap();
    tessellate(&cylindrical_cavity.brep, 0.01, 2_000_000).unwrap();

    let sphere_host =
        primitives::sphere("sphere-host".into(), Frame3::IDENTITY, 3.0, accuracy).unwrap();
    let cylinder_cutter = primitives::cylinder(
        "cylinder-cutter".into(),
        Frame3 {
            origin: [0.0, 0.0, -0.5],
            ..Frame3::IDENTITY
        },
        0.5,
        1.0,
        accuracy,
    )
    .unwrap();
    let spherical_cavity = boolean_brep(
        &sphere_host,
        &cylinder_cutter,
        BooleanOp::Subtraction,
        "spherical-cavity".into(),
    )
    .unwrap();
    assert_eq!(spherical_cavity.brep.topology.faces.len(), 4);
    assert_eq!(spherical_cavity.brep.solids[0].cavity_shells, vec![1]);
    assert!(spherical_cavity.brep.topology.faces[1..]
        .iter()
        .all(|face| face.provenance.role == FaceRole::Cut && face.provenance.reversed));
    spherical_cavity.brep.validate().unwrap();
    tessellate(&spherical_cavity.brep, 0.01, 2_000_000).unwrap();

    let union = boolean_brep(
        &cylinder_host,
        &sphere_cutter,
        BooleanOp::Union,
        "union".into(),
    )
    .unwrap();
    let intersection = boolean_brep(
        &cylinder_host,
        &sphere_cutter,
        BooleanOp::Intersection,
        "intersection".into(),
    )
    .unwrap();
    assert_eq!(union.brep.topology.faces.len(), 3);
    assert_eq!(intersection.brep.topology.faces.len(), 1);
    let crossing = primitives::sphere(
        "crossing".into(),
        Frame3 {
            origin: [1.8, 0.0, 2.0],
            ..Frame3::IDENTITY
        },
        0.5,
        accuracy,
    )
    .unwrap();
    for (name, left, right, operation) in [
        (
            "crossing-union",
            &cylinder_host,
            &crossing,
            BooleanOp::Union,
        ),
        (
            "crossing-intersection",
            &cylinder_host,
            &crossing,
            BooleanOp::Intersection,
        ),
        (
            "crossing-cylinder-cut",
            &cylinder_host,
            &crossing,
            BooleanOp::Subtraction,
        ),
        (
            "crossing-sphere-cut",
            &crossing,
            &cylinder_host,
            BooleanOp::Subtraction,
        ),
    ] {
        let result = boolean_brep(left, right, operation, name.into())
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert!(
            matches!(result.report.quality, GeometryQuality::Analytic),
            "{name}"
        );
        assert_eq!(result.brep.solids.len(), 1, "{name}");
        assert!(result.brep.topology.faces.len() >= 2, "{name}");
        assert!(!result.brep.topology.edges.is_empty(), "{name}");
        assert!(
            result
                .brep
                .topology
                .faces
                .iter()
                .all(|face| !face.provenance.sources.is_empty()),
            "{name}"
        );
        assert!(result
            .brep
            .geometry
            .curves
            .iter()
            .any(|curve| matches!(curve, CurveGeometry::Intersection { .. })));
        result.brep.validate().unwrap();
        tessellate(&result.brep, 0.01, 2_000_000)
            .unwrap_or_else(|error| panic!("{name} tessellation: {error}"));
    }
}

#[test]
fn coaxial_sphere_cylinder_intersection_builds_shared_analytic_circles() {
    let accuracy = cylinder("accuracy", 0.0, 1.0, false).accuracy;
    let sphere = primitives::sphere("sphere".into(), Frame3::IDENTITY, 1.5, accuracy).unwrap();
    let cylinder = primitives::cylinder(
        "cylinder".into(),
        Frame3 {
            origin: [0.0, 0.0, -2.0],
            ..Frame3::IDENTITY
        },
        0.75,
        4.0,
        accuracy,
    )
    .unwrap();
    let union = boolean_brep(&sphere, &cylinder, BooleanOp::Union, "union".into()).unwrap();
    let intersection = boolean_brep(
        &sphere,
        &cylinder,
        BooleanOp::Intersection,
        "intersection".into(),
    )
    .unwrap();
    let sphere_cut = boolean_brep(
        &sphere,
        &cylinder,
        BooleanOp::Subtraction,
        "sphere-cut".into(),
    )
    .unwrap();
    let cylinder_cut = boolean_brep(
        &cylinder,
        &sphere,
        BooleanOp::Subtraction,
        "cylinder-cut".into(),
    )
    .unwrap();
    assert_eq!(union.brep.topology.faces.len(), 5);
    assert_eq!(intersection.brep.topology.faces.len(), 3);
    assert_eq!(sphere_cut.brep.topology.faces.len(), 2);
    assert_eq!(cylinder_cut.brep.topology.faces.len(), 6);
    assert_eq!(cylinder_cut.brep.solids.len(), 2);
    assert!(sphere_cut
        .brep
        .topology
        .faces
        .iter()
        .any(|face| face.provenance.role == FaceRole::Cut && face.provenance.reversed));
    assert_eq!(
        cylinder_cut
            .brep
            .topology
            .faces
            .iter()
            .filter(|face| face.provenance.role == FaceRole::Cut && face.provenance.reversed)
            .count(),
        2
    );
    for (name, result) in [
        ("union", &union),
        ("intersection", &intersection),
        ("sphere-cut", &sphere_cut),
        ("cylinder-cut", &cylinder_cut),
    ] {
        result.brep.validate().unwrap();
        let mesh = tessellate(&result.brep, 0.01, 2_000_000)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert!(!mesh.indices.is_empty());
        assert_eq!(result.report.face_mappings.len(), 4);
        export_step(&result.brep, "metre").unwrap_or_else(|error| panic!("{name} STEP: {error}"));
    }
    let sphere_volume = 4.0 * std::f64::consts::PI * 1.5_f64.powi(3) / 3.0;
    let cylinder_volume = std::f64::consts::PI * 0.75_f64.powi(2) * 4.0;
    let intersection_volume = volume(&intersection.brep).abs();
    assert!(
        (volume(&union.brep).abs() + intersection_volume - sphere_volume - cylinder_volume).abs()
            < 0.3
    );
    assert!((volume(&sphere_cut.brep).abs() + intersection_volume - sphere_volume).abs() < 0.3);
    assert!((volume(&cylinder_cut.brep).abs() + intersection_volume - cylinder_volume).abs() < 0.3);

    let noncoaxial = primitives::cylinder(
        "noncoaxial".into(),
        Frame3 {
            origin: [0.2, 0.0, -2.0],
            ..Frame3::IDENTITY
        },
        0.75,
        4.0,
        accuracy,
    )
    .unwrap();
    assert!(matches!(
        boolean_brep(&sphere, &noncoaxial, BooleanOp::Union, "gap".into()),
        Err(GeometryError::CoverageGap { .. })
    ));
}
