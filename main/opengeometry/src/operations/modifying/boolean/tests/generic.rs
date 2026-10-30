use super::support::{cylinder, sphere};
use crate::brep::{Accuracy, CurveGeometry, FaceRole, Frame3};
use crate::operations::modifying::boolean::dispatch::boolean_brep;
use crate::operations::modifying::boolean::types::BooleanOp;
use crate::primitives;
use crate::tessellation::tessellate;

#[test]
fn provably_disjoint_surface_families_use_exact_regularized_results() {
    let cylinder_accuracy = sphere("reference", [0.0; 3], 1.0).accuracy;
    let torus_accuracy = Accuracy {
        geometric: cylinder_accuracy.geometric * 2.0,
        intersection: cylinder_accuracy.intersection * 2.0,
        tessellation: cylinder_accuracy.tessellation * 2.0,
        exchange: cylinder_accuracy.exchange * 2.0,
    };
    let cylinder = primitives::cylinder(
        "cylinder".into(),
        Frame3::IDENTITY,
        1.0,
        2.0,
        cylinder_accuracy,
    )
    .unwrap();
    let torus = primitives::torus(
        "torus".into(),
        Frame3 {
            origin: [12.0, 0.0, 0.0],
            ..Frame3::IDENTITY
        },
        3.0,
        1.0,
        torus_accuracy,
    )
    .unwrap();

    let union = boolean_brep(&cylinder, &torus, BooleanOp::Union, "union".into()).unwrap();
    assert_eq!(union.brep.solids.len(), 2);
    assert_eq!(union.brep.accuracy.geometric, torus_accuracy.geometric);
    assert_eq!(union.brep.topology.faces.len(), 4);
    assert_eq!(union.report.face_mappings.len(), 4);
    assert!(union
        .brep
        .topology
        .faces
        .iter()
        .all(|face| face.provenance.role == FaceRole::Preserved));

    let intersection = boolean_brep(
        &cylinder,
        &torus,
        BooleanOp::Intersection,
        "intersection".into(),
    )
    .unwrap();
    assert!(intersection.brep.solids.is_empty());
    assert!(intersection
        .report
        .face_mappings
        .iter()
        .all(|mapping| mapping.result_faces.is_empty()));

    let subtraction = boolean_brep(
        &cylinder,
        &torus,
        BooleanOp::Subtraction,
        "subtraction".into(),
    )
    .unwrap();
    assert_eq!(subtraction.brep.solids.len(), 1);
    assert_eq!(subtraction.brep.topology.faces.len(), 3);
    assert_eq!(
        subtraction.brep.topology.faces[0].provenance.sources[0].entity,
        "cylinder"
    );
}

#[test]
fn generic_closed_loop_imprints_an_off_axis_sphere_cone_intersection() {
    let accuracy = cylinder("accuracy", 0.0, 1.0, false).accuracy;
    let cone = primitives::cone("cone".into(), Frame3::IDENTITY, 2.0, 2.0, accuracy).unwrap();
    let sphere = primitives::sphere(
        "sphere".into(),
        Frame3 {
            origin: [1.42, 0.0, 1.0],
            ..Frame3::IDENTITY
        },
        0.35,
        accuracy,
    )
    .unwrap();
    for operation in [
        BooleanOp::Union,
        BooleanOp::Intersection,
        BooleanOp::Subtraction,
    ] {
        let result = boolean_brep(
            &cone,
            &sphere,
            operation,
            format!("sphere-cone-{operation:?}"),
        )
        .unwrap_or_else(|error| panic!("{operation:?}: {error}"));
        assert_eq!(result.brep.solids.len(), 1);
        assert!(result
            .brep
            .geometry
            .curves
            .iter()
            .any(|curve| matches!(curve, CurveGeometry::Intersection { .. })));
        assert!(result
            .brep
            .topology
            .faces
            .iter()
            .all(|face| !face.provenance.sources.is_empty()));
        result.brep.validate().unwrap();
        tessellate(&result.brep, 0.01, 2_000_000)
            .unwrap_or_else(|error| panic!("{operation:?} tessellation: {error}"));
    }
}

#[test]
fn generic_closed_loop_handles_periodic_winding() {
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
    let result = boolean_brep(&cone, &sphere, BooleanOp::Intersection, "winding".into()).unwrap();
    result.brep.validate().unwrap();
    tessellate(&result.brep, 0.01, 2_000_000).unwrap();
}

#[test]
fn identical_general_analytic_body_uses_coincident_ownership() {
    let accuracy = sphere("accuracy", [0.0; 3], 1.0).accuracy;
    let host =
        primitives::annular_cylinder("host".into(), Frame3::IDENTITY, 0.4, 2.0, 3.0, accuracy)
            .unwrap();
    let union = boolean_brep(&host, &host, BooleanOp::Union, "same".into()).unwrap();
    assert!(union.report.coincident);
    assert_eq!(union.brep.topology.faces.len(), host.topology.faces.len());
    assert!(union
        .brep
        .topology
        .faces
        .iter()
        .all(|face| face.provenance.role == FaceRole::Coincident));
    let empty = boolean_brep(&host, &host, BooleanOp::Subtraction, "empty".into()).unwrap();
    assert!(empty.brep.solids.is_empty());
}

#[test]
fn generic_pipeline_classifies_noncanonical_planar_solid_containment() {
    let accuracy = sphere("accuracy", [0.0; 3], 1.0).accuracy;
    let prism = primitives::linear_extrusion(
        "prism".into(),
        Frame3 {
            origin: [0.0, 0.0, 0.8],
            ..Frame3::IDENTITY
        },
        vec![[-0.2, -0.2], [0.2, -0.2], [0.0, 0.2]],
        Vec::new(),
        0.4,
        accuracy,
    )
    .unwrap();
    let cylinder =
        primitives::cylinder("cylinder".into(), Frame3::IDENTITY, 2.0, 2.0, accuracy).unwrap();
    let union = boolean_brep(&cylinder, &prism, BooleanOp::Union, "union".into()).unwrap();
    assert_eq!(
        union.brep.topology.faces.len(),
        cylinder.topology.faces.len()
    );
    let intersection = boolean_brep(
        &cylinder,
        &prism,
        BooleanOp::Intersection,
        "intersection".into(),
    )
    .unwrap();
    assert_eq!(
        intersection.brep.topology.faces.len(),
        prism.topology.faces.len()
    );
    let subtraction = boolean_brep(
        &cylinder,
        &prism,
        BooleanOp::Subtraction,
        "subtraction".into(),
    )
    .unwrap();
    assert_eq!(subtraction.brep.solids.len(), 1);
    assert_eq!(subtraction.brep.solids[0].cavity_shells.len(), 1);
    subtraction.brep.validate().unwrap();
    tessellate(&subtraction.brep, 0.01, 2_000_000).unwrap();
}
