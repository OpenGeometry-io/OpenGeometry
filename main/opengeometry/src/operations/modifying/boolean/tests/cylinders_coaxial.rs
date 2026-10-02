use super::support::{cylinder, volume};
use crate::brep::{placed, FaceRole, Frame3, GeometryError};
use crate::operations::modifying::boolean::dispatch::boolean_brep;
use crate::operations::modifying::boolean::operands::full_cylinder;
use crate::operations::modifying::boolean::types::BooleanOp;
use crate::primitives;
use crate::tessellation::tessellate;

#[test]
fn coaxial_cylinder_booleans_split_material_and_keep_cut_cap_ancestry() {
    let a = cylinder("a", 0.0, 4.0, false);
    let b = cylinder("b", 3.0, 2.0, true);
    for (operation, solids, faces, expected_volume) in [
        (BooleanOp::Union, 1, 3, 4.0 * std::f64::consts::PI),
        (BooleanOp::Intersection, 1, 3, 2.0 * std::f64::consts::PI),
        (BooleanOp::Subtraction, 2, 6, 2.0 * std::f64::consts::PI),
    ] {
        let result = boolean_brep(&a, &b, operation, format!("{operation:?}")).unwrap();
        assert_eq!(result.brep.solids.len(), solids);
        assert_eq!(result.brep.topology.faces.len(), faces);
        let measured = volume(&result.brep).abs();
        assert!(
            (measured - expected_volume).abs() < 0.2,
            "{operation:?}: measured {measured}, expected {expected_volume}"
        );
        assert_eq!(result.report.face_mappings.len(), 6);
        if operation == BooleanOp::Subtraction {
            let cut_caps: Vec<_> = result
                .brep
                .topology
                .faces
                .iter()
                .filter(|face| face.provenance.role == FaceRole::Cut)
                .collect();
            assert_eq!(cut_caps.len(), 2);
            assert!(cut_caps.iter().all(|face| {
                face.provenance.reversed
                    && face.provenance.sources.len() == 1
                    && face.provenance.sources[0].entity == "b"
            }));
        }
    }
}

#[test]
fn coaxial_cylinder_contact_is_regularized_and_near_gap_is_unresolved() {
    let a = cylinder("a", 0.0, 4.0, false);
    let touching = cylinder("b", 4.0, 2.0, false);
    let union = boolean_brep(&a, &touching, BooleanOp::Union, "union".into()).unwrap();
    assert_eq!(union.brep.solids.len(), 1);
    let union_volume = volume(&union.brep).abs();
    assert!(
        (union_volume - 6.0 * std::f64::consts::PI).abs() < 0.3,
        "measured union volume {union_volume}"
    );
    let intersection = boolean_brep(
        &a,
        &touching,
        BooleanOp::Intersection,
        "intersection".into(),
    )
    .unwrap();
    assert!(intersection.brep.solids.is_empty());
    let subtraction =
        boolean_brep(&a, &touching, BooleanOp::Subtraction, "subtraction".into()).unwrap();
    assert_eq!(subtraction.brep.solids.len(), 1);
    let subtraction_volume = volume(&subtraction.brep).abs();
    assert!(
        (subtraction_volume - 4.0 * std::f64::consts::PI).abs() < 0.2,
        "measured subtraction volume {subtraction_volume}"
    );

    let near = cylinder("near", 4.0 + 2e-9, 2.0, false);
    assert!(matches!(
        boolean_brep(&a, &near, BooleanOp::Union, "near".into()),
        Err(GeometryError::UnresolvedIntersection(_))
    ));
}

#[test]
fn positive_similarity_roundoff_does_not_reject_full_cylinder_operands() {
    let rotation = std::f64::consts::PI / 7.0;
    let source_frame = Frame3 {
        origin: [0.0, 1.2, 0.0],
        x: [1.0, 0.0, 0.0],
        y: [0.0, 0.0, -1.0],
        z: [0.0, 1.0, 0.0],
    };
    let source = primitives::cylinder(
        "c".into(),
        source_frame,
        1.0,
        2.0,
        cylinder("accuracy", 0.0, 1.0, false).accuracy,
    )
    .unwrap();
    let placed = placed(
        &source,
        Frame3 {
            origin: [0.2, 0.1, 0.2],
            x: [rotation.cos(), 0.0, -rotation.sin()],
            y: [0.0, 1.0, 0.0],
            z: [rotation.sin(), 0.0, rotation.cos()],
        },
        1.25,
    )
    .unwrap();
    assert!(full_cylinder(&placed).is_ok());
}

#[test]
fn translated_y_axis_cylinder_can_split_into_two_valid_solids() {
    let frame = Frame3 {
        origin: [0.0; 3],
        x: [1.0, 0.0, 0.0],
        y: [0.0, 0.0, -1.0],
        z: [0.0, 1.0, 0.0],
    };
    let accuracy = cylinder("accuracy", 0.0, 1.0, false).accuracy;
    let host = primitives::cylinder("host".into(), frame, 1.0, 2.0, accuracy).unwrap();
    let cutter = primitives::cylinder("cutter".into(), frame, 1.0, 0.8, accuracy).unwrap();
    let cutter = placed(
        &cutter,
        Frame3 {
            origin: [0.0, 0.6, 0.0],
            ..Frame3::IDENTITY
        },
        1.0,
    )
    .unwrap();
    let result = boolean_brep(&host, &cutter, BooleanOp::Subtraction, "result".into()).unwrap();
    assert_eq!(result.brep.solids.len(), 2);
    result.brep.validate().unwrap();
    tessellate(&result.brep, 0.01, 2_000_000).unwrap();
}

#[test]
fn coextensive_unequal_cylinders_produce_exact_radial_results() {
    let accuracy = cylinder("accuracy", 0.0, 1.0, false).accuracy;
    let outer = primitives::cylinder("outer".into(), Frame3::IDENTITY, 2.0, 3.0, accuracy).unwrap();
    let inner = primitives::cylinder("inner".into(), Frame3::IDENTITY, 1.0, 3.0, accuracy).unwrap();
    let union = boolean_brep(&outer, &inner, BooleanOp::Union, "union".into()).unwrap();
    let intersection = boolean_brep(
        &outer,
        &inner,
        BooleanOp::Intersection,
        "intersection".into(),
    )
    .unwrap();
    let subtraction = boolean_brep(&outer, &inner, BooleanOp::Subtraction, "tube".into()).unwrap();
    assert_eq!(union.brep.topology.faces.len(), 3);
    assert_eq!(intersection.brep.topology.faces.len(), 3);
    assert_eq!(subtraction.brep.topology.faces.len(), 4);
    assert_eq!(subtraction.brep.topology.faces[2].trim.holes.len(), 1);
    assert_eq!(subtraction.brep.topology.faces[3].trim.holes.len(), 1);
    assert_eq!(
        subtraction.brep.topology.faces[1].provenance.role,
        FaceRole::Cut
    );
    assert!(subtraction.brep.topology.faces[1].provenance.reversed);
    assert_eq!(subtraction.report.face_mappings.len(), 6);
    let measured = volume(&subtraction.brep).abs();
    let expected = std::f64::consts::PI * (4.0 - 1.0) * 3.0;
    assert!(
        (measured - expected).abs() < 0.2,
        "{measured} != {expected}"
    );
    subtraction.brep.validate().unwrap();
    tessellate(&subtraction.brep, 0.01, 2_000_000).unwrap();

    let empty = boolean_brep(&inner, &outer, BooleanOp::Subtraction, "empty".into()).unwrap();
    assert!(empty.brep.solids.is_empty());
    let shifted = primitives::cylinder(
        "shifted".into(),
        Frame3 {
            origin: [0.0, 0.0, 0.5],
            ..Frame3::IDENTITY
        },
        1.0,
        3.0,
        accuracy,
    )
    .unwrap();
    assert!(matches!(
        boolean_brep(&outer, &shifted, BooleanOp::Subtraction, "gap".into()),
        Err(GeometryError::CoverageGap { .. })
    ));
}

#[test]
fn enclosed_unequal_cylinder_becomes_an_oriented_cavity_shell() {
    let accuracy = cylinder("accuracy", 0.0, 1.0, false).accuracy;
    let host = primitives::cylinder("host".into(), Frame3::IDENTITY, 2.0, 4.0, accuracy).unwrap();
    let cutter = primitives::cylinder(
        "cutter".into(),
        Frame3 {
            origin: [0.0, 0.0, 1.0],
            ..Frame3::IDENTITY
        },
        0.75,
        2.0,
        accuracy,
    )
    .unwrap();
    let subtraction =
        boolean_brep(&host, &cutter, BooleanOp::Subtraction, "cavity".into()).unwrap();
    assert_eq!(subtraction.brep.solids.len(), 1);
    assert_eq!(subtraction.brep.topology.shells.len(), 2);
    assert_eq!(subtraction.brep.solids[0].cavity_shells, vec![1]);
    assert_eq!(subtraction.brep.topology.faces.len(), 6);
    assert!(subtraction.brep.topology.faces[3..]
        .iter()
        .all(|face| face.provenance.role == FaceRole::Cut && face.provenance.reversed));
    let measured = volume(&subtraction.brep).abs();
    let expected = std::f64::consts::PI * (2.0_f64.powi(2) * 4.0 - 0.75_f64.powi(2) * 2.0);
    assert!(
        (measured - expected).abs() < 0.3,
        "{measured} != {expected}"
    );
    subtraction.brep.validate().unwrap();
    tessellate(&subtraction.brep, 0.01, 2_000_000).unwrap();

    let union = boolean_brep(&host, &cutter, BooleanOp::Union, "union".into()).unwrap();
    let intersection = boolean_brep(
        &host,
        &cutter,
        BooleanOp::Intersection,
        "intersection".into(),
    )
    .unwrap();
    assert_eq!(union.brep.topology.faces.len(), 3);
    assert_eq!(intersection.brep.topology.faces.len(), 3);
    let empty = boolean_brep(&cutter, &host, BooleanOp::Subtraction, "empty".into()).unwrap();
    assert!(empty.brep.solids.is_empty());
}
