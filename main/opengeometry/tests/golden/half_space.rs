use crate::body::record_body;
use crate::boolean_case::record_operands;
use crate::fixtures::standard;
use crate::graph_case::create_options;
use crate::json::pretty;
use crate::kernel::{
    multi_tool_boolean, BooleanOp, BrepEnvelope, CreateOptions, CreatingOperation, EditScope,
    ModifyingOperation, Plane, Primitive, Transform, WorldGraph,
};
use crate::matrix::record_verdict;
use crate::record::{Failure, Record};
use crate::runner::Case;
use opengeometry::operations::modifying::boolean::MultiToolOutcome;
use opengeometry::primitives::{ProfileSegment, SegmentArc};
use opengeometry_test_support::volume::estimate;
use serde_json::{json, to_value, Value};
use std::f64::consts::PI;
use std::fs;

const CLIPPER: &str = include_str!("../booleans/fixtures/floorkit-section-clipper.json");

const OUTCOMES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/cases/half-space"
);

const CENTRE: [f64; 3] = [1.12, 0.7, 0.0];

const DEFLECTION: f64 = 0.001;

type Build = fn(&mut WorldGraph) -> Result<(), Failure>;

const KINDS: [(&str, Build, f64); 6] = [
    ("ring", ring, 1.0 - PI * 0.09),
    ("tilted-cuboid", tilted_cuboid, 1.0),
    ("sweep-elbow", sweep_elbow, 0.18),
    ("cylinder-sector", cylinder_sector, PI * 0.25 / 3.0),
    (
        "revolved-bowl",
        revolved_bowl,
        2.0 * PI * (0.125 - 0.064) / 3.0,
    ),
    ("torus", torus, 2.0 * PI * PI * 0.4 * 0.01),
];

fn create(
    graph: &mut WorldGraph,
    og_id: &str,
    primitive: Primitive,
    plane: Option<Plane>,
) -> Result<(), Failure> {
    let options = CreateOptions {
        plane,
        ..create_options(og_id)
    };
    graph.create_primitive(primitive, options)?;
    Ok(())
}

fn upright() -> Option<Plane> {
    Some(Plane {
        origin: None,
        normal: Some([0.0, 0.0, 1.0]),
        x_direction: Some([1.0, 0.0, 0.0]),
    })
}

fn transform(graph: &mut WorldGraph, og_id: &str, transform: Transform) -> Result<(), Failure> {
    graph.transform(og_id, transform)?;
    Ok(())
}

fn about_x(graph: &mut WorldGraph, og_id: &str, degrees: f64) -> Result<(), Failure> {
    let axis = [1.0, 0.0, 0.0];
    transform(
        graph,
        og_id,
        Transform::Rotate {
            axis,
            degrees,
            pivot: Some(CENTRE),
        },
    )
}

fn centred(graph: &mut WorldGraph, og_id: &str, origin: [f64; 3]) -> Result<(), Failure> {
    let offset = std::array::from_fn(|axis| CENTRE[axis] - origin[axis]);
    transform(graph, og_id, Transform::Translate { offset })
}

fn ring(graph: &mut WorldGraph) -> Result<(), Failure> {
    let cuboid = Primitive::Cuboid {
        width: 1.0,
        height: 1.0,
        depth: 1.0,
    };
    create(graph, "body", cuboid, None)?;
    create(
        graph,
        "bore",
        Primitive::Cylinder {
            radius: 0.3,
            height: 2.0,
        },
        None,
    )?;
    centred(graph, "bore", [0.0, 1.0, 0.0])?;
    about_x(graph, "bore", 90.0)?;
    centred(graph, "body", [0.0, 0.5, 0.0])?;
    graph.operate(
        "body",
        ModifyingOperation::Subtract,
        &["bore".into()],
        EditScope::Node,
    )?;
    Ok(())
}

fn tilted_cuboid(graph: &mut WorldGraph) -> Result<(), Failure> {
    let cuboid = Primitive::Cuboid {
        width: 1.0,
        height: 1.0,
        depth: 1.0,
    };
    create(graph, "body", cuboid, None)?;
    centred(graph, "body", [0.0, 0.5, 0.0])?;
    about_x(graph, "body", 15.0)
}

fn sweep_elbow(graph: &mut WorldGraph) -> Result<(), Failure> {
    let profile = Primitive::Rectangle {
        width: 0.3,
        breadth: 0.3,
    };
    create(graph, "profile", profile, None)?;
    let points = vec![[0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 1.0, 0.0]];
    let path = Primitive::Polyline {
        points,
        closed: false,
    };
    create(graph, "path", path, None)?;
    let sweep = CreatingOperation::Sweep {
        profile: "profile".into(),
        path: "path".into(),
        caps: None,
    };
    graph.create_operation(sweep, create_options("body"))?;
    centred(graph, "body", [0.5, 0.5, 0.0])
}

fn cylinder_sector(graph: &mut WorldGraph) -> Result<(), Failure> {
    let sector = Primitive::CylinderSector {
        radius: 0.5,
        height: 1.0,
        start_angle: -PI / 3.0,
        sweep_angle: 2.0 * PI / 3.0,
    };
    create(graph, "body", sector, None)?;
    centred(graph, "body", [0.0, 0.5, 0.0])?;
    about_x(graph, "body", 90.0)
}

fn segment(to: [f64; 2], arc: Option<bool>) -> ProfileSegment {
    ProfileSegment {
        to,
        arc: arc.map(|counter_clockwise| SegmentArc {
            center: [0.0, 0.0],
            counter_clockwise,
        }),
    }
}

fn revolved_bowl(graph: &mut WorldGraph) -> Result<(), Failure> {
    let profile = Primitive::Profile {
        start: [0.0, -0.5],
        segments: vec![
            segment([0.5, 0.0], Some(true)),
            segment([0.4, 0.0], None),
            segment([0.0, -0.4], Some(false)),
            segment([0.0, -0.5], None),
        ],
        closed: true,
    };
    create(graph, "profile", profile, upright())?;
    let revolve = CreatingOperation::Revolve {
        profile: "profile".into(),
        axis: None,
    };
    graph.create_operation(revolve, create_options("body"))?;
    centred(graph, "body", [0.0, -0.25, 0.0])
}

fn torus(graph: &mut WorldGraph) -> Result<(), Failure> {
    let torus = Primitive::Torus {
        major_radius: 0.4,
        minor_radius: 0.1,
    };
    create(graph, "body", torus, None)?;
    centred(graph, "body", [0.0, 0.0, 0.0])
}

struct Outcome {
    host: BrepEnvelope,
    clipper: BrepEnvelope,
    id: String,
    cut: MultiToolOutcome,
}

fn subtracted(kind: &str, build: Build) -> Result<Outcome, Failure> {
    let mut graph = WorldGraph::new(standard())?;
    build(&mut graph)?;
    let host = (*graph.placed_brep("body")?).clone();
    let clipper = BrepEnvelope::from_json(CLIPPER)?;
    let id = format!("half-space-{kind}");
    let tools = std::slice::from_ref(&clipper);
    let cut = multi_tool_boolean(&host, tools, BooleanOp::Subtraction, id.clone());
    Ok(Outcome {
        host,
        clipper,
        id,
        cut,
    })
}

fn summary(cut: &MultiToolOutcome) -> Result<Value, Failure> {
    Ok(match &cut.result {
        Ok(result) => json!({
            "faces": result.brep.topology.faces.len(),
            "handlers": cut.handlers,
        }),
        Err(error) => json!({
            "error": to_value(error)?,
            "families": cut.families,
            "attempted": cut.attempted,
        }),
    })
}

fn stored(kind: &str) -> Result<Value, Failure> {
    let path = format!("{OUTCOMES}/{kind}.json");
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

fn record_half(record: &mut Record, brep: &BrepEnvelope, exact: f64) {
    let volume = estimate(brep, DEFLECTION);
    let valid = brep.validate().is_ok();
    let within = (volume.value - exact / 2.0).abs() <= volume.error_bound;
    record.section("half");
    record.field("valid", valid);
    record.field("half-volume-within-bound", within);
    if !(valid && within) {
        record.disagree("half");
    }
}

fn half_space_case(kind: &'static str, build: Build, exact: f64) -> Case {
    Case::new(format!("booleans.half-space.{kind}"), move |record| {
        let Outcome {
            host,
            clipper,
            id,
            cut,
        } = subtracted(kind, build)?;
        record_operands(record, &host, &clipper);
        record.section("boolean");
        record.field("id", &id);
        record.debug("handlers", &cut.handlers);
        record.debug("attempted", &cut.attempted);
        record.debug("families", &cut.families);
        record.cover_all(&cut.handlers);
        let actual = summary(&cut)?;
        let expected = stored(kind).inspect_err(|_| record.disagree("fixture-unreadable"))?;
        record.section("fixture");
        record.block("outcome", &pretty(Ok(actual.clone())));
        record_verdict(record, "outcome", actual == expected);
        match cut.result {
            Ok(result) => {
                record_half(record, &result.brep, exact);
                record.block("report", &pretty(to_value(&result.report)));
                record_body(record, "result", &result.brep);
            }
            Err(error) => record.error("result", error),
        }
        Ok(())
    })
    .requiring_verdicts(&["outcome"])
}

pub(crate) fn cases() -> Vec<Case> {
    KINDS
        .iter()
        .map(|&(kind, build, exact)| half_space_case(kind, build, exact))
        .collect()
}
