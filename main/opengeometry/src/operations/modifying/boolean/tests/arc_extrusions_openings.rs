use super::support::sphere;
use crate::brep::{BrepEnvelope, Frame3, Orientation, SurfaceGeometry};
use crate::operations::modifying::boolean::batch::subtract_planar_cutters;
use crate::operations::modifying::boolean::dispatch::boolean_brep;
use crate::operations::modifying::boolean::types::BooleanOp;
use crate::primitives;
use crate::query::{classify_point, PointClassification};
use crate::tessellation::tessellate;

#[test]
fn annular_sector_accepts_an_exact_vertical_sector_opening() {
    let accuracy = sphere("accuracy", [0.0; 3], 1.0).accuracy;
    let host = primitives::annular_cylinder(
        "ring-host".into(),
        Frame3::IDENTITY,
        1.9,
        2.1,
        3.0,
        accuracy,
    )
    .unwrap();
    let reach = 2.3;
    let cutter = primitives::linear_extrusion(
        "ring-opening".into(),
        Frame3 {
            origin: [0.0, 0.0, 0.5],
            ..Frame3::IDENTITY
        },
        vec![
            [0.0, 0.0],
            [reach * (-0.1_f64).cos(), reach * (-0.1_f64).sin()],
            [reach * 0.1_f64.cos(), reach * 0.1_f64.sin()],
        ],
        vec![],
        2.0,
        accuracy,
    )
    .unwrap();
    let cut = boolean_brep(&host, &cutter, BooleanOp::Subtraction, "ring-cut".into()).unwrap();
    cut.brep.validate().unwrap();
    tessellate(&cut.brep, 0.01, 2_000_000).unwrap();
    assert_eq!(
        classify_point(&cut.brep, [2.0, 0.0, 1.0]).unwrap(),
        PointClassification::Outside
    );
    assert_eq!(
        classify_point(&cut.brep, [0.0, 2.0, 1.0]).unwrap(),
        PointClassification::Inside
    );
    assert_eq!(
        classify_point(&cut.brep, [0.0, 0.0, 1.0]).unwrap(),
        PointClassification::Outside
    );
    let second = primitives::linear_extrusion(
        "second-ring-opening".into(),
        Frame3 {
            origin: [0.0, 0.0, 0.8],
            ..Frame3::IDENTITY
        },
        vec![
            [0.0, 0.0],
            [
                reach * (std::f64::consts::FRAC_PI_2 - 0.1).cos(),
                reach * (std::f64::consts::FRAC_PI_2 - 0.1).sin(),
            ],
            [
                reach * (std::f64::consts::FRAC_PI_2 + 0.1).cos(),
                reach * (std::f64::consts::FRAC_PI_2 + 0.1).sin(),
            ],
        ],
        vec![],
        1.3,
        accuracy,
    )
    .unwrap();
    let chained = boolean_brep(
        &cut.brep,
        &second,
        BooleanOp::Subtraction,
        "ring-cut-chained".into(),
    )
    .unwrap();
    chained.brep.validate().unwrap();
    tessellate(&chained.brep, 0.01, 2_000_000).unwrap();
    for point in [[2.0, 0.0, 1.0], [0.0, 2.0, 1.0]] {
        assert_eq!(
            classify_point(&chained.brep, point).unwrap(),
            PointClassification::Outside
        );
    }
    assert_eq!(
        classify_point(&chained.brep, [-2.0, 0.0, 1.0]).unwrap(),
        PointClassification::Inside
    );
}

#[test]
fn arc_edged_host_accepts_a_vertical_rectangular_opening_cut() {
    let accuracy = sphere("accuracy", [0.0; 3], 1.0).accuracy;
    let quarter = std::f64::consts::FRAC_PI_2;
    let host = primitives::arc_edged_extrusion(
        "arc-host".into(),
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
    let cutter = primitives::cuboid(
        "opening".into(),
        Frame3 {
            origin: [1.1, 0.9, 0.5],
            ..Frame3::IDENTITY
        },
        [1.0, 0.35, 1.5],
        accuracy,
    )
    .unwrap();
    let assert_curved_normals = |brep: &BrepEnvelope| {
        for (radius, expected) in [(2.0, Orientation::Forward), (1.5, Orientation::Reverse)] {
            let faces = brep.topology.faces.iter().filter(|face| {
                matches!(
                    brep.geometry.surfaces[face.surface as usize],
                    SurfaceGeometry::Cylinder { radius: value, .. } if (value - radius).abs() < 1e-9
                )
            }).collect::<Vec<_>>();
            assert!(!faces.is_empty(), "missing curved face at radius {radius}");
            assert!(
                faces.iter().all(|face| face.sense == expected),
                "reconstructed curved face at radius {radius} points into material: {:?}",
                faces
                    .iter()
                    .map(|face| (&face.key, face.sense))
                    .collect::<Vec<_>>()
            );
        }
    };
    let cut = boolean_brep(&host, &cutter, BooleanOp::Subtraction, "arc-cut".into()).unwrap();
    cut.brep.validate().unwrap();
    assert_curved_normals(&cut.brep);
    tessellate(&cut.brep, 0.01, 2_000_000).unwrap();
    assert_eq!(
        classify_point(&cut.brep, [1.5, 1.1, 1.2]).unwrap(),
        PointClassification::Outside
    );
    let second_cutter = primitives::cuboid(
        "second-opening".into(),
        Frame3 {
            origin: [0.6, 1.3, 0.8],
            ..Frame3::IDENTITY
        },
        [0.45, 0.8, 1.2],
        accuracy,
    )
    .unwrap();
    let batch = subtract_planar_cutters(
        &host,
        &[cutter, second_cutter.clone()],
        "arc-cut-batch".into(),
    )
    .unwrap();
    batch.brep.validate().unwrap();
    tessellate(&batch.brep, 0.01, 2_000_000).unwrap();
    assert_curved_normals(&batch.brep);
    for point in [[1.5, 1.1, 1.2], [0.9, 1.65, 1.2]] {
        assert_eq!(
            classify_point(&batch.brep, point).unwrap(),
            PointClassification::Outside
        );
    }
    let chained = boolean_brep(
        &cut.brep,
        &second_cutter,
        BooleanOp::Subtraction,
        "arc-cut-chained".into(),
    )
    .unwrap();
    chained.brep.validate().unwrap();
    assert_curved_normals(&chained.brep);
    for point in [[1.5, 1.1, 1.2], [0.9, 1.65, 1.2]] {
        assert_eq!(
            classify_point(&chained.brep, point).unwrap(),
            PointClassification::Outside
        );
    }
    let overlapping = primitives::cuboid(
        "overlapping-opening".into(),
        Frame3 {
            origin: [1.4, 0.95, 0.2],
            ..Frame3::IDENTITY
        },
        [0.5, 0.4, 2.2],
        accuracy,
    )
    .unwrap();
    let third = boolean_brep(
        &chained.brep,
        &overlapping,
        BooleanOp::Subtraction,
        "arc-cut-third".into(),
    )
    .unwrap();
    third.brep.validate().unwrap();
    tessellate(&third.brep, 0.01, 2_000_000).unwrap();
    assert_curved_normals(&third.brep);
    for point in [
        [1.5, 1.1, 1.2],
        [0.9, 1.65, 1.2],
        [1.45, 1.2, 0.3],
        [1.45, 1.2, 2.2],
    ] {
        assert_eq!(
            classify_point(&third.brep, point).unwrap(),
            PointClassification::Outside
        );
    }
}

#[test]
fn curved_cut_provenance_selects_the_trimmed_coaxial_source_face() {
    let accuracy = sphere("accuracy", [0.0; 3], 1.0).accuracy;
    let quarter = std::f64::consts::FRAC_PI_2;
    let host = primitives::arc_edged_extrusion(
        "split-arc-host".into(),
        Frame3::IDENTITY,
        vec![
            primitives::ProfileEdge::Arc {
                center: [0.0, 0.0],
                radius: 2.0,
                start_angle: 0.0,
                sweep_angle: quarter / 2.0,
            },
            primitives::ProfileEdge::Arc {
                center: [0.0, 0.0],
                radius: 2.0,
                start_angle: quarter / 2.0,
                sweep_angle: quarter / 2.0,
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
    let cutter = primitives::cuboid(
        "split-arc-opening".into(),
        Frame3 {
            origin: [1.1, 0.9, 0.5],
            ..Frame3::IDENTITY
        },
        [1.0, 0.35, 1.5],
        accuracy,
    )
    .unwrap();
    let cut = boolean_brep(
        &host,
        &cutter,
        BooleanOp::Subtraction,
        "split-arc-cut".into(),
    )
    .unwrap();
    cut.brep.validate().unwrap();
    let mut outer_faces = 0;
    for face in &cut.brep.topology.faces {
        let SurfaceGeometry::Cylinder { radius, .. } =
            &cut.brep.geometry.surfaces[face.surface as usize]
        else {
            continue;
        };
        if (*radius - 2.0).abs() > accuracy.geometric {
            continue;
        }
        outer_faces += 1;
        assert_eq!(
            face.provenance
                .sources
                .iter()
                .filter(|source| source.entity == host.id)
                .count(),
            1,
            "output face {} should belong to exactly one trimmed outer arc",
            face.id
        );
    }
    assert!(outer_faces >= 2);
}

#[test]
fn wide_curved_opening_uses_multiple_planar_sector_cutters() {
    let accuracy = sphere("accuracy", [0.0; 3], 1.0).accuracy;
    let start = -std::f64::consts::FRAC_PI_2;
    let sweep = 3.0 * std::f64::consts::FRAC_PI_2;
    let point = |radius: f64, angle: f64| [radius * angle.cos(), radius * angle.sin()];
    let host = primitives::arc_edged_extrusion(
        "wide-arc".into(),
        Frame3::IDENTITY,
        vec![
            primitives::ProfileEdge::Arc {
                center: [0.0, 0.0],
                radius: 2.1,
                start_angle: start,
                sweep_angle: sweep,
            },
            primitives::ProfileEdge::Line {
                from: point(2.1, start + sweep),
                to: point(1.9, start + sweep),
            },
            primitives::ProfileEdge::Arc {
                center: [0.0, 0.0],
                radius: 1.9,
                start_angle: start + sweep,
                sweep_angle: -sweep,
            },
            primitives::ProfileEdge::Line {
                from: point(1.9, start),
                to: point(2.1, start),
            },
        ],
        3.0,
        accuracy,
    )
    .unwrap();
    let opening_start = start + 1.2 / 2.0;
    let opening_end = start + 8.2 / 2.0;
    let cutters = (0..3)
        .map(|index| {
            let from = opening_start + (opening_end - opening_start) * index as f64 / 3.0;
            let to = opening_start + (opening_end - opening_start) * (index + 1) as f64 / 3.0;
            let reach = 2.1 / ((to - from) / 2.0).cos() + 0.2;
            primitives::linear_extrusion(
                format!("sector-{index}"),
                Frame3::IDENTITY,
                vec![[0.0, 0.0], point(reach, from), point(reach, to)],
                vec![],
                2.0,
                accuracy,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let cut = subtract_planar_cutters(&host, &cutters, "wide-cut".into()).unwrap();
    cut.brep.validate().unwrap();
    tessellate(&cut.brep, 0.01, 2_000_000).unwrap();
    let angle = start + 4.7 / 2.0;
    assert_eq!(
        classify_point(&cut.brep, [2.0 * angle.cos(), 2.0 * angle.sin(), 1.0]).unwrap(),
        PointClassification::Outside
    );
}
