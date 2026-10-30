use crate::boolean_case::result_body;
use crate::fixtures::fine;
use crate::graph_case::{
    copy_options, create_options, cuboid as graph_cuboid, graph_case, inspect, operation, polyline,
    primitive, rectangle,
};
use crate::graph_record::{changes, created};
use crate::json::brep_sha256;
use crate::kernel::{
    primitives, shell_brep, BrepEnvelope, ChangeSet, CreatingOperation, EditScope, Frame3,
    GeometryError, GraphError, ProfileEdge, WorldGraph,
};
use crate::planar_shapes::profile_shell;
use crate::profiles::{arc_at, line, rounded_outer};
use crate::record::{Failure, Record};
use crate::runner::Case;
use opengeometry::world_graph::ErrorCode;
use opengeometry_test_support::volume::estimate;
use std::f64::consts::{FRAC_PI_2, PI};

type Build = fn() -> Result<BrepEnvelope, GeometryError>;

const PAN_LENGTH: f64 = 6.0;
const PAN_RADIUS: f64 = 2.0;
const PAN_HEIGHT: f64 = 1.0;
const PAN_WALL: f64 = 0.1;
const PAN_DEFLECTION: f64 = 1e-4;

fn cuboid() -> Result<BrepEnvelope, GeometryError> {
    primitives::cuboid("box".into(), Frame3::IDENTITY, [2.0; 3], fine())
}

fn cylinder() -> Result<BrepEnvelope, GeometryError> {
    primitives::cylinder("cylinder".into(), Frame3::IDENTITY, 1.0, 2.0, fine())
}

fn sphere() -> Result<BrepEnvelope, GeometryError> {
    primitives::sphere("sphere".into(), Frame3::IDENTITY, 1.0, fine())
}

fn torus() -> Result<BrepEnvelope, GeometryError> {
    primitives::torus("torus".into(), Frame3::IDENTITY, 3.0, 1.0, fine())
}

fn cone() -> Result<BrepEnvelope, GeometryError> {
    primitives::cone("cone".into(), Frame3::IDENTITY, 2.0, 3.0, fine())
}

fn frustum() -> Result<BrepEnvelope, GeometryError> {
    primitives::frustum("frustum".into(), Frame3::IDENTITY, 2.0, 1.0, 3.0, fine())
}

fn arc_edged() -> Result<BrepEnvelope, GeometryError> {
    primitives::arc_edged_extrusion(
        "arc-edged".into(),
        Frame3::IDENTITY,
        rounded_outer(),
        2.0,
        fine(),
    )
}

fn pan_outline() -> Vec<ProfileEdge> {
    let width = 2.0 * PAN_RADIUS;
    vec![
        line([0.0, 0.0], [PAN_LENGTH, 0.0]),
        arc_at([PAN_LENGTH, PAN_RADIUS], PAN_RADIUS, -FRAC_PI_2, PI),
        line([PAN_LENGTH, width], [0.0, width]),
        line([0.0, width], [0.0, 0.0]),
    ]
}

fn d_pan() -> Result<BrepEnvelope, GeometryError> {
    primitives::arc_edged_extrusion(
        "d-pan".into(),
        Frame3::IDENTITY,
        pan_outline(),
        PAN_HEIGHT,
        fine(),
    )
}

fn pan_area(inset: f64) -> f64 {
    (PAN_LENGTH - inset) * (2.0 * PAN_RADIUS - 2.0 * inset)
        + PI * (PAN_RADIUS - inset).powi(2) / 2.0
}

fn pan_volume() -> f64 {
    pan_area(0.0) * PAN_HEIGHT - pan_area(PAN_WALL) * (PAN_HEIGHT - 2.0 * PAN_WALL)
}

const SHELLS: [(&str, Build, f64, bool); 11] = [
    ("box", cuboid, 0.2, true),
    ("cylinder", cylinder, 0.2, true),
    ("sphere", sphere, 0.2, true),
    ("torus", torus, 0.2, true),
    ("cone", cone, 0.2, true),
    ("frustum", frustum, 0.2, true),
    ("profile", profile_shell, 0.2, true),
    ("sphere-collapsed", sphere, 1.0, false),
    ("box-below-resolution", cuboid, 1e-9, false),
    ("profile-collapsed", profile_shell, 0.6, false),
    ("arc-edged", arc_edged, 0.2, true),
];

fn shell(name: &str, build: Build, thickness: f64, builds: bool) -> Case {
    Case::new(format!("shell.{name}"), move |record| {
        let input = build()?;
        record.section("shell");
        record.field("input", &input.id);
        record.field("input.sha256", brep_sha256(&input));
        record.debug("thickness", thickness);
        let result = shell_brep(&input, thickness, format!("{}-shell", input.id));
        if result.is_ok() != builds {
            record.break_case(if builds { "did not build" } else { "built" });
        }
        result_body(record, result);
        Ok(())
    })
}

fn pan_case() -> Case {
    Case::new("shell.d-pan", |record| {
        let input = d_pan()?;
        record.section("shell");
        record.field("input.sha256", brep_sha256(&input));
        record.debug("thickness", PAN_WALL);
        let result = shell_brep(&input, PAN_WALL, "d-pan-shell".into())?;
        let volume = estimate(&result.brep, PAN_DEFLECTION);
        let body = serde_json::to_value(&result.brep).unwrap_or_default();
        if body["solids"].as_array().map(Vec::len) != Some(1)
            || body["solids"][0]["cavity_shells"].as_array().map(Vec::len) != Some(1)
        {
            record.break_case("is not one solid with one cavity");
        }
        if (volume.value - pan_volume()).abs() > volume.error_bound {
            record.break_case(&format!(
                "volume {} is not {} within {}",
                volume.value,
                pan_volume(),
                volume.error_bound
            ));
        }
        result_body(record, Ok(result));
        Ok(())
    })
}

fn expect_code(
    record: &mut Record,
    label: &str,
    result: Result<ChangeSet, GraphError>,
    code: ErrorCode,
) {
    let actual = result.as_ref().err().map(|error| error.error_code());
    changes(record, label, result);
    if actual != Some(code) {
        record.break_case(&format!("{label} gave {actual:?}, not {code:?}"));
    }
}

fn check_mappings(record: &mut Record, graph: &WorldGraph, og_id: &str) -> Result<(), Failure> {
    let report = graph.report(og_id)?.cloned().unwrap_or_default();
    let mappings = report["report"]["face_mappings"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let pairs = mappings
        .iter()
        .filter(|mapping| mapping["result_faces"].as_array().map(Vec::len) == Some(2))
        .count();
    if mappings.len() != 6 || pairs != 6 {
        record.break_case("does not map six faces to one outer and one inner face each");
    }
    if report["revision"] != serde_json::json!(graph.brep(og_id)?.revision) {
        record.break_case("report revision is not the shape revision");
    }
    Ok(())
}

fn graph_cuboid_shell(graph: &mut WorldGraph, record: &mut Record) -> Result<(), Failure> {
    primitive(
        record,
        graph,
        graph_cuboid(2.0, 2.0, 2.0),
        create_options("pan"),
    );
    let before = graph.brep("pan")?.revision;
    let result = graph.shell("pan", 0.2, EditScope::Node);
    if !changes(record, "shell pan 0.2", result) {
        record.break_case("cuboid shell did not build");
    }
    if graph.brep("pan")?.revision != before + 1 {
        record.break_case("revision did not move by one");
    }
    check_mappings(record, graph, "pan")?;
    inspect(record, graph, "pan")
}

fn add_sheet(record: &mut Record, graph: &mut WorldGraph) {
    let path = [[0.0, 0.0, 0.0], [0.0, 2.0, 0.0]];
    primitive(
        record,
        graph,
        rectangle(1.0, 1.0),
        create_options("sheet-profile"),
    );
    primitive(
        record,
        graph,
        polyline(&path, false),
        create_options("sheet-path"),
    );
    let sweep = CreatingOperation::Sweep {
        profile: "sheet-profile".into(),
        path: "sheet-path".into(),
        caps: Some(false),
    };
    operation(record, graph, sweep, create_options("sheet"));
}

fn graph_shell_refusals(graph: &mut WorldGraph, record: &mut Record) -> Result<(), Failure> {
    primitive(
        record,
        graph,
        graph_cuboid(1.0, 1.0, 1.0),
        create_options("box"),
    );
    for (thickness, code) in [
        (f64::NAN, ErrorCode::InvalidParameter),
        (f64::INFINITY, ErrorCode::InvalidParameter),
        (0.0, ErrorCode::InvalidParameter),
        (1e-9, ErrorCode::InvalidParameter),
        (0.6, ErrorCode::UnresolvedIntersection),
    ] {
        let label = format!("shell box {thickness}");
        expect_code(
            record,
            &label,
            graph.shell("box", thickness, EditScope::Node),
            code,
        );
    }
    let copy = graph.instance("box", copy_options("box-copy", None));
    created(record, "instance box-copy", copy);
    let shared = graph.shell("box", 0.1, EditScope::Node);
    expect_code(record, "shell shared box", shared, ErrorCode::SharedShape);
    if !changes(
        record,
        "shell all instances",
        graph.shell("box", 0.1, EditScope::AllInstances),
    ) {
        record.break_case("shell of all instances did not build");
    }
    add_sheet(record, graph);
    let sheet = graph.shell("sheet", 0.1, EditScope::Node);
    expect_code(record, "shell open sheet", sheet, ErrorCode::CoverageGap);
    let wire = graph.shell("sheet-profile", 0.1, EditScope::Node);
    expect_code(record, "shell wire", wire, ErrorCode::InvalidOperand);
    inspect(record, graph, "box")
}

pub(crate) fn cases() -> Vec<Case> {
    SHELLS
        .iter()
        .map(|(name, build, thickness, builds)| shell(name, *build, *thickness, *builds))
        .chain([
            pan_case(),
            graph_case("shell.graph.cuboid", graph_cuboid_shell),
            graph_case("shell.graph.refusals", graph_shell_refusals),
        ])
        .collect()
}

fn d_outline(size: f64, origin: [f64; 2]) -> Vec<ProfileEdge> {
    let at = |x: f64, y: f64| [origin[0] + size * x, origin[1] + size * y];
    vec![
        line(at(-2.0, -2.0), at(2.0, -2.0)),
        arc_at(at(2.0, 0.0), 2.0 * size, -FRAC_PI_2, PI),
        line(at(2.0, 2.0), at(-2.0, 2.0)),
        line(at(-2.0, 2.0), at(-2.0, -2.0)),
    ]
}

fn d_area(inset: f64) -> f64 {
    (4.0 - inset) * (4.0 - 2.0 * inset) + PI * (2.0 - inset).powi(2) / 2.0
}

fn inner_faces(input: &BrepEnvelope, thickness: f64) -> Vec<Vec<u64>> {
    let result = shell_brep(input, thickness, format!("{}-shell", input.id)).unwrap();
    let body = serde_json::to_value(&result.brep).unwrap();
    let cavity = &body["solids"][0]["cavity_shells"][0];
    let shells = body["topology"]["shells"].as_array().unwrap();
    let shell = shells.iter().find(|shell| &shell["id"] == cavity).unwrap();
    let inner = shell["faces"].as_array().unwrap();
    let report = serde_json::to_value(&result.report).unwrap();
    report["face_mappings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|mapping| {
            mapping["result_faces"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|face| inner.contains(face))
                .map(|face| face.as_u64().unwrap())
                .collect()
        })
        .collect()
}

#[test]
fn a_hole_near_the_wall_maps_each_hole_half_to_the_grown_hole_wall() {
    let square = vec![
        line([0.0, 0.0], [4.0, 0.0]),
        line([4.0, 0.0], [4.0, 4.0]),
        line([4.0, 4.0], [0.0, 4.0]),
        line([0.0, 4.0], [0.0, 0.0]),
    ];
    let hole = vec![
        arc_at([2.0, 0.8], 0.5, 0.0, -PI),
        arc_at([2.0, 0.8], 0.5, -PI, -PI),
    ];
    let input = primitives::arc_edged_extrusion_with_holes(
        "hole-near-wall".into(),
        Frame3::IDENTITY,
        square,
        vec![hole],
        1.0,
        fine(),
    )
    .unwrap();
    assert_eq!(
        inner_faces(&input, 0.2),
        [
            vec![8],
            vec![9],
            vec![10, 12],
            vec![13],
            vec![14],
            vec![15],
            vec![11],
            vec![11]
        ]
    );
}

#[test]
fn a_d_with_split_arc_and_split_line_maps_every_piece_to_an_inner_wall() {
    let outline = vec![
        line([-2.0, -2.0], [2.0, -2.0]),
        arc_at([2.0, 0.0], 2.0, -FRAC_PI_2, 0.15),
        arc_at([2.0, 0.0], 2.0, -FRAC_PI_2 + 0.15, PI - 0.15),
        line([2.0, 2.0], [1.75, 2.0]),
        line([1.75, 2.0], [-2.0, 2.0]),
        line([-2.0, 2.0], [-2.0, -2.0]),
    ];
    let input =
        primitives::arc_edged_extrusion("split-d".into(), Frame3::IDENTITY, outline, 1.0, fine())
            .unwrap();
    assert_eq!(
        inner_faces(&input, 0.2),
        [
            vec![8],
            vec![9],
            vec![10],
            vec![11],
            vec![11],
            vec![12],
            vec![12],
            vec![13]
        ]
    );
}

#[test]
fn a_small_d_far_from_the_origin_shells_to_its_closed_form_volume() {
    let (size, wall, height) = (0.025, 0.01, 0.1);
    let input = primitives::arc_edged_extrusion(
        "far-d".into(),
        Frame3::IDENTITY,
        d_outline(size, [1e5, 1e5]),
        height,
        fine(),
    )
    .unwrap();
    let result = shell_brep(&input, wall, "far-d-shell".into()).unwrap();
    let expected =
        size * size * (d_area(0.0) * height - d_area(wall / size) * (height - 2.0 * wall));
    let volume = estimate(&result.brep, 1e-6);
    let body = serde_json::to_value(&result.brep).unwrap();
    assert_eq!(
        body["solids"][0]["cavity_shells"].as_array().map(Vec::len),
        Some(1)
    );
    assert!(
        (volume.value - expected).abs() <= volume.error_bound,
        "volume {} is not {expected} within {}",
        volume.value,
        volume.error_bound
    );
}

#[test]
fn an_inset_pinched_below_resolution_at_a_waist_is_unresolved() {
    let centre = 0.85_f64;
    let half = (1.0 - centre * centre).sqrt();
    let open = half.atan2(centre);
    let sweep = 2.0 * (PI - open);
    let outline = vec![
        arc_at([centre, 0.0], 1.0, open - PI, sweep),
        arc_at([-centre, 0.0], 1.0, open, sweep),
    ];
    let input =
        primitives::arc_edged_extrusion("waist".into(), Frame3::IDENTITY, outline, 3.0, fine())
            .unwrap();
    for offset in [-1e-10, -1e-9, 0.0] {
        let result = shell_brep(&input, half + offset, "waist-shell".into());
        assert!(
            matches!(result, Err(GeometryError::UnresolvedIntersection(_))),
            "thickness {} gave {:?}",
            half + offset,
            result.map(|result| result.brep.topology.faces.len())
        );
    }
}
