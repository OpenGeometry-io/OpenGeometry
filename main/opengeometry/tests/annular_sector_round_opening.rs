use opengeometry::analytic::{
    booleans::{boolean_brep, BooleanOp},
    export_curve::fit_intersection_curve,
    face_intersection::intersect_breps,
    primitives,
    query::{classify_point, PointClassification},
    tessellation::tessellate,
    topology::Accuracy,
    Frame3,
};

#[test]
fn horizontal_cylinder_cuts_a_tessellated_round_opening_through_an_annular_sector_extrusion() {
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
    let host = primitives::annular_sector_extrusion(
        "host".into(),
        host_frame,
        3.0,
        0.3,
        3.0,
        0.8,
        -1.6,
        accuracy,
    )
    .unwrap();
    let cutter_frame = Frame3 {
        origin: [2.7, 1.5, 0.0],
        x: [0.0, 0.0, -1.0],
        y: [0.0, 1.0, 0.0],
        z: [1.0, 0.0, 0.0],
    };
    let cutter =
        primitives::cylinder("round-opening".into(), cutter_frame, 0.5, 0.6, accuracy).unwrap();

    let result = boolean_brep(
        &host,
        &cutter,
        BooleanOp::Subtraction,
        "host-with-round-opening".into(),
    )
    .unwrap();
    result.brep.validate().unwrap();
    let mesh = tessellate(&result.brep, 0.01, 2_000_000).unwrap();

    assert_eq!(result.brep.topology.faces.len(), 7);
    assert!(!mesh.indices.is_empty());
    assert!(mesh.triangle_face_ids.contains(&6));
}

#[test]
fn horizontal_cylinder_cuts_the_equivalent_arc_edged_extrusion() {
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
    let outer_start = 0.8_f64;
    let inner_end = -0.8_f64;
    let point = |radius: f64, angle: f64| [radius * angle.cos(), radius * angle.sin()];
    let host = primitives::arc_edged_extrusion(
        "arc-host".into(),
        host_frame,
        vec![
            primitives::ProfileEdge::Arc {
                center: [0.0, 0.0],
                radius: 3.0,
                start_angle: outer_start,
                sweep_angle: -1.6,
            },
            primitives::ProfileEdge::Line {
                from: point(3.0, inner_end),
                to: point(2.7, inner_end),
            },
            primitives::ProfileEdge::Arc {
                center: [0.0, 0.0],
                radius: 2.7,
                start_angle: inner_end,
                sweep_angle: 1.6,
            },
            primitives::ProfileEdge::Line {
                from: point(2.7, outer_start),
                to: point(3.0, outer_start),
            },
        ],
        3.0,
        accuracy,
    )
    .unwrap();
    let aperture_radius = 0.5_f64;
    // The radial cutter must start inside the cavity across its full aperture.
    // The inner curved face reaches sqrt(2.7² − 0.5²) ≈ 2.6533 m there;
    // starting at nominal radius 2.7 m is tangent, not a through-opening.
    let cutter_start = 2.65;
    let cutter = primitives::cylinder(
        "round-opening".into(),
        Frame3 {
            origin: [cutter_start, 1.5, 0.0],
            x: [0.0, 0.0, -1.0],
            y: [0.0, 1.0, 0.0],
            z: [1.0, 0.0, 0.0],
        },
        aperture_radius,
        3.05 - cutter_start,
        accuracy,
    )
    .unwrap();
    let graph = intersect_breps(&host, &cutter).unwrap();
    assert_eq!(graph.pairs.len(), 2);
    for pair in &graph.pairs {
        assert_eq!(pair.graph.branches.len(), 1);
        assert!(!pair.graph.coincident);
        assert!(pair.graph.contacts.is_empty());
        let branch = &pair.graph.branches[0];
        assert!(branch.endpoints[0]
            .iter()
            .zip(branch.endpoints[1])
            .all(|(left, right)| (left - right).abs() <= accuracy.geometric));
    }
    let result = boolean_brep(
        &host,
        &cutter,
        BooleanOp::Subtraction,
        "arc-host-with-round-opening".into(),
    )
    .unwrap();
    result.brep.validate().unwrap();
    tessellate(&result.brep, 0.01, 2_000_000).unwrap();
    for definition in 0..result.brep.geometry.intersections.len() {
        let fit = fit_intersection_curve(
            &result.brep.geometry,
            definition as u32,
            accuracy.exchange,
            20_000,
        )
        .unwrap();
        assert!(fit.error_bound <= accuracy.exchange);
        let trace = &result.brep.geometry.intersections[definition];
        for (segment, pair) in trace.anchors.windows(2).enumerate().step_by(11) {
            let lo = pair[0].parameter + 0.1 * (pair[1].parameter - pair[0].parameter);
            let hi = pair[0].parameter + 0.9 * (pair[1].parameter - pair[0].parameter);
            let range = opengeometry::math::interval::Interval::new(lo, hi).unwrap();
            let bound = trace
                .certified_chord_deviation(range, &result.brep.geometry)
                .unwrap()
                .expect("perpendicular cylinder segment has a bound");
            let a = trace.evaluate(lo, &result.brep.geometry).unwrap().point;
            let b = trace.evaluate(hi, &result.brep.geometry).unwrap().point;
            let chord = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let chord_squared = chord.iter().map(|value| value * value).sum::<f64>();
            for sample in 0..=32 {
                let t = lo + (hi - lo) * sample as f64 / 32.0;
                let point = trace.evaluate(t, &result.brep.geometry).unwrap().point;
                let offset = [point[0] - a[0], point[1] - a[1], point[2] - a[2]];
                let portion = (offset.iter().zip(chord).map(|(x, y)| x * y).sum::<f64>()
                    / chord_squared)
                    .clamp(0.0, 1.0);
                let distance = (0..3)
                    .map(|axis| (point[axis] - a[axis] - portion * chord[axis]).powi(2))
                    .sum::<f64>()
                    .sqrt();
                assert!(
                    distance <= bound,
                    "trace {definition} segment {segment}: {distance} > {bound}"
                );
            }
        }
    }
    let (step, report) =
        opengeometry::analytic::exchange::export_step(&result.brep, "metre").unwrap();
    assert_eq!(report.solids, 1);
    assert!(step.contains("B_SPLINE_CURVE_WITH_KNOTS"));
    assert_eq!(
        classify_point(&result.brep, [2.8, 1.5, 0.0]).unwrap(),
        PointClassification::Outside
    );
    assert_eq!(
        classify_point(&result.brep, [2.8, 2.2, 0.0]).unwrap(),
        PointClassification::Inside
    );
    assert_eq!(
        classify_point(&result.brep, [2.8, 1.5, 0.7]).unwrap(),
        PointClassification::Inside
    );
}

#[test]
fn arched_annular_sector_extrusion_opening_reuses_the_spring_edges_and_an_analytic_cylinder_cap() {
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
    let host = primitives::annular_sector_extrusion_with_arched_opening(
        "arched-host".into(),
        host_frame,
        3.0,
        0.3,
        3.0,
        1.6,
        -1.6,
        primitives::AnnularSectorOpening {
            id: "arched-opening".into(),
            angle: 0.8,
            width: 1.0,
            bottom: 0.5,
            height: 1.25,
        },
        accuracy,
    )
    .unwrap();

    host.validate().unwrap();
    assert_eq!(host.topology.faces.len(), 10);
    assert!(host
        .topology
        .faces
        .iter()
        .any(|face| face.key == "opening-0-arch"));
    assert!(host.geometry.curves.iter().any(|curve| matches!(
        curve,
        opengeometry::analytic::CurveGeometry::Intersection { .. }
    )));
    let mesh = tessellate(&host, 0.01, 2_000_000).unwrap();
    assert!(!mesh.indices.is_empty());
}

#[test]
fn box_arched_opening_has_shared_circle_edges_and_a_cylindrical_arch_cap() {
    let accuracy = Accuracy {
        geometric: 4.0e-8,
        intersection: 1.0e-8,
        tessellation: 0.01,
        exchange: 1e-5,
    };
    let host = primitives::box_with_arched_opening(
        "straight-arched-host".into(),
        Frame3::IDENTITY,
        4.0,
        0.3,
        3.0,
        primitives::BoxArchedOpening {
            id: "arched-opening".into(),
            station: 2.0,
            width: 1.0,
            bottom: 0.4,
            height: 1.4,
        },
        accuracy,
    )
    .unwrap();

    host.validate().unwrap();
    let arch_cap = host
        .topology
        .faces
        .iter()
        .find(|face| face.key == "arched-opening-arched-cap")
        .unwrap();
    assert!(matches!(
        host.geometry.surfaces[arch_cap.surface as usize],
        opengeometry::analytic::SurfaceGeometry::Cylinder { .. }
    ));
    assert_eq!(
        host.topology
            .edges
            .iter()
            .filter(|edge| matches!(
                edge.geometry,
                opengeometry::analytic::topology::EdgeGeometry::Curve { curve, .. }
                    if matches!(host.geometry.curves[curve as usize], opengeometry::analytic::CurveGeometry::Circle { .. })
            ))
            .count(),
        2
    );
    let mesh = tessellate(&host, 0.01, 2_000_000).unwrap();
    assert!(mesh.triangle_face_ids.contains(&arch_cap.id));
    let (_, report) = opengeometry::analytic::exchange::export_step(&host, "metre").unwrap();
    assert_eq!(report.solids, 1);
}
