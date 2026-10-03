use super::support::{extrusion, extrusion_span, volume, volume_at_deflection};
use crate::brep::{FaceRole, Frame3, GeometryError, SurfaceGeometry};
use crate::exchange::export_step;
use crate::operations::modifying::boolean::batch::subtract_planar_cutters;
use crate::operations::modifying::boolean::dispatch::boolean_brep;
use crate::operations::modifying::boolean::types::BooleanOp;
use crate::primitives;
use crate::query::{classify_point, face_contains_uv, PointClassification};
use crate::tessellation::tessellate;

#[test]
fn planar_extrusion_round_through_cutter_keeps_analytic_brep() {
    let host = extrusion(
        "round-host",
        vec![[0.0, 0.0], [4.0, 0.0], [4.0, 0.3], [0.0, 0.3]],
        Vec::new(),
    );
    let cutter = primitives::cylinder(
        "round-opening".into(),
        Frame3 {
            origin: [2.0, -0.5, 1.5],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 0.0, -1.0],
            z: [0.0, 1.0, 0.0],
        },
        0.5,
        1.3,
        host.accuracy,
    )
    .unwrap();
    let result = boolean_brep(
        &host,
        &cutter,
        BooleanOp::Subtraction,
        "round-host-cut".into(),
    )
    .unwrap();
    assert_eq!(result.brep.solids.len(), 1);
    result.brep.validate().unwrap();
    assert_eq!(
        classify_point(&result.brep, [2.0, 0.15, 1.5]).unwrap(),
        PointClassification::Outside
    );
    assert_eq!(
        classify_point(&result.brep, [2.0, 0.15, 2.2]).unwrap(),
        PointClassification::Inside
    );
    let cut_face = result
        .brep
        .topology
        .faces
        .iter()
        .find(|face| face.provenance.role == FaceRole::Cut)
        .unwrap();
    assert_eq!(
        face_contains_uv(&result.brep, cut_face, [0.0, 0.65]).unwrap(),
        Some(true)
    );
    tessellate(&result.brep, 0.01, 2_000_000).unwrap();
    export_step(&result.brep, "metre").unwrap();
}

#[test]
fn rectangular_cut_after_round_cut_never_loses_the_cylindrical_cap() {
    let host = extrusion(
        "arched-host",
        vec![[0.0, 0.0], [4.0, 0.0], [4.0, 0.3], [0.0, 0.3]],
        Vec::new(),
    );
    let round = primitives::cylinder(
        "arched-cap".into(),
        Frame3 {
            origin: [2.0, -0.5, 2.0],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 0.0, -1.0],
            z: [0.0, 1.0, 0.0],
        },
        0.5,
        1.3,
        host.accuracy,
    )
    .unwrap();
    let lower = primitives::linear_extrusion(
        "arched-lower".into(),
        Frame3 {
            origin: [0.0, 0.8, 0.0],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 0.0, 1.0],
            z: [0.0, -1.0, 0.0],
        },
        vec![[1.5, 0.5], [2.5, 0.5], [2.5, 2.0], [1.5, 2.0]],
        Vec::new(),
        1.3,
        host.accuracy,
    )
    .unwrap();
    let batch = subtract_planar_cutters(
        &host,
        &[round.clone(), lower.clone()],
        "arched-batch".into(),
    )
    .unwrap();
    batch.brep.validate().unwrap();
    assert_eq!(
        classify_point(&batch.brep, [2.0, 0.15, 2.2]).unwrap(),
        PointClassification::Outside
    );
    assert_eq!(
        classify_point(&batch.brep, [2.0, 0.15, 1.0]).unwrap(),
        PointClassification::Outside
    );
    assert!(batch.brep.topology.faces.iter().any(|face| {
        matches!(
            batch.brep.geometry.surfaces[face.surface as usize],
            SurfaceGeometry::Cylinder { .. }
        )
    }));
    for source in ["arched-cap", "arched-lower"] {
        assert!(batch.report.face_mappings.iter().any(|mapping| {
            mapping.source.entity == source && !mapping.result_faces.is_empty()
        }));
    }
    let expected = 4.0 * 0.3 * 3.0 - (1.0 * 1.5 + std::f64::consts::PI * 0.5 * 0.5 / 2.0) * 0.3;
    assert!((volume_at_deflection(&batch.brep, 0.0001).abs() - expected).abs() < 1.0e-4);
    export_step(&batch.brep, "metre").unwrap();
    let cap = boolean_brep(&host, &round, BooleanOp::Subtraction, "cap".into()).unwrap();
    match boolean_brep(&cap.brep, &lower, BooleanOp::Subtraction, "arch".into()) {
        Ok(result) => {
            result.brep.validate().unwrap();
            assert!(result.brep.topology.faces.iter().any(|face| {
                matches!(
                    result.brep.geometry.surfaces[face.surface as usize],
                    SurfaceGeometry::Cylinder { .. }
                )
            }));
            assert_eq!(
                classify_point(&result.brep, [2.0, 0.15, 2.2]).unwrap(),
                PointClassification::Outside
            );
            assert_eq!(
                classify_point(&result.brep, [2.0, 0.15, 1.0]).unwrap(),
                PointClassification::Outside
            );
        }
        Err(GeometryError::CoverageGap { .. }) => {}
        Err(error) => panic!("unexpected chained arch failure: {error:?}"),
    }
}

#[test]
fn disjoint_mixed_cutter_batch_preserves_both_openings_in_any_order() {
    let host = extrusion(
        "mixed-host",
        vec![[0.0, 0.0], [6.0, 0.0], [6.0, 0.3], [0.0, 0.3]],
        Vec::new(),
    );
    let round = primitives::cylinder(
        "round".into(),
        Frame3 {
            origin: [1.5, -0.5, 1.5],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 0.0, -1.0],
            z: [0.0, 1.0, 0.0],
        },
        0.4,
        1.3,
        host.accuracy,
    )
    .unwrap();
    let rect = primitives::linear_extrusion(
        "rectangle".into(),
        Frame3 {
            origin: [0.0, 0.8, 0.0],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 0.0, 1.0],
            z: [0.0, -1.0, 0.0],
        },
        vec![[3.5, 0.7], [4.5, 0.7], [4.5, 2.2], [3.5, 2.2]],
        Vec::new(),
        1.3,
        host.accuracy,
    )
    .unwrap();
    for cutters in [[round.clone(), rect.clone()], [rect.clone(), round.clone()]] {
        let result = subtract_planar_cutters(&host, &cutters, "mixed".into()).unwrap();
        result.brep.validate().unwrap();
        assert_eq!(
            classify_point(&result.brep, [1.5, 0.15, 1.5]).unwrap(),
            PointClassification::Outside
        );
        assert_eq!(
            classify_point(&result.brep, [4.0, 0.15, 1.5]).unwrap(),
            PointClassification::Outside
        );
        assert_eq!(
            classify_point(&result.brep, [2.5, 0.15, 1.5]).unwrap(),
            PointClassification::Inside
        );
        let expected_volume =
            6.0 * 0.3 * 3.0 - 1.0 * 0.3 * 1.5 - std::f64::consts::PI * 0.4 * 0.4 * 0.3;
        let measured_volume = volume_at_deflection(&result.brep, 0.0001).abs();
        assert!(
            (measured_volume - expected_volume).abs() < 1.0e-4,
            "mixed volume {measured_volume} differs from expected {expected_volume}",
        );
        assert!(result.brep.topology.faces.iter().any(|face| {
            matches!(
                result.brep.geometry.surfaces[face.surface as usize],
                SurfaceGeometry::Cylinder { .. }
            )
        }));
        for source in ["round", "rectangle"] {
            assert!(
                result.brep.topology.faces.iter().any(|face| {
                    face.provenance
                        .sources
                        .iter()
                        .any(|origin| origin.entity == source)
                }),
                "missing source {source}: {:?}",
                result
                    .brep
                    .topology
                    .faces
                    .iter()
                    .flat_map(|face| face.provenance.sources.iter().map(|origin| &origin.entity))
                    .collect::<Vec<_>>()
            );
            assert!(result.report.face_mappings.iter().any(|mapping| {
                mapping.source.entity == source && !mapping.result_faces.is_empty()
            }));
        }
        export_step(&result.brep, "metre").unwrap();
    }
}

#[test]
fn two_round_cutter_batch_keeps_both_analytic_voids() {
    let host = extrusion(
        "two-round-host",
        vec![[0.0, 0.0], [6.0, 0.0], [6.0, 0.3], [0.0, 0.3]],
        Vec::new(),
    );
    let make_round = |name: &str, station: f64| {
        primitives::cylinder(
            name.into(),
            Frame3 {
                origin: [station, 0.0, 1.5],
                x: [1.0, 0.0, 0.0],
                y: [0.0, 0.0, -1.0],
                z: [0.0, 1.0, 0.0],
            },
            0.4,
            0.3,
            host.accuracy,
        )
        .unwrap()
    };
    let first = make_round("round-a", 1.5);
    let second = make_round("round-b", 4.5);
    for cutters in [
        [first.clone(), second.clone()],
        [second.clone(), first.clone()],
    ] {
        let result = subtract_planar_cutters(&host, &cutters, "two-round".into()).unwrap();
        result.brep.validate().unwrap();
        assert_eq!(result.brep.solids.len(), 1);
        for station in [1.5, 4.5] {
            assert_eq!(
                classify_point(&result.brep, [station, 0.15, 1.5]).unwrap(),
                PointClassification::Outside
            );
        }
        assert_eq!(
            classify_point(&result.brep, [3.0, 0.15, 1.5]).unwrap(),
            PointClassification::Inside
        );
        let expected = 6.0 * 0.3 * 3.0 - 2.0 * std::f64::consts::PI * 0.4 * 0.4 * 0.3;
        assert!((volume_at_deflection(&result.brep, 0.0001).abs() - expected).abs() < 1.0e-4);
        for source in ["round-a", "round-b"] {
            assert!(
                result
                    .report
                    .face_mappings
                    .iter()
                    .any(|mapping| mapping.source.entity == source
                        && !mapping.result_faces.is_empty())
            );
        }
        let (_, report) = export_step(&result.brep, "metre").unwrap();
        assert_eq!(report.solids, 1);
    }
}

#[test]
fn mixed_profile_batch_preserves_disconnected_full_height_host_parts() {
    let host = extrusion(
        "split-mixed-host",
        vec![[0.0, 0.0], [6.0, 0.0], [6.0, 0.3], [0.0, 0.3]],
        Vec::new(),
    );
    let round = primitives::cylinder(
        "split-round".into(),
        Frame3 {
            origin: [1.5, -0.5, 1.5],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 0.0, -1.0],
            z: [0.0, 1.0, 0.0],
        },
        0.4,
        1.3,
        host.accuracy,
    )
    .unwrap();
    let through = primitives::linear_extrusion(
        "full-height".into(),
        Frame3 {
            origin: [0.0, 0.3, 0.0],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 0.0, 1.0],
            z: [0.0, -1.0, 0.0],
        },
        vec![[2.5, 0.0], [3.5, 0.0], [3.5, 3.0], [2.5, 3.0]],
        Vec::new(),
        0.3,
        host.accuracy,
    )
    .unwrap();
    for cutters in [
        [round.clone(), through.clone()],
        [through.clone(), round.clone()],
    ] {
        let result = subtract_planar_cutters(&host, &cutters, "split-mixed".into()).unwrap();
        result.brep.validate().unwrap();
        assert_eq!(result.brep.solids.len(), 2);
        for point in [[1.5, 0.15, 1.5], [3.0, 0.15, 1.5]] {
            assert_eq!(
                classify_point(&result.brep, point).unwrap(),
                PointClassification::Outside
            );
        }
        for point in [[0.5, 0.15, 1.5], [5.0, 0.15, 1.5]] {
            assert_eq!(
                classify_point(&result.brep, point).unwrap(),
                PointClassification::Inside
            );
        }
        let expected = 6.0 * 0.3 * 3.0 - 1.0 * 0.3 * 3.0 - std::f64::consts::PI * 0.4 * 0.4 * 0.3;
        assert!((volume_at_deflection(&result.brep, 0.0001).abs() - expected).abs() < 1.0e-4);
        for source in ["split-round", "full-height"] {
            assert!(
                result
                    .report
                    .face_mappings
                    .iter()
                    .any(|mapping| mapping.source.entity == source
                        && !mapping.result_faces.is_empty())
            );
        }
        let (_, report) = export_step(&result.brep, "metre").unwrap();
        assert_eq!(report.solids, 2);
    }
}

#[test]
fn flush_mixed_cutters_cross_a_prior_planar_face_split_without_losing_the_round_hole() {
    let host = extrusion(
        "flush-mixed-host",
        vec![[0.0, 0.0], [6.0, 0.0], [6.0, 0.3], [0.0, 0.3]],
        Vec::new(),
    );
    let round = primitives::cylinder(
        "flush-round".into(),
        Frame3 {
            origin: [1.5, 0.0, 1.5],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 0.0, -1.0],
            z: [0.0, 1.0, 0.0],
        },
        0.4,
        0.3,
        host.accuracy,
    )
    .unwrap();
    let rectangle = primitives::linear_extrusion(
        "flush-rectangle".into(),
        Frame3 {
            origin: [0.0, 0.3, 0.0],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 0.0, 1.0],
            z: [0.0, -1.0, 0.0],
        },
        vec![[3.5, 0.7], [4.5, 0.7], [4.5, 1.5], [3.5, 1.5]],
        Vec::new(),
        0.3,
        host.accuracy,
    )
    .unwrap();
    for cutters in [
        [round.clone(), rectangle.clone()],
        [rectangle.clone(), round.clone()],
    ] {
        let result = subtract_planar_cutters(&host, &cutters, "flush-mixed".into()).unwrap();
        result.brep.validate().unwrap();
        assert_eq!(
            classify_point(&result.brep, [1.5, 0.15, 1.5]).unwrap(),
            PointClassification::Outside
        );
        assert_eq!(
            classify_point(&result.brep, [4.0, 0.15, 1.4]).unwrap(),
            PointClassification::Outside
        );
        assert_eq!(
            classify_point(&result.brep, [2.5, 0.15, 1.5]).unwrap(),
            PointClassification::Inside
        );
        assert!(result.brep.topology.faces.iter().any(|face| {
            matches!(
                result.brep.geometry.surfaces[face.surface as usize],
                SurfaceGeometry::Cylinder { .. }
            )
        }));
        for source in ["flush-round", "flush-rectangle"] {
            assert!(result.report.face_mappings.iter().any(|mapping| {
                mapping.source.entity == source && !mapping.result_faces.is_empty()
            }));
        }
        export_step(&result.brep, "metre").unwrap();
    }
}

#[test]
fn ten_flush_planar_cutters_use_one_rectilinear_arrangement() {
    let host = extrusion(
        "host",
        vec![[0.0, 0.0], [20.0, 0.0], [20.0, 0.3], [0.0, 0.3]],
        Vec::new(),
    );
    let cutters = (0..10)
        .map(|index| {
            let left = 1.0 + index as f64 * 1.8;
            extrusion_span(
                &format!("opening-{index}"),
                0.0,
                2.1,
                vec![
                    [left, 0.0],
                    [left + 0.5, 0.0],
                    [left + 0.5, 0.3],
                    [left, 0.3],
                ],
                Vec::new(),
            )
        })
        .collect::<Vec<_>>();
    let result = subtract_planar_cutters(&host, &cutters, "batch".into()).unwrap();
    result.brep.validate().unwrap();
    assert!((volume(&result.brep).abs() - 14.85).abs() < 1.0e-6);
    for index in 0..10 {
        let station = 1.25 + index as f64 * 1.8;
        assert_eq!(
            classify_point(&result.brep, [station, 0.15, 1.0]).unwrap(),
            PointClassification::Outside,
        );
    }
    assert!(result
        .report
        .face_mappings
        .iter()
        .any(|mapping| mapping.source.entity == "opening-9"));
}
