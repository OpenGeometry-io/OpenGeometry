use opengeometry::brep::{BodyType, Frame3};
use opengeometry::primitives;
use opengeometry::query::{classify_point, PointClassification};
use opengeometry_test_support::accuracy::accuracy;
use opengeometry_test_support::volume::estimate;

#[test]
fn independent_volumes_match_analytic_formulas() {
    let frame = Frame3::IDENTITY;
    let tolerance = accuracy(1e-6);
    let shapes = [
        (
            primitives::cuboid("box".into(), frame, [2.0, 3.0, 4.0], tolerance).unwrap(),
            24.0,
        ),
        (
            primitives::cylinder("cylinder".into(), frame, 1.0, 2.0, tolerance).unwrap(),
            2.0 * std::f64::consts::PI,
        ),
        (
            primitives::sphere("sphere".into(), frame, 1.0, tolerance).unwrap(),
            4.0 * std::f64::consts::PI / 3.0,
        ),
        (
            primitives::cone("cone".into(), frame, 1.0, 2.0, tolerance).unwrap(),
            2.0 * std::f64::consts::PI / 3.0,
        ),
        (
            primitives::frustum("frustum".into(), frame, 1.0, 0.4, 2.0, tolerance).unwrap(),
            2.0 * std::f64::consts::PI * (1.0 + 0.4 + 0.16) / 3.0,
        ),
        (
            primitives::torus("torus".into(), frame, 2.0, 0.5, tolerance).unwrap(),
            std::f64::consts::PI.powi(2),
        ),
        (
            primitives::annular_cylinder("tube".into(), frame, 0.4, 1.0, 2.0, tolerance).unwrap(),
            2.0 * std::f64::consts::PI * (1.0 - 0.16),
        ),
    ];
    for (body, expected) in shapes {
        assert_eq!(body.body_type().unwrap(), BodyType::Solid);
        let measured = estimate(&body, 0.01);
        assert!(
            (measured.value - expected).abs() <= measured.error_bound,
            "{}: {} versus {} with bound {}",
            body.id,
            measured.value,
            expected,
            measured.error_bound
        );
    }
}

#[test]
fn wire_types_and_point_classification_are_independent_of_fixtures() {
    let rectangle = primitives::rectangle(
        "rectangle".into(),
        Frame3::IDENTITY,
        4.0,
        2.0,
        accuracy(1e-6),
    )
    .unwrap();
    let (polyline, polyline_keys) = primitives::polyline_with_keys(
        "polyline".into(),
        &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]],
        false,
        accuracy(1e-6),
    )
    .unwrap();
    assert_eq!(polyline_keys, ["seg-0", "seg-1"]);
    assert_eq!(rectangle.body_type().unwrap(), BodyType::Wire);
    assert_eq!(polyline.body_type().unwrap(), BodyType::Wire);
    assert_eq!(rectangle.topology.edges.len(), 4);
    assert_eq!(polyline.topology.edges.len(), 2);
    let box_body =
        primitives::cuboid("box".into(), Frame3::IDENTITY, [2.0; 3], accuracy(1e-6)).unwrap();
    assert_eq!(
        classify_point(&box_body, [1.0; 3]).unwrap(),
        PointClassification::Inside
    );
    assert_eq!(
        classify_point(&box_body, [3.0; 3]).unwrap(),
        PointClassification::Outside
    );
}

#[test]
fn ground_anchors_fit_the_conservative_bounds_and_edge_order() {
    let ground = Frame3 {
        x: [1.0, 0.0, 0.0],
        y: [0.0, 0.0, -1.0],
        z: [0.0, 1.0, 0.0],
        ..Frame3::IDENTITY
    };
    let (rectangle, rectangle_keys) =
        primitives::rectangle_with_keys("rectangle".into(), ground, 4.0, 2.0, accuracy(1e-6))
            .unwrap();
    assert_eq!(rectangle_keys, ["edge-0", "edge-1", "edge-2", "edge-3"]);
    let bounds = rectangle.bounds().unwrap().unwrap();
    assert!(bounds.contains([-2.0, 0.0, -1.0]));
    assert!(bounds.contains([2.0, 0.0, 1.0]));
    let edges = &rectangle.topology.halfedges;
    let vertices = &rectangle.topology.vertices;
    let endpoints = |index: usize| {
        (
            vertices[edges[index].from as usize].position,
            vertices[edges[index].to as usize].position,
        )
    };
    let (a, b) = endpoints(0);
    assert_eq!((a[0], b[0]), (2.0, 2.0));
    let (a, b) = endpoints(1);
    assert_eq!((a[2], b[2]), (-1.0, -1.0));
    let (a, b) = endpoints(2);
    assert_eq!((a[0], b[0]), (-2.0, -2.0));
    let (a, b) = endpoints(3);
    assert_eq!((a[2], b[2]), (1.0, 1.0));

    let cuboid_frame = Frame3 {
        origin: [-2.0, 0.0, 1.5],
        ..ground
    };
    let cuboid = primitives::cuboid(
        "cuboid".into(),
        cuboid_frame,
        [4.0, 3.0, 5.0],
        accuracy(1e-6),
    )
    .unwrap();
    let bounds = cuboid.bounds().unwrap().unwrap();
    assert!(bounds.contains([-2.0, 0.0, -1.5]));
    assert!(bounds.contains([2.0, 5.0, 1.5]));
    let cylinder =
        primitives::cylinder("cylinder".into(), ground, 1.0, 3.0, accuracy(1e-6)).unwrap();
    let bounds = cylinder.bounds().unwrap().unwrap();
    for point in [
        [-1.0, 0.0, 0.0],
        [1.0, 3.0, 0.0],
        [0.0, 1.5, -1.0],
        [0.0, 1.5, 1.0],
    ] {
        assert!(bounds.contains(point));
    }
}
