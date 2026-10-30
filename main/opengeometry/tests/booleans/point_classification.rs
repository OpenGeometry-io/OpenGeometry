use opengeometry::brep::{BrepEnvelope, Frame3};
use opengeometry::operations::modifying::boolean::{boolean_brep, BooleanOp};
use opengeometry::primitives;
use opengeometry::query::{classify_point, PointClassification};
use opengeometry_test_support::accuracy::accuracy;

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
