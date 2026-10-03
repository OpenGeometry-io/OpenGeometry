use crate::display::write_buffers;
use crate::graph_record::{counters, node_state};
use crate::json::pretty;
use crate::kernel::{static_bucket, WorldGraph};
use crate::record::Record;
use serde_json::to_value;
use std::collections::BTreeSet;

pub(crate) struct Tracked {
    ids: BTreeSet<String>,
    revisions: BTreeSet<u64>,
}

impl Tracked {
    pub(crate) fn new() -> Self {
        Self {
            ids: BTreeSet::new(),
            revisions: BTreeSet::from([0]),
        }
    }

    pub(crate) fn add(&mut self, og_id: Option<String>, fallback: &str) -> String {
        match og_id {
            Some(og_id) => {
                self.ids.insert(og_id.clone());
                og_id
            }
            None => fallback.to_string(),
        }
    }

    pub(crate) fn remember(&mut self, graph: &WorldGraph) {
        self.revisions.insert(graph.revision());
    }
}

fn history(record: &mut Record, graph: &WorldGraph, tracked: &Tracked) {
    let current = graph.revision();
    let mut probes = tracked.revisions.clone();
    probes.insert(current / 2);
    probes.insert(current.saturating_sub(1));
    probes.insert(current + 1);
    for revision in probes {
        match graph.changes_since(revision) {
            Ok(changes) => record.block(
                &format!("changes_since {revision}"),
                &pretty(to_value(&changes)),
            ),
            Err(error) => record.graph_error(&format!("changes_since {revision}"), &error),
        }
    }
}

fn display_state(record: &mut Record, graph: &WorldGraph, og_id: &str) {
    let Ok(node) = graph.node(og_id) else {
        return;
    };
    let Some(shape_id) = node.shape.clone() else {
        return;
    };
    let title = format!("buffers {og_id}");
    match graph.brep(og_id).map(|brep| static_bucket(&brep)) {
        Ok(Ok(bucket)) => match graph.buffers(&shape_id, bucket, 2_000_000) {
            Ok(buffers) => write_buffers(record, &title, bucket, &buffers),
            Err(error) => record.graph_error(&title, &error),
        },
        Ok(Err(error)) => record.error(&title, error),
        Err(error) => record.graph_error(&title, &error),
    }
}

pub(crate) fn state(record: &mut Record, graph: &WorldGraph, tracked: &Tracked, label: &str) {
    record.section(&format!("state {label}"));
    counters(record, graph);
    for og_id in &tracked.ids {
        node_state(record, graph, og_id);
        match graph.children(og_id) {
            Ok(children) => record.debug("children", children),
            Err(error) => record.graph_error_field("children.error", &error),
        }
        match graph.parent(og_id) {
            Ok(parent) => record.debug("parent", parent),
            Err(error) => record.graph_error_field("parent.error", &error),
        }
        match graph.instance_count(og_id) {
            Ok(count) => record.field("instance_count", count),
            Err(error) => record.graph_error_field("instance_count.error", &error),
        }
        display_state(record, graph, og_id);
    }
    history(record, graph, tracked);
}
