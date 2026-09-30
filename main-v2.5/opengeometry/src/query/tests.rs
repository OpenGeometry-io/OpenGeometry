use super::classification::PointClassification;
use super::classify::classify_point;
use crate::brep::{coarse_accuracy, Frame3};
use crate::primitives;

#[test]
fn i3_unknown_classification_is_never_treated_as_outside() {
    assert!(!PointClassification::Unknown.is_proven_outside());
    assert!(!PointClassification::Boundary.is_proven_outside());
    assert!(!PointClassification::Inside.is_proven_outside());
    assert!(PointClassification::Outside.is_proven_outside());
}
#[test]
fn analytic_ray_classification_covers_all_surface_families() {
    let bodies = [
        (
            primitives::cuboid(
                "box".into(),
                Frame3::IDENTITY,
                [2.0; 3],
                coarse_accuracy(1e-5),
            )
            .unwrap(),
            [1.0, 1.0, 1.0],
            [3.0, 3.0, 3.0],
        ),
        (
            primitives::sphere(
                "sphere".into(),
                Frame3::IDENTITY,
                1.0,
                coarse_accuracy(1e-5),
            )
            .unwrap(),
            [0.0; 3],
            [2.0, 0.0, 0.0],
        ),
        (
            primitives::cylinder(
                "cylinder".into(),
                Frame3::IDENTITY,
                1.0,
                2.0,
                coarse_accuracy(1e-5),
            )
            .unwrap(),
            [0.0, 0.0, 1.0],
            [2.0, 0.0, 1.0],
        ),
        (
            primitives::cone(
                "cone".into(),
                Frame3::IDENTITY,
                1.0,
                2.0,
                coarse_accuracy(1e-5),
            )
            .unwrap(),
            [0.0, 0.0, 1.0],
            [2.0, 0.0, 1.0],
        ),
        (
            primitives::torus(
                "torus".into(),
                Frame3::IDENTITY,
                3.0,
                1.0,
                coarse_accuracy(1e-5),
            )
            .unwrap(),
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
