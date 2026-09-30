use crate::graph_case::{
    child, circle, copy_options, create_options, extrude, graph_case, inspect, on_plane, operation,
    polyline, primitive, rectangle, sweep,
};
use crate::graph_record::{changes, created, node_state};
use crate::kernel::{CreatingOperation, EditScope, Point3, Primitive, Transform, WorldGraph};
use crate::record::{Failure, Record};
use crate::runner::Case;

type Profiles = fn() -> Vec<(&'static str, Primitive)>;

type Extrusion = (&'static str, Profiles, &'static [&'static str], f64);

type Sweep = (&'static str, fn() -> Primitive, &'static [Point3]);

const L_PATH: [Point3; 3] = [[0.0, 0.0, 0.0], [0.0, 3.0, 0.0], [3.0, 3.0, 0.0]];

const SPATIAL_PATH: [Point3; 4] = [
    [0.0, 0.0, 0.0],
    [0.0, 3.0, 0.0],
    [3.0, 3.0, 0.0],
    [3.0, 3.0, 3.0],
];

const SQUARE: [Point3; 4] = [
    [-1.0, 0.0, -1.0],
    [1.0, 0.0, -1.0],
    [1.0, 0.0, 1.0],
    [-1.0, 0.0, 1.0],
];

fn solid(
    record: &mut Record,
    graph: &mut WorldGraph,
    value: CreatingOperation,
) -> Result<(), Failure> {
    if operation(record, graph, value, create_options("solid")).is_some() {
        inspect(record, graph, "solid")?;
    }
    Ok(())
}

fn extruded(entry: &Extrusion) -> Case {
    let (name, profiles, holes, distance) = *entry;
    graph_case(&format!("creating.extrude.{name}"), move |graph, record| {
        for (og_id, shape) in profiles() {
            primitive(record, graph, shape, create_options(og_id));
        }
        solid(record, graph, extrude("profile", holes, distance))
    })
}

fn swept(entry: &Sweep) -> Case {
    let (name, profile, path) = *entry;
    graph_case(&format!("creating.sweep.{name}"), move |graph, record| {
        primitive(record, graph, profile(), create_options("profile"));
        primitive(record, graph, polyline(path, false), create_options("path"));
        solid(record, graph, sweep("profile", "path"))
    })
}

fn rectangle_profile() -> Vec<(&'static str, Primitive)> {
    vec![("profile", rectangle(2.0, 3.0))]
}

fn square_profile() -> Vec<(&'static str, Primitive)> {
    vec![("profile", rectangle(2.0, 2.0))]
}

fn circle_profile() -> Vec<(&'static str, Primitive)> {
    vec![("profile", circle(2.0))]
}

fn closed_polyline() -> Vec<(&'static str, Primitive)> {
    let points = [[0.0; 3], [2.0, 0.0, 0.0], [2.0, 3.0, 0.0], [0.0, 3.0, 0.0]];
    vec![("profile", polyline(&points, true))]
}

fn ground_forward() -> Vec<(&'static str, Primitive)> {
    vec![("profile", polyline(&SQUARE, true))]
}

fn ground_reverse() -> Vec<(&'static str, Primitive)> {
    let points = [SQUARE[3], SQUARE[2], SQUARE[1], SQUARE[0]];
    vec![("profile", polyline(&points, true))]
}

fn annular() -> Vec<(&'static str, Primitive)> {
    vec![("profile", circle(2.0)), ("hole", circle(1.0))]
}

fn rectangle_round_hole() -> Vec<(&'static str, Primitive)> {
    vec![("profile", rectangle(4.0, 4.0)), ("hole", circle(1.0))]
}

fn polyline_hole() -> Vec<(&'static str, Primitive)> {
    let hole = [
        [-0.5, 0.0, -0.5],
        [0.5, 0.0, -0.5],
        [0.5, 0.0, 0.5],
        [-0.5, 0.0, 0.5],
    ];
    vec![
        ("profile", polyline(&SQUARE, true)),
        ("hole", polyline(&hole, true)),
    ]
}

fn self_crossing() -> Vec<(&'static str, Primitive)> {
    let points = [
        [-1.0, 0.0, -1.0],
        [1.0, 0.0, 1.0],
        [-1.0, 0.0, 1.0],
        [1.0, 0.0, -1.0],
    ];
    vec![("profile", polyline(&points, true))]
}

fn open_polyline() -> Vec<(&'static str, Primitive)> {
    vec![("profile", polyline(&L_PATH, false))]
}

const EXTRUSIONS: [Extrusion; 12] = [
    ("rectangle", rectangle_profile, &[], 4.0),
    ("circle", circle_profile, &[], 3.0),
    ("closed-polyline", closed_polyline, &[], 4.0),
    ("ground-polyline-forward", ground_forward, &[], 2.0),
    ("ground-polyline-reverse", ground_reverse, &[], 2.0),
    ("negative-annular", annular, &["hole"], -3.0),
    ("rectangle-round-hole", rectangle_round_hole, &["hole"], 2.0),
    ("polyline-with-polyline-hole", polyline_hole, &["hole"], 1.0),
    ("too-short", square_profile, &[], 1e-9),
    ("self-crossing", self_crossing, &[], 2.0),
    ("hole-is-profile", square_profile, &["profile"], 2.0),
    ("open-polyline-profile", open_polyline, &[], 2.0),
];

fn square_one() -> Primitive {
    rectangle(1.0, 1.0)
}

fn square_two() -> Primitive {
    rectangle(2.0, 2.0)
}

fn slot() -> Primitive {
    rectangle(1.0, 2.0)
}

fn disc_half() -> Primitive {
    circle(0.5)
}

fn disc_small() -> Primitive {
    circle(0.4)
}

fn disc_wide() -> Primitive {
    circle(0.6)
}

const SWEEPS: [Sweep; 11] = [
    ("rectangle-l-path", square_two, &L_PATH),
    ("rectangle-spatial-path", square_one, &SPATIAL_PATH),
    ("circle-l-path", disc_half, &L_PATH),
    ("circle-spatial-path", disc_small, &SPATIAL_PATH),
    ("straight-path", slot, &[[0.0; 3], [0.0, 3.0, 0.0]]),
    (
        "collinear-path",
        square_one,
        &[[0.0; 3], [0.0, 1.5, 0.0], [0.0, 3.0, 0.0], [3.0, 3.0, 0.0]],
    ),
    (
        "path-off-profile",
        square_two,
        &[[0.0, 1.0, 0.0], [0.0, 3.0, 0.0], [3.0, 3.0, 0.0]],
    ),
    (
        "miter-collision",
        disc_wide,
        &[[0.0; 3], [0.0, 0.2, 0.0], [2.0, 0.2, 0.0]],
    ),
    (
        "separated-collision",
        disc_wide,
        &[[0.0; 3], [0.0, 3.0, 0.0], [1.0, 3.0, 0.0], [1.0, 0.0, 0.0]],
    ),
    (
        "reversing-path",
        square_one,
        &[[0.0; 3], [0.0, 3.0, 0.0], [0.0, 1.0, 0.0]],
    ),
    (
        "circle-horizontal-path",
        disc_half,
        &[[0.0; 3], [3.0, 0.0, 0.0]],
    ),
];

fn offset_hole(graph: &mut WorldGraph, record: &mut Record) -> Result<(), Failure> {
    primitive(record, graph, circle(2.0), create_options("profile"));
    primitive(record, graph, circle(0.5), create_options("hole"));
    let offset = Transform::Translate {
        offset: [0.8, 0.0, 0.0],
    };
    changes(record, "move hole", graph.transform("hole", offset));
    solid(record, graph, extrude("profile", &["hole"], 1.5))
}

fn far_parent(graph: &mut WorldGraph, record: &mut Record) -> Result<(), Failure> {
    created(
        record,
        "assembly",
        graph.create_system_assembly(create_options("assembly")),
    );
    let offset = Transform::Translate {
        offset: [10_000.0, 0.0, 0.0],
    };
    changes(
        record,
        "translate assembly",
        graph.transform("assembly", offset),
    );
    primitive(
        record,
        graph,
        rectangle(2.0, 2.0),
        child("profile", "assembly"),
    );
    solid(record, graph, extrude("profile", &[], 3.0))
}

fn noncoplanar_hole(graph: &mut WorldGraph, record: &mut Record) -> Result<(), Failure> {
    primitive(
        record,
        graph,
        rectangle(4.0, 4.0),
        create_options("profile"),
    );
    let options = on_plane(
        create_options("hole"),
        [0.0, 0.5, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, 0.0, 0.0],
    );
    primitive(record, graph, circle(1.0), options);
    solid(record, graph, extrude("profile", &["hole"], 2.0))
}

fn unknown_profile(graph: &mut WorldGraph, record: &mut Record) -> Result<(), Failure> {
    solid(record, graph, extrude("missing", &[], 1.0))
}

fn offset_profile(graph: &mut WorldGraph, record: &mut Record) -> Result<(), Failure> {
    primitive(record, graph, circle(0.3), create_options("profile"));
    let offset = Transform::Translate {
        offset: [1.0, 0.0, 0.0],
    };
    changes(record, "move profile", graph.transform("profile", offset));
    primitive(
        record,
        graph,
        polyline(&L_PATH, false),
        create_options("path"),
    );
    solid(record, graph, sweep("profile", "path"))
}

fn closed_path(graph: &mut WorldGraph, record: &mut Record) -> Result<(), Failure> {
    primitive(
        record,
        graph,
        rectangle(1.0, 1.0),
        create_options("profile"),
    );
    primitive(
        record,
        graph,
        polyline(&SPATIAL_PATH, true),
        create_options("path"),
    );
    solid(record, graph, sweep("profile", "path"))
}

fn circle_path(graph: &mut WorldGraph, record: &mut Record) -> Result<(), Failure> {
    primitive(
        record,
        graph,
        rectangle(1.0, 1.0),
        create_options("profile"),
    );
    primitive(record, graph, circle(3.0), create_options("path"));
    solid(record, graph, sweep("profile", "path"))
}

fn rebuild_keeps_placement(graph: &mut WorldGraph, record: &mut Record) -> Result<(), Failure> {
    primitive(
        record,
        graph,
        rectangle(2.0, 2.0),
        create_options("profile"),
    );
    operation(
        record,
        graph,
        extrude("profile", &[], 2.0),
        create_options("solid"),
    );
    let offset = Transform::Translate {
        offset: [5.0, 0.0, 0.0],
    };
    changes(record, "translate", graph.transform("solid", offset));
    let rebuilt = graph.rebuild_operation("solid", extrude("profile", &[], 3.0), EditScope::Node);
    changes(record, "rebuild", rebuilt);
    inspect(record, graph, "solid")
}

fn rebuild_shared(graph: &mut WorldGraph, record: &mut Record) -> Result<(), Failure> {
    primitive(
        record,
        graph,
        rectangle(2.0, 2.0),
        create_options("profile"),
    );
    operation(
        record,
        graph,
        extrude("profile", &[], 2.0),
        create_options("solid"),
    );
    let copy = copy_options("instance", None);
    created(record, "instance", graph.instance("solid", copy));
    for scope in [EditScope::Node, EditScope::AllInstances] {
        let rebuilt = graph.rebuild_operation("solid", extrude("profile", &[], 3.0), scope);
        changes(record, &format!("rebuild {scope:?}"), rebuilt);
    }
    inspect(record, graph, "solid")?;
    node_state(record, graph, "instance");
    Ok(())
}

fn rebuild_sweep(graph: &mut WorldGraph, record: &mut Record) -> Result<(), Failure> {
    primitive(
        record,
        graph,
        rectangle(1.0, 1.0),
        create_options("profile"),
    );
    primitive(
        record,
        graph,
        polyline(&SPATIAL_PATH, false),
        create_options("path"),
    );
    operation(
        record,
        graph,
        sweep("profile", "path"),
        create_options("solid"),
    );
    let same = graph.rebuild_operation("solid", sweep("profile", "path"), EditScope::Node);
    changes(record, "rebuild same", same);
    let path = graph.rebuild_primitive("path", polyline(&L_PATH, false), EditScope::Node);
    changes(record, "rebuild path", path);
    let again = graph.rebuild_operation("solid", sweep("profile", "path"), EditScope::Node);
    changes(record, "rebuild sweep", again);
    let mismatch = graph.rebuild_primitive("path", circle(1.0), EditScope::Node);
    changes(record, "rebuild type mismatch", mismatch);
    inspect(record, graph, "solid")
}

pub(crate) fn cases() -> Vec<Case> {
    let mut cases = EXTRUSIONS.iter().map(extruded).collect::<Vec<_>>();
    cases.extend(SWEEPS.iter().map(swept));
    cases.extend([
        graph_case("creating.extrude.offset-circle-hole", offset_hole),
        graph_case("creating.extrude.far-parent", far_parent),
        graph_case("creating.extrude.noncoplanar-hole", noncoplanar_hole),
        graph_case("creating.extrude.unknown-profile", unknown_profile),
        graph_case("creating.sweep.circle-offset-profile", offset_profile),
        graph_case("creating.sweep.closed-path", closed_path),
        graph_case("creating.sweep.circle-path", circle_path),
        graph_case("creating.rebuild.keeps-placement", rebuild_keeps_placement),
        graph_case("creating.rebuild.shared-scope", rebuild_shared),
        graph_case("creating.rebuild.sweep-and-path", rebuild_sweep),
    ]);
    cases
}
