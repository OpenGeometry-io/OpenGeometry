use super::support::cylinder;
use crate::brep::{
    placed, Accuracy, BrepEnvelope, FaceRole, Frame3, GeometryError, Orientation, SurfaceGeometry,
};
use crate::math::{cross, dot, Point3};
use crate::operations::modifying::boolean::dispatch::boolean_brep;
use crate::operations::modifying::boolean::handlers::boolean_boxes;
use crate::operations::modifying::boolean::types::BooleanOp;
use crate::primitives;
use crate::primitives::cuboid;
use crate::tessellation::tessellate;

fn box_at(id: &str, origin: Point3, size: Point3) -> BrepEnvelope {
    cuboid(
        id.into(),
        Frame3 {
            origin,
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
            z: [0.0, 0.0, 1.0],
        },
        size,
        Accuracy {
            geometric: 1e-8,
            intersection: 1e-9,
            tessellation: 1e-3,
            exchange: 1e-6,
        },
    )
    .unwrap()
}

fn volume(brep: &BrepEnvelope) -> f64 {
    let mesh = tessellate(brep, 0.01, 100_000).unwrap();
    mesh.indices
        .chunks_exact(3)
        .map(|ids| {
            let p: [Point3; 3] = std::array::from_fn(|j| {
                std::array::from_fn(|i| mesh.positions[3 * ids[j] as usize + i])
            });
            dot(p[0], cross(p[1], p[2])) / 6.0
        })
        .sum()
}

#[test]
fn aligned_box_boolean_volume_topology_provenance_and_exchange() {
    let a = box_at("host", [0.0; 3], [2.0; 3]);
    let b = box_at("cutter", [1.0, 0.5, 0.5], [2.0; 3]);
    for (op, expected) in [
        (BooleanOp::Union, 13.75),
        (BooleanOp::Intersection, 2.25),
        (BooleanOp::Subtraction, 5.75),
    ] {
        let result = boolean_boxes(&a, &b, op, "result".into()).unwrap();
        assert!((volume(&result.brep) - expected).abs() < 1e-10);
        assert_eq!(result.brep.solids.len(), 1);
        assert_eq!(result.report.face_mappings.len(), 12);
        assert!(result
            .brep
            .topology
            .faces
            .iter()
            .all(|f| !f.provenance.sources.is_empty()));
        assert!(result
            .brep
            .geometry
            .surfaces
            .iter()
            .all(|s| matches!(s, SurfaceGeometry::Plane { .. })));
        let fine = tessellate(&result.brep, 0.001, 100_000).unwrap();
        let coarse = tessellate(&result.brep, 0.1, 100_000).unwrap();
        assert_eq!(fine.indices.len(), coarse.indices.len());
        if op == BooleanOp::Union {
            assert!(fine.outline_edge_ids.len() < result.brep.topology.edges.len());
        }
        if op == BooleanOp::Subtraction {
            assert!(result
                .brep
                .topology
                .faces
                .iter()
                .any(|f| f.provenance.role == FaceRole::Cut
                    && f.provenance.reversed
                    && f.sense == Orientation::Reverse));
        }
    }
}

#[test]
fn aligned_box_cavities_split_solids_and_contacts() {
    let a = box_at("host", [0.0; 3], [3.0; 3]);
    let b = box_at("cavity", [1.0; 3], [1.0; 3]);
    let result = boolean_boxes(&a, &b, BooleanOp::Subtraction, "cavity-result".into()).unwrap();
    assert_eq!(result.brep.solids[0].cavity_shells.len(), 1);
    assert!((volume(&result.brep) - 26.0).abs() < 1e-10);
    let cutter = box_at("through", [1.0, -1.0, -1.0], [1.0, 5.0, 5.0]);
    let result = boolean_boxes(&a, &cutter, BooleanOp::Subtraction, "split".into()).unwrap();
    assert_eq!(result.brep.solids.len(), 2);
    assert!((volume(&result.brep) - 18.0).abs() < 1e-10);
    for (origin, solids, contacts) in [
        ([3.0, 0.0, 0.0], 1, 4),
        ([3.0, 3.0, 0.0], 2, 2),
        ([3.0; 3], 2, 1),
    ] {
        let b = box_at("contact", origin, [3.0; 3]);
        let result = boolean_boxes(&a, &b, BooleanOp::Union, "contact-result".into()).unwrap();
        assert_eq!(result.brep.solids.len(), solids);
        assert_eq!(result.report.contacts.len(), contacts);
        let intersection = boolean_boxes(&a, &b, BooleanOp::Intersection, "empty".into()).unwrap();
        assert!(intersection.brep.solids.is_empty());
    }
}

#[test]
fn aligned_box_boolean_identities_determinism_and_scale() {
    let a = box_at("a", [0.0; 3], [2.0; 3]);
    for op in [
        BooleanOp::Union,
        BooleanOp::Intersection,
        BooleanOp::Subtraction,
    ] {
        let result = boolean_brep(&a, &a, op, "identity".into()).unwrap();
        let expected = if op == BooleanOp::Subtraction {
            0.0
        } else {
            8.0
        };
        assert!((volume(&result.brep) - expected).abs() < 1e-10);
        let repeat = boolean_brep(&a, &a, op, "identity".into()).unwrap();
        assert_eq!(
            result.brep.to_json().unwrap(),
            repeat.brep.to_json().unwrap()
        );
        assert_eq!(result.brep.revision, a.revision + 1);
    }
    for factor in [1e-8, 1.0, 1e5] {
        let b = box_at("b", [1.0, 0.5, 0.5], [2.0; 3]);
        let frame = Frame3::from_axis([0.0; 3], [0.0, 1.0, 0.0], [0.8, 0.0, 0.6]).unwrap();
        let a = placed(&a, frame, factor).unwrap();
        let b = placed(&b, frame, factor).unwrap();
        let result = boolean_boxes(&a, &b, BooleanOp::Subtraction, "scaled".into()).unwrap();
        let mesh = tessellate(&result.brep, 0.01 * factor, 100_000).unwrap();
        let measured: f64 = mesh
            .indices
            .chunks_exact(3)
            .map(|ids| {
                let p: [Point3; 3] = std::array::from_fn(|j| {
                    std::array::from_fn(|i| mesh.positions[3 * ids[j] as usize + i] / factor)
                });
                dot(p[0], cross(p[1], p[2])) / 6.0
            })
            .sum();
        assert!((measured - 5.75).abs() < 1e-9);
    }
}

#[test]
fn aligned_box_similarity_and_resolution_errors() {
    let a = box_at("a", [0.0; 3], [2.0; 3]);
    let b = box_at("b", [1.0, 0.5, 0.5], [2.0; 3]);
    let frame = Frame3::from_axis([5.0, 6.0, 7.0], [0.0, 1.0, 0.0], [0.8, 0.0, 0.6]).unwrap();
    let a = placed(&a, frame, 1.25).unwrap();
    let b = placed(&b, frame, 1.25).unwrap();
    let result = boolean_boxes(&a, &b, BooleanOp::Subtraction, "placed".into()).unwrap();
    assert!((volume(&result.brep) - 5.75 * 1.25_f64.powi(3)).abs() < 1e-9);
    let other = box_at("different-axes", [0.0; 3], [2.0; 3]);
    assert!(matches!(
        boolean_boxes(&a, &other, BooleanOp::Union, "gap".into()),
        Err(GeometryError::CoverageGap { .. })
    ));
    let a = box_at("a", [0.0; 3], [2.0; 3]);
    let b = box_at("b", [2.0 + 2.0 * a.accuracy.geometric, 0.0, 0.0], [2.0; 3]);
    assert!(matches!(
        boolean_boxes(&a, &b, BooleanOp::Union, "unresolved".into()),
        Err(GeometryError::UnresolvedIntersection(_))
    ));
}

#[test]
fn mixed_cuboid_containment_builds_exact_curved_and_planar_cavities() {
    let accuracy = cylinder("accuracy", 0.0, 1.0, false).accuracy;
    let box_host = primitives::cuboid(
        "box".into(),
        Frame3 {
            origin: [-2.0, -2.0, -2.0],
            ..Frame3::IDENTITY
        },
        [4.0; 3],
        accuracy,
    )
    .unwrap();
    let sphere_cutter =
        primitives::sphere("sphere".into(), Frame3::IDENTITY, 0.5, accuracy).unwrap();
    let cylinder_cutter = primitives::cylinder(
        "cylinder".into(),
        Frame3 {
            origin: [0.0, 0.0, -0.5],
            ..Frame3::IDENTITY
        },
        0.5,
        1.0,
        accuracy,
    )
    .unwrap();
    for (name, cutter, expected_faces) in [
        ("sphere", &sphere_cutter, 7),
        ("cylinder", &cylinder_cutter, 9),
    ] {
        let result = boolean_brep(
            &box_host,
            cutter,
            BooleanOp::Subtraction,
            format!("{name}-cavity"),
        )
        .unwrap();
        assert_eq!(result.brep.solids.len(), 1);
        assert_eq!(result.brep.topology.shells.len(), 2);
        assert_eq!(result.brep.solids[0].cavity_shells, vec![1]);
        assert_eq!(result.brep.topology.faces.len(), expected_faces);
        assert!(result.brep.topology.faces[6..]
            .iter()
            .all(|face| face.provenance.role == FaceRole::Cut && face.provenance.reversed));
        result.brep.validate().unwrap();
        tessellate(&result.brep, 0.01, 2_000_000).unwrap();
    }

    let sphere_host =
        primitives::sphere("sphere-host".into(), Frame3::IDENTITY, 4.0, accuracy).unwrap();
    let small_box = primitives::cuboid(
        "small-box".into(),
        Frame3 {
            origin: [-0.5; 3],
            ..Frame3::IDENTITY
        },
        [1.0; 3],
        accuracy,
    )
    .unwrap();
    let sphere_box = boolean_brep(
        &sphere_host,
        &small_box,
        BooleanOp::Subtraction,
        "sphere-box".into(),
    )
    .unwrap();
    assert_eq!(sphere_box.brep.topology.faces.len(), 7);
    assert_eq!(sphere_box.brep.solids[0].cavity_shells, vec![1]);

    let cylinder_host = primitives::cylinder(
        "cylinder-host".into(),
        Frame3 {
            origin: [0.0, 0.0, -2.0],
            ..Frame3::IDENTITY
        },
        3.0,
        4.0,
        accuracy,
    )
    .unwrap();
    let cylinder_box = boolean_brep(
        &cylinder_host,
        &small_box,
        BooleanOp::Subtraction,
        "cylinder-box".into(),
    )
    .unwrap();
    assert_eq!(cylinder_box.brep.topology.faces.len(), 9);
    assert_eq!(cylinder_box.brep.solids[0].cavity_shells, vec![1]);

    let crossing = primitives::sphere(
        "crossing".into(),
        Frame3 {
            origin: [1.8, 0.0, 0.0],
            ..Frame3::IDENTITY
        },
        0.5,
        accuracy,
    )
    .unwrap();
    let crossing_union = boolean_brep(
        &box_host,
        &crossing,
        BooleanOp::Union,
        "crossing-union".into(),
    )
    .unwrap();
    assert_eq!(crossing_union.brep.topology.faces.len(), 7);
    assert_eq!(
        crossing_union
            .brep
            .topology
            .faces
            .iter()
            .filter(|face| !face.trim.holes.is_empty())
            .count(),
        1
    );
    crossing_union.brep.validate().unwrap();
    tessellate(&crossing_union.brep, 0.01, 2_000_000).unwrap();
}
