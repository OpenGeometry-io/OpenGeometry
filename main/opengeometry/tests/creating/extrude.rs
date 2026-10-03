use super::support::extrude;
use opengeometry::world_graph::{
    CreateOptions, CreatingOperation, ErrorCode, Primitive, Transform,
};
use opengeometry_test_support::volume;
use opengeometry_test_support::world_graph::{graph, named};

#[test]
fn rectangle_extrusion_has_independent_volume() {
    let mut world = graph();
    world
        .create_primitive(
            Primitive::Rectangle {
                width: 2.0,
                breadth: 3.0,
            },
            named("profile"),
        )
        .unwrap();
    world
        .create_operation(extrude("profile", 4.0), named("solid"))
        .unwrap();
    let measured = volume::estimate(&world.brep("solid").unwrap(), 0.001);
    assert!((measured.value - 24.0).abs() <= measured.error_bound);
}

#[test]
fn circle_extrusion_matches_cylinder_builder() {
    let mut world = graph();
    world
        .create_primitive(Primitive::Circle { radius: 2.0 }, named("profile"))
        .unwrap();
    world
        .create_operation(extrude("profile", 3.0), named("solid"))
        .unwrap();
    world
        .create_primitive(
            Primitive::Cylinder {
                radius: 2.0,
                height: 3.0,
            },
            named("cylinder"),
        )
        .unwrap();
    let a = world.brep("solid").unwrap();
    let b = world.brep("cylinder").unwrap();
    let mut a = (*a).clone();
    let mut b = (*b).clone();
    a.id = "same".into();
    b.id = "same".into();
    for body in [&mut a, &mut b] {
        for face in &mut body.topology.faces {
            for source in &mut face.provenance.sources {
                source.entity = "same".into();
                source.body = "same".into();
            }
        }
    }
    assert_eq!(a.to_json().unwrap(), b.to_json().unwrap());
}

#[test]
fn negative_extrusion_and_circular_hole_have_expected_volume() {
    let mut world = graph();
    world
        .create_primitive(Primitive::Circle { radius: 2.0 }, named("outer"))
        .unwrap();
    world
        .create_primitive(Primitive::Circle { radius: 1.0 }, named("hole"))
        .unwrap();
    world
        .create_operation(
            CreatingOperation::Extrude {
                profile: "outer".into(),
                holes: vec!["hole".into()],
                distance: -3.0,
            },
            named("solid"),
        )
        .unwrap();
    let measured = volume::estimate(&world.brep("solid").unwrap(), 0.001);
    assert!((measured.value - 9.0 * std::f64::consts::PI).abs() <= measured.error_bound);
    let bounds = world.bounds("solid").unwrap().unwrap();
    assert!((bounds[1] + 3.0).abs() < 2e-8);
    assert!(bounds[4] < 2e-8);
}

#[test]
fn rectangle_with_round_hole_uses_curved_extrusion() {
    let mut world = graph();
    world
        .create_primitive(
            Primitive::Rectangle {
                width: 4.0,
                breadth: 4.0,
            },
            named("outer"),
        )
        .unwrap();
    world
        .create_primitive(Primitive::Circle { radius: 1.0 }, named("hole"))
        .unwrap();
    world
        .create_operation(
            CreatingOperation::Extrude {
                profile: "outer".into(),
                holes: vec!["hole".into()],
                distance: 2.0,
            },
            named("solid"),
        )
        .unwrap();
    let measured = volume::estimate(&world.brep("solid").unwrap(), 0.001);
    assert!((measured.value - 2.0 * (16.0 - std::f64::consts::PI)).abs() <= measured.error_bound);
}

#[test]
fn tilted_polyline_extrudes_along_its_normal() {
    let mut world = graph();
    world
        .create_primitive(
            Primitive::Polyline {
                points: vec![
                    [0.0, 0.0, 0.0],
                    [2.0, 0.0, 0.0],
                    [2.0, 3.0, 0.0],
                    [0.0, 3.0, 0.0],
                ],
                closed: true,
            },
            named("profile"),
        )
        .unwrap();
    world
        .create_operation(extrude("profile", 4.0), named("solid"))
        .unwrap();
    let measured = volume::estimate(&world.brep("solid").unwrap(), 0.001);
    assert!((measured.value - 24.0).abs() <= measured.error_bound);
    let bounds = world.bounds("solid").unwrap().unwrap();
    assert!(bounds[2] >= -2e-8, "{bounds:?}");
    assert!((bounds[5] - 4.0).abs() < 2e-8, "{bounds:?}");
}

#[test]
fn concave_tilted_polyline_extrudes_to_right_hand_rule_side() {
    let first = [0.0, 0.0, 0.0];
    let normal = [0.0, -0.8, 0.6];
    let mut world = graph();
    world
        .create_primitive(
            Primitive::Polyline {
                points: vec![
                    first,
                    [1.0, 0.6, 0.8],
                    [2.0, 0.0, 0.0],
                    [2.0, 1.2, 1.6],
                    [0.0, 1.2, 1.6],
                ],
                closed: true,
            },
            named("profile"),
        )
        .unwrap();
    world
        .create_operation(extrude("profile", 2.0), named("solid"))
        .unwrap();
    let heights = world
        .brep("solid")
        .unwrap()
        .topology
        .vertices
        .iter()
        .map(|vertex| {
            (0..3)
                .map(|axis| (vertex.position[axis] - first[axis]) * normal[axis])
                .sum::<f64>()
        })
        .collect::<Vec<_>>();
    assert!(heights.iter().all(|height| *height >= -4e-8), "{heights:?}");
    assert!(
        heights.iter().any(|height| *height >= 2.0 - 4e-8),
        "{heights:?}"
    );
}

#[test]
fn profile_tilted_1e_6_rad_is_not_flattened() {
    let points = vec![
        [0.0, 0.0, 0.0],
        [0.0, -2.9999999999995e-6, 2.9999999999985],
        [2.0, -2.9999999999995e-6, 2.9999999999985],
        [2.0, 0.0, 0.0],
    ];
    let normal = [0.0, 0.9999999999995, 9.999999999998333e-7];
    let mut world = graph();
    world
        .create_primitive(
            Primitive::Polyline {
                points: points.clone(),
                closed: true,
            },
            named("profile"),
        )
        .unwrap();
    world
        .create_operation(extrude("profile", 2.0), named("solid"))
        .unwrap();
    let body = world.brep("solid").unwrap();
    let near_vertex = |point: [f64; 3]| {
        body.topology.vertices.iter().any(|vertex| {
            (0..3)
                .map(|axis| (vertex.position[axis] - point[axis]).powi(2))
                .sum::<f64>()
                .sqrt()
                <= 4e-8
        })
    };
    for point in points {
        assert!(near_vertex(point), "{point:?}");
        let top = [0, 1, 2].map(|axis| point[axis] + 2.0 * normal[axis]);
        assert!(near_vertex(top), "{top:?}");
    }
}

#[test]
fn translated_parent_and_far_profile_keep_local_geometry_small() {
    let mut world = graph();
    world.create_system_assembly(named("assembly")).unwrap();
    world
        .transform(
            "assembly",
            Transform::Translate {
                offset: [10_000.0, 0.0, 0.0],
            },
        )
        .unwrap();
    world
        .create_primitive(
            Primitive::Rectangle {
                width: 2.0,
                breadth: 2.0,
            },
            CreateOptions {
                parent: Some("assembly".into()),
                ..named("profile")
            },
        )
        .unwrap();
    world
        .create_operation(extrude("profile", 3.0), named("solid"))
        .unwrap();
    let local = world.brep("solid").unwrap().bounds().unwrap().unwrap();
    assert!(local.axes[0].hi < 2.0);
    let bounds = world.bounds("solid").unwrap().unwrap();
    assert!((bounds[0] - 9999.0).abs() < 2e-8);
    assert!((bounds[3] - 10001.0).abs() < 2e-8);
}

#[test]
fn closed_polyline_profile_rejects_self_crossing() {
    let mut world = graph();
    world
        .create_primitive(
            Primitive::Polyline {
                points: vec![
                    [-1.0, 0.0, -1.0],
                    [1.0, 0.0, 1.0],
                    [-1.0, 0.0, 1.0],
                    [1.0, 0.0, -1.0],
                ],
                closed: true,
            },
            named("profile"),
        )
        .unwrap();
    assert_eq!(
        world
            .create_operation(extrude("profile", 2.0), named("solid"))
            .unwrap_err()
            .error_code(),
        ErrorCode::InvalidParameter
    );
}

#[test]
fn ground_polyline_normal_does_not_depend_on_order() {
    let points = vec![
        [-1.0, 0.0, -1.0],
        [1.0, 0.0, -1.0],
        [1.0, 0.0, 1.0],
        [-1.0, 0.0, 1.0],
    ];
    let mut world = graph();
    world
        .create_primitive(
            Primitive::Polyline {
                points: points.clone(),
                closed: true,
            },
            named("forward"),
        )
        .unwrap();
    world
        .create_primitive(
            Primitive::Polyline {
                points: points.into_iter().rev().collect(),
                closed: true,
            },
            named("reverse"),
        )
        .unwrap();
    world
        .create_operation(extrude("forward", 2.0), named("a"))
        .unwrap();
    world
        .create_operation(extrude("reverse", 2.0), named("b"))
        .unwrap();
    assert!(world.bounds("a").unwrap().unwrap()[1] >= -2e-8);
    assert!(world.bounds("b").unwrap().unwrap()[1] >= -2e-8);
    assert!((world.bounds("b").unwrap().unwrap()[4] - 2.0).abs() < 2e-8);
}
