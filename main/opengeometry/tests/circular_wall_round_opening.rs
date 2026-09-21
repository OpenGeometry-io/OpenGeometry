use opengeometry::analytic::{
    booleans::{boolean_brep, BooleanOp},
    primitives,
    tessellation::tessellate,
    topology::Accuracy,
    Frame3,
};

#[test]
fn horizontal_cylinder_cuts_a_tessellated_round_opening_through_a_circular_wall() {
    let accuracy = Accuracy {
        geometric: 6.3e-8,
        intersection: 1.575e-8,
        tessellation: 0.01,
        exchange: 1e-5,
    };
    let wall_frame = Frame3 {
        origin: [0.0, 0.0, 0.0],
        x: [1.0, 0.0, 0.0],
        y: [0.0, 0.0, -1.0],
        z: [0.0, 1.0, 0.0],
    };
    let wall = primitives::circular_wall(
        "wall".into(),
        wall_frame,
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
        &wall,
        &cutter,
        BooleanOp::Subtraction,
        "wall-with-round-opening".into(),
    )
    .unwrap();
    result.brep.validate().unwrap();
    let mesh = tessellate(&result.brep, 0.01, 2_000_000).unwrap();

    assert_eq!(result.brep.topology.faces.len(), 7);
    assert!(!mesh.indices.is_empty());
    assert!(mesh.triangle_face_ids.contains(&6));
}

#[test]
fn arched_circular_wall_opening_reuses_the_spring_edges_and_an_analytic_cylinder_cap() {
    let accuracy = Accuracy {
        geometric: 6.3e-8,
        intersection: 1.575e-8,
        tessellation: 0.01,
        exchange: 1e-5,
    };
    let wall_frame = Frame3 {
        origin: [0.0, 0.0, 0.0],
        x: [1.0, 0.0, 0.0],
        y: [0.0, 0.0, -1.0],
        z: [0.0, 1.0, 0.0],
    };
    let wall = primitives::circular_wall_with_arched_opening(
        "arched-wall".into(),
        wall_frame,
        3.0,
        0.3,
        3.0,
        1.6,
        -1.6,
        primitives::CircularWallOpening {
            id: "arched-opening".into(),
            angle: 0.8,
            width: 1.0,
            bottom: 0.5,
            height: 1.25,
        },
        accuracy,
    )
    .unwrap();

    wall.validate().unwrap();
    assert_eq!(wall.topology.faces.len(), 10);
    assert!(wall
        .topology
        .faces
        .iter()
        .any(|face| face.key == "opening-0-arch"));
    assert!(wall.geometry.curves.iter().any(|curve| matches!(
        curve,
        opengeometry::analytic::CurveGeometry::Intersection { .. }
    )));
    let mesh = tessellate(&wall, 0.01, 2_000_000).unwrap();
    assert!(!mesh.indices.is_empty());
}

#[test]
fn straight_wall_arched_opening_has_shared_circle_edges_and_a_cylindrical_header() {
    let accuracy = Accuracy {
        geometric: 4.0e-8,
        intersection: 1.0e-8,
        tessellation: 0.01,
        exchange: 1e-5,
    };
    let wall = primitives::straight_wall_with_arched_opening(
        "straight-arched-wall".into(),
        Frame3::IDENTITY,
        4.0,
        0.3,
        3.0,
        primitives::StraightWallArchedOpening {
            id: "arched-opening".into(),
            station: 2.0,
            width: 1.0,
            bottom: 0.4,
            height: 1.4,
        },
        accuracy,
    )
    .unwrap();

    wall.validate().unwrap();
    let header = wall
        .topology
        .faces
        .iter()
        .find(|face| face.key == "arched-opening-arched-header")
        .unwrap();
    assert!(matches!(
        wall.geometry.surfaces[header.surface as usize],
        opengeometry::analytic::SurfaceGeometry::Cylinder { .. }
    ));
    assert_eq!(
        wall.topology
            .edges
            .iter()
            .filter(|edge| matches!(
                edge.geometry,
                opengeometry::analytic::topology::EdgeGeometry::Curve { curve, .. }
                    if matches!(wall.geometry.curves[curve as usize], opengeometry::analytic::CurveGeometry::Circle { .. })
            ))
            .count(),
        2
    );
    let mesh = tessellate(&wall, 0.01, 2_000_000).unwrap();
    assert!(mesh.triangle_face_ids.contains(&header.id));
}
