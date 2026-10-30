use super::support::{extrusion, extrusion_span, volume};
use crate::brep::{Accuracy, BrepEnvelope, FaceRole, Frame3, GeometryError, GeometryQuality};
use crate::math::Point3;
use crate::operations::modifying::boolean::dispatch::boolean_brep;
use crate::operations::modifying::boolean::types::BooleanOp;
use crate::primitives;
use crate::query::{classify_point, PointClassification};

#[test]
fn coextensive_profile_extrusions_use_exact_planar_arrangements() {
    let a = extrusion(
        "a",
        vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]],
        Vec::new(),
    );
    let b = extrusion(
        "b",
        vec![[2.0, -1.0], [5.0, -1.0], [5.0, 2.0], [2.0, 2.0]],
        Vec::new(),
    );
    for (operation, expected_volume) in [
        (BooleanOp::Union, 63.0),
        (BooleanOp::Intersection, 12.0),
        (BooleanOp::Subtraction, 36.0),
    ] {
        let result = boolean_brep(&a, &b, operation, format!("{operation:?}")).unwrap();
        result.brep.validate().unwrap();
        assert!(matches!(result.brep.quality, GeometryQuality::Analytic));
        assert!((volume(&result.brep).abs() - expected_volume).abs() < 1.0e-6);
        assert!(result
            .report
            .face_mappings
            .iter()
            .any(|mapping| !mapping.result_faces.is_empty()));
    }
}

#[test]
fn profile_subtraction_builds_holes_disconnected_solids_and_cut_provenance() {
    let host = extrusion(
        "host",
        vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]],
        Vec::new(),
    );
    let inner = extrusion(
        "inner",
        vec![[4.0, 4.0], [6.0, 4.0], [6.0, 6.0], [4.0, 6.0]],
        Vec::new(),
    );
    let cavity = boolean_brep(&host, &inner, BooleanOp::Subtraction, "cavity".into()).unwrap();
    assert_eq!(cavity.brep.solids.len(), 1);
    assert_eq!(cavity.brep.topology.faces[0].trim.holes.len(), 1);
    assert!((volume(&cavity.brep).abs() - 288.0).abs() < 1.0e-6);
    assert!(cavity
        .brep
        .topology
        .faces
        .iter()
        .any(|face| { face.provenance.role == FaceRole::Cut && face.provenance.reversed }));

    let band = extrusion(
        "band",
        vec![[-1.0, 4.0], [11.0, 4.0], [11.0, 6.0], [-1.0, 6.0]],
        Vec::new(),
    );
    let split = boolean_brep(&host, &band, BooleanOp::Subtraction, "split".into()).unwrap();
    assert_eq!(split.brep.solids.len(), 2);
    assert!((volume(&split.brep).abs() - 240.0).abs() < 1.0e-6);
}

#[test]
fn matching_profile_extrusions_use_exact_axial_interval_booleans() {
    let profile = vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]];
    let a = extrusion_span("a", 0.0, 4.0, profile.clone(), Vec::new());
    let b = extrusion_span(
        "b",
        2.0,
        4.0,
        vec![[4.0, 4.0], [0.0, 4.0], [0.0, 0.0], [4.0, 0.0]],
        Vec::new(),
    );
    for (operation, expected_volume) in [
        (BooleanOp::Union, 96.0),
        (BooleanOp::Intersection, 32.0),
        (BooleanOp::Subtraction, 32.0),
    ] {
        let result = boolean_brep(&a, &b, operation, format!("axial-{operation:?}")).unwrap();
        result.brep.validate().unwrap();
        assert_eq!(result.brep.solids.len(), 1);
        assert!((volume(&result.brep).abs() - expected_volume).abs() < 1.0e-6);
        assert!(result
            .report
            .face_mappings
            .iter()
            .any(|mapping| !mapping.result_faces.is_empty()));
    }

    let embedded = extrusion_span("embedded", 1.0, 2.0, profile.clone(), Vec::new());
    let split = boolean_brep(&a, &embedded, BooleanOp::Subtraction, "axial-split".into()).unwrap();
    split.brep.validate().unwrap();
    assert_eq!(split.brep.solids.len(), 2);
    assert!((volume(&split.brep).abs() - 32.0).abs() < 1.0e-6);
    assert_eq!(
        split
            .brep
            .topology
            .faces
            .iter()
            .filter(|face| face.provenance.role == FaceRole::Cut && face.provenance.reversed)
            .count(),
        2
    );

    let disjoint = extrusion_span("disjoint", 6.0, 2.0, profile, Vec::new());
    let union = boolean_brep(&a, &disjoint, BooleanOp::Union, "axial-disjoint".into()).unwrap();
    assert_eq!(union.brep.solids.len(), 2);
    assert!((volume(&union.brep).abs() - 96.0).abs() < 1.0e-6);
    let intersection =
        boolean_brep(&a, &disjoint, BooleanOp::Intersection, "axial-empty".into()).unwrap();
    assert!(intersection.brep.solids.is_empty());
}

#[test]
fn different_height_planar_openings_support_chained_flush_cuts() {
    let host = extrusion(
        "host",
        vec![[0.0, 0.0], [10.0, 0.0], [10.0, 0.3], [0.0, 0.3]],
        Vec::new(),
    );
    let first = extrusion_span(
        "lower_cutout",
        0.0,
        2.1,
        vec![[2.0, 0.0], [3.0, 0.0], [3.0, 0.3], [2.0, 0.3]],
        Vec::new(),
    );
    let cut = boolean_brep(
        &host,
        &first,
        BooleanOp::Subtraction,
        "one-lower_cutout".into(),
    )
    .unwrap();
    cut.brep.validate().unwrap();
    assert!((volume(&cut.brep).abs() - 8.37).abs() < 1.0e-6);
    assert_eq!(
        classify_point(&cut.brep, [2.5, 0.15, 1.0]).unwrap(),
        PointClassification::Outside
    );
    assert_eq!(
        classify_point(&cut.brep, [2.5, 0.15, 2.5]).unwrap(),
        PointClassification::Inside
    );
    let second = extrusion_span(
        "raised_cutout",
        0.8,
        1.2,
        vec![[6.0, 0.0], [7.0, 0.0], [7.0, 0.3], [6.0, 0.3]],
        Vec::new(),
    );
    let chained = boolean_brep(
        &cut.brep,
        &second,
        BooleanOp::Subtraction,
        "lower_cutout-raised_cutout".into(),
    )
    .unwrap();
    chained.brep.validate().unwrap();
    assert!((volume(&chained.brep).abs() - 8.01).abs() < 1.0e-6);
    assert_eq!(
        classify_point(&chained.brep, [6.5, 0.15, 1.2]).unwrap(),
        PointClassification::Outside
    );
}

#[test]
fn chained_mitered_host_openings_keep_top_cap_provenance() {
    let host = extrusion_span(
        "mitered-host",
        0.0,
        3.8,
        vec![
            [-0.16, -0.14],
            [0.16, -0.46],
            [13.84, -0.46],
            [14.16, -0.14],
        ],
        Vec::new(),
    );
    let lower_cutout = extrusion_span(
        "lower_cutout",
        0.0,
        2.45,
        vec![[1.2, -0.14], [2.8, -0.14], [2.8, -0.46], [1.2, -0.46]],
        Vec::new(),
    );
    let raised_cutout = extrusion_span(
        "raised_cutout",
        0.3,
        2.8,
        vec![
            [3.625, -0.14],
            [5.175, -0.14],
            [5.175, -0.46],
            [3.625, -0.46],
        ],
        Vec::new(),
    );
    let first = boolean_brep(
        &host,
        &lower_cutout,
        BooleanOp::Subtraction,
        "lower_cutout-cut".into(),
    )
    .unwrap();
    let second = boolean_brep(
        &first.brep,
        &raised_cutout,
        BooleanOp::Subtraction,
        "raised_cutout-cut".into(),
    )
    .unwrap();
    second.brep.validate().unwrap();
    assert_eq!(
        classify_point(&second.brep, [2.0, -0.3, 1.0]).unwrap(),
        PointClassification::Outside,
    );
    assert_eq!(
        classify_point(&second.brep, [4.4, -0.3, 1.0]).unwrap(),
        PointClassification::Outside,
    );
}

#[test]
fn angled_host_cap_accepts_flush_lower_cutout_cutter() {
    let host = extrusion(
        "angled-host",
        vec![[0.0, 0.0], [10.0, 0.0], [10.0, 0.3], [0.3, 0.3], [0.0, 0.1]],
        Vec::new(),
    );
    let cutter = extrusion_span(
        "lower_cutout",
        0.0,
        2.1,
        vec![[2.0, 0.0], [3.0, 0.0], [3.0, 0.3], [2.0, 0.3]],
        Vec::new(),
    );
    let result = boolean_brep(&host, &cutter, BooleanOp::Subtraction, "angled-cut".into()).unwrap();
    result.brep.validate().unwrap();
    assert!((volume(&result.brep).abs() - (volume(&host).abs() - 0.63)).abs() < 1.0e-6);
    assert_eq!(
        classify_point(&result.brep, [2.5, 0.15, 1.0]).unwrap(),
        PointClassification::Outside
    );
    let raised_cutout = extrusion_span(
        "raised_cutout",
        0.8,
        1.2,
        vec![[6.0, 0.0], [7.0, 0.0], [7.0, 0.3], [6.0, 0.3]],
        Vec::new(),
    );
    let chained = boolean_brep(
        &result.brep,
        &raised_cutout,
        BooleanOp::Subtraction,
        "angled-chain".into(),
    )
    .unwrap();
    chained.brep.validate().unwrap();
    assert!((volume(&chained.brep).abs() - (volume(&host).abs() - 0.99)).abs() < 1.0e-6);
    assert_eq!(
        classify_point(&chained.brep, [6.5, 0.15, 1.2]).unwrap(),
        PointClassification::Outside
    );
    assert_eq!(
        classify_point(&chained.brep, [6.5, 0.15, 2.5]).unwrap(),
        PointClassification::Inside
    );
}

#[test]
fn full_height_planar_opening_splits_angled_host_into_two_solids() {
    let host = extrusion(
        "angled-host",
        vec![[0.0, 0.0], [10.0, 0.0], [10.0, 0.3], [0.3, 0.3], [0.0, 0.1]],
        Vec::new(),
    );
    let cutter = extrusion(
        "through-lower_cutout",
        vec![[4.0, 0.0], [5.0, 0.0], [5.0, 0.3], [4.0, 0.3]],
        Vec::new(),
    );
    let result = boolean_brep(
        &host,
        &cutter,
        BooleanOp::Subtraction,
        "split-angled".into(),
    )
    .unwrap();
    result.brep.validate().unwrap();
    assert_eq!(result.brep.solids.len(), 2);
    assert!((volume(&result.brep).abs() - (volume(&host).abs() - 0.9)).abs() < 1.0e-6);
}

#[test]
fn overlapping_flush_openings_on_angled_host_remove_the_union_once() {
    let host = extrusion(
        "angled-host",
        vec![[0.0, 0.0], [10.0, 0.0], [10.0, 0.3], [0.3, 0.3], [0.0, 0.1]],
        Vec::new(),
    );
    let first = extrusion_span(
        "first",
        0.0,
        2.1,
        vec![[2.0, 0.0], [3.0, 0.0], [3.0, 0.3], [2.0, 0.3]],
        Vec::new(),
    );
    let second = extrusion_span(
        "second",
        0.0,
        2.1,
        vec![[2.5, 0.0], [3.5, 0.0], [3.5, 0.3], [2.5, 0.3]],
        Vec::new(),
    );
    let once = boolean_brep(&host, &first, BooleanOp::Subtraction, "once".into()).unwrap();
    let twice = boolean_brep(&once.brep, &second, BooleanOp::Subtraction, "twice".into()).unwrap();
    twice.brep.validate().unwrap();
    assert!((volume(&twice.brep).abs() - (volume(&host).abs() - 0.945)).abs() < 1.0e-6);
}

#[test]
fn layered_planar_cut_supports_an_internal_cavity_shell() {
    let host = extrusion(
        "angled-host",
        vec![
            [0.0, 0.0],
            [10.0, 0.0],
            [10.0, 10.0],
            [0.3, 10.0],
            [0.0, 9.8],
        ],
        Vec::new(),
    );
    let cutter = extrusion_span(
        "cavity",
        1.0,
        1.0,
        vec![[4.0, 4.0], [6.0, 4.0], [6.0, 6.0], [4.0, 6.0]],
        Vec::new(),
    );
    let result = boolean_brep(
        &host,
        &cutter,
        BooleanOp::Subtraction,
        "cavity-result".into(),
    )
    .unwrap();
    result.brep.validate().unwrap();
    assert_eq!(result.brep.solids.len(), 1);
    assert_eq!(result.brep.solids[0].cavity_shells.len(), 1);
    assert!((volume(&result.brep).abs() - (volume(&host).abs() - 4.0)).abs() < 1.0e-6);
}

#[test]
fn angled_host_accepts_rotated_planar_cutter_frame() {
    let host = extrusion(
        "angled-host",
        vec![[0.0, 0.0], [10.0, 0.0], [10.0, 0.3], [0.3, 0.3], [0.0, 0.1]],
        Vec::new(),
    );
    let angle = std::f64::consts::FRAC_PI_6;
    let cutter = primitives::linear_extrusion(
        "rotated-cutter".into(),
        Frame3 {
            origin: [2.5, 0.15, 0.0],
            x: [angle.cos(), angle.sin(), 0.0],
            y: [-angle.sin(), angle.cos(), 0.0],
            z: [0.0, 0.0, 1.0],
        },
        vec![[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]],
        Vec::new(),
        2.1,
        host.accuracy,
    )
    .unwrap();
    let result =
        boolean_brep(&host, &cutter, BooleanOp::Subtraction, "rotated-cut".into()).unwrap();
    result.brep.validate().unwrap();
    assert_eq!(
        classify_point(&result.brep, [2.5, 0.15, 1.0]).unwrap(),
        PointClassification::Outside
    );
    assert_eq!(
        classify_point(&result.brep, [2.5, 0.15, 2.5]).unwrap(),
        PointClassification::Inside
    );
    assert!(volume(&result.brep).abs() < volume(&host).abs());
}

fn region_prism(id: &str, outer: [f64; 4], hole: Option<[f64; 4]>, geometric: f64) -> BrepEnvelope {
    let ring = |b: [f64; 4]| vec![[b[0], b[1]], [b[2], b[1]], [b[2], b[3]], [b[0], b[3]]];
    primitives::linear_extrusion(
        id.into(),
        Frame3::IDENTITY,
        ring(outer),
        hole.map(ring).into_iter().collect(),
        3.0,
        Accuracy {
            geometric,
            intersection: geometric / 4.0,
            tessellation: 0.01,
            exchange: 1e-5,
        },
    )
    .unwrap()
}

fn material_at(brep: &BrepEnvelope, points: &[Point3]) -> Vec<bool> {
    points
        .iter()
        .map(|point| classify_point(brep, *point).unwrap() == PointClassification::Inside)
        .collect()
}

#[test]
fn union_of_a_frame_prism_and_an_island_prism_keeps_each_hole_in_both_orders() {
    let frame = region_prism(
        "frame",
        [0.0, 0.0, 10.0, 10.0],
        Some([1.0, 1.0, 9.0, 9.0]),
        1e-7,
    );
    let island = region_prism(
        "island",
        [3.0, 3.0, 7.0, 7.0],
        Some([4.0, 4.0, 6.0, 6.0]),
        1e-7,
    );
    for (first, second) in [(&frame, &island), (&island, &frame)] {
        let united = boolean_brep(first, second, BooleanOp::Union, "united".into())
            .unwrap()
            .brep;
        assert_eq!(united.solids.len(), 2);
        assert_eq!(united.topology.faces.len(), 20);
        assert_eq!(
            material_at(
                &united,
                &[
                    [0.5, 0.5, 1.5],
                    [2.0, 2.0, 1.5],
                    [3.5, 3.5, 1.5],
                    [5.0, 5.0, 1.5]
                ]
            ),
            [true, false, true, false]
        );
    }
}

#[test]
fn a_profile_hole_inside_the_old_probe_reach_of_its_edge_survives_a_planar_boolean() {
    let host = region_prism(
        "host",
        [0.0, 0.0, 1.0, 1.0],
        Some([2.0e-5, 0.2, 0.5, 0.8]),
        1e-6,
    );
    let cutter = region_prism("cutter", [0.6, -0.5, 0.7, 1.5], None, 1e-6);
    let points = [
        [1.0e-5, 0.5, 1.5],
        [0.25, 0.5, 1.5],
        [0.65, 0.5, 1.5],
        [0.9, 0.5, 1.5],
    ];
    let cut = boolean_brep(&host, &cutter, BooleanOp::Subtraction, "cut".into())
        .unwrap()
        .brep;
    assert_eq!(cut.solids.len(), 2);
    assert_eq!(cut.topology.faces.len(), 16);
    assert_eq!(material_at(&cut, &points), [true, false, false, true]);
    let united = boolean_brep(&host, &cutter, BooleanOp::Union, "united".into())
        .unwrap()
        .brep;
    assert_eq!(united.solids.len(), 1);
    assert_eq!(united.topology.faces.len(), 18);
    assert_eq!(material_at(&united, &points), [true, false, true, true]);
}

#[test]
fn a_region_boundary_that_cannot_be_closed_is_an_invalid_topology_error() {
    let long = region_prism("long", [0.0, 0.0, 1000.0, 1.0], None, 1e-5);
    let beside_its_end = region_prism("beside", [999.9995, 1.0, 1000.5, 2.0], None, 1e-5);
    let result = boolean_brep(&long, &beside_its_end, BooleanOp::Union, "united".into());
    assert!(
        matches!(&result, Err(GeometryError::InvalidTopology(message)) if message == "layered planar Boolean has an open boundary"),
        "{:?}",
        result.map(|result| result.brep.topology.faces.len())
    );
}
