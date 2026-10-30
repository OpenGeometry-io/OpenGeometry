use super::Similarity3;
use crate::math::Point3;

fn near(a: Point3, b: Point3) {
    for axis in 0..3 {
        assert!((a[axis] - b[axis]).abs() < 1e-12, "{a:?} != {b:?}");
    }
}

#[test]
fn axis_angle_rotates_about_the_requested_axis() {
    let rotation = Similarity3::from_axis_angle(
        [2.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        std::f64::consts::FRAC_PI_2,
        1.0,
    )
    .unwrap();
    near(rotation.apply_point([1.0, 0.0, 0.0]), [2.0, 1.0, 0.0]);
    assert_eq!(rotation.frame.x, [0.0, 1.0, 0.0]);
    rotation.validate().unwrap();
}

#[test]
fn repeated_composition_preserves_orthonormality() {
    let step = Similarity3::from_axis_angle([0.0; 3], [0.1, 0.2, 0.3], 0.007, 1.0).unwrap();
    let mut combined = Similarity3::IDENTITY;
    for _ in 0..10000 {
        combined = combined.compose(&step);
    }
    combined.validate().unwrap();
    near(
        combined
            .inverse()
            .apply_point(combined.apply_point([1.0, 2.0, 3.0])),
        [1.0, 2.0, 3.0],
    );
}
