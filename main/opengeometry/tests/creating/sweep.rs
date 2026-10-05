use super::support::extrude;
use opengeometry::world_graph::{
    CreateOptions, CreatingOperation, EditScope, ErrorCode, Plane, Primitive, Transform,
};
use opengeometry_test_support::volume;
use opengeometry_test_support::world_graph::{graph, named};

#[test]
fn l_path_sweep_has_centroid_path_volume() {
    let mut world = graph();
    world
        .create_primitive(
            Primitive::Rectangle {
                width: 2.0,
                breadth: 2.0,
            },
            named("profile"),
        )
        .unwrap();
    world
        .create_primitive(
            Primitive::Polyline {
                points: vec![[0.0, 0.0, 0.0], [0.0, 3.0, 0.0], [3.0, 3.0, 0.0]],
                closed: false,
            },
            named("path"),
        )
        .unwrap();
    world
        .create_operation(
            CreatingOperation::Sweep {
                profile: "profile".into(),
                path: "path".into(),
            },
            named("solid"),
        )
        .unwrap();
    let body = world.brep("solid").unwrap();
    assert_eq!(body.topology.faces.len(), 10);
    let measured = volume::estimate(&body, 0.001);
    assert!(
        (measured.value - 24.0).abs() <= measured.error_bound,
        "{}",
        measured.value
    );
}

#[test]
fn noncoplanar_path_sweep_is_valid_and_repeatable() {
    let mut world = graph();
    world
        .create_primitive(
            Primitive::Rectangle {
                width: 1.0,
                breadth: 1.0,
            },
            named("profile"),
        )
        .unwrap();
    world
        .create_primitive(
            Primitive::Polyline {
                points: vec![
                    [0.0, 0.0, 0.0],
                    [0.0, 3.0, 0.0],
                    [3.0, 3.0, 0.0],
                    [3.0, 3.0, 3.0],
                ],
                closed: false,
            },
            named("path"),
        )
        .unwrap();
    let operation = CreatingOperation::Sweep {
        profile: "profile".into(),
        path: "path".into(),
    };
    world
        .create_operation(operation.clone(), named("solid"))
        .unwrap();
    let first = world.brep("solid").unwrap().to_json().unwrap();
    world
        .rebuild_operation("solid", operation, EditScope::Node)
        .unwrap();
    let mut second = (*world.brep("solid").unwrap()).clone();
    second.revision = 0;
    assert_eq!(first, second.to_json().unwrap());
}

#[test]
fn circular_l_path_sweep_is_watertight() {
    let mut world = graph();
    world
        .create_primitive(Primitive::Circle { radius: 0.5 }, named("profile"))
        .unwrap();
    world
        .create_primitive(
            Primitive::Polyline {
                points: vec![[0.0, 0.0, 0.0], [0.0, 3.0, 0.0], [3.0, 3.0, 0.0]],
                closed: false,
            },
            named("path"),
        )
        .unwrap();
    world
        .create_operation(
            CreatingOperation::Sweep {
                profile: "profile".into(),
                path: "path".into(),
            },
            named("solid"),
        )
        .unwrap();
    let body = world.brep("solid").unwrap();
    body.validate().unwrap();
    assert_eq!(body.topology.faces.len(), 6);
    let measured = volume::estimate(&body, 0.002);
    assert!((measured.value - std::f64::consts::PI * 0.25 * 6.0).abs() <= measured.error_bound);
    let mesh = opengeometry::tessellation::tessellate(&body, 0.02, 1_000_000).unwrap();
    let point = |index: u32| {
        let offset = index as usize * 3;
        [
            mesh.positions[offset].to_bits(),
            mesh.positions[offset + 1].to_bits(),
            mesh.positions[offset + 2].to_bits(),
        ]
    };
    let mut edges = std::collections::BTreeMap::new();
    for triangle in mesh.indices.chunks_exact(3) {
        for (from, to) in [
            (triangle[0], triangle[1]),
            (triangle[1], triangle[2]),
            (triangle[2], triangle[0]),
        ] {
            let mut pair = [point(from), point(to)];
            pair.sort();
            *edges.entry(pair).or_insert(0usize) += 1;
        }
    }
    assert!(edges.values().all(|count| *count == 2));
}

#[test]
fn circular_noncoplanar_sweep_is_valid() {
    let mut world = graph();
    world
        .create_primitive(Primitive::Circle { radius: 0.4 }, named("profile"))
        .unwrap();
    world
        .create_primitive(
            Primitive::Polyline {
                points: vec![
                    [0.0, 0.0, 0.0],
                    [0.0, 3.0, 0.0],
                    [3.0, 3.0, 0.0],
                    [3.0, 3.0, 3.0],
                ],
                closed: false,
            },
            named("path"),
        )
        .unwrap();
    world
        .create_operation(
            CreatingOperation::Sweep {
                profile: "profile".into(),
                path: "path".into(),
            },
            named("solid"),
        )
        .unwrap();
    world.brep("solid").unwrap().validate().unwrap();
}

#[test]
fn circular_sweep_keeps_profile_offset_from_path() {
    let mut world = graph();
    world
        .create_primitive(Primitive::Circle { radius: 0.3 }, named("profile"))
        .unwrap();
    world
        .transform(
            "profile",
            Transform::Translate {
                offset: [1.0, 0.0, 0.0],
            },
        )
        .unwrap();
    world
        .create_primitive(
            Primitive::Polyline {
                points: vec![[0.0, 0.0, 0.0], [0.0, 3.0, 0.0], [3.0, 3.0, 0.0]],
                closed: false,
            },
            named("path"),
        )
        .unwrap();
    world
        .create_operation(
            CreatingOperation::Sweep {
                profile: "profile".into(),
                path: "path".into(),
            },
            named("solid"),
        )
        .unwrap();
    let measured = volume::estimate(&world.brep("solid").unwrap(), 0.001);
    assert!((measured.value - std::f64::consts::PI * 0.09 * 4.0).abs() <= measured.error_bound);
}

#[test]
fn invalid_sweep_and_extrude_leave_graph_unchanged() {
    let mut world = graph();
    world
        .create_primitive(
            Primitive::Rectangle {
                width: 2.0,
                breadth: 2.0,
            },
            named("profile"),
        )
        .unwrap();
    world
        .create_primitive(
            Primitive::Polyline {
                points: vec![[0.0, 1.0, 0.0], [0.0, 3.0, 0.0], [3.0, 3.0, 0.0]],
                closed: false,
            },
            named("path"),
        )
        .unwrap();
    let revision = world.revision();
    let nodes = world.node_count();
    let error = world
        .create_operation(
            CreatingOperation::Sweep {
                profile: "profile".into(),
                path: "path".into(),
            },
            named("bad"),
        )
        .unwrap_err();
    assert_eq!(error.error_code(), ErrorCode::InvalidParameter);
    assert_eq!(
        world
            .create_operation(extrude("profile", 1e-9), named("bad"))
            .unwrap_err()
            .error_code(),
        ErrorCode::InvalidParameter
    );
    assert_eq!(world.revision(), revision);
    assert_eq!(world.node_count(), nodes);
}

#[test]
fn sweep_rejects_miter_and_separated_segment_collisions() {
    for points in [
        vec![[0.0, 0.0, 0.0], [0.0, 0.2, 0.0], [2.0, 0.2, 0.0]],
        vec![
            [0.0, 0.0, 0.0],
            [0.0, 3.0, 0.0],
            [1.0, 3.0, 0.0],
            [1.0, 0.0, 0.0],
        ],
    ] {
        let mut world = graph();
        world
            .create_primitive(Primitive::Circle { radius: 0.6 }, named("profile"))
            .unwrap();
        world
            .create_primitive(
                Primitive::Polyline {
                    points,
                    closed: false,
                },
                named("path"),
            )
            .unwrap();
        let error = world
            .create_operation(
                CreatingOperation::Sweep {
                    profile: "profile".into(),
                    path: "path".into(),
                },
                named("solid"),
            )
            .unwrap_err();
        assert_eq!(error.error_code(), ErrorCode::SweepSelfIntersection);
    }
}

#[test]
fn sweep_profile_tilted_1e_6_rad_gives_invalid_parameter() {
    let tilt = 1e-6_f64;
    let mut world = graph();
    world
        .create_primitive(
            Primitive::Rectangle {
                width: 2.0,
                breadth: 2.0,
            },
            CreateOptions {
                og_id: Some("tilted".into()),
                plane: Some(Plane {
                    normal: Some([0.0, -tilt.sin(), tilt.cos()]),
                    ..Plane::default()
                }),
                ..CreateOptions::default()
            },
        )
        .unwrap();
    world
        .create_primitive(
            Primitive::Polyline {
                points: vec![[0.0, 0.0, 0.0], [0.0, 0.0, 3.0], [3.0, 0.0, 3.0]],
                closed: false,
            },
            named("rise"),
        )
        .unwrap();
    let revision = world.revision();
    let nodes = world.node_count();
    let error = world
        .create_operation(
            CreatingOperation::Sweep {
                profile: "tilted".into(),
                path: "rise".into(),
            },
            named("tilted-sweep"),
        )
        .unwrap_err();
    assert_eq!(error.error_code(), ErrorCode::InvalidParameter, "{error:?}");
    assert_eq!((world.revision(), world.node_count()), (revision, nodes));
}

#[test]
fn collinear_diagonal_path_merges() {
    let mut world = graph();
    world
        .create_primitive(
            Primitive::Rectangle {
                width: 0.1,
                breadth: 0.1,
            },
            CreateOptions {
                og_id: Some("square".into()),
                plane: Some(Plane {
                    normal: Some([1.0, 2.0, 3.0]),
                    ..Plane::default()
                }),
                ..CreateOptions::default()
            },
        )
        .unwrap();
    world
        .create_primitive(
            Primitive::Polyline {
                points: vec![[0.0, 0.0, 0.0], [0.1, 0.2, 0.3], [0.3, 0.6, 0.9]],
                closed: false,
            },
            named("diagonal"),
        )
        .unwrap();
    world
        .create_operation(
            CreatingOperation::Sweep {
                profile: "square".into(),
                path: "diagonal".into(),
            },
            named("prism"),
        )
        .unwrap();
    let prism = world.brep("prism").unwrap();
    assert_eq!(prism.topology.faces.len(), 6);
    let expected = 0.01 * 1.1224972160321824;
    let measured = volume::estimate(&prism, 0.0005);
    assert!(
        (measured.value - expected).abs() <= measured.error_bound,
        "{} vs {expected}",
        measured.value
    );
}
