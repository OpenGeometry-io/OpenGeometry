use opengeometry::brep::{BodyType, BrepEnvelope, Frame3, GeometryError, SurfaceGeometry};
use opengeometry::primitives::{self, ProfileEdge};
use opengeometry::query::{classify_point, PointClassification};
use opengeometry_test_support::accuracy::accuracy;
use opengeometry_test_support::volume::estimate;
use std::f64::consts::{FRAC_PI_2, PI};

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

fn line(from: [f64; 2], to: [f64; 2]) -> ProfileEdge {
    ProfileEdge::Line { from, to }
}

fn arc(center: [f64; 2], radius: f64, angles: [f64; 2]) -> ProfileEdge {
    let [start_angle, sweep_angle] = angles;
    ProfileEdge::Arc {
        center,
        radius,
        start_angle,
        sweep_angle,
    }
}

fn polygon(points: &[[f64; 2]]) -> Vec<ProfileEdge> {
    let next = points.iter().cycle().skip(1);
    points
        .iter()
        .zip(next)
        .map(|(&from, &to)| line(from, to))
        .collect()
}

fn revolved(edges: Vec<ProfileEdge>) -> Result<BrepEnvelope, GeometryError> {
    primitives::revolved_profile("revolved".into(), Frame3::IDENTITY, edges, accuracy(1e-6))
}

fn semicircle() -> Vec<ProfileEdge> {
    vec![
        arc([0.0, 0.0], 1.0, [-FRAC_PI_2, PI]),
        line([0.0, 1.0], [0.0, -1.0]),
    ]
}

fn off_axis_circle() -> Vec<ProfileEdge> {
    vec![
        arc([2.0, 0.0], 0.5, [-FRAC_PI_2, PI]),
        arc([2.0, 0.0], 0.5, [FRAC_PI_2, PI]),
    ]
}

fn bowl(outer: f64, inner: f64) -> Vec<ProfileEdge> {
    vec![
        arc([0.0, 0.0], outer, [-FRAC_PI_2, FRAC_PI_2]),
        line([outer, 0.0], [inner, 0.0]),
        arc([0.0, 0.0], inner, [0.0, -FRAC_PI_2]),
        line([0.0, -inner], [0.0, -outer]),
    ]
}

fn revolved_volume_cases() -> Vec<(&'static str, Vec<ProfileEdge>, f64)> {
    let tube = [[0.4, 0.0], [1.0, 0.0], [1.0, 2.0], [0.4, 2.0]];
    let mut clockwise = tube;
    clockwise.reverse();
    vec![
        ("tube", polygon(&tube), PI * 2.0 * (1.0 - 0.16)),
        (
            "clockwise tube",
            polygon(&clockwise),
            PI * 2.0 * (1.0 - 0.16),
        ),
        ("ball", semicircle(), 4.0 * PI / 3.0),
        ("ring", off_axis_circle(), 2.0 * PI * PI * 2.0 * 0.25),
        (
            "cone",
            polygon(&[[0.0, 0.0], [1.0, 0.0], [0.0, 2.0]]),
            2.0 * PI / 3.0,
        ),
        (
            "funnel",
            polygon(&[[0.0, 0.0], [1.0, 2.0], [0.0, 2.0]]),
            2.0 * PI / 3.0,
        ),
        (
            "frustum",
            polygon(&[[0.0, 0.0], [1.0, 0.0], [0.4, 2.0], [0.0, 2.0]]),
            2.0 * PI * (1.0 + 0.4 + 0.16) / 3.0,
        ),
        ("bowl", bowl(1.0, 0.8), 2.0 * PI * (1.0 - 0.512) / 3.0),
        (
            "concave torus wall",
            concave_wall(),
            PI * (76.0 / 3.0 - 4.0 * PI),
        ),
        ("ice cream", ice_cream(), 4.0 * PI / 3.0),
    ]
}

fn concave_wall() -> Vec<ProfileEdge> {
    vec![
        line([1.0, 0.0], [3.0, 0.0]),
        arc([4.0, 1.0], 2.0_f64.sqrt(), [1.25 * PI, -FRAC_PI_2]),
        line([3.0, 2.0], [1.0, 2.0]),
        line([1.0, 2.0], [1.0, 0.0]),
    ]
}

fn ice_cream() -> Vec<ProfileEdge> {
    vec![
        arc([0.0, 0.0], 1.0, [-FRAC_PI_2, FRAC_PI_2]),
        line([1.0, 0.0], [0.0, 2.0]),
        line([0.0, 2.0], [0.0, -1.0]),
    ]
}

#[test]
fn revolved_profiles_match_analytic_volumes() {
    for (name, edges, expected) in revolved_volume_cases() {
        let body = revolved(edges).unwrap_or_else(|error| panic!("{name}: {error:?}"));
        body.validate().unwrap();
        assert_eq!(body.body_type().unwrap(), BodyType::Solid, "{name}");
        let measured = estimate(&body, 0.01);
        assert!(
            (measured.value - expected).abs() <= measured.error_bound,
            "{name}: {} versus {} with bound {}",
            measured.value,
            expected,
            measured.error_bound
        );
    }
}

fn surface_kinds(body: &BrepEnvelope) -> Vec<&'static str> {
    let mut kinds = body
        .geometry
        .surfaces
        .iter()
        .map(|surface| match surface {
            SurfaceGeometry::Plane { .. } => "plane",
            SurfaceGeometry::Sphere { .. } => "sphere",
            SurfaceGeometry::Cylinder { .. } => "cylinder",
            SurfaceGeometry::Cone { .. } => "cone",
            SurfaceGeometry::Torus { .. } => "torus",
        })
        .collect::<Vec<_>>();
    kinds.sort();
    kinds
}

#[test]
fn revolved_profiles_build_one_analytic_face_per_edge_off_the_axis() {
    let frustum = polygon(&[[0.0, 0.0], [1.0, 0.0], [0.4, 2.0], [0.0, 2.0]]);
    let tube = polygon(&[[0.4, 0.0], [1.0, 0.0], [1.0, 2.0], [0.4, 2.0]]);
    for (edges, expected) in [
        (tube, vec!["cylinder", "cylinder", "plane", "plane"]),
        (semicircle(), vec!["sphere"]),
        (off_axis_circle(), vec!["torus", "torus"]),
        (frustum, vec!["cone", "plane", "plane"]),
        (bowl(1.0, 0.8), vec!["plane", "sphere", "sphere"]),
    ] {
        let body = revolved(edges).unwrap();
        assert_eq!(surface_kinds(&body), expected);
        assert_eq!(body.topology.faces.len(), expected.len());
    }
}

#[test]
fn revolved_tube_classifies_its_wall_and_its_bore() {
    let body = revolved(polygon(&[[0.4, 0.0], [1.0, 0.0], [1.0, 2.0], [0.4, 2.0]])).unwrap();
    assert_eq!(
        classify_point(&body, [0.0, 0.7, 1.0]).unwrap(),
        PointClassification::Inside
    );
    assert_eq!(
        classify_point(&body, [0.0, 0.0, 1.0]).unwrap(),
        PointClassification::Outside
    );
}

fn refusal(edges: Vec<ProfileEdge>) -> GeometryError {
    revolved(edges).expect_err("the profile is refused")
}

#[test]
fn revolve_refuses_an_open_profile() {
    let open = vec![line([1.0, 0.0], [2.0, 0.0]), line([2.0, 0.0], [2.0, 1.0])];
    assert_eq!(
        refusal(open),
        GeometryError::InvalidGeometry("profile edges do not close".into())
    );
}

#[test]
fn revolve_refuses_a_sloped_line_too_flat_for_a_cone() {
    let nearly_flat = polygon(&[[1.0, 0.0], [3.0, 0.0], [3.0, 1.0], [1.0, 1.0 + 2e-8]]);
    assert_eq!(
        refusal(nearly_flat),
        GeometryError::InvalidGeometry(
            "revolved profile line is too close to a constant-z line for a cone".into()
        )
    );
}

#[test]
fn revolve_refuses_a_profile_on_both_sides_of_the_axis() {
    let crossing = polygon(&[[-0.5, 0.0], [1.0, 0.0], [1.0, 1.0], [-0.5, 1.0]]);
    assert_eq!(
        refusal(crossing),
        GeometryError::InvalidGeometry("revolved profile crosses the revolve axis".into())
    );
}

#[test]
fn revolve_refuses_an_arc_whose_circle_crosses_or_touches_the_axis() {
    let chord = |center: [f64; 2], radius: f64| {
        let top = [center[0] + radius * 0.5, radius * 0.75_f64.sqrt()];
        vec![
            arc(center, radius, [-PI / 3.0, 2.0 * PI / 3.0]),
            line(top, [top[0], -top[1]]),
        ]
    };
    for (center, radius) in [([0.5, 0.0], 1.0), ([1.0, 0.0], 1.0)] {
        assert_eq!(
            refusal(chord(center, radius)),
            GeometryError::InvalidGeometry(
                "revolved profile arc circle meets the revolve axis".into()
            )
        );
    }
}

#[test]
fn revolve_refuses_an_arc_through_a_pole() {
    let around = vec![
        arc([0.0, 0.0], 1.0, [0.0, 1.75 * PI]),
        line([0.5_f64.sqrt(), -(0.5_f64.sqrt())], [1.0, 0.0]),
    ];
    assert_eq!(
        refusal(around),
        GeometryError::InvalidGeometry(
            "revolved profile arc passes through a pole of the revolve axis".into()
        )
    );
}

#[test]
fn revolve_refuses_a_profile_touching_the_axis_at_a_point() {
    let pinched = polygon(&[[0.0, 0.0], [1.0, -1.0], [1.0, 1.0]]);
    assert_eq!(
        refusal(pinched),
        GeometryError::InvalidGeometry(
            "revolved profile touches the revolve axis at a point".into()
        )
    );
}

#[test]
fn revolve_refuses_a_profile_meeting_the_axis_along_two_segments() {
    let cavity = polygon(&[
        [0.0, 0.0],
        [1.0, 0.0],
        [1.0, 3.0],
        [0.0, 3.0],
        [0.0, 2.0],
        [0.5, 2.0],
        [0.5, 1.0],
        [0.0, 1.0],
    ]);
    assert_eq!(
        refusal(cavity),
        GeometryError::InvalidGeometry(
            "revolved profile meets the revolve axis along more than one segment".into()
        )
    );
}

#[test]
fn revolve_refuses_sub_resolution_edges() {
    let short = polygon(&[[1.0, 0.0], [2.0, 0.0], [2.0, 1.0], [1.0, 1.0], [1.0, 1e-8]]);
    assert_eq!(
        refusal(short),
        GeometryError::UnresolvedIntersection("profile line is below resolution".into())
    );
    let sliver = vec![
        line([1.0, 0.0], [2.0, 0.0]),
        line([2.0, 0.0], [2.0, 1.0]),
        line([2.0, 1.0], [1.0, 1.0]),
        arc([0.0, 1.0], 1.0, [0.0, -1e-9]),
        line([1.0, 1.0 - 1e-9], [1.0, 0.0]),
    ];
    assert_eq!(
        refusal(sliver),
        GeometryError::InvalidGeometry("profile arc is unresolved".into())
    );
}
