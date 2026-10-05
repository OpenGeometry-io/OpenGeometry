use super::face_view::FaceView;
use super::{intersect_breps, intersect_faces};
use crate::brep::{coarse_accuracy, Accuracy, Curve, CurveGeometry, Frame3, Surface};
use crate::math::{norm, sub};
use crate::primitives;

#[test]
fn plane_face_graph_clips_the_shared_curve_to_both_trims() {
    let a = primitives::cuboid(
        "a".into(),
        Frame3::IDENTITY,
        [2.0; 3],
        coarse_accuracy(1e-5),
    )
    .unwrap();
    let b = primitives::cuboid(
        "b".into(),
        Frame3 {
            origin: [-1.0, 0.5, 1.0],
            ..Frame3::IDENTITY
        },
        [2.0; 3],
        coarse_accuracy(1e-5),
    )
    .unwrap();
    let graph = intersect_faces(
        FaceView { brep: &a, face: 1 },
        FaceView { brep: &b, face: 3 },
    )
    .unwrap();
    assert_eq!(graph.branches.len(), 1);
    let mut endpoints = graph.branches[0].endpoints;
    endpoints.sort_by(|a, b| a[1].total_cmp(&b[1]));
    for (point, expected_y) in endpoints.into_iter().zip([0.5, 2.0]) {
        assert!((point[0] - 1.0).abs() <= coarse_accuracy(1e-5).intersection);
        assert!((point[1] - expected_y).abs() <= coarse_accuracy(1e-5).intersection);
        assert!((point[2] - 2.0).abs() <= coarse_accuracy(1e-5).intersection);
    }
}

#[test]
fn vertical_plane_cylinder_generators_remain_exact_and_bounded() {
    let quarter = std::f64::consts::FRAC_PI_2;
    let host = primitives::arc_edged_extrusion(
        "arc-profile".into(),
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
        coarse_accuracy(1e-5),
    )
    .unwrap();
    let cutter = primitives::cuboid(
        "opening".into(),
        Frame3 {
            origin: [1.1, 0.9, 0.5],
            ..Frame3::IDENTITY
        },
        [1.0, 0.35, 1.5],
        coarse_accuracy(1e-5),
    )
    .unwrap();
    let graph = intersect_breps(&host, &cutter).unwrap();
    assert!(!graph.pairs.is_empty());
    let branches = graph
        .pairs
        .iter()
        .flat_map(|pair| &pair.graph.branches)
        .collect::<Vec<_>>();
    assert!(!branches.is_empty());
    for pair in &graph.pairs {
        for branch in &pair.graph.branches {
            assert!(branch.range.lo.is_finite() && branch.range.hi.is_finite());
            assert!(!matches!(
                pair.graph.geometry.curves[branch.curve as usize],
                CurveGeometry::Intersection { .. }
            ));
        }
    }
}

#[test]
fn bounded_nonparallel_cylinder_faces_use_universal_ssi() {
    let accuracy = Accuracy {
        geometric: 1e-5,
        intersection: 2.5e-6,
        tessellation: 0.01,
        exchange: 1e-5,
    };
    let a = primitives::cylinder(
        "a".into(),
        Frame3 {
            origin: [0.0, 0.0, -2.0],
            ..Frame3::IDENTITY
        },
        1.0,
        4.0,
        accuracy,
    )
    .unwrap();
    let b_frame = Frame3::from_axis([-1.0, 0.0, 0.2], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let b = primitives::cylinder("b".into(), b_frame, 0.6, 2.0, accuracy).unwrap();
    let graph = intersect_faces(
        FaceView { brep: &a, face: 0 },
        FaceView { brep: &b, face: 0 },
    )
    .unwrap();
    assert!(!graph.branches.is_empty());
    for branch in &graph.branches {
        assert!(matches!(
            graph.geometry.curves[branch.curve as usize],
            CurveGeometry::Intersection { .. }
        ));
        let parameter = branch.range.midpoint();
        let point = graph
            .geometry
            .curve(branch.curve)
            .unwrap()
            .point_at(parameter)
            .unwrap();
        for side in 0..2 {
            let uv = graph
                .geometry
                .pcurve_at(branch.pcurves[side], parameter)
                .unwrap();
            let support = graph.geometry.surfaces[side].point_at(uv).unwrap();
            assert!(norm(sub(point, support)) <= accuracy.intersection);
        }
    }
}

#[test]
fn body_graph_uses_patch_bounds_before_face_pair_ssi() {
    let accuracy = Accuracy {
        geometric: 1e-5,
        intersection: 2.5e-6,
        tessellation: 0.01,
        exchange: 1e-5,
    };
    let a = primitives::cylinder(
        "a".into(),
        Frame3 {
            origin: [0.0, 0.0, -2.0],
            ..Frame3::IDENTITY
        },
        1.0,
        4.0,
        accuracy,
    )
    .unwrap();
    let b = primitives::cylinder(
        "b".into(),
        Frame3::from_axis([-2.0, 0.0, 0.2], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap(),
        0.6,
        4.0,
        accuracy,
    )
    .unwrap();
    let graph = intersect_breps(&a, &b).unwrap();
    assert_eq!(graph.pairs.len(), 1);
    assert_eq!(graph.pairs[0].faces, [0, 0]);
    assert_eq!(graph.pairs[0].graph.branches.len(), 2);
}
