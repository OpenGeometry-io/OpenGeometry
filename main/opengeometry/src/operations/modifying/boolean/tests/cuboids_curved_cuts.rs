use crate::brep::{Accuracy, FaceRole, Frame3, SurfaceGeometry};
use crate::operations::modifying::boolean::dispatch::boolean_brep;
use crate::operations::modifying::boolean::types::BooleanOp;
use crate::primitives;
use crate::query::{classify_point, PointClassification};
use crate::tessellation::tessellate;

#[test]
fn perpendicular_cylinders_cut_exact_round_holes_through_every_cuboid_axis() {
    let accuracy = Accuracy {
        geometric: 1.0e-9,
        intersection: 1.0e-10,
        tessellation: 0.01,
        exchange: 1.0e-5,
    };
    let size = [4.0, 3.0, 2.0];
    for axis in 0..3 {
        let host = primitives::cuboid("box".into(), Frame3::IDENTITY, size, accuracy).unwrap();
        let mut local_origin = [2.0, 1.5, 1.0];
        local_origin[axis] = -1.0;
        let direction = std::array::from_fn(|coordinate| f64::from(coordinate == axis));
        let reference = std::array::from_fn(|coordinate| f64::from(coordinate == (axis + 1) % 3));
        let cutter = primitives::cylinder(
            "round-opening".into(),
            Frame3::from_axis(local_origin, direction, reference).unwrap(),
            0.4,
            size[axis] + 2.0,
            accuracy,
        )
        .unwrap();

        let subtraction = boolean_brep(
            &host,
            &cutter,
            BooleanOp::Subtraction,
            format!("cut-{axis}"),
        )
        .unwrap();
        subtraction.brep.validate().unwrap();
        assert_eq!(subtraction.brep.topology.faces.len(), 7);
        assert_eq!(
            subtraction
                .brep
                .topology
                .faces
                .iter()
                .filter(|face| !face.trim.holes.is_empty())
                .count(),
            2
        );
        assert!(matches!(
            subtraction.brep.geometry.surfaces.last(),
            Some(SurfaceGeometry::Cylinder { .. })
        ));
        assert_eq!(
            classify_point(&subtraction.brep, [2.0, 1.5, 1.0]).unwrap(),
            PointClassification::Outside
        );
        assert_eq!(
            classify_point(&subtraction.brep, [0.2, 0.2, 0.2]).unwrap(),
            PointClassification::Inside
        );
        assert_eq!(subtraction.report.face_mappings.len(), 9);

        let union =
            boolean_brep(&host, &cutter, BooleanOp::Union, format!("union-{axis}")).unwrap();
        union.brep.validate().unwrap();
        assert_eq!(union.brep.solids.len(), 1);
        assert_eq!(union.brep.topology.faces.len(), 10);
        assert_eq!(
            union
                .brep
                .topology
                .faces
                .iter()
                .filter(|face| !face.trim.holes.is_empty())
                .count(),
            2
        );
        for coordinate in [-0.5, size[axis] + 0.5] {
            let mut point = [2.0, 1.5, 1.0];
            point[axis] = coordinate;
            assert_eq!(
                classify_point(&union.brep, point).unwrap(),
                PointClassification::Inside
            );
        }
        assert_eq!(
            classify_point(&union.brep, [0.2, 0.2, 0.2]).unwrap(),
            PointClassification::Inside
        );
        tessellate(&union.brep, 0.01, 2_000_000).unwrap();

        let intersection = boolean_brep(
            &host,
            &cutter,
            BooleanOp::Intersection,
            format!("intersection-{axis}"),
        )
        .unwrap();
        intersection.brep.validate().unwrap();
        assert_eq!(intersection.brep.topology.faces.len(), 3);
        assert_eq!(
            classify_point(&intersection.brep, [2.0, 1.5, 1.0]).unwrap(),
            PointClassification::Inside
        );
        assert_eq!(
            classify_point(&intersection.brep, [0.2, 0.2, 0.2]).unwrap(),
            PointClassification::Outside
        );

        let cutter_remainder = boolean_brep(
            &cutter,
            &host,
            BooleanOp::Subtraction,
            format!("cylinder-minus-box-{axis}"),
        )
        .unwrap();
        cutter_remainder.brep.validate().unwrap();
        assert_eq!(cutter_remainder.brep.solids.len(), 2);
        assert_eq!(cutter_remainder.brep.topology.faces.len(), 6);
        assert_eq!(
            cutter_remainder
                .brep
                .topology
                .faces
                .iter()
                .filter(|face| face.provenance.role == FaceRole::Cut && face.provenance.reversed)
                .count(),
            2
        );
        for coordinate in [-0.5, size[axis] + 0.5] {
            let mut point = [2.0, 1.5, 1.0];
            point[axis] = coordinate;
            assert_eq!(
                classify_point(&cutter_remainder.brep, point).unwrap(),
                PointClassification::Inside
            );
        }
        assert_eq!(
            classify_point(&cutter_remainder.brep, [2.0, 1.5, 1.0]).unwrap(),
            PointClassification::Outside
        );
    }
}

#[test]
fn perpendicular_cylinders_cut_exact_blind_pockets_from_both_cuboid_sides() {
    let accuracy = Accuracy {
        geometric: 1.0e-9,
        intersection: 1.0e-10,
        tessellation: 0.01,
        exchange: 1.0e-5,
    };
    let size = [4.0, 3.0, 2.0];
    let pocket_depth = 0.75;
    for axis in 0..3 {
        for entry_from_low in [true, false] {
            let host = primitives::cuboid("box".into(), Frame3::IDENTITY, size, accuracy).unwrap();
            let mut origin = [2.0, 1.5, 1.0];
            origin[axis] = if entry_from_low {
                -1.0
            } else {
                size[axis] + 1.0
            };
            let direction = std::array::from_fn(|coordinate| {
                if coordinate == axis {
                    if entry_from_low {
                        1.0
                    } else {
                        -1.0
                    }
                } else {
                    0.0
                }
            });
            let reference =
                std::array::from_fn(|coordinate| f64::from(coordinate == (axis + 1) % 3));
            let cutter = primitives::cylinder(
                "pocket".into(),
                Frame3::from_axis(origin, direction, reference).unwrap(),
                0.4,
                1.0 + pocket_depth,
                accuracy,
            )
            .unwrap();

            let subtraction = boolean_brep(
                &host,
                &cutter,
                BooleanOp::Subtraction,
                format!("pocket-{axis}-{entry_from_low}"),
            )
            .unwrap();
            subtraction.brep.validate().unwrap();
            assert_eq!(subtraction.brep.topology.faces.len(), 8);
            assert_eq!(
                subtraction
                    .brep
                    .topology
                    .faces
                    .iter()
                    .filter(|face| !face.trim.holes.is_empty())
                    .count(),
                1
            );
            assert_eq!(
                subtraction
                    .brep
                    .topology
                    .faces
                    .iter()
                    .filter(|face| face.provenance.role == FaceRole::Cut)
                    .count(),
                2
            );
            assert!(subtraction
                .brep
                .topology
                .faces
                .iter()
                .filter(|face| face.provenance.role == FaceRole::Cut)
                .all(|face| face.provenance.reversed));

            let union = boolean_brep(
                &host,
                &cutter,
                BooleanOp::Union,
                format!("pocket-union-{axis}-{entry_from_low}"),
            )
            .unwrap();
            union.brep.validate().unwrap();
            assert_eq!(union.brep.solids.len(), 1);
            assert_eq!(union.brep.topology.faces.len(), 8);
            assert_eq!(
                union
                    .brep
                    .topology
                    .faces
                    .iter()
                    .filter(|face| !face.trim.holes.is_empty())
                    .count(),
                1
            );

            let mut void_point = [2.0, 1.5, 1.0];
            void_point[axis] = if entry_from_low {
                pocket_depth / 2.0
            } else {
                size[axis] - pocket_depth / 2.0
            };
            assert_eq!(
                classify_point(&subtraction.brep, void_point).unwrap(),
                PointClassification::Outside
            );
            let mut material_point = void_point;
            material_point[axis] = if entry_from_low {
                pocket_depth + 0.25
            } else {
                size[axis] - pocket_depth - 0.25
            };
            assert_eq!(
                classify_point(&subtraction.brep, material_point).unwrap(),
                PointClassification::Inside
            );

            let intersection = boolean_brep(
                &host,
                &cutter,
                BooleanOp::Intersection,
                format!("pocket-intersection-{axis}-{entry_from_low}"),
            )
            .unwrap();
            intersection.brep.validate().unwrap();
            assert_eq!(intersection.brep.topology.faces.len(), 3);
            assert_eq!(
                classify_point(&intersection.brep, void_point).unwrap(),
                PointClassification::Inside
            );
            assert_eq!(
                classify_point(&intersection.brep, material_point).unwrap(),
                PointClassification::Outside
            );

            let cutter_remainder = boolean_brep(
                &cutter,
                &host,
                BooleanOp::Subtraction,
                format!("pocket-cutter-minus-box-{axis}-{entry_from_low}"),
            )
            .unwrap();
            cutter_remainder.brep.validate().unwrap();
            assert_eq!(cutter_remainder.brep.solids.len(), 1);
            assert_eq!(cutter_remainder.brep.topology.faces.len(), 3);
            assert_eq!(
                cutter_remainder
                    .brep
                    .topology
                    .faces
                    .iter()
                    .filter(|face| face.provenance.role == FaceRole::Cut
                        && face.provenance.reversed)
                    .count(),
                1
            );
            let mut exterior_point = [2.0, 1.5, 1.0];
            exterior_point[axis] = if entry_from_low {
                -0.5
            } else {
                size[axis] + 0.5
            };
            assert_eq!(
                classify_point(&cutter_remainder.brep, exterior_point).unwrap(),
                PointClassification::Inside
            );
            assert_eq!(
                classify_point(&union.brep, exterior_point).unwrap(),
                PointClassification::Inside
            );
            assert_eq!(
                classify_point(&union.brep, material_point).unwrap(),
                PointClassification::Inside
            );
            tessellate(&union.brep, 0.01, 2_000_000).unwrap();
            assert_eq!(
                classify_point(&cutter_remainder.brep, void_point).unwrap(),
                PointClassification::Outside
            );
        }
    }
}

#[test]
fn spheres_cut_exact_single_face_pockets_on_every_cuboid_side() {
    let accuracy = Accuracy {
        geometric: 1.0e-9,
        intersection: 1.0e-10,
        tessellation: 0.01,
        exchange: 1.0e-5,
    };
    let size = [4.0, 3.0, 2.0];
    let radius = 0.75;
    for axis in 0..3 {
        for upper in [false, true] {
            for center_inside in [false, true] {
                let host =
                    primitives::cuboid("box".into(), Frame3::IDENTITY, size, accuracy).unwrap();
                let mut center = [2.0, 1.5, 1.0];
                center[axis] = match (upper, center_inside) {
                    (false, false) => -0.25,
                    (false, true) => 0.25,
                    (true, false) => size[axis] + 0.25,
                    (true, true) => size[axis] - 0.25,
                };
                let cutter = primitives::sphere(
                    "spherical-pocket".into(),
                    Frame3 {
                        origin: center,
                        ..Frame3::IDENTITY
                    },
                    radius,
                    accuracy,
                )
                .unwrap();

                let subtraction = boolean_brep(
                    &host,
                    &cutter,
                    BooleanOp::Subtraction,
                    format!("sphere-pocket-{axis}-{upper}-{center_inside}"),
                )
                .unwrap();
                subtraction.brep.validate().unwrap();
                assert_eq!(subtraction.brep.topology.faces.len(), 7);
                assert_eq!(
                    subtraction
                        .brep
                        .topology
                        .faces
                        .iter()
                        .filter(|face| !face.trim.holes.is_empty())
                        .count(),
                    1
                );
                assert_eq!(
                    subtraction
                        .brep
                        .topology
                        .faces
                        .iter()
                        .filter(|face| face.provenance.role == FaceRole::Cut
                            && face.provenance.reversed)
                        .count(),
                    1
                );
                assert_eq!(subtraction.report.face_mappings.len(), 7);

                let inward_sign = if upper { -1.0 } else { 1.0 };
                let mut void_point = center;
                void_point[axis] = if upper { size[axis] - 0.25 } else { 0.25 };
                let mut material_point = center;
                material_point[axis] += inward_sign * (radius + 0.25);
                let mut sphere_only_point = center;
                sphere_only_point[axis] = if upper { size[axis] + 0.25 } else { -0.25 };
                assert_eq!(
                    classify_point(&subtraction.brep, void_point).unwrap(),
                    PointClassification::Outside
                );
                assert_eq!(
                    classify_point(&subtraction.brep, material_point).unwrap(),
                    PointClassification::Inside
                );

                let intersection = boolean_brep(
                    &host,
                    &cutter,
                    BooleanOp::Intersection,
                    format!("sphere-intersection-{axis}-{upper}-{center_inside}"),
                )
                .unwrap();
                intersection.brep.validate().unwrap();
                assert_eq!(intersection.brep.topology.faces.len(), 2);
                assert_eq!(
                    classify_point(&intersection.brep, void_point).unwrap(),
                    PointClassification::Inside
                );
                assert_eq!(
                    classify_point(&intersection.brep, material_point).unwrap(),
                    PointClassification::Outside
                );

                let union = boolean_brep(
                    &host,
                    &cutter,
                    BooleanOp::Union,
                    format!("sphere-union-{axis}-{upper}-{center_inside}"),
                )
                .unwrap();
                union.brep.validate().unwrap();
                assert_eq!(union.brep.topology.faces.len(), 7);
                for point in [void_point, material_point, sphere_only_point] {
                    assert_eq!(
                        classify_point(&union.brep, point).unwrap(),
                        PointClassification::Inside
                    );
                }

                let sphere_cut = boolean_brep(
                    &cutter,
                    &host,
                    BooleanOp::Subtraction,
                    format!("sphere-cut-{axis}-{upper}-{center_inside}"),
                )
                .unwrap();
                sphere_cut.brep.validate().unwrap();
                assert_eq!(sphere_cut.brep.topology.faces.len(), 2);
                assert!(sphere_cut.brep.topology.faces.iter().any(|face| {
                    face.provenance.role == FaceRole::Cut && face.provenance.reversed
                }));
                assert_eq!(
                    classify_point(&sphere_cut.brep, void_point).unwrap(),
                    PointClassification::Outside
                );
                assert_eq!(
                    classify_point(&sphere_cut.brep, sphere_only_point).unwrap(),
                    PointClassification::Inside
                );
                tessellate(&subtraction.brep, 0.01, 2_000_000).unwrap();
                tessellate(&intersection.brep, 0.01, 2_000_000).unwrap();
                tessellate(&union.brep, 0.01, 2_000_000).unwrap();
                tessellate(&sphere_cut.brep, 0.01, 2_000_000).unwrap();
            }
        }
    }
}
