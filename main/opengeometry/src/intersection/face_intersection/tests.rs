use super::clip::piece_box;
use super::face_view::FaceView;
use super::{intersect_breps, intersect_faces};
use crate::brep::{
    coarse_accuracy, Accuracy, Curve, CurveGeometry, Frame3, GeometryStore, IntersectionDefinition,
    IntersectionSide, PcurveGeometry, Surface, SurfaceGeometry, TraceAnchor,
};
use crate::math::{norm, sub, Interval};
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

#[test]
fn arch_side_crossing_a_ring_face_is_clipped_where_the_shared_tube_spans_its_spring() {
    let accuracy = Accuracy {
        geometric: 1e-8,
        intersection: 1e-9,
        tessellation: 0.01,
        exchange: 1e-5,
    };
    let ring =
        primitives::annular_cylinder("ring".into(), Frame3::IDENTITY, 2.7, 3.0, 3.0, accuracy)
            .unwrap();
    let (sin, cos) = 0.7_f64.sin_cos();
    let frame = Frame3::from_axis(
        [2.55 * cos, 2.55 * sin, 0.0],
        [cos, sin, 0.0],
        [-sin, cos, 0.0],
    )
    .unwrap();
    let corners = [[-0.45, 0.2], [0.45, 0.2], [0.45, 2.2], [-0.45, 2.2]];
    let mut profile = corners
        .windows(2)
        .map(|pair| primitives::ProfileEdge::Line {
            from: pair[0],
            to: pair[1],
        })
        .collect::<Vec<_>>();
    profile[2] = primitives::ProfileEdge::Arc {
        center: [0.0, 2.2],
        radius: 0.45,
        start_angle: 0.0,
        sweep_angle: std::f64::consts::PI,
    };
    profile.push(primitives::ProfileEdge::Line {
        from: corners[3],
        to: corners[0],
    });
    let arch =
        primitives::arc_edged_extrusion("arch".into(), frame, profile, 0.6, accuracy).unwrap();
    let graph = intersect_faces(
        FaceView {
            brep: &ring,
            face: 0,
        },
        FaceView {
            brep: &arch,
            face: 4,
        },
    )
    .unwrap();
    assert_eq!(graph.branches.len(), 1);
    for point in graph.branches[0].endpoints {
        let height = point[2];
        assert!((height - 2.2).abs() <= 1e-6, "{point:?}");
    }
}

#[test]
fn piece_box_encloses_an_axial_peak_between_its_end_samples() {
    let mut store = GeometryStore::new();
    store.surfaces.push(SurfaceGeometry::Cylinder {
        frame: Frame3::IDENTITY,
        radius: 3.0,
    });
    store.surfaces.push(SurfaceGeometry::Cylinder {
        frame: Frame3 {
            origin: [1.9503475775754455, 1.642755102456112, 2.2],
            x: [-0.644217687237691, 0.7648421872844885, 0.0],
            y: [0.0, -0.0, 1.0],
            z: [0.7648421872844885, 0.644217687237691, 0.0],
        },
        radius: 0.45,
    });
    let ends = [
        (
            0.7128960125719093,
            [2.319031360088156, 1.903179852491002, 2.6483647006160944],
            [0.6872233929727672, 2.648364700617094],
            [-4.627110592037641, 0.4497551408002707],
        ),
        (
            0.7865256619134415,
            [2.2716265395193083, 1.9595185288615027, 2.6486132842284404],
            [0.7117670855789375, 2.6486132842255756],
            [-4.790915085586551, 0.4497923059420013],
        ),
    ];
    let anchors = ends
        .iter()
        .map(|&(parameter, point, uv_a, uv_b)| TraceAnchor {
            parameter,
            point,
            uv_a,
            uv_b,
        })
        .collect();
    store.intersections.push(IntersectionDefinition {
        surfaces: [0, 1],
        anchors,
        uv_tubes: vec![[
            Interval::new(0.6859962083424562, 0.7129942702092485).unwrap(),
            Interval::new(2.6226562499999906, 2.6742187500000094).unwrap(),
            Interval::new(-5.118524072684371, -4.299501604939821).unwrap(),
            Interval::new(0.4496808105168094, 0.4498666362254626).unwrap(),
        ]],
        residual_tolerance: 1e-9,
    });
    store.pcurves.push(PcurveGeometry::IntersectionSide {
        definition: 0,
        side: IntersectionSide::B,
    });
    let peak = store.pcurve_at(0, 0.7313034249072923).unwrap();
    assert!(peak[1] > 0.4498666362254626);
    let piece = piece_box(&store, 0, ends[0].0, ends[1].0, 1e-9).unwrap();
    assert!(
        piece[0].contains(peak[0]) && piece[1].contains(peak[1]),
        "{piece:?} misses {peak:?}"
    );
}
