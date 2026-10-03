use super::support::sphere;
use crate::brep::{Frame3, SurfaceGeometry};
use crate::math::dot;
use crate::operations::modifying::boolean::dispatch::boolean_brep;
use crate::operations::modifying::boolean::types::BooleanOp;
use crate::primitives;
use crate::query::{classify_point, PointClassification};
use crate::tessellation::tessellate;

#[test]
fn curved_internal_cut_creates_one_cavity_shell_in_its_host_solid() {
    let accuracy = sphere("accuracy", [0.0; 3], 1.0).accuracy;
    let quarter = std::f64::consts::FRAC_PI_2;
    let host = primitives::arc_edged_extrusion(
        "curved-cavity-host".into(),
        Frame3::IDENTITY,
        vec![
            primitives::ProfileEdge::Arc {
                center: [0.0, 0.0],
                radius: 2.0,
                start_angle: 0.0,
                sweep_angle: quarter,
            },
            primitives::ProfileEdge::Line {
                from: [0.0, 2.0],
                to: [0.0, 1.0],
            },
            primitives::ProfileEdge::Arc {
                center: [0.0, 0.0],
                radius: 1.0,
                start_angle: quarter,
                sweep_angle: -quarter,
            },
            primitives::ProfileEdge::Line {
                from: [1.0, 0.0],
                to: [2.0, 0.0],
            },
        ],
        3.0,
        accuracy,
    )
    .unwrap();
    let cutter = primitives::linear_extrusion(
        "curved-interior-cutter".into(),
        Frame3 {
            origin: [0.0, 0.0, 1.0],
            ..Frame3::IDENTITY
        },
        vec![[1.0, 1.0], [1.2, 1.0], [1.2, 1.2], [1.0, 1.2]],
        vec![],
        1.0,
        accuracy,
    )
    .unwrap();
    let result = boolean_brep(
        &host,
        &cutter,
        BooleanOp::Subtraction,
        "curved-cavity".into(),
    )
    .unwrap();
    result.brep.validate().unwrap();
    tessellate(&result.brep, 0.01, 2_000_000).unwrap();
    assert_eq!(result.brep.solids.len(), 1);
    assert_eq!(result.brep.topology.shells.len(), 2);
    assert_eq!(result.brep.solids[0].cavity_shells.len(), 1);
    assert_eq!(
        classify_point(&result.brep, [1.1, 1.1, 1.5]).unwrap(),
        PointClassification::Outside
    );
    assert_eq!(
        classify_point(&result.brep, [1.5, 0.5, 1.5]).unwrap(),
        PointClassification::Inside
    );

    let split_sector = primitives::linear_extrusion(
        "curved-split-sector".into(),
        Frame3 {
            origin: [0.0, 0.0, -1.0],
            ..Frame3::IDENTITY
        },
        vec![
            [0.0, 0.0],
            [3.0 * 0.2_f64.cos(), 3.0 * 0.2_f64.sin()],
            [3.0 * 0.3_f64.cos(), 3.0 * 0.3_f64.sin()],
        ],
        vec![],
        5.0,
        accuracy,
    )
    .unwrap();
    let split = boolean_brep(
        &host,
        &split_sector,
        BooleanOp::Subtraction,
        "curved-split".into(),
    )
    .unwrap();
    assert_eq!(split.brep.solids.len(), 2);
    let split_with_cavity = boolean_brep(
        &split.brep,
        &cutter,
        BooleanOp::Subtraction,
        "curved-split-with-cavity".into(),
    )
    .unwrap();
    split_with_cavity.brep.validate().unwrap();
    assert_eq!(split_with_cavity.brep.solids.len(), 2);
    assert_eq!(split_with_cavity.brep.topology.shells.len(), 3);
    assert_eq!(
        split_with_cavity
            .brep
            .solids
            .iter()
            .map(|solid| solid.cavity_shells.len())
            .sum::<usize>(),
        1
    );
    assert_eq!(
        classify_point(&split_with_cavity.brep, [1.1, 1.1, 1.5]).unwrap(),
        PointClassification::Outside
    );

    let second_cutter = primitives::linear_extrusion(
        "curved-other-component-cutter".into(),
        Frame3 {
            origin: [0.0, 0.0, 1.1],
            ..Frame3::IDENTITY
        },
        vec![[1.55, 0.1], [1.67, 0.1], [1.67, 0.22], [1.55, 0.22]],
        vec![],
        0.8,
        accuracy,
    )
    .unwrap();
    let same_component_cavities = boolean_brep(
        &result.brep,
        &second_cutter,
        BooleanOp::Subtraction,
        "curved-same-component-cavities".into(),
    )
    .unwrap();
    same_component_cavities.brep.validate().unwrap();
    assert_eq!(same_component_cavities.brep.solids.len(), 1);
    assert_eq!(same_component_cavities.brep.topology.shells.len(), 3);
    assert_eq!(
        same_component_cavities.brep.solids[0].cavity_shells.len(),
        2
    );
    let two_cavities = boolean_brep(
        &split_with_cavity.brep,
        &second_cutter,
        BooleanOp::Subtraction,
        "curved-two-cavities".into(),
    )
    .unwrap();
    two_cavities.brep.validate().unwrap();
    assert_eq!(two_cavities.brep.solids.len(), 2);
    assert_eq!(two_cavities.brep.topology.shells.len(), 4);
    assert!(two_cavities
        .brep
        .solids
        .iter()
        .all(|solid| solid.cavity_shells.len() == 1));
    for point in [[1.1, 1.1, 1.5], [1.6, 0.16, 1.5]] {
        assert_eq!(
            classify_point(&two_cavities.brep, point).unwrap(),
            PointClassification::Outside
        );
    }
}

#[test]
fn chained_curved_cut_keeps_each_split_top_cap_on_its_source_region() {
    let accuracy = sphere("accuracy", [0.0; 3], 1.0).accuracy;
    let quarter = std::f64::consts::FRAC_PI_2;
    let host = primitives::arc_edged_extrusion(
        "split-cap-arc-host".into(),
        Frame3::IDENTITY,
        vec![
            primitives::ProfileEdge::Arc {
                center: [0.0, 0.0],
                radius: 2.0,
                start_angle: 0.0,
                sweep_angle: quarter,
            },
            primitives::ProfileEdge::Line {
                from: [0.0, 2.0],
                to: [0.0, 1.5],
            },
            primitives::ProfileEdge::Arc {
                center: [0.0, 0.0],
                radius: 1.5,
                start_angle: quarter,
                sweep_angle: -quarter,
            },
            primitives::ProfileEdge::Line {
                from: [1.5, 0.0],
                to: [2.0, 0.0],
            },
        ],
        3.0,
        accuracy,
    )
    .unwrap();
    let sector = |id: &str, start: f64, end: f64, bottom: f64, height: f64| {
        let reach = 3.0;
        primitives::linear_extrusion(
            id.into(),
            Frame3 {
                origin: [0.0, 0.0, bottom],
                ..Frame3::IDENTITY
            },
            vec![
                [0.0, 0.0],
                [reach * start.cos(), reach * start.sin()],
                [reach * end.cos(), reach * end.sin()],
            ],
            Vec::new(),
            height,
            accuracy,
        )
        .unwrap()
    };
    let first_cutter = sector("full-height-sector", 0.7, 0.9, -1.0, 5.0);
    let first = boolean_brep(
        &host,
        &first_cutter,
        BooleanOp::Subtraction,
        "split-cap-first".into(),
    )
    .unwrap();
    first.brep.validate().unwrap();
    assert_eq!(first.brep.solids.len(), 2);
    let second_cutter = sector("lower_cutout-sector", 0.2, 0.3, 0.5, 1.5);
    let second = boolean_brep(
        &first.brep,
        &second_cutter,
        BooleanOp::Subtraction,
        "split-cap-second".into(),
    )
    .unwrap();
    second.brep.validate().unwrap();
    let mut caps = 0;
    for face in &second.brep.topology.faces {
        let SurfaceGeometry::Plane { frame } =
            &second.brep.geometry.surfaces[face.surface as usize]
        else {
            continue;
        };
        if dot(frame.z, [0.0, 0.0, 1.0]) < 1.0 - 1.0e-10
            || (frame.origin[2] - 3.0).abs() > accuracy.intersection
        {
            continue;
        }
        caps += 1;
        assert_eq!(
            face.provenance
                .sources
                .iter()
                .filter(|source| source.entity == first.brep.id)
                .count(),
            1,
            "top cap {} should come from one prior top region",
            face.id
        );
    }
    assert_eq!(caps, 2);
}
