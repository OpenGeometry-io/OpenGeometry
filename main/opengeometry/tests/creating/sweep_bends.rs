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
use std::f64::consts::{FRAC_PI_2, PI};

const DUCT_AREA: f64 = 0.4 * 0.25;
const DUCT_BEND: f64 = 1.5 * 0.4;

fn placed(id: &str, origin: [f64; 3], normal: [f64; 3], x_direction: [f64; 3]) -> CreateOptions {
    CreateOptions {
        plane: Some(Plane {
            origin: Some(origin),
            normal: Some(normal),
            x_direction: Some(x_direction),
        }),
        ..named(id)
    }
}

fn arc_path(radius: f64, sweep_angle: f64) -> Primitive {
    Primitive::Arc {
        radius,
        start_angle: 0.0,
        sweep_angle,
    }
}

fn sweep(caps: Option<bool>) -> CreatingOperation {
    CreatingOperation::Sweep {
        profile: "profile".into(),
        path: "path".into(),
        caps,
    }
}

fn built(
    profile: (Primitive, CreateOptions),
    path: Primitive,
) -> (WorldGraph, Result<String, GraphError>) {
    let mut world = graph();
    world.create_primitive(profile.0, profile.1).unwrap();
    world.create_primitive(path, named("path")).unwrap();
    let result = world
        .create_operation(sweep(None), named("solid"))
        .map(|(id, _)| id);
    (world, result)
}

fn swept(
    profile: (Primitive, CreateOptions),
    path: Primitive,
) -> (WorldGraph, Result<String, ErrorCode>) {
    let (world, result) = built(profile, path);
    (world, result.map_err(|error| error.error_code()))
}

fn duct_elbow(sweep_angle: f64) -> (WorldGraph, Result<String, ErrorCode>) {
    let rectangle = Primitive::Rectangle {
        width: 0.4,
        breadth: 0.25,
    };
    let at_start = placed(
        "profile",
        [DUCT_BEND, 0.0, 0.0],
        [0.0, 0.0, -1.0],
        [1.0, 0.0, 0.0],
    );
    swept((rectangle, at_start), arc_path(DUCT_BEND, sweep_angle))
}

fn pipe_elbow(sweep_angle: f64) -> (WorldGraph, Result<String, ErrorCode>) {
    let at_start = placed(
        "profile",
        [0.3, 0.0, 0.0],
        [0.0, 0.0, -1.0],
        [1.0, 0.0, 0.0],
    );
    swept(
        (Primitive::Circle { radius: 0.1 }, at_start),
        arc_path(0.3, sweep_angle),
    )
}

fn surface_kinds(body: &BrepEnvelope) -> Vec<&'static str> {
    let mut kinds = body
        .topology
        .faces
        .iter()
        .map(|face| match body.geometry.surfaces[face.surface as usize] {
            SurfaceGeometry::Plane { .. } => "plane",
            SurfaceGeometry::Cylinder { .. } => "cylinder",
            SurfaceGeometry::Cone { .. } => "cone",
            SurfaceGeometry::Torus { .. } => "torus",
            SurfaceGeometry::Sphere { .. } => "sphere",
        })
        .collect::<Vec<_>>();
    kinds.sort_unstable();
    kinds
}

fn assert_volume(body: &BrepEnvelope, expected: f64) {
    let measured = volume::estimate(body, 0.001);
    assert!(
        (measured.value - expected).abs() <= measured.error_bound,
        "{} against {expected}",
        measured.value
    );
}

fn exported(world: &WorldGraph) -> Document {
    let options = StepOptions {
        unit: "metre".into(),
        up_axis: "Y".into(),
        ..StepOptions::default()
    };
    let (text, report) = world.export_step(&["solid".into()], &options).unwrap();
    assert_eq!(report.pcurveless_edges, 0);
    let document = Document::parse(&text).unwrap();
    document.check_length_unit("metre").unwrap();
    let report = serde_json::to_value(&report).unwrap();
    check_report_matches(&document, &text, &report).unwrap();
    let mut expected = ExpectedValues::default();
    expected.add_body(&world.brep("solid").unwrap(), 1.0);
    check_values_belong(&document, &expected).unwrap();
    assert_eq!(document.pcurveless_edges().unwrap(), 0);
    document
}

fn trapezoid_elbow() -> (WorldGraph, Result<String, ErrorCode>) {
    let points = [[0.4, -0.1], [0.5, 0.1], [0.7, 0.1], [0.8, -0.1]]
        .map(|[radial, axial]| [radial, axial, 0.0])
        .to_vec();
    let polygon = Primitive::Polyline {
        points,
        closed: true,
    };
    swept((polygon, named("profile")), arc_path(DUCT_BEND, FRAC_PI_2))
}

fn line(to: [f64; 2]) -> ProfileSegment {
    ProfileSegment { to, arc: None }
}

fn arc(to: [f64; 2], center: [f64; 2]) -> ProfileSegment {
    ProfileSegment {
        to,
        arc: Some(SegmentArc {
            center,
            counter_clockwise: true,
        }),
    }
}

fn open_profile(segments: Vec<ProfileSegment>) -> Primitive {
    Primitive::Profile {
        start: [0.0, 0.0],
        segments,
        closed: false,
    }
}

fn duct_at_origin() -> (Primitive, CreateOptions) {
    let rectangle = Primitive::Rectangle {
        width: 0.4,
        breadth: 0.25,
    };
    (
        rectangle,
        placed("profile", [0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
    )
}

fn pipe_at_origin() -> (Primitive, CreateOptions) {
    let at_start = placed("profile", [0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    (Primitive::Circle { radius: 0.1 }, at_start)
}

fn duct_on(path: Primitive) -> (WorldGraph, Result<String, ErrorCode>) {
    swept(duct_at_origin(), path)
}

fn cornered_arc(radius: f64, corner: f64) -> Primitive {
    let (sin, cos) = corner.sin_cos();
    let centre = [2.0 - radius * sin, radius * cos];
    let end = [centre[0] + radius * cos, centre[1] + radius * sin];
    open_profile(vec![line([2.0, 0.0]), arc(end, centre)])
}

fn line_arc_line(radius: f64) -> Primitive {
    open_profile(vec![
        line([2.0, 0.0]),
        arc([2.0 + radius, radius], [2.0, radius]),
        line([2.0 + radius, 2.0 + radius]),
    ])
}

#[test]
fn default_mep_duct_elbow_builds_as_one_arc_segment_and_exports_to_step() {
    let (world, result) = duct_elbow(FRAC_PI_2);
    result.unwrap();
    let body = world.brep("solid").unwrap();
    assert_eq!(body.body_type().unwrap(), BodyType::Solid);
    assert_eq!(
        surface_kinds(&body),
        ["cylinder", "cylinder", "plane", "plane", "plane", "plane"]
    );
    assert_volume(&body, DUCT_AREA * DUCT_BEND * FRAC_PI_2);
    assert_eq!(exported(&world).count("CYLINDRICAL_SURFACE"), 2);
}

#[test]
fn round_pipe_elbow_gives_torus_faces() {
    let (world, result) = pipe_elbow(FRAC_PI_2);
    result.unwrap();
    let body = world.brep("solid").unwrap();
    assert_eq!(surface_kinds(&body), ["plane", "plane", "torus", "torus"]);
    assert_volume(&body, PI * 0.01 * 0.3 * FRAC_PI_2);
    assert_eq!(exported(&world).count("TOROIDAL_SURFACE"), 2);
}

#[test]
fn trapezoid_elbow_gives_cone_faces_that_export_to_step() {
    let (world, result) = trapezoid_elbow();
    result.unwrap();
    let body = world.brep("solid").unwrap();
    assert_eq!(
        surface_kinds(&body),
        ["cone", "cone", "plane", "plane", "plane", "plane"]
    );
    assert_volume(&body, 0.06 * DUCT_BEND * FRAC_PI_2);
    assert_eq!(exported(&world).count("CONICAL_SURFACE"), 2);
}

#[test]
fn line_arc_line_duct_has_no_mitre_at_its_tangent_joints() {
    let (world, result) = duct_on(line_arc_line(DUCT_BEND));
    result.unwrap();
    let body = world.brep("solid").unwrap();
    assert_eq!(body.topology.faces.len(), 14);
    assert_eq!(
        surface_kinds(&body)
            .iter()
            .filter(|kind| **kind == "cylinder")
            .count(),
        2
    );
    assert_volume(&body, DUCT_AREA * (4.0 + DUCT_BEND * FRAC_PI_2));
    exported(&world);
}

#[test]
fn arc_meeting_a_line_at_a_corner_is_invalid_parameter() {
    let path = open_profile(vec![line([2.0, 0.0]), arc([2.6, -0.6], [2.6, 0.0])]);
    let error = built(duct_at_origin(), path).1.unwrap_err();
    assert_eq!(error.error_code(), ErrorCode::InvalidParameter);
    assert!(
        error.message().contains("path has a corner at an arc"),
        "{}",
        error.message()
    );
}

#[test]
fn corner_of_a_micro_radian_at_an_arc_is_refused_and_one_of_a_tenth_of_a_nano_radian_builds() {
    for (profile, radius) in [(duct_at_origin(), DUCT_BEND), (pipe_at_origin(), 0.3)] {
        let error = built(profile.clone(), cornered_arc(radius, 1e-6))
            .1
            .unwrap_err();
        assert!(
            error.message().contains("path has a corner at an arc"),
            "{}",
            error.message()
        );
        let (world, result) = built(profile, cornered_arc(radius, 1e-10));
        result.unwrap();
        world.brep("solid").unwrap().validate().unwrap();
    }
}

#[test]
fn round_pipe_line_arc_line_has_tangent_rings_after_its_lines() {
    let (world, result) = swept(pipe_at_origin(), line_arc_line(0.3));
    result.unwrap();
    let body = world.brep("solid").unwrap();
    assert_eq!(
        surface_kinds(&body),
        ["cylinder", "cylinder", "cylinder", "cylinder", "plane", "plane", "torus", "torus"]
    );
    assert_volume(&body, PI * 0.01 * (4.0 + 0.3 * FRAC_PI_2));
    assert_eq!(exported(&world).count("TOROIDAL_SURFACE"), 2);
}

#[test]
fn circle_sweep_round_a_corner_of_ten_nano_radians_validates() {
    let path = Primitive::Polyline {
        points: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 1e-8, 0.0]],
        closed: false,
    };
    let (world, result) = swept(pipe_at_origin(), path);
    result.unwrap();
    world.brep("solid").unwrap().validate().unwrap();
}

#[test]
fn open_pipe_elbow_is_a_sheet_of_two_torus_faces() {
    let (mut world, _) = pipe_elbow(FRAC_PI_2);
    world
        .create_operation(sweep(Some(false)), named("sheet"))
        .unwrap();
    let body = world.brep("sheet").unwrap();
    assert_eq!(body.body_type().unwrap(), BodyType::Sheet);
    assert_eq!(surface_kinds(&body), ["torus", "torus"]);
}

#[test]
fn arc_above_half_a_turn_splits_into_equal_bends() {
    let (world, result) = pipe_elbow(1.5 * PI);
    result.unwrap();
    let body = world.brep("solid").unwrap();
    assert_eq!(
        surface_kinds(&body),
        ["plane", "plane", "torus", "torus", "torus", "torus"]
    );
    let json: serde_json::Value = serde_json::from_str(&body.to_json().unwrap()).unwrap();
    let spans = json["topology"]["faces"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|face| face["key"].as_str().unwrap().starts_with("seg-"))
        .map(|face| {
            let key = face["key"]
                .as_str()
                .unwrap()
                .split(':')
                .next()
                .unwrap()
                .to_string();
            let u = &face["trim"]["uv_bounds"][0];
            (key, u["hi"].as_f64().unwrap() - u["lo"].as_f64().unwrap())
        })
        .collect::<Vec<_>>();
    assert_eq!(spans.len(), 4);
    assert!(
        spans
            .iter()
            .all(|(_, width)| (width - 0.75 * PI).abs() < 1e-12),
        "{spans:?}"
    );
    assert!(
        spans.iter().any(|(key, _)| key == "seg-0") && spans.iter().any(|(key, _)| key == "seg-1")
    );
    assert_volume(&body, PI * 0.01 * 0.3 * 1.5 * PI);
}

#[test]
fn bend_tighter_than_the_profile_reach_self_intersects() {
    let at_start = placed(
        "profile",
        [0.1, 0.0, 0.0],
        [0.0, 0.0, -1.0],
        [1.0, 0.0, 0.0],
    );
    let (_, result) = swept(
        (Primitive::Circle { radius: 0.1 }, at_start),
        arc_path(0.1, FRAC_PI_2),
    );
    assert_eq!(result, Err(ErrorCode::SweepSelfIntersection));
}

#[test]
fn open_duct_elbow_is_a_sheet() {
    let (mut world, _) = duct_elbow(FRAC_PI_2);
    world
        .create_operation(sweep(Some(false)), named("sheet"))
        .unwrap();
    let body = world.brep("sheet").unwrap();
    assert_eq!(body.body_type().unwrap(), BodyType::Sheet);
    assert_eq!(body.topology.faces.len(), 4);
}

fn ring_elbow(angle: f64, chords: usize) -> Result<String, ErrorCode> {
    let chords = (0..=chords)
        .map(|step| {
            let turned = step as f64 * angle / chords as f64;
            [0.3 * turned.cos(), 0.0, -0.3 * turned.sin()]
        })
        .collect::<Vec<_>>();
    let along = [chords[1][0] - 0.3, 0.0, chords[1][2]];
    let length = along[0].hypot(along[2]);
    let across = [-along[2] / length, 0.0, along[0] / length];
    let ring = (0..24)
        .map(|index| {
            let angle = index as f64 * PI / 12.0;
            let (sin, cos) = angle.sin_cos();
            [
                0.3 + 0.1 * cos * across[0],
                0.1 * sin,
                0.1 * cos * across[2],
            ]
        })
        .collect();
    let polygon = Primitive::Polyline {
        points: ring,
        closed: true,
    };
    let path = Primitive::Polyline {
        points: chords,
        closed: false,
    };
    swept((polygon, named("profile")), path).1
}

#[test]
fn tessellated_24_point_ring_elbows_still_self_intersect() {
    assert_eq!(
        ring_elbow(FRAC_PI_2, 4),
        Err(ErrorCode::SweepSelfIntersection)
    );
    assert_eq!(
        ring_elbow(PI / 3.0, 3),
        Err(ErrorCode::SweepSelfIntersection)
    );
}
