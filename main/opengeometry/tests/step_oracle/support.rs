use opengeometry::brep::{placed, BrepEnvelope, Frame3, Similarity3};
use opengeometry::exchange::StepExportReport;
use opengeometry::world_graph::{Primitive, StepOptions, Transform, WorldGraph};
use opengeometry_test_support::part21::{
    check_report_matches, check_single_report_matches, check_values_belong, Document,
    ExpectedValues, SolidEntry,
};
use opengeometry_test_support::world_graph::named;
use serde_json::Value;
use std::{fs, path::Path};

pub(super) struct MatchedExport {
    pub(super) text: String,
    pub(super) report: Value,
    pub(super) document: Document,
}

pub(super) fn fixture_text(relative: &str) -> Option<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/cases")
        .join(relative);
    path.exists()
        .then(|| fs::read_to_string(&path).unwrap_or_else(|error| panic!("{relative}: {error}")))
}

pub(super) fn fixture_names(directory: &str) -> Vec<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/cases")
        .join(directory);
    let mut names = fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    names.sort();
    names
}

pub(super) fn unit_scale(unit: &str) -> f64 {
    if unit == "millimetre" {
        1000.0
    } else {
        1.0
    }
}

fn export_placement(world: &WorldGraph, og_id: &str, up_axis: &str) -> Similarity3 {
    let mut conversion = Similarity3::IDENTITY;
    if up_axis == "Z" {
        conversion.frame = Frame3 {
            origin: [0.0; 3],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 0.0, 1.0],
            z: [0.0, -1.0, 0.0],
        };
    }
    conversion.compose(&world.world_placement(og_id).unwrap())
}

pub(super) fn placed_export_bodies(
    world: &WorldGraph,
    report: &StepExportReport,
    options: &StepOptions,
) -> Vec<BrepEnvelope> {
    report
        .bodies
        .iter()
        .map(|body| {
            let placement = export_placement(world, &body.og_id, &options.up_axis);
            let shape = world.brep(&body.og_id).unwrap();
            let mut brep = if identity_bits(placement) {
                (*shape).clone()
            } else {
                placed(&shape, placement.frame, similarity_scale(placement)).unwrap()
            };
            brep.id = body.og_id.clone();
            brep
        })
        .collect()
}

fn identity_bits(placement: Similarity3) -> bool {
    let entries = |similarity: Similarity3| {
        let frame = similarity.frame;
        [frame.origin, frame.x, frame.y, frame.z]
            .into_iter()
            .flatten()
            .chain([similarity_scale(similarity)])
            .map(f64::to_bits)
            .collect::<Vec<_>>()
    };
    entries(placement) == entries(Similarity3::IDENTITY)
}

fn similarity_scale(placement: Similarity3) -> f64 {
    serde_json::to_value(placement).unwrap()["scale"]
        .as_f64()
        .unwrap()
}

pub(super) fn parse_and_match(
    text: &str,
    report: &Value,
    unit: &str,
    bodies: &[BrepEnvelope],
) -> Document {
    let document = Document::parse(text).unwrap_or_else(|error| panic!("{error}"));
    document
        .check_length_unit(unit)
        .unwrap_or_else(|error| panic!("{error}"));
    if report.get("length_unit").is_some() {
        check_single_report_matches(&document, report)
    } else {
        check_report_matches(&document, text, report)
    }
    .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(document.solids().unwrap(), solid_entries(bodies));
    let mut expected = ExpectedValues::default();
    for body in bodies {
        expected.add_body(body, unit_scale(unit));
    }
    check_values_belong(&document, &expected).unwrap_or_else(|error| panic!("{error}"));
    document
}

fn solid_entries(bodies: &[BrepEnvelope]) -> Vec<SolidEntry> {
    let mut entries = Vec::new();
    for body in bodies {
        let json: Value = serde_json::from_str(&body.to_json().unwrap()).unwrap();
        for solid in json["solids"].as_array().unwrap() {
            let voids = solid["cavity_shells"].as_array().unwrap().len();
            entries.push(SolidEntry {
                voids,
                with_voids: voids > 0,
            });
        }
    }
    entries
}

pub(super) fn export_matched(
    world: &WorldGraph,
    nodes: &[&str],
    options: &StepOptions,
) -> MatchedExport {
    let nodes = nodes
        .iter()
        .map(|node| node.to_string())
        .collect::<Vec<_>>();
    let (text, report) = world.export_step(&nodes, options).unwrap();
    let bodies = placed_export_bodies(world, &report, options);
    let report = serde_json::to_value(&report).unwrap();
    let document = parse_and_match(&text, &report, &options.unit, &bodies);
    MatchedExport {
        text,
        report,
        document,
    }
}

pub(super) fn add_cuboid(world: &mut WorldGraph, og_id: &str, size: [f64; 3], offset: [f64; 3]) {
    world
        .create_primitive(
            Primitive::Cuboid {
                width: size[0],
                height: size[1],
                depth: size[2],
            },
            named(og_id),
        )
        .unwrap();
    world
        .transform(og_id, Transform::Translate { offset })
        .unwrap();
}

pub(super) fn cylinder(world: &mut WorldGraph, og_id: &str) {
    world
        .create_primitive(
            Primitive::Cylinder {
                radius: 1.0,
                height: 2.0,
            },
            named(og_id),
        )
        .unwrap();
}
