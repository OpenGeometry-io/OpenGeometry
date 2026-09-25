use opengeometry::analytic::{
    booleans::subtract_planar_cutters,
    primitives,
    query::{classify_point, PointClassification},
    tessellation::tessellate,
    topology::Accuracy,
    Frame3,
};

#[test]
fn disjoint_planar_and_round_openings_share_a_curved_host_height_level() {
    let accuracy = Accuracy {
        geometric: 6.3e-8,
        intersection: 1.575e-8,
        tessellation: 0.01,
        exchange: 1e-5,
    };
    let host_frame = Frame3 {
        origin: [0.0, 0.0, 0.0],
        x: [1.0, 0.0, 0.0],
        y: [0.0, 0.0, -1.0],
        z: [0.0, 1.0, 0.0],
    };
    let point = |radius: f64, angle: f64| [radius * angle.cos(), radius * angle.sin()];
    let host = primitives::arc_edged_extrusion(
        "mixed-curved-host".into(),
        host_frame,
        vec![
            primitives::ProfileEdge::Arc {
                center: [0.0, 0.0],
                radius: 3.0,
                start_angle: 0.8,
                sweep_angle: -1.6,
            },
            primitives::ProfileEdge::Line {
                from: point(3.0, -0.8),
                to: point(2.7, -0.8),
            },
            primitives::ProfileEdge::Arc {
                center: [0.0, 0.0],
                radius: 2.7,
                start_angle: -0.8,
                sweep_angle: 1.6,
            },
            primitives::ProfileEdge::Line {
                from: point(2.7, 0.8),
                to: point(3.0, 0.8),
            },
        ],
        3.0,
        accuracy,
    )
    .unwrap();
    let rectangle = primitives::cuboid(
        "rectangular-opening".into(),
        Frame3 {
            origin: [2.2, 1.0, 1.1],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
            z: [0.0, 0.0, 1.0],
        },
        [1.0, 0.5, 0.4],
        accuracy,
    )
    .unwrap();
    let round = primitives::cylinder(
        "round-opening".into(),
        Frame3 {
            origin: [2.65, 1.5, 0.0],
            x: [0.0, 0.0, -1.0],
            y: [0.0, 1.0, 0.0],
            z: [1.0, 0.0, 0.0],
        },
        0.5,
        0.4,
        accuracy,
    )
    .unwrap();
    for cutters in [
        [rectangle.clone(), round.clone()],
        [round.clone(), rectangle.clone()],
    ] {
        let result = subtract_planar_cutters(&host, &cutters, "mixed-curved-cut".into()).unwrap();
        result.brep.validate().unwrap();
        tessellate(&result.brep, 0.01, 2_000_000).unwrap();
        assert_eq!(
            classify_point(&result.brep, [2.6, 1.25, 1.3]).unwrap(),
            PointClassification::Outside
        );
        assert_eq!(
            classify_point(&result.brep, [2.8, 1.5, 0.0]).unwrap(),
            PointClassification::Outside
        );
        assert_eq!(
            classify_point(&result.brep, [2.8, 2.5, 0.0]).unwrap(),
            PointClassification::Inside
        );
        let (_, report) =
            opengeometry::analytic::exchange::export_step(&result.brep, "metre").unwrap();
        assert_eq!(report.solids, 1);
    }
}
