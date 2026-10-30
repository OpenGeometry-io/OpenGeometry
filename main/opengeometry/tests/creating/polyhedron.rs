use opengeometry::brep::SurfaceGeometry;
use opengeometry::math::Point3;
use opengeometry::world_graph::{
    EditScope, ErrorCode, ModifyingOperation, Primitive, StepOptions, Transform, WorldGraph,
};
use opengeometry_test_support::part21::Document;
use opengeometry_test_support::volume;
use opengeometry_test_support::world_graph::{graph, named};

const HIP_FACES: [&[u32]; 5] = [
    &[0, 1, 2, 3],
    &[1, 0, 4, 5],
    &[2, 1, 5],
    &[3, 2, 5, 4],
    &[0, 3, 4],
];

const EAVES: [Point3; 4] = [
    [0.0, 0.0, 0.0],
    [4.0, 0.0, 0.0],
    [4.0, 0.0, 2.0],
    [0.0, 0.0, 2.0],
];

fn polyhedron(points: &[Point3], faces: &[&[u32]]) -> Primitive {
    Primitive::Polyhedron {
        points: points.to_vec(),
        faces: faces.iter().map(|ring| ring.to_vec()).collect(),
    }
}

fn roof(ridge: [Point3; 2]) -> Primitive {
    let points = EAVES.iter().chain(&ridge).copied().collect::<Vec<_>>();
    polyhedron(&points, &HIP_FACES)
}

fn hip_roof() -> Primitive {
    roof([[1.0, 1.0, 1.0], [3.0, 1.0, 1.0]])
}

fn gable_roof() -> Primitive {
    roof([[0.0, 1.0, 1.0], [4.0, 1.0, 1.0]])
}

fn l_roof() -> Primitive {
    let points = [
        [0.0, 0.0, 0.0],
        [4.0, 0.0, 0.0],
        [4.0, 0.0, 2.0],
        [2.0, 0.0, 2.0],
        [2.0, 0.0, 4.0],
        [0.0, 0.0, 4.0],
        [1.0, 1.0, 1.0],
        [3.0, 1.0, 1.0],
        [1.0, 1.0, 3.0],
    ];
    let faces: [&[u32]; 7] = [
        &[0, 1, 2, 3, 4, 5],
        &[1, 0, 6, 7],
        &[2, 1, 7],
        &[3, 2, 7, 6],
        &[4, 3, 6, 8],
        &[5, 4, 8],
        &[0, 5, 8, 6],
    ];
    polyhedron(&points, &faces)
}

fn measured(world: &WorldGraph, og_id: &str, expected: f64) {
    let body = world.brep(og_id).unwrap();
    body.validate().unwrap();
    let estimate = volume::estimate(&body, 0.001);
    assert!(
        (estimate.value - expected).abs() <= estimate.error_bound,
        "{og_id}: {} versus {expected} with bound {}",
        estimate.value,
        estimate.error_bound
    );
}

fn refused(primitive: Primitive) -> ErrorCode {
    let mut world = graph();
    let error = world
        .create_primitive(primitive, named("roof"))
        .unwrap_err();
    assert_eq!(world.node_count(), 0);
    error.error_code()
}

fn subtracted(world: &mut WorldGraph, host: &str) -> serde_json::Value {
    world
        .operate(
            host,
            ModifyingOperation::Subtract,
            &["tool".into()],
            EditScope::Node,
        )
        .unwrap();
    world.report(host).unwrap().unwrap()["handlers"].clone()
}

fn moved(world: &mut WorldGraph, og_id: &str, offset: Point3) {
    world
        .transform(og_id, Transform::Translate { offset })
        .unwrap();
}

#[test]
fn roofs_have_their_volumes_and_planar_faces() {
    let mut world = graph();
    for (og_id, primitive, expected, counts) in [
        ("hip", hip_roof(), 10.0 / 3.0, [6, 9, 5]),
        ("gable", gable_roof(), 4.0, [6, 9, 5]),
        ("l", l_roof(), 16.0 / 3.0, [9, 14, 7]),
    ] {
        world.create_primitive(primitive, named(og_id)).unwrap();
        measured(&world, og_id, expected);
        let body = world.brep(og_id).unwrap();
        let topology = &body.topology;
        assert_eq!(
            [
                topology.vertices.len(),
                topology.edges.len(),
                topology.faces.len()
            ],
            counts
        );
        assert!(body
            .geometry
            .surfaces
            .iter()
            .all(|surface| matches!(surface, SurfaceGeometry::Plane { .. })));
    }
}

#[test]
fn roofs_export_to_step() {
    let mut world = graph();
    for (og_id, primitive, faces) in [
        ("hip", hip_roof(), 5),
        ("gable", gable_roof(), 5),
        ("l", l_roof(), 7),
    ] {
        world.create_primitive(primitive, named(og_id)).unwrap();
        let (text, _) = world
            .export_step(&[og_id.into()], &StepOptions::default())
            .unwrap();
        let document = Document::parse(&text).unwrap();
        assert_eq!(document.count("MANIFOLD_SOLID_BREP"), 1);
        assert_eq!(document.count("ADVANCED_FACE"), faces);
        assert_eq!(document.count("PLANE"), faces);
    }
}

#[test]
fn non_planar_face_is_invalid_parameter() {
    assert_eq!(
        refused(roof([[1.0, 1.0, 1.0], [3.0, 1.0 + 1e-6, 1.0]])),
        ErrorCode::InvalidParameter
    );
}

#[test]
fn open_shell_and_inward_rings_are_invalid_topology() {
    let points = EAVES
        .iter()
        .chain(&[[1.0, 1.0, 1.0], [3.0, 1.0, 1.0]])
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(
        refused(polyhedron(&points, &HIP_FACES[..4])),
        ErrorCode::InvalidTopology
    );
    let inward = HIP_FACES
        .iter()
        .map(|ring| ring.iter().rev().copied().collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let inward = inward.iter().map(Vec::as_slice).collect::<Vec<_>>();
    assert_eq!(
        refused(polyhedron(&points, &inward)),
        ErrorCode::InvalidTopology
    );
}

#[test]
fn roof_minus_chimney_reaches_planar_polyhedra() {
    let mut world = graph();
    world.create_primitive(hip_roof(), named("roof")).unwrap();
    let chimney = Primitive::Cuboid {
        width: 0.5,
        height: 3.0,
        depth: 0.5,
    };
    world.create_primitive(chimney, named("tool")).unwrap();
    moved(&mut world, "tool", [2.0, -1.0, 1.0]);
    assert_eq!(
        subtracted(&mut world, "roof"),
        serde_json::json!(["subtract_planar_polyhedra"])
    );
    measured(&world, "roof", 299.0 / 96.0);
}

#[test]
fn wall_minus_roof_cut_reaches_planar_polyhedra() {
    let mut world = graph();
    let wall = Primitive::Cuboid {
        width: 0.2,
        height: 4.0,
        depth: 2.0,
    };
    world.create_primitive(wall, named("wall")).unwrap();
    moved(&mut world, "wall", [2.0, 0.0, 1.0]);
    measured(&world, "wall", 1.6);
    let points = [
        [0.0, 1.5, -0.5],
        [4.0, 1.5, -0.5],
        [4.0, 3.0, 1.0],
        [0.0, 3.0, 1.0],
        [0.0, 1.5, 2.5],
        [4.0, 1.5, 2.5],
        [0.0, 5.0, -0.5],
        [4.0, 5.0, -0.5],
        [4.0, 5.0, 2.5],
        [0.0, 5.0, 2.5],
    ];
    let faces: [&[u32]; 7] = [
        &[0, 1, 2, 3],
        &[3, 2, 5, 4],
        &[0, 6, 7, 1],
        &[4, 5, 8, 9],
        &[1, 7, 8, 5, 2],
        &[0, 3, 4, 9, 6],
        &[6, 9, 8, 7],
    ];
    world
        .create_primitive(polyhedron(&points, &faces), named("tool"))
        .unwrap();
    assert_eq!(
        subtracted(&mut world, "wall"),
        serde_json::json!(["subtract_planar_polyhedra"])
    );
    measured(&world, "wall", 1.0);
}

#[test]
fn polyhedron_json_takes_points_and_faces() {
    let json = r#"{"kind":"Polyhedron","points":[[0,0,0]],"faces":[[0]]}"#;
    assert!(serde_json::from_str::<Primitive>(json).is_ok());
    let json = r#"{"kind":"Polyhedron","vertices":[[0,0,0]],"faces":[[0]]}"#;
    assert!(serde_json::from_str::<Primitive>(json).is_err());
}

#[test]
fn huge_finite_coordinates_are_invalid_parameter() {
    let points = EAVES
        .iter()
        .chain(&[[1.0, 1.0, 1.0], [3.0, 1.0, 1.0]])
        .map(|point| point.map(|value| value * 1e200))
        .collect::<Vec<_>>();
    assert_eq!(
        refused(polyhedron(&points, &HIP_FACES)),
        ErrorCode::InvalidParameter
    );
}
