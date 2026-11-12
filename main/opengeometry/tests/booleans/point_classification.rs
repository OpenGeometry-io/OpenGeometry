use opengeometry::brep::{BrepEnvelope, Frame3, GeometryError};
use opengeometry::operations::modifying::boolean::{boolean_brep, BooleanOp};
use opengeometry::primitives;
use opengeometry::query::{classify_point, PointClassification};
use opengeometry::world_graph::{CreatingOperation, Primitive};
use opengeometry_test_support::accuracy::accuracy;
use opengeometry_test_support::volume;
use opengeometry_test_support::world_graph::{graph, named};

#[test]
fn analytic_ray_classification_covers_all_surface_families() {
    let bodies = [
        (
            primitives::cuboid("box".into(), Frame3::IDENTITY, [2.0; 3], accuracy(1e-5)).unwrap(),
            [1.0, 1.0, 1.0],
            [3.0, 3.0, 3.0],
        ),
        (
            primitives::sphere("sphere".into(), Frame3::IDENTITY, 1.0, accuracy(1e-5)).unwrap(),
            [0.0; 3],
            [2.0, 0.0, 0.0],
        ),
        (
            primitives::cylinder(
                "cylinder".into(),
                Frame3::IDENTITY,
                1.0,
                2.0,
                accuracy(1e-5),
            )
            .unwrap(),
            [0.0, 0.0, 1.0],
            [2.0, 0.0, 1.0],
        ),
        (
            primitives::cone("cone".into(), Frame3::IDENTITY, 1.0, 2.0, accuracy(1e-5)).unwrap(),
            [0.0, 0.0, 1.0],
            [2.0, 0.0, 1.0],
        ),
        (
            primitives::torus("torus".into(), Frame3::IDENTITY, 3.0, 1.0, accuracy(1e-5)).unwrap(),
            [3.0, 0.0, 0.0],
            [0.0; 3],
        ),
    ];
    for (body, inside, outside) in bodies {
        assert_eq!(
            classify_point(&body, inside).unwrap(),
            PointClassification::Inside,
            "{}",
            body.id
        );
        assert_eq!(
            classify_point(&body, outside).unwrap(),
            PointClassification::Outside,
            "{}",
            body.id
        );
    }
}

#[test]
fn cavity_shell_uses_regularized_material_occupancy() {
    let host =
        primitives::torus("host".into(), Frame3::IDENTITY, 3.0, 1.0, accuracy(1e-5)).unwrap();
    let cutter =
        primitives::torus("cutter".into(), Frame3::IDENTITY, 3.0, 0.4, accuracy(1e-5)).unwrap();
    let result = boolean_brep(&host, &cutter, BooleanOp::Subtraction, "shell".into()).unwrap();
    assert_eq!(
        classify_point(&result.brep, [3.0, 0.0, 0.0]).unwrap(),
        PointClassification::Outside
    );
    assert_eq!(
        classify_point(&result.brep, [3.7, 0.0, 0.0]).unwrap(),
        PointClassification::Inside
    );
    assert_eq!(
        classify_point(&result.brep, [0.0; 3]).unwrap(),
        PointClassification::Outside
    );
}

fn solid_count(body: &BrepEnvelope) -> usize {
    serde_json::to_value(body).unwrap()["solids"]
        .as_array()
        .unwrap()
        .len()
}

fn tangent_cylinders() -> BrepEnvelope {
    let cylinder = |id: &str, y: f64| {
        let frame = Frame3 {
            origin: [0.0, y, 0.0],
            ..Frame3::IDENTITY
        };
        primitives::cylinder(id.into(), frame, 1.0, 2.0, accuracy(1e-5)).unwrap()
    };
    let lower = cylinder("lower", 0.0);
    let upper = cylinder("upper", 2.0);
    let union = boolean_brep(&lower, &upper, BooleanOp::Union, "pair".into()).unwrap();
    assert_eq!(solid_count(&union.brep), 2);
    union.brep
}

#[test]
fn a_point_whose_ray_crosses_the_contact_of_two_solids_is_inside_its_solid() {
    let body = tangent_cylinders();
    let length = (1.0_f64 + 0.371 * 0.371 + 0.127 * 0.127).sqrt();
    let direction = [1.0 / length, 0.371 / length, 0.127 / length];
    let contact = [0.0, 1.0, 1.0];
    let point = std::array::from_fn(|axis| contact[axis] - 0.5 * direction[axis]);
    assert_eq!(
        classify_point(&body, point).unwrap(),
        PointClassification::Inside
    );
    assert_eq!(
        classify_point(&body, [0.0, 2.0, 1.0]).unwrap(),
        PointClassification::Inside
    );
    assert_eq!(
        classify_point(&body, contact).unwrap(),
        PointClassification::Boundary
    );
    assert_eq!(
        classify_point(&body, [1.0, 1.0, 1.0]).unwrap(),
        PointClassification::Outside
    );
}

#[test]
fn each_solid_of_a_body_cut_in_two_classifies_its_own_points() {
    let host = primitives::cuboid(
        "host".into(),
        Frame3::IDENTITY,
        [10.0, 1.0, 3.0],
        accuracy(1e-5),
    )
    .unwrap();
    let frame = Frame3 {
        origin: [4.0, -1.0, -1.0],
        ..Frame3::IDENTITY
    };
    let slot = primitives::cuboid("slot".into(), frame, [1.0, 3.0, 5.0], accuracy(1e-5)).unwrap();
    let split = boolean_brep(&host, &slot, BooleanOp::Subtraction, "split".into()).unwrap();
    assert_eq!(solid_count(&split.brep), 2);
    for (point, expected) in [
        ([2.0, 0.5, 1.5], PointClassification::Inside),
        ([7.0, 0.5, 1.5], PointClassification::Inside),
        ([4.5, 0.5, 1.5], PointClassification::Outside),
        ([4.0, 0.5, 1.5], PointClassification::Boundary),
    ] {
        assert_eq!(
            classify_point(&split.brep, point).unwrap(),
            expected,
            "{point:?}"
        );
    }
}

fn ball(id: &str, origin: [f64; 3], radius: f64) -> BrepEnvelope {
    let frame = Frame3 {
        origin,
        ..Frame3::IDENTITY
    };
    primitives::sphere(id.into(), frame, radius, accuracy(1e-5)).unwrap()
}

fn cube(id: &str, origin: [f64; 3], size: f64) -> BrepEnvelope {
    let frame = Frame3 {
        origin,
        ..Frame3::IDENTITY
    };
    primitives::cuboid(id.into(), frame, [size; 3], accuracy(1e-5)).unwrap()
}

fn assert_refused(a: &BrepEnvelope, b: &BrepEnvelope) {
    for (first, second) in [(a, b), (b, a)] {
        for operation in [
            BooleanOp::Union,
            BooleanOp::Intersection,
            BooleanOp::Subtraction,
        ] {
            let Err(error) = boolean_brep(first, second, operation, "refused".into()) else {
                panic!("{} {operation:?} {} built", first.id, second.id);
            };
            assert!(
                matches!(
                    &error,
                    GeometryError::UnresolvedIntersection(message)
                        if message == "generic boolean containment classification is unresolved"
                ),
                "{} {operation:?} {}: {error:?}",
                first.id,
                second.id
            );
        }
    }
}

#[test]
fn ball_around_the_cavity_of_a_hollow_cube_is_refused_not_measured_wrong() {
    let outer = cube("outer", [0.0; 3], 4.0);
    let hollow = boolean_brep(
        &outer,
        &cube("cavity", [1.0; 3], 2.0),
        BooleanOp::Subtraction,
        "hollow".into(),
    )
    .unwrap()
    .brep;
    assert_refused(&hollow, &ball("ball", [2.0; 3], 1.9));
}

#[test]
fn two_balls_in_one_body_against_a_cube_holding_one_are_refused_not_measured_wrong() {
    let pair = boolean_brep(
        &ball("near", [2.0; 3], 0.5),
        &ball("far", [12.0, 2.0, 2.0], 0.5),
        BooleanOp::Union,
        "balls".into(),
    )
    .unwrap()
    .brep;
    assert_eq!(solid_count(&pair), 2);
    assert_refused(&pair, &cube("cube", [0.0; 3], 4.0));
}

#[test]
fn cube_minus_two_balls_inside_it_keeps_both_balls_as_cavities() {
    let pair = boolean_brep(
        &ball("left", [2.0, 4.0, 4.0], 0.5),
        &ball("right", [6.0, 4.0, 4.0], 0.5),
        BooleanOp::Union,
        "balls".into(),
    )
    .unwrap()
    .brep;
    let cube = cube("cube", [0.0; 3], 8.0);
    let result = boolean_brep(&cube, &pair, BooleanOp::Subtraction, "hollowed".into())
        .unwrap()
        .brep;
    assert_eq!(solid_count(&result), 1);
    for (point, place) in [
        ([2.0, 4.0, 4.0], PointClassification::Outside),
        ([6.0, 4.0, 4.0], PointClassification::Outside),
        ([4.0, 4.0, 4.0], PointClassification::Inside),
    ] {
        assert_eq!(classify_point(&result, point).unwrap(), place, "{point:?}");
    }
    let measured = volume::estimate(&result, 0.001);
    let expected = 512.0 - 2.0 * std::f64::consts::PI / 6.0;
    assert!((measured.value - expected).abs() <= measured.error_bound);
}

#[test]
fn open_tube_inside_a_cube_is_refused() {
    let mut world = graph();
    world
        .create_primitive(Primitive::Circle { radius: 0.3 }, named("profile"))
        .unwrap();
    world
        .create_primitive(
            Primitive::Polyline {
                points: vec![[0.0; 3], [0.0, 1.0, 0.0]],
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
                caps: Some(false),
            },
            named("tube"),
        )
        .unwrap();
    let tube = world.placed_brep("tube").unwrap().as_ref().clone();
    let cube = cube("cube", [-2.0, -1.5, -2.0], 4.0);
    for (first, second) in [(&cube, &tube), (&tube, &cube)] {
        for operation in [
            BooleanOp::Union,
            BooleanOp::Intersection,
            BooleanOp::Subtraction,
        ] {
            assert!(
                boolean_brep(first, second, operation, "sheet".into()).is_err(),
                "{} {operation:?} {}",
                first.id,
                second.id
            );
        }
    }
}

#[test]
fn ball_inside_the_second_of_two_cubes_becomes_that_cubes_cavity() {
    let cubes = boolean_brep(
        &cube("first", [0.0; 3], 4.0),
        &cube("second", [10.0, 0.0, 0.0], 4.0),
        BooleanOp::Union,
        "cubes".into(),
    )
    .unwrap()
    .brep;
    assert_eq!(solid_count(&cubes), 2);
    let ball = ball("ball", [12.0, 2.0, 2.0], 1.0);
    let result = boolean_brep(&cubes, &ball, BooleanOp::Subtraction, "hollowed".into())
        .unwrap()
        .brep;
    result.validate().unwrap();
    let solids = serde_json::to_value(&result).unwrap()["solids"].clone();
    assert_eq!(solids[0]["cavity_shells"].as_array().unwrap().len(), 0);
    assert_eq!(solids[1]["cavity_shells"].as_array().unwrap().len(), 1);
    assert_eq!(
        classify_point(&result, [12.0, 2.0, 2.0]).unwrap(),
        PointClassification::Outside
    );
    let measured = volume::estimate(&result, 0.001);
    let expected = 128.0 - 4.0 / 3.0 * std::f64::consts::PI;
    assert!((measured.value - expected).abs() <= measured.error_bound);
}
