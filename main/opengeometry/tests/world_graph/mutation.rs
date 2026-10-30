use super::support::{box_primitive, copy_named};
use opengeometry::world_graph::{
    ChangeSet, CreateOptions, EditScope, GraphError, Node, Primitive, Transform, WorldGraph,
};
use opengeometry_test_support::world_graph::{graph, named};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

#[derive(Clone, Debug, PartialEq)]
struct Entry {
    node: Node,
    revision: Option<u64>,
}

type State = BTreeMap<String, Entry>;

struct Script {
    world: WorldGraph,
    ids: BTreeSet<String>,
    parents: BTreeMap<String, String>,
}

#[derive(Default)]
struct Lists {
    added: Vec<Value>,
    changed: Vec<Value>,
    removed: Vec<Value>,
}

impl Lists {
    fn holds(&self, id: &str) -> bool {
        [&self.added, &self.changed, &self.removed]
            .iter()
            .any(|list| list.iter().any(|item| item["ogId"] == id))
    }

    fn add_node_diff(&mut self, id: &str, before: &State, after: &State) -> bool {
        let old = before.get(id);
        let new = after.get(id);
        if old.map(|entry| &entry.node) == new.map(|entry| &entry.node) {
            return false;
        }
        match (old, new) {
            (None, Some(entry)) => self.added.push(change(entry)),
            (Some(entry), None) => self.removed.push(change(entry)),
            (Some(previous), Some(entry)) if previous.node.generation != entry.node.generation => {
                self.removed.push(change(previous));
                self.added.push(change(entry));
            }
            (Some(_), Some(entry)) => self.changed.push(change(entry)),
            (None, None) => {}
        }
        true
    }

    fn set(self, revision: u64) -> Value {
        json!({
            "revision": revision,
            "added": self.added,
            "changed": self.changed,
            "removed": self.removed,
        })
    }
}

fn change(entry: &Entry) -> Value {
    json!({
        "ogId": entry.node.og_id,
        "handle": entry.node.handle,
        "generation": entry.node.generation,
        "shapeId": entry.node.shape,
        "shapeRevision": entry.revision,
    })
}

fn shape_revisions(state: &State) -> BTreeMap<String, u64> {
    state
        .values()
        .filter_map(|entry| Some((entry.node.shape.clone()?, entry.revision?)))
        .collect()
}

fn moved_shapes(before: &State, after: &State) -> Vec<String> {
    let old = shape_revisions(before);
    let new = shape_revisions(after);
    old.keys()
        .chain(new.keys())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|id| old.get(*id) != new.get(*id))
        .cloned()
        .collect()
}

fn users<'a>(state: &'a State, shape: &'a str) -> impl Iterator<Item = &'a Entry> {
    state
        .values()
        .filter(move |entry| entry.node.shape.as_deref() == Some(shape))
}

fn keys(before: &State, after: &State) -> BTreeSet<String> {
    before.keys().chain(after.keys()).cloned().collect()
}

fn committed(before: &State, after: &State, affected: &BTreeSet<String>, revision: u64) -> Value {
    let mut lists = Lists::default();
    for id in keys(before, after) {
        if !lists.add_node_diff(&id, before, after) && affected.contains(&id) {
            if let Some(entry) = after.get(&id) {
                lists.changed.push(change(entry));
            }
        }
    }
    let new = shape_revisions(after);
    for shape in moved_shapes(before, after) {
        if !new.contains_key(&shape) {
            continue;
        }
        for entry in users(after, &shape) {
            let id = &entry.node.og_id;
            let listed = [&lists.added, &lists.changed]
                .iter()
                .any(|list| list.iter().any(|item| item["ogId"] == **id));
            if !listed {
                lists.changed.push(change(entry));
            }
        }
    }
    lists.set(revision)
}

impl Script {
    fn new(live: usize) -> Self {
        let mut script = Self {
            world: graph(),
            ids: BTreeSet::new(),
            parents: BTreeMap::new(),
        };
        for index in 0..live {
            let id = format!("fill-{index}");
            script
                .world
                .create_primitive(box_primitive(), named(&id))
                .unwrap();
            script.world.world_placement(&id).unwrap();
            script.ids.insert(id);
        }
        script
    }

    fn state(&self) -> State {
        self.ids
            .iter()
            .filter_map(|id| {
                let node = self.world.node(id).ok()?.clone();
                let revision = self.world.shape(id).ok().map(|shape| shape.revision);
                Some((id.clone(), Entry { node, revision }))
            })
            .collect()
    }

    fn subtree(&self, state: &State, root: &str) -> BTreeSet<String> {
        state
            .keys()
            .filter(|id| {
                let mut cursor = Some(id.as_str());
                while let Some(current) = cursor {
                    if current == root {
                        return true;
                    }
                    cursor = self.parents.get(current).map(String::as_str);
                }
                false
            })
            .cloned()
            .collect()
    }

    fn rolled_back(&self, before: &State, after: &State, revision: u64) -> Value {
        let mut lists = Lists::default();
        let mut affected = BTreeSet::new();
        for id in keys(before, after) {
            if lists.add_node_diff(&id, before, after) {
                affected.extend(self.subtree(before, &id));
                affected.extend(self.subtree(after, &id));
            }
        }
        for shape in moved_shapes(before, after) {
            affected.extend(users(after, &shape).map(|entry| entry.node.og_id.clone()));
        }
        for id in affected {
            if !lists.holds(&id) {
                if let Some(entry) = after.get(&id) {
                    lists.changed.push(change(entry));
                }
            }
        }
        lists.set(revision)
    }

    fn step(&mut self, affected: &[&str], run: impl FnOnce(&mut WorldGraph) -> ChangeSet) {
        let before = self.state();
        let revision = self.world.revision();
        let actual = run(&mut self.world);
        let after = self.state();
        let mut touched = BTreeSet::new();
        for id in affected {
            touched.extend(self.subtree(&before, id));
            touched.insert(id.to_string());
        }
        let expected = committed(&before, &after, &touched, revision + 1);
        assert_eq!(serde_json::to_value(&actual).unwrap(), expected);
        assert_eq!(self.world.revision(), revision + 1);
    }

    fn create(&mut self, id: &str, parent: Option<&str>) {
        let options = CreateOptions {
            parent: parent.map(str::to_string),
            ..named(id)
        };
        self.ids.insert(id.into());
        if let Some(parent) = parent {
            self.parents.insert(id.into(), parent.into());
        }
        self.step(&[id], |world| {
            world.create_primitive(box_primitive(), options).unwrap().1
        });
    }

    fn assert_failure_leaves_graph(
        &mut self,
        expected: &str,
        run: impl FnOnce(&mut WorldGraph) -> GraphError,
    ) {
        let before = self.state();
        let revision = self.world.revision();
        let counts = (self.world.node_count(), self.world.shape_count());
        let message = run(&mut self.world).message();
        assert!(message.contains(expected), "{message}");
        assert_eq!(self.state(), before);
        assert_eq!(self.world.revision(), revision);
        assert_eq!((self.world.node_count(), self.world.shape_count()), counts);
        let since = self.world.changes_since(revision).unwrap();
        assert!(since.added.is_empty() && since.changed.is_empty() && since.removed.is_empty());
    }

    fn fail_three_ways(&mut self) {
        self.assert_failure_leaves_graph("scale", |world| {
            let factor = Transform::Scale {
                factor: 0.0,
                pivot: None,
            };
            world.transform("a", factor).unwrap_err()
        });
        self.assert_failure_leaves_graph("missing", |world| {
            let children = ["c".to_string(), "missing".to_string()];
            world.add_child("group", &children, false).unwrap_err()
        });
        self.assert_failure_leaves_graph("precision extent", |world| {
            let huge = Primitive::Cuboid {
                width: 1.0e9,
                height: 1.0,
                depth: 1.0,
            };
            world
                .create_primitive(huge, CreateOptions::default())
                .unwrap_err()
        });
    }
}

fn wider(width: f64) -> Primitive {
    Primitive::Cuboid {
        width,
        height: 2.0,
        depth: 2.0,
    }
}

fn shift(x: f64) -> Transform {
    Transform::Translate {
        offset: [x, 0.0, 0.0],
    }
}

fn run_mixed_script(live: usize) {
    let mut script = Script::new(live);
    script.ids.insert("group".into());
    script.step(&["group"], |world| {
        world.create_system_assembly(named("group")).unwrap().1
    });
    script.create("a", Some("group"));
    script.create("b", Some("group"));
    script.ids.insert("a-copy".into());
    script.parents.insert("a-copy".into(), "group".into());
    script.step(&["a-copy"], |world| {
        world.instance("a", copy_named("a-copy")).unwrap().1
    });
    script.step(&["group"], |world| {
        world.transform("group", shift(1.0)).unwrap()
    });
    script.step(&[], |world| {
        world
            .rebuild_primitive("a", wider(3.0), EditScope::AllInstances)
            .unwrap()
    });
    let marked = script.state();
    let mark = script.world.mark().unwrap();
    script.step(&["b"], |world| world.dispose("b").unwrap());
    script.step(&["a-copy"], |world| {
        world.transform("a-copy", shift(2.0)).unwrap()
    });
    script.create("c", None);
    script.step(&[], |world| {
        world
            .rebuild_primitive("c", wider(4.0), EditScope::Node)
            .unwrap()
    });
    script.fail_three_ways();
    script.step(&["group"], |world| world.dispose("group").unwrap());
    if live > 0 {
        script.step(&["fill-0"], |world| world.dispose("fill-0").unwrap());
    }
    let before = script.state();
    let revision = script.world.revision();
    let actual = script.world.rollback(mark).unwrap();
    let after = script.state();
    assert_eq!(after, marked);
    let expected = script.rolled_back(&before, &after, revision + 1);
    assert_eq!(serde_json::to_value(&actual).unwrap(), expected);
    script.create("d", Some("group"));
}

#[test]
fn mixed_mutations_report_the_full_map_difference_at_every_live_count() {
    for live in [0, 10, 1_000] {
        run_mixed_script(live);
    }
}

#[test]
fn failed_mutations_leave_a_graph_of_a_thousand_bodies_unchanged() {
    let mut script = Script::new(1_000);
    script
        .ids
        .extend(["group", "a", "a-copy", "c"].map(String::from));
    script.world.create_system_assembly(named("group")).unwrap();
    script
        .world
        .create_primitive(
            box_primitive(),
            CreateOptions {
                parent: Some("group".into()),
                ..named("a")
            },
        )
        .unwrap();
    script.world.instance("a", copy_named("a-copy")).unwrap();
    script
        .world
        .create_primitive(box_primitive(), named("c"))
        .unwrap();
    let (probe, _) = script
        .world
        .create_primitive(box_primitive(), CreateOptions::default())
        .unwrap();
    let probed = script.world.node(&probe).unwrap().clone();
    script.world.dispose(&probe).unwrap();
    script.fail_three_ways();
    let (next, _) = script
        .world
        .create_primitive(box_primitive(), CreateOptions::default())
        .unwrap();
    let node = script.world.node(&next).unwrap();
    assert_ne!(next, probe);
    assert_eq!(node.handle, probed.handle + 1);
    assert_eq!(node.generation, probed.generation + 1);
}

fn round_seconds(script: &mut Script) -> f64 {
    let start = Instant::now();
    for _ in 0..20 {
        let (id, _) = script
            .world
            .create_primitive(box_primitive(), CreateOptions::default())
            .unwrap();
        script.world.transform(&id, shift(1.0)).unwrap();
        script.world.world_placement(&id).unwrap();
        script.world.dispose(&id).unwrap();
    }
    start.elapsed().as_secs_f64()
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

#[test]
fn one_mutation_costs_the_same_with_a_hundred_or_four_thousand_live_bodies() {
    let mut small = Script::new(100);
    let mut large = Script::new(4_000);
    let mut small_rounds = Vec::new();
    let mut large_rounds = Vec::new();
    for _ in 0..5 {
        small_rounds.push(round_seconds(&mut small));
        large_rounds.push(round_seconds(&mut large));
    }
    let small_time = median(small_rounds);
    let large_time = median(large_rounds);
    assert!(
        large_time <= 3.0 * small_time,
        "create, transform and dispose: {large_time:.4} s per round at 4,000 live, {small_time:.4} s at 100"
    );
}

const ADDED: usize = 0;
const CHANGED: usize = 1;
const REMOVED: usize = 2;

fn compacted(sets: &[Value], since: u64, revision: u64) -> Value {
    let mut entries: BTreeMap<(String, u64, u64), (usize, Value)> = BTreeMap::new();
    for set in sets
        .iter()
        .filter(|set| set["revision"].as_u64().unwrap() > since)
    {
        for (kind, name) in ["added", "changed", "removed"].into_iter().enumerate() {
            for entry in set[name].as_array().unwrap() {
                let key = (
                    entry["ogId"].as_str().unwrap().to_string(),
                    entry["handle"].as_u64().unwrap(),
                    entry["generation"].as_u64().unwrap(),
                );
                match (entries.get(&key).map(|(kind, _)| *kind), kind) {
                    (Some(ADDED), REMOVED) => {
                        entries.remove(&key);
                    }
                    (Some(ADDED), CHANGED) => {
                        entries.insert(key, (ADDED, entry.clone()));
                    }
                    (Some(REMOVED), ADDED) => {
                        entries.insert(key, (CHANGED, entry.clone()));
                    }
                    (Some(CHANGED), REMOVED) => {
                        entries.insert(key, (REMOVED, entry.clone()));
                    }
                    _ => {
                        entries.insert(key, (kind, entry.clone()));
                    }
                }
            }
        }
    }
    let mut lists = [Vec::new(), Vec::new(), Vec::new()];
    for (kind, entry) in entries.into_values() {
        lists[kind].push(entry);
    }
    let [added, changed, removed] = lists;
    json!({ "revision": revision, "added": added, "changed": changed, "removed": removed })
}

#[test]
fn changes_since_an_old_or_a_recent_revision_compacts_exactly_the_later_change_sets() {
    let mut world = graph();
    let mut sets = Vec::new();
    let mut record = |set: ChangeSet| sets.push(serde_json::to_value(set).unwrap());
    for index in 0..30 {
        record(
            world
                .create_primitive(box_primitive(), named(&format!("body-{index}")))
                .unwrap()
                .1,
        );
    }
    let mark = world.mark().unwrap();
    record(world.transform("body-3", shift(1.0)).unwrap());
    record(world.dispose("body-4").unwrap());
    record(
        world
            .create_primitive(box_primitive(), named("body-4"))
            .unwrap()
            .1,
    );
    record(
        world
            .rebuild_primitive("body-5", wider(3.0), EditScope::Node)
            .unwrap(),
    );
    record(world.rollback(mark).unwrap());
    record(world.transform("body-6", shift(2.0)).unwrap());
    record(world.dispose("body-7").unwrap());
    let current = world.revision();
    for since in [0, 1, 12, current - 4, current - 1, current] {
        let actual = serde_json::to_value(world.changes_since(since).unwrap()).unwrap();
        assert_eq!(actual, compacted(&sets, since, current), "since {since}");
    }
}
