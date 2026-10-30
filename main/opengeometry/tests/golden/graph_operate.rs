use crate::graph_case::{
    child, copy_options, create_options, cuboid, extrude, graph_case, inspect, operation, polyline,
    primitive, rectangle, sweep, Script,
};
use crate::graph_record::{changes, created, node_state, record_operation};
use crate::kernel::{EditScope, ModifyingOperation, Primitive, Transform, WorldGraph};
use crate::record::{Failure, Record};
use crate::runner::Case;

const SUBTRACT: ModifyingOperation = ModifyingOperation::Subtract;

fn translate(record: &mut Record, graph: &mut WorldGraph, og_id: &str, offset: [f64; 3]) {
    let result = graph.transform(og_id, Transform::Translate { offset });
    changes(record, &format!("translate {og_id}"), result);
}

fn operate(
    record: &mut Record,
    graph: &mut WorldGraph,
    target: &str,
    kind: ModifyingOperation,
    tools: &[&str],
    scope: EditScope,
) {
    let label = format!("operate {target} {kind:?} {tools:?} {scope:?}");
    record_operation(record, graph, &label, target, kind, tools, scope);
}

fn instance(record: &mut Record, graph: &mut WorldGraph, source: &str, og_id: &str) {
    let options = copy_options(og_id, None);
    created(
        record,
        &format!("instance {og_id}"),
        graph.instance(source, options),
    );
}

fn host_and_tool(record: &mut Record, graph: &mut WorldGraph, offset: [f64; 3]) {
    primitive(record, graph, cuboid(2.0, 2.0, 2.0), create_options("host"));
    primitive(record, graph, cuboid(2.0, 2.0, 2.0), create_options("tool"));
    translate(record, graph, "tool", offset);
}

fn box_subtraction(graph: &mut WorldGraph, record: &mut Record) -> Result<(), Failure> {
    host_and_tool(record, graph, [1.0, 0.0, 0.0]);
    operate(record, graph, "host", SUBTRACT, &["tool"], EditScope::Node);
    inspect(record, graph, "host")?;
    node_state(record, graph, "tool");
    Ok(())
}

fn shared_target(graph: &mut WorldGraph, record: &mut Record) -> Result<(), Failure> {
    host_and_tool(record, graph, [1.0, 0.0, 0.0]);
    instance(record, graph, "host", "host-copy");
    instance(record, graph, "tool", "tool-copy");
    operate(
        record,
        graph,
        "host",
        SUBTRACT,
        &["tool-copy"],
        EditScope::Node,
    );
    operate(
        record,
        graph,
        "host",
        SUBTRACT,
        &["tool-copy"],
        EditScope::AllInstances,
    );
    operate(
        record,
        graph,
        "host",
        SUBTRACT,
        &["host-copy"],
        EditScope::AllInstances,
    );
    inspect(record, graph, "host")?;
    node_state(record, graph, "host-copy");
    Ok(())
}

fn empty_and_limits(graph: &mut WorldGraph, record: &mut Record) -> Result<(), Failure> {
    host_and_tool(record, graph, [10.0, 0.0, 0.0]);
    let intersect = ModifyingOperation::Intersect;
    operate(record, graph, "host", intersect, &["tool"], EditScope::Node);
    let union = ModifyingOperation::Union;
    operate(record, graph, "host", union, &[], EditScope::Node);
    operate(record, graph, "host", union, &["host"], EditScope::Node);
    operate(record, graph, "host", union, &["missing"], EditScope::Node);
    node_state(record, graph, "host");
    Ok(())
}

fn empty_subtraction(graph: &mut WorldGraph, record: &mut Record) -> Result<(), Failure> {
    primitive(
        record,
        graph,
        cuboid(1.0, 1.0, 1.0),
        create_options("small"),
    );
    primitive(
        record,
        graph,
        cuboid(2.0, 2.0, 2.0),
        create_options("large"),
    );
    operate(
        record,
        graph,
        "small",
        SUBTRACT,
        &["large"],
        EditScope::Node,
    );
    node_state(record, graph, "small");
    Ok(())
}

fn unique_then_operate(graph: &mut WorldGraph, record: &mut Record) -> Result<(), Failure> {
    host_and_tool(record, graph, [1.0, 0.0, 0.0]);
    instance(record, graph, "host", "instance");
    let unique = graph.make_unique("host").map(|changes| changes.is_some());
    record.debug("make_unique", unique);
    operate(record, graph, "host", SUBTRACT, &["tool"], EditScope::Node);
    inspect(record, graph, "host")?;
    node_state(record, graph, "instance");
    Ok(())
}

fn planar_batch(graph: &mut WorldGraph, record: &mut Record) -> Result<(), Failure> {
    primitive(record, graph, cuboid(6.0, 2.0, 2.0), create_options("host"));
    for (name, x) in [("left", -1.5), ("right", 1.5)] {
        primitive(record, graph, cuboid(1.0, 2.0, 2.0), create_options(name));
        translate(record, graph, name, [x, 0.0, 0.0]);
    }
    operate(
        record,
        graph,
        "host",
        SUBTRACT,
        &["left", "right"],
        EditScope::Node,
    );
    inspect(record, graph, "host")
}

fn mixed_batch(graph: &mut WorldGraph, record: &mut Record) -> Result<(), Failure> {
    primitive(
        record,
        graph,
        rectangle(6.0, 0.3),
        create_options("profile"),
    );
    operation(
        record,
        graph,
        extrude("profile", &[], 3.0),
        create_options("wall"),
    );
    primitive(record, graph, cuboid(0.8, 3.0, 1.0), create_options("rect"));
    translate(record, graph, "rect", [1.5, 0.0, 0.5]);
    let round = Primitive::Cylinder {
        radius: 0.4,
        height: 2.0,
    };
    primitive(record, graph, round, create_options("round"));
    let rotate = Transform::Rotate {
        axis: [1.0, 0.0, 0.0],
        degrees: 90.0,
        pivot: Some([0.0; 3]),
    };
    changes(record, "rotate round", graph.transform("round", rotate));
    translate(record, graph, "round", [-1.5, 1.5, -1.0]);
    operate(
        record,
        graph,
        "wall",
        SUBTRACT,
        &["rect", "round"],
        EditScope::Node,
    );
    inspect(record, graph, "wall")
}

fn serial_tools(graph: &mut WorldGraph, record: &mut Record) -> Result<(), Failure> {
    primitive(record, graph, cuboid(4.0, 2.0, 2.0), create_options("host"));
    for (name, x) in [("first", 1.0), ("second", -1.0)] {
        primitive(record, graph, cuboid(2.0, 2.0, 2.0), create_options(name));
        translate(record, graph, name, [x, 0.5, 0.0]);
    }
    let union = ModifyingOperation::Union;
    operate(
        record,
        graph,
        "host",
        union,
        &["first", "second"],
        EditScope::Node,
    );
    inspect(record, graph, "host")?;
    let intersect = ModifyingOperation::Intersect;
    operate(
        record,
        graph,
        "host",
        intersect,
        &["first", "second"],
        EditScope::Node,
    );
    node_state(record, graph, "host");
    Ok(())
}

fn scaled_tool(graph: &mut WorldGraph, record: &mut Record) -> Result<(), Failure> {
    host_and_tool(record, graph, [1.0, 0.0, 0.0]);
    let scale = Transform::Scale {
        factor: 2.0,
        pivot: None,
    };
    changes(record, "scale tool", graph.transform("tool", scale));
    operate(record, graph, "host", SUBTRACT, &["tool"], EditScope::Node);
    Ok(())
}

fn swept_host(graph: &mut WorldGraph, record: &mut Record) -> Result<(), Failure> {
    primitive(
        record,
        graph,
        rectangle(2.0, 2.0),
        create_options("profile"),
    );
    let path = polyline(&[[0.0; 3], [0.0, 3.0, 0.0], [3.0, 3.0, 0.0]], false);
    primitive(record, graph, path, create_options("path"));
    operation(
        record,
        graph,
        sweep("profile", "path"),
        create_options("sweep"),
    );
    primitive(record, graph, cuboid(1.0, 1.0, 1.0), create_options("tool"));
    operate(record, graph, "sweep", SUBTRACT, &["tool"], EditScope::Node);
    inspect(record, graph, "sweep")
}

fn level_members(record: &mut Record, graph: &mut WorldGraph, wall: bool) {
    created(
        record,
        "level",
        graph.create_system_assembly(create_options("level")),
    );
    if wall {
        primitive(
            record,
            graph,
            rectangle(4.0, 0.5),
            child("profile", "level"),
        );
        let host = child("host", "level");
        operation(record, graph, extrude("profile", &[], 3.0), host);
        primitive(record, graph, cuboid(0.8, 3.0, 1.0), child("tool", "level"));
        translate(record, graph, "tool", [0.0, 0.0, 0.5]);
    } else {
        primitive(record, graph, cuboid(2.0, 2.0, 2.0), child("host", "level"));
        primitive(record, graph, cuboid(2.0, 2.0, 2.0), child("tool", "level"));
        translate(record, graph, "tool", [1.0, 0.0, 0.0]);
    }
}

fn corotated(degrees: f64, wall: bool) -> impl Script {
    move |graph: &mut WorldGraph, record: &mut Record| {
        level_members(record, graph, wall);
        let rotate = Transform::Rotate {
            axis: [0.0, 1.0, 0.0],
            degrees,
            pivot: Some([0.0; 3]),
        };
        changes(record, "rotate level", graph.transform("level", rotate));
        operate(record, graph, "host", SUBTRACT, &["tool"], EditScope::Node);
        inspect(record, graph, "host")
    }
}

pub(crate) fn cases() -> Vec<Case> {
    let mut cases = vec![
        graph_case("graph.operate.box-subtraction", box_subtraction),
        graph_case("graph.operate.shared-target", shared_target),
        graph_case("graph.operate.empty-and-limits", empty_and_limits),
        graph_case("graph.operate.empty-subtraction", empty_subtraction),
        graph_case(
            "graph.operate.make-unique-then-operate",
            unique_then_operate,
        ),
        graph_case("graph.operate.planar-batch", planar_batch),
        graph_case("graph.operate.mixed-cutter-batch", mixed_batch),
        graph_case("graph.operate.serial-union-and-intersection", serial_tools),
        graph_case("graph.operate.scaled-tool", scaled_tool),
        graph_case("graph.operate.sweep-result", swept_host),
    ];
    for degrees in [0.0, 30.0, 90.0] {
        let box_name = format!("graph.operate.corotated-box-{degrees}");
        cases.push(graph_case(&box_name, corotated(degrees, false)));
        let wall_name = format!("graph.operate.corotated-wall-{degrees}");
        cases.push(graph_case(&wall_name, corotated(degrees, true)));
    }
    cases
}
