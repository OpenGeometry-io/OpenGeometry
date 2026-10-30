use opengeometry::brep::{BodyType, BrepEnvelope, SurfaceGeometry};
use opengeometry::primitives::{ProfileSegment, SegmentArc};
use opengeometry::world_graph::{
    CreateOptions, CreatingOperation, ErrorCode, GraphError, Plane, Primitive, StepOptions,
    WorldGraph,
};
use opengeometry_test_support::part21::{check_report_matches, check_values_belong};
use opengeometry_test_support::part21::{Document, ExpectedValues};
use opengeometry_test_support::volume;
use opengeometry_test_support::world_graph::{graph, named};
use std::f64::consts::PI;

const RECTANGLE: [[f64; 3]; 4] = [
    [0.0, 0.0, 0.0],
    [4.0, 0.0, 0.0],
    [4.0, 3.0, 0.0],
    [0.0, 3.0, 0.0],
];

fn placed(origin: [f64; 3], normal: [f64; 3], x_direction: [f64; 3]) -> CreateOptions {
    CreateOptions {
        plane: Some(Plane {
            origin: Some(origin),
            normal: Some(normal),
            x_direction: Some(x_direction),
        }),
        ..named("profile")
    }
}

fn square(options: CreateOptions) -> (Primitive, CreateOptions) {
    let square = Primitive::Rectangle {
        width: 0.2,
        breadth: 0.2,
    };
    (square, options)
}

fn closed_polyline(points: &[[f64; 3]]) -> Primitive {
    Primitive::Polyline {
        points: points.to_vec(),
        closed: true,
    }
}

fn swept(
    profile: (Primitive, CreateOptions),
    path: Primitive,
    caps: Option<bool>,
) -> (WorldGraph, Result<String, GraphError>) {
    let mut world = graph();
    world.create_primitive(profile.0, profile.1).unwrap();
    world.create_primitive(path, named("path")).unwrap();
    let operation = CreatingOperation::Sweep {
        profile: "profile".into(),
        path: "path".into(),
        caps,
    };
    let result = world
        .create_operation(operation, named("solid"))
        .map(|(id, _)| id);
    (world, result)
}

fn surface_counts(body: &BrepEnvelope) -> [usize; 4] {
    let mut counts = [0; 4];
    for face in &body.topology.faces {
        let index = match body.geometry.surfaces[face.surface as usize] {
            SurfaceGeometry::Plane { .. } => 0,
            SurfaceGeometry::Cylinder { .. } => 1,
            SurfaceGeometry::Torus { .. } => 2,
            _ => 3,
        };
        counts[index] += 1;
    }
    counts
}

fn face_keys(body: &BrepEnvelope) -> Vec<String> {
    let json: serde_json::Value = serde_json::from_str(&body.to_json().unwrap()).unwrap();
    json["topology"]["faces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|face| face["key"].as_str().unwrap().to_string())
        .collect()
}

fn assert_closed_solid(world: &WorldGraph, faces: usize, expected: f64) -> BrepEnvelope {
    let body = (*world.brep("solid").unwrap()).clone();
    body.validate().unwrap();
    assert_eq!(body.body_type().unwrap(), BodyType::Solid);
    assert_eq!(body.topology.faces.len(), faces);
    assert!(face_keys(&body).iter().all(|key| !key.starts_with("cap-")));
    let measured = volume::estimate(&body, 0.001);
    assert!(
        (measured.value - expected).abs() <= measured.error_bound,
        "{} against {expected}",
        measured.value
    );
    body
}

fn exported(world: &WorldGraph) -> Document {
    let options = StepOptions {
        unit: "metre".into(),
        up_axis: "Y".into(),
        ..StepOptions::default()
    };
    let (text, report) = world.export_step(&["solid".into()], &options).unwrap();
    let document = Document::parse(&text).unwrap();
    let report = serde_json::to_value(&report).unwrap();
    check_report_matches(&document, &text, &report).unwrap();
    let mut expected = ExpectedValues::default();
    expected.add_body(&world.brep("solid").unwrap(), 1.0);
    check_values_belong(&document, &expected).unwrap();
    document
}

fn rounded_rectangle(width: f64, height: f64, radius: f64) -> Primitive {
    let line = |to: [f64; 2]| ProfileSegment { to, arc: None };
    let arc = |to: [f64; 2], center: [f64; 2]| ProfileSegment {
        to,
        arc: Some(SegmentArc {
            center,
            counter_clockwise: true,
        }),
    };
    Primitive::Profile {
        start: [radius, 0.0],
        segments: vec![
            line([width - radius, 0.0]),
            arc([width, radius], [width - radius, radius]),
            line([width, height - radius]),
            arc([width - radius, height], [width - radius, height - radius]),
            line([radius, height]),
            arc([0.0, height - radius], [radius, height - radius]),
            line([0.0, radius]),
            arc([radius, 0.0], [radius, radius]),
        ],
        closed: true,
    }
}

#[test]
fn closed_rectangular_path_builds_a_capless_solid_that_exports_to_step() {
    let profile = square(placed([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]));
    let (world, result) = swept(profile, closed_polyline(&RECTANGLE), None);
    result.unwrap();
    let body = assert_closed_solid(&world, 16, 0.04 * 14.0);
    assert_eq!(surface_counts(&body), [16, 0, 0, 0]);
    assert_eq!(exported(&world).count("PLANE"), 16);
}

#[test]
fn rounded_rectangle_path_builds_with_a_bend_at_each_corner() {
    let rectangle = Primitive::Rectangle {
        width: 0.4,
        breadth: 0.25,
    };
    let profile = (
        rectangle,
        placed([0.6, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
    );
    let (world, result) = swept(profile, rounded_rectangle(4.0, 3.0, 0.6), None);
    result.unwrap();
    let length = 2.0 * (4.0 - 1.2) + 2.0 * (3.0 - 1.2) + 2.0 * PI * 0.6;
    let body = assert_closed_solid(&world, 32, 0.1 * length);
    assert_eq!(surface_counts(&body), [24, 8, 0, 0]);
    exported(&world);
}

#[test]
fn full_circle_path_with_a_circle_profile_is_a_torus_ring_of_two_bends() {
    let disc = Primitive::Circle { radius: 0.2 };
    let profile = (
        disc,
        placed([1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [1.0, 0.0, 0.0]),
    );
    let (world, result) = swept(profile, Primitive::Circle { radius: 1.0 }, None);
    result.unwrap();
    let body = assert_closed_solid(&world, 4, 2.0 * PI * PI * 0.04);
    assert_eq!(surface_counts(&body), [0, 0, 4, 0]);
    exported(&world);
}

#[test]
fn twisting_closed_path_is_invalid_parameter_for_a_square_and_a_circle() {
    let path = [
        [0.0, 0.0, 0.0],
        [4.0, 0.0, 0.0],
        [4.0, 4.0, 0.0],
        [4.0, 4.0, 4.0],
    ];
    let options = || placed([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    for profile in [square(options()), disc(options())] {
        let error = swept(profile, closed_polyline(&path), None).1.unwrap_err();
        assert_eq!(error.error_code(), ErrorCode::InvalidParameter);
        assert!(
            error
                .message()
                .contains("closed path does not return the profile to its start"),
            "{}",
            error.message()
        );
    }
}

#[test]
fn open_ends_asked_for_on_a_closed_path_are_invalid_parameter() {
    let profile = square(placed([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]));
    let mut world = graph();
    world.create_primitive(profile.0, profile.1).unwrap();
    world
        .create_primitive(closed_polyline(&RECTANGLE), named("path"))
        .unwrap();
    let before = world.node_count();
    let operation = CreatingOperation::Sweep {
        profile: "profile".into(),
        path: "path".into(),
        caps: Some(false),
    };
    let error = world
        .create_operation(operation, CreateOptions::default())
        .unwrap_err();
    assert_eq!(error.error_code(), ErrorCode::InvalidParameter);
    assert!(
        error.message().contains("closed path has no ends"),
        "{}",
        error.message()
    );
    assert_eq!(world.node_count(), before);
}

fn disc(options: CreateOptions) -> (Primitive, CreateOptions) {
    (Primitive::Circle { radius: 0.1 }, options)
}

#[test]
fn circle_profile_on_a_closed_rectangle_mitres_its_first_ring_and_exports_to_step() {
    let profile = disc(placed([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]));
    let (world, result) = swept(profile, closed_polyline(&RECTANGLE), None);
    result.unwrap();
    let body = assert_closed_solid(&world, 8, PI * 0.01 * 14.0);
    assert_eq!(surface_counts(&body), [0, 8, 0, 0]);
    assert!(body.to_json().unwrap().contains("ProjectedCurve"));
    exported(&world);
}

#[test]
fn circle_profile_on_a_closed_path_that_starts_on_a_straight_edge_builds() {
    let path = [
        [2.0, 0.0, 0.0],
        [4.0, 0.0, 0.0],
        [4.0, 3.0, 0.0],
        [0.0, 3.0, 0.0],
        [0.0, 0.0, 0.0],
    ];
    for profile in [
        disc(placed([2.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0])),
        square(placed([2.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0])),
    ] {
        let (world, result) = swept(profile, closed_polyline(&path), None);
        result.unwrap();
        world.brep("solid").unwrap().validate().unwrap();
    }
    let profile = disc(placed([2.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]));
    let (world, _) = swept(profile, closed_polyline(&path), None);
    assert_closed_solid(&world, 10, PI * 0.01 * 14.0);
}

#[test]
fn closed_path_with_a_corner_where_it_closes_onto_an_arc_is_invalid_parameter() {
    let line = |to: [f64; 2]| ProfileSegment { to, arc: None };
    let path = Primitive::Profile {
        start: [0.0, 0.0],
        segments: vec![
            ProfileSegment {
                to: [2.0, 0.0],
                arc: Some(SegmentArc {
                    center: [1.0, 0.0],
                    counter_clockwise: false,
                }),
            },
            line([2.0, -2.0]),
            line([-1.0, -2.0]),
            line([0.0, 0.0]),
        ],
        closed: true,
    };
    let rectangle = Primitive::Rectangle {
        width: 0.4,
        breadth: 0.25,
    };
    let profile = (
        rectangle,
        placed([0.0; 3], [0.0, 0.0, -1.0], [1.0, 0.0, 0.0]),
    );
    let error = swept(profile, path, None).1.unwrap_err();
    assert_eq!(error.error_code(), ErrorCode::InvalidParameter);
    assert!(
        error.message().contains("path has a corner at an arc"),
        "{}",
        error.message()
    );
}
