use crate::boolean_case::result_body;
use crate::creating::SPATIAL_PATH;
use crate::digest::hex;
use crate::fixtures::standard;
use crate::graph_case::{create_options, polyline, rectangle, sweep};
use crate::json::pretty;
use crate::kernel::{
    classify_point, multi_tool_boolean, tessellate, BooleanOp, BrepEnvelope, StepOptions,
    WorldGraph,
};
use crate::record::{Failure, Record};
use opengeometry_test_support::scenes::acceptance::{acceptance_scene, level_export};
use serde_json::{to_value, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

pub(crate) const SWEEP_KEY: &str = "sweep-3d";

const FALLBACK_DIRECTORY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/parity/batch-matrix"
);

const FALLBACK_ROWS: [&str; 3] = [
    "planar.oblique-cutters",
    "planar.overlapping-cutters",
    "prismatic.single-round",
];

const LEVEL_UNITS: [&str; 2] = ["metre", "millimetre"];

const LEVEL_UP_AXES: [&str; 2] = ["Y", "Z"];

const SWEEP_PROBES: [[f64; 3]; 3] = [[0.0, 1.0, 0.0], [0.0, 1.0, 2.0], [3.0, 3.0, 3.0]];

#[derive(Clone)]
pub(crate) struct SweepOutputs {
    pub(crate) brep_json: Vec<u8>,
    pub(crate) positions: Vec<f64>,
    pub(crate) normals: Vec<f32>,
    pub(crate) indices: Vec<u32>,
    pub(crate) face_ids: Vec<u32>,
    pub(crate) verdicts: Vec<Vec<u8>>,
    pub(crate) step: Vec<u8>,
    pub(crate) report: Vec<u8>,
}

fn sweep_graph() -> Result<WorldGraph, Failure> {
    let mut world = WorldGraph::new(standard())?;
    world.create_primitive(rectangle(1.0, 1.0), create_options("profile"))?;
    world.create_primitive(polyline(&SPATIAL_PATH, false), create_options("path"))?;
    world.create_operation(sweep("profile", "path"), create_options("solid"))?;
    Ok(world)
}

pub(crate) fn sweep_outputs() -> Result<SweepOutputs, Failure> {
    let world = sweep_graph()?;
    let body = world.brep("solid")?;
    let mesh = tessellate(&body, 0.01, 2_000_000)?;
    let verdicts = SWEEP_PROBES
        .iter()
        .map(|point| Ok(format!("{:?}", classify_point(&body, *point)?).into_bytes()))
        .collect::<Result<Vec<_>, Failure>>()?;
    let (step, report) = world.export_step(&["solid".into()], &StepOptions::default())?;
    Ok(SweepOutputs {
        brep_json: body.to_json()?.into_bytes(),
        positions: mesh.positions,
        normals: mesh.normals,
        indices: mesh.indices,
        face_ids: mesh.triangle_face_ids,
        verdicts,
        step: step.into_bytes(),
        report: serde_json::to_vec(&report)?,
    })
}

pub(crate) fn sweep_digest(outputs: &SweepOutputs) -> String {
    let mut hash = Sha256::new();
    hash.update(&outputs.brep_json);
    for value in &outputs.positions {
        hash.update(value.to_bits().to_le_bytes());
    }
    for value in &outputs.normals {
        hash.update(value.to_bits().to_le_bytes());
    }
    for value in outputs.indices.iter().chain(&outputs.face_ids) {
        hash.update(value.to_le_bytes());
    }
    for verdict in &outputs.verdicts {
        hash.update(verdict);
    }
    hash.update(&outputs.step);
    hash.update(&outputs.report);
    hex(&hash.finalize())
}

fn fallback_record(name: &str) -> Result<(String, String), Failure> {
    let path = Path::new(FALLBACK_DIRECTORY).join(format!("{name}.fallback.json"));
    let fixture: Value = serde_json::from_slice(&fs::read(path)?)?;
    let host: BrepEnvelope = serde_json::from_value(fixture["host"].clone())?;
    let cutters: Vec<BrepEnvelope> = serde_json::from_value(fixture["cutters"].clone())?;
    let key = format!("fallback.{name}");
    let id = format!("batch-{name}");
    let mut record = Record::new(&key);
    record.section("fallback");
    record.field("id", &id);
    let outcome = multi_tool_boolean(&host, &cutters, BooleanOp::Subtraction, id);
    record.debug("handlers", &outcome.handlers);
    record.field("tool_index", outcome.tool_index);
    match outcome.result {
        Ok(result) => result_body(&mut record, Ok(result)),
        Err(error) => record.block("result.error", &pretty(to_value(&error))),
    }
    Ok((key, record.finish().0))
}

fn level_records() -> Vec<(String, String)> {
    let scene = acceptance_scene();
    let mut records = Vec::new();
    for unit in LEVEL_UNITS {
        for axis in LEVEL_UP_AXES {
            let key = format!("level.step.{unit}.{}", axis.to_lowercase());
            let (text, report) = level_export(&scene, unit, axis);
            let mut record = Record::new(&key);
            record.block("report", &pretty(to_value(&report)));
            record.block("text", &text);
            records.push((key, record.finish().0));
        }
    }
    records
}

pub(crate) fn records() -> Result<Vec<(String, String)>, Failure> {
    let mut records = FALLBACK_ROWS
        .into_iter()
        .map(fallback_record)
        .collect::<Result<Vec<_>, _>>()?;
    records.extend(level_records());
    Ok(records)
}
