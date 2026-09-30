use crate::exceptions::{enforce, ListSpec, Violation};
use crate::frozen_lists::{MODULE_CYCLES, MUTUAL_IMPORTS, SIBLING_CYCLES, UPWARD_IMPORTS};
use crate::graph::{cycles, import_edges, top_module, Edge};
use crate::source_tree::SourceTree;
use crate::{kernel_sources, rule_list};
use std::collections::{BTreeMap, BTreeSet};

const LAYERS: [(&str, usize); 11] = [
    ("math", 0),
    ("brep", 1),
    ("geom2d", 2),
    ("query", 3),
    ("primitives", 4),
    ("tessellation", 4),
    ("intersection", 5),
    ("operations", 6),
    ("exchange", 7),
    ("world_graph", 8),
    ("bindings", 9),
];
const TEST_FIXTURE_LAYERS: usize = 7;
const BOOLEAN: [&str; 3] = ["operations", "modifying", "boolean"];
const BOOLEAN_ORDER: [&str; 8] = [
    "types",
    "operands",
    "assembly",
    "handlers",
    "dispatch",
    "batch",
    "multi_tool",
    "shell",
];

fn layer_of(module: &str) -> Option<usize> {
    LAYERS
        .iter()
        .find(|(name, _)| *name == module)
        .map(|(_, layer)| *layer)
}

fn may_import(from: &str, to: &str) -> bool {
    if from == to {
        return true;
    }
    if from == "exchange" && to == "operations" {
        return false;
    }
    match (layer_of(from), layer_of(to)) {
        (Some(importer), Some(imported)) => imported < importer,
        _ => false,
    }
}

fn test_may_import(from: &str, to: &str) -> bool {
    may_import(from, to) || layer_of(to).is_some_and(|layer| layer <= TEST_FIXTURE_LAYERS)
}

fn top_modules<'a>(tree: &'a SourceTree, edge: &Edge) -> Option<(&'a str, &'a str)> {
    let from = top_module(&tree.files[edge.from])?;
    let to = top_module(&tree.files[edge.to])?;
    Some((from, to))
}

fn edge_layer_violation(
    tree: &SourceTree,
    edge: &Edge,
    allowed: fn(&str, &str) -> bool,
) -> Option<Violation> {
    let file = &tree.files[edge.from].path;
    let from = top_module(&tree.files[edge.from])?;
    let detail = match top_module(&tree.files[edge.to]) {
        None => format!("`{}` resolves to lib.rs, not to a layer module", edge.path),
        Some(to) if !allowed(from, to) => {
            format!("`{from}` may not import `{to}` (`{}`)", edge.path)
        }
        Some(_) => return None,
    };
    Some(Violation::new(file, edge.line, &[file, &edge.path], detail))
}

fn layer_violations(tree: &SourceTree, edges: &[Edge]) -> Vec<Violation> {
    edges
        .iter()
        .filter(|edge| !edge.test)
        .filter_map(|edge| edge_layer_violation(tree, edge, may_import))
        .collect()
}

fn test_layer_violations(tree: &SourceTree, edges: &[Edge]) -> Vec<Violation> {
    edges
        .iter()
        .filter(|edge| edge.test)
        .filter_map(|edge| edge_layer_violation(tree, edge, test_may_import))
        .collect()
}

fn cycle_violations(tree: &SourceTree, edges: &[Edge]) -> Vec<Violation> {
    let mut adjacency: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for edge in edges.iter().filter(|edge| !edge.test) {
        if let Some((from, to)) = top_modules(tree, edge) {
            if from != to {
                adjacency.entry(from.into()).or_default().insert(to.into());
            }
        }
    }
    let mut violations = Vec::new();
    for cycle in cycles(&adjacency) {
        let members: Vec<&str> = cycle.iter().map(String::as_str).collect();
        let key = members.join("<->");
        let closing = edges.iter().find(|edge| {
            !edge.test
                && top_modules(tree, edge).is_some_and(|(from, to)| {
                    from != to
                        && cycle.contains(from)
                        && cycle.contains(to)
                        && !may_import(from, to)
                })
        });
        let (file, line) = closing.map_or(("lib.rs", 0), |edge| {
            (tree.files[edge.from].path.as_str(), edge.line)
        });
        let detail = format!("top-level modules {} import each other", members.join(", "));
        violations.push(Violation::new(file, line, &[&key], detail));
    }
    violations
}

fn mutual_import_violations(tree: &SourceTree, edges: &[Edge]) -> Vec<Violation> {
    let mut first_lines: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    for edge in edges {
        first_lines.entry((edge.from, edge.to)).or_insert(edge.line);
    }
    let mut violations = Vec::new();
    for (&(from, to), &line) in &first_lines {
        let (first, second) = (&tree.files[from].path, &tree.files[to].path);
        if first < second && first_lines.contains_key(&(to, from)) {
            let detail = format!("`{first}` and `{second}` import each other");
            violations.push(Violation::new(first, line, &[first, second], detail));
        }
    }
    violations
}

fn sibling_modules(edge: &Edge) -> Option<(&[String], &str, &str)> {
    let (from, to) = (&edge.from_module, &edge.defining_module);
    let common = from.iter().zip(to).take_while(|(a, b)| a == b).count();
    let siblings = common > 0 && common < from.len() && common < to.len();
    siblings.then(|| (&from[..common], from[common].as_str(), to[common].as_str()))
}

fn sibling_cycle_violations(tree: &SourceTree, edges: &[Edge]) -> Vec<Violation> {
    let mut parents: BTreeMap<&[String], BTreeMap<String, BTreeSet<String>>> = BTreeMap::new();
    let mut first_edges: BTreeMap<(&[String], &str, &str), &Edge> = BTreeMap::new();
    for edge in edges.iter().filter(|edge| !edge.test) {
        if let Some((parent, from, to)) = sibling_modules(edge) {
            let adjacency = parents.entry(parent).or_default();
            adjacency.entry(from.into()).or_default().insert(to.into());
            first_edges.entry((parent, from, to)).or_insert(edge);
        }
    }
    let mut violations = Vec::new();
    for (parent, adjacency) in &parents {
        for cycle in cycles(adjacency) {
            let members: Vec<&str> = cycle.iter().map(String::as_str).collect();
            let key = format!("{}::{}", parent.join("::"), members.join("<->"));
            let edge = first_edges
                .iter()
                .find(|((owner, from, to), _)| {
                    owner == parent && cycle.contains(*from) && cycle.contains(*to)
                })
                .map(|(_, edge)| *edge);
            let (file, line) = edge.map_or(("lib.rs", 0), |edge| {
                (tree.files[edge.from].path.as_str(), edge.line)
            });
            let detail = format!(
                "sibling modules {} under `{}` import each other",
                members.join(", "),
                parent.join("::")
            );
            violations.push(Violation::new(file, line, &[&key], detail));
        }
    }
    violations
}

fn boolean_stage(module: &[String]) -> Option<&str> {
    let inside = module.len() > BOOLEAN.len() && module.iter().zip(BOOLEAN).all(|(a, b)| a == b);
    inside.then(|| module[BOOLEAN.len()].as_str())
}

fn stage_rank(stage: &str) -> Option<usize> {
    BOOLEAN_ORDER.iter().position(|name| *name == stage)
}

fn boolean_order_violations(tree: &SourceTree, edges: &[Edge]) -> Vec<Violation> {
    let mut violations = Vec::new();
    for file in tree.files.iter().filter(|file| !file.test) {
        if let Some(stage) = boolean_stage(&file.module).filter(|stage| stage_rank(stage).is_none())
        {
            let detail = format!("boolean stage `{stage}` is not in the boolean order");
            violations.push(Violation::new(&file.path, 1, &[&file.path], detail));
        }
    }
    for edge in edges.iter().filter(|edge| !edge.test) {
        let (from, to) = (&tree.files[edge.from], &tree.files[edge.to]);
        let Some(from_rank) = boolean_stage(&from.module).and_then(stage_rank) else {
            continue;
        };
        let inside_boolean =
            to.module.len() >= BOOLEAN.len() && to.module[..BOOLEAN.len()] == BOOLEAN;
        if !inside_boolean {
            continue;
        }
        let to_rank = boolean_stage(&to.module).and_then(stage_rank);
        if to_rank.is_none_or(|rank| rank > from_rank) {
            let stage = boolean_stage(&to.module).unwrap_or("the boolean root");
            let detail = format!(
                "`{}` may not import `{stage}` (`{}`)",
                BOOLEAN_ORDER[from_rank], edge.path
            );
            violations.push(Violation::new(
                &from.path,
                edge.line,
                &[&from.path, &edge.path],
                detail,
            ));
        }
    }
    violations
}

#[test]
fn non_test_imports_follow_the_layer_table() {
    enforce_on_import_edges("layer table", layer_violations, Some(&UPWARD_IMPORTS));
}

#[test]
fn top_level_modules_have_no_import_cycle() {
    enforce_on_import_edges(
        "no cycle between top-level modules",
        cycle_violations,
        Some(&MODULE_CYCLES),
    );
}

#[test]
fn no_two_files_import_each_other() {
    enforce_on_import_edges(
        "no two files import each other",
        mutual_import_violations,
        Some(&MUTUAL_IMPORTS),
    );
}

#[test]
fn no_two_sibling_modules_import_each_other() {
    enforce_on_import_edges(
        "no import cycle between sibling modules",
        sibling_cycle_violations,
        Some(&SIBLING_CYCLES),
    );
}

#[test]
fn test_code_imports_world_graph_and_bindings_only_from_inside_them() {
    enforce_on_import_edges(
        "test code imports L0-L7, and world_graph or bindings only from inside them",
        test_layer_violations,
        None,
    );
}

#[test]
fn boolean_stages_import_only_earlier_stages() {
    enforce_on_import_edges("boolean order types > operands > assembly > handlers > dispatch > batch > multi_tool > shell", boolean_order_violations, None);
}

fn enforce_on_import_edges(
    rule: &str,
    check: fn(&SourceTree, &[Edge]) -> Vec<Violation>,
    spec: Option<&ListSpec>,
) {
    let tree = kernel_sources();
    let list = spec.map(rule_list);
    enforce(rule, &check(&tree, &import_edges(&tree)), list.as_ref());
}
