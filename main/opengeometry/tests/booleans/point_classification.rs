use opengeometry::brep::Frame3;
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
