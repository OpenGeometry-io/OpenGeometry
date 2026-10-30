use opengeometry::brep::Frame3;
use opengeometry::primitives;
use opengeometry_test_support::accuracy::accuracy;
use opengeometry_test_support::volume::estimate;

#[test]
fn planar_volume_is_exact_under_translation() {
    let frame = Frame3 {
        origin: [10.0, -7.0, 4.0],
        ..Frame3::IDENTITY
    };
    let body =
        primitives::cuboid("translated".into(), frame, [2.0, 3.0, 4.0], accuracy(1e-6)).unwrap();
    let measured = estimate(&body, 0.01);
    assert!((measured.value - 24.0).abs() < 1e-10);
    assert!(measured.error_bound < 1e-8);
}
