use crate::digest::{bits_list, sha256};
use crate::json::pretty;
use crate::kernel::{
    ChangeSet, EditScope, GraphError, ModifyingOperation, StepOptions, WorldGraph,
};
use crate::record::Record;
use serde_json::{to_value, Value};

pub(crate) fn changes(
    record: &mut Record,
    label: &str,
    result: Result<ChangeSet, GraphError>,
) -> bool {
    match result {
        Ok(changes) => {
            record.block(&format!("step {label}"), &pretty(to_value(&changes)));
            true
        }
        Err(error) => {
            record.error(&format!("step {label}"), error);
            false
        }
    }
}

pub(crate) fn record_operation(
    record: &mut Record,
    graph: &mut WorldGraph,
    label: &str,
    target: &str,
    kind: ModifyingOperation,
    tools: &[&str],
    scope: EditScope,
) {
    let tools = tools
        .iter()
        .map(|tool| tool.to_string())
        .collect::<Vec<_>>();
    changes(record, label, graph.operate(target, kind, &tools, scope));
}

pub(crate) fn created(
    record: &mut Record,
    label: &str,
    result: Result<(String, ChangeSet), GraphError>,
) -> Option<String> {
    match result {
        Ok((og_id, changes)) => {
            record.section(&format!("step {label}"));
            record.field("created", &og_id);
            record.line(&pretty(to_value(&changes)));
            Some(og_id)
        }
        Err(error) => {
            record.error(&format!("step {label}"), error);
            None
        }
    }
}

pub(crate) fn counters(record: &mut Record, graph: &WorldGraph) {
    record.field("revision", graph.revision());
    record.field("node_count", graph.node_count());
    record.field("shape_count", graph.shape_count());
    record.debug("mark_stats", graph.mark_stats());
    match graph.reserve(1, &[]) {
        Ok(reserved) => record.debug("next_reservation", reserved),
        Err(error) => record.debug("next_reservation.error", error),
    }
}

fn placement_state(record: &mut Record, graph: &WorldGraph, og_id: &str) {
    match graph.placement(og_id) {
        Ok(placement) => record.debug("placement", placement),
        Err(error) => record.debug("placement.error", error),
    }
    match graph.world_placement(og_id) {
        Ok(world) => record.debug("world_placement", world),
        Err(error) => record.debug("world_placement.error", error),
    }
    match graph.world_matrix(og_id) {
        Ok(matrix) => record.field("world_matrix", bits_list(&matrix)),
        Err(error) => record.debug("world_matrix.error", error),
    }
    match graph.bounds(og_id) {
        Ok(Some(bounds)) => record.field("bounds", bits_list(&bounds)),
        Ok(None) => record.line("bounds: none"),
        Err(error) => record.debug("bounds.error", error),
    }
}

fn cover_report(record: &mut Record, report: &Value) {
    let names = report.get("handlers").and_then(Value::as_array);
    for name in names.into_iter().flatten().filter_map(Value::as_str) {
        record.cover(name);
    }
}

fn shape_state(record: &mut Record, graph: &WorldGraph, shape_id: &str, og_id: &str) {
    match graph.shape(og_id) {
        Ok(shape) => {
            record.field("shape.revision", shape.revision);
            record.field("shape.users", shape.users);
            record.debug("shape.edge_keys", &shape.edge_keys);
            record.debug("shape.body_type", shape.brep.body_type());
            match &shape.report {
                Some(report) => {
                    record.field("shape.report", pretty(to_value(report)));
                    cover_report(record, report);
                }
                None => record.line("shape.report: none"),
            }
        }
        Err(error) => record.debug("shape.error", error),
    }
    match graph.snapshot(shape_id) {
        Ok(bytes) => record.field("snapshot.sha256", sha256(&bytes)),
        Err(error) => record.debug("snapshot.error", error),
    }
}

pub(crate) fn node_state(record: &mut Record, graph: &WorldGraph, og_id: &str) {
    record.section(&format!("node {og_id}"));
    let node = match graph.node(og_id) {
        Ok(node) => node.clone(),
        Err(error) => {
            record.debug("error", error);
            return;
        }
    };
    record.debug("node", &node);
    match graph.node_by_handle(node.handle, node.generation) {
        Ok(found) => record.field("by_handle", &found.og_id),
        Err(error) => record.debug("by_handle.error", error),
    }
    placement_state(record, graph, og_id);
    if let Some(shape_id) = &node.shape {
        shape_state(record, graph, shape_id, og_id);
    }
}

pub(crate) fn export(
    record: &mut Record,
    graph: &WorldGraph,
    label: &str,
    nodes: &[String],
    options: &StepOptions,
) {
    let title = format!("export {label}");
    match graph.export_step(nodes, options) {
        Ok((text, report)) => {
            record.section(&title);
            record.debug("nodes", nodes);
            record.line(&pretty(to_value(options)));
            record.field("text.sha256", sha256(text.as_bytes()));
            record.line(&pretty(to_value(&report)));
            record.block(&format!("{title}.text"), &text);
        }
        Err(error) => record.error(&title, error),
    }
}
