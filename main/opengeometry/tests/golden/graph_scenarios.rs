use crate::graph_case::{
    child, copy_options, create_options, cuboid, graph_case, on_plane, polyline, primitive,
};
use crate::graph_record::{changes, counters, created, node_state};
use crate::kernel::{CreateOptions, EditScope, Primitive, Transform, WorldGraph};
use crate::record::{Failure, Record};
use crate::runner::Case;

fn unit_box() -> Primitive {
    cuboid(2.0, 2.0, 2.0)
}

fn nodes(record: &mut Record, graph: &WorldGraph, ids: &[&str]) {
    for og_id in ids {
        node_state(record, graph, og_id);
    }
    record.section("counters");
    counters(record, graph);
}

fn sharing(graph: &mut WorldGraph, record: &mut Record) -> Result<(), Failure> {
    primitive(record, graph, unit_box(), create_options("body"));
    created(
        record,
        "instance",
        graph.instance("body", copy_options("instance", None)),
    );
    record.graph_result(
        "ensure node",
        &graph
            .ensure_editable("body", EditScope::Node)
            .map(|shape| shape.revision),
    );
    record.graph_result(
        "ensure all",
        &graph
            .ensure_editable("body", EditScope::AllInstances)
            .map(|shape| shape.revision),
    );
    created(
        record,
        "duplicate",
        graph.duplicate("body", copy_options("duplicate", None)),
    );
    record.graph_result(
        "make unique instance",
        &graph
            .make_unique("instance")
            .map(|changes| changes.is_some()),
    );
    record.graph_result(
        "make unique body",
        &graph.make_unique("body").map(|changes| changes.is_some()),
    );
    nodes(record, graph, &["body", "instance", "duplicate"]);
    Ok(())
}

fn reparenting(graph: &mut WorldGraph, record: &mut Record) -> Result<(), Failure> {
    created(
        record,
        "a",
        graph.create_system_assembly(create_options("a")),
    );
    created(
        record,
        "b",
        graph.create_system_assembly(create_options("b")),
    );
    primitive(record, graph, unit_box(), child("child", "a"));
    changes(
        record,
        "move a",
        graph.transform(
            "a",
            Transform::Translate {
                offset: [10.0, 0.0, 0.0],
            },
        ),
    );
    changes(
        record,
        "to b local",
        graph.add_child("b", &["child".into()], false),
    );
    changes(
        record,
        "to a world",
        graph.add_child("a", &["child".into()], true),
    );
    changes(
        record,
        "remove world",
        graph.remove_child("a", "child", true),
    );
    nodes(record, graph, &["a", "b", "child"]);
    Ok(())
}

fn place_round_trip(graph: &mut WorldGraph, record: &mut Record) -> Result<(), Failure> {
    primitive(record, graph, unit_box(), create_options("body"));
    let empty = Transform::Place {
        origin: None,
        x_direction: None,
        normal: None,
        scale: None,
    };
    changes(record, "place empty", graph.transform("body", empty));
    for (label, transform) in [
        (
            "rotate quarter",
            Transform::Rotate {
                axis: [0.0, 1.0, 0.0],
                degrees: 90.0,
                pivot: Some([0.0; 3]),
            },
        ),
        (
            "rotate skew",
            Transform::Rotate {
                axis: [0.1, 0.2, 0.3],
                degrees: 13.37,
                pivot: None,
            },
        ),
        (
            "scale about pivot",
            Transform::Scale {
                factor: 1.5,
                pivot: Some([1.0, 0.0, 0.0]),
            },
        ),
    ] {
        changes(record, label, graph.transform("body", transform));
        let placement = graph.placement("body")?;
        let place = Transform::Place {
            origin: Some(placement.origin),
            x_direction: Some(placement.x_direction),
            normal: Some(placement.normal),
            scale: Some(placement.scale),
        };
        changes(
            record,
            &format!("{label} place round trip"),
            graph.transform("body", place),
        );
        node_state(record, graph, "body");
    }
    Ok(())
}

fn failed_calls(graph: &mut WorldGraph, record: &mut Record) -> Result<(), Failure> {
    created(
        record,
        "a",
        graph.create_system_assembly(create_options("a")),
    );
    created(record, "b", graph.create_system_assembly(child("b", "a")));
    changes(record, "cycle", graph.add_child("b", &["a".into()], false));
    created(
        record,
        "duplicate id",
        graph.create_system_assembly(create_options("a")),
    );
    changes(
        record,
        "negative scale",
        graph.transform(
            "a",
            Transform::Scale {
                factor: -1.0,
                pivot: None,
            },
        ),
    );
    nodes(record, graph, &["a", "b"]);
    Ok(())
}

fn rejected_primitives(graph: &mut WorldGraph, record: &mut Record) -> Result<(), Failure> {
    let zero = Primitive::Cylinder {
        radius: 0.0,
        height: 2.0,
    };
    created(
        record,
        "zero radius",
        graph.create_primitive(zero, CreateOptions::default()),
    );
    created(
        record,
        "short polyline",
        graph.create_primitive(
            polyline(&[[0.0; 3], [0.0; 3]], false),
            CreateOptions::default(),
        ),
    );
    let huge = Primitive::Cuboid {
        width: 2e6,
        height: 2.0,
        depth: 2.0,
    };
    created(
        record,
        "precision guard",
        graph.create_primitive(huge, CreateOptions::default()),
    );
    let bad_plane = on_plane(
        CreateOptions::default(),
        [0.0; 3],
        [0.0; 3],
        [1.0, 0.0, 0.0],
    );
    created(
        record,
        "singular plane",
        graph.create_primitive(unit_box(), bad_plane),
    );
    let generated = created(
        record,
        "generated",
        graph.create_primitive(unit_box(), CreateOptions::default()),
    );
    if let Some(generated) = generated {
        let node = graph.node(&generated)?.clone();
        changes(record, "dispose generated", graph.dispose(&generated));
        record.graph_result(
            "disposed handle",
            &graph
                .node_by_handle(node.handle, node.generation)
                .map(|node| node.og_id.clone()),
        );
    }
    created(
        record,
        "next generated",
        graph.create_primitive(unit_box(), CreateOptions::default()),
    );
    counters(record, graph);
    Ok(())
}

fn marks(graph: &mut WorldGraph, record: &mut Record) -> Result<(), Failure> {
    primitive(record, graph, unit_box(), create_options("body"));
    let mark = graph.mark()?;
    created(
        record,
        "instance",
        graph.instance("body", copy_options("instance", None)),
    );
    record.graph_result(
        "make unique",
        &graph
            .make_unique("instance")
            .map(|changes| changes.is_some()),
    );
    let later = graph.mark()?;
    changes(record, "dispose body", graph.dispose("body"));
    record.debug("stats before rollback", graph.mark_stats());
    changes(record, "rollback", graph.rollback(mark));
    changes(record, "rollback later", graph.rollback(later));
    created(
        record,
        "after rollback",
        graph.create_primitive(unit_box(), CreateOptions::default()),
    );
    record.graph_result("release", &graph.release(mark));
    nodes(record, graph, &["body", "instance"]);
    Ok(())
}

fn nested_marks(graph: &mut WorldGraph, record: &mut Record) -> Result<(), Failure> {
    let baseline = graph.revision();
    let outer = graph.mark()?;
    primitive(record, graph, unit_box(), create_options("body"));
    let inner = graph.mark()?;
    created(
        record,
        "temporary",
        graph.instance("body", copy_options("temporary", None)),
    );
    record.debug("stats with inner", graph.mark_stats());
    changes(record, "rollback inner", graph.rollback(inner));
    record.graph_result("release inner", &graph.release(inner));
    changes(record, "rollback outer", graph.rollback(outer));
    changes(
        record,
        "changes since baseline",
        graph.changes_since(baseline),
    );
    record.graph_result("release outer", &graph.release(outer));
    nodes(record, graph, &["body", "temporary"]);
    Ok(())
}

fn shared_rebuild(graph: &mut WorldGraph, record: &mut Record) -> Result<(), Failure> {
    primitive(record, graph, unit_box(), create_options("body"));
    created(
        record,
        "instance",
        graph.instance("body", copy_options("instance", None)),
    );
    changes(
        record,
        "rebuild node",
        graph.rebuild_primitive("body", unit_box(), EditScope::Node),
    );
    let mark = graph.mark()?;
    changes(
        record,
        "rebuild wider",
        graph.rebuild_primitive("body", cuboid(3.0, 2.0, 2.0), EditScope::AllInstances),
    );
    changes(record, "rollback", graph.rollback(mark));
    changes(
        record,
        "rebuild widest",
        graph.rebuild_primitive("body", cuboid(4.0, 2.0, 2.0), EditScope::AllInstances),
    );
    let rectangle = Primitive::Rectangle {
        width: 2.0,
        breadth: 2.0,
    };
    changes(
        record,
        "rebuild type mismatch",
        graph.rebuild_primitive("body", rectangle, EditScope::AllInstances),
    );
    record.graph_result("release", &graph.release(mark));
    nodes(record, graph, &["body", "instance"]);
    Ok(())
}

fn subtree_dispose(graph: &mut WorldGraph, record: &mut Record) -> Result<(), Failure> {
    created(
        record,
        "parent",
        graph.create_system_assembly(create_options("parent")),
    );
    let mark = graph.mark()?;
    let options = CreateOptions {
        parent: Some("parent".into()),
        ..CreateOptions::default()
    };
    created(
        record,
        "generated child",
        graph.create_primitive(unit_box(), options),
    );
    changes(record, "dispose parent", graph.dispose("parent"));
    changes(record, "rollback", graph.rollback(mark));
    created(
        record,
        "next generated",
        graph.create_primitive(unit_box(), CreateOptions::default()),
    );
    record.graph_result("release", &graph.release(mark));
    nodes(record, graph, &["parent"]);
    Ok(())
}

fn copies_and_planes(graph: &mut WorldGraph, record: &mut Record) -> Result<(), Failure> {
    created(
        record,
        "a",
        graph.create_system_assembly(create_options("a")),
    );
    created(
        record,
        "b",
        graph.create_system_assembly(create_options("b")),
    );
    changes(
        record,
        "move a",
        graph.transform(
            "a",
            Transform::Translate {
                offset: [7.0, 0.0, 0.0],
            },
        ),
    );
    changes(
        record,
        "move b",
        graph.transform(
            "b",
            Transform::Translate {
                offset: [-3.0, 0.0, 0.0],
            },
        ),
    );
    primitive(record, graph, unit_box(), child("body", "a"));
    created(
        record,
        "instance under b",
        graph.instance("body", copy_options("copy", Some(Some("b")))),
    );
    created(
        record,
        "duplicate at root",
        graph.duplicate("body", copy_options("root-copy", Some(None))),
    );
    let planned = on_plane(
        child("planned", "a"),
        [4.0, 5.0, 6.0],
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 0.0],
    );
    primitive(record, graph, unit_box(), planned);
    record.graph_result(
        "relative copy to body",
        &graph.relative_placement("copy", "body"),
    );
    nodes(
        record,
        graph,
        &["a", "b", "body", "copy", "root-copy", "planned"],
    );
    Ok(())
}

fn many_rotations(graph: &mut WorldGraph, record: &mut Record) -> Result<(), Failure> {
    created(
        record,
        "pivot",
        graph.create_system_assembly(create_options("pivot")),
    );
    let rotate = Transform::Rotate {
        axis: [0.1, 0.2, 0.3],
        degrees: 0.001,
        pivot: None,
    };
    let mut failures = 0_usize;
    for _ in 0..100_000 {
        if graph.transform("pivot", rotate).is_err() {
            failures += 1;
        }
    }
    record.section("rotations");
    record.field("failures", failures);
    nodes(record, graph, &["pivot"]);
    Ok(())
}

pub(crate) fn cases() -> Vec<Case> {
    vec![
        graph_case("graph.scenario.instances-and-duplicates", sharing),
        graph_case("graph.scenario.reparenting", reparenting),
        graph_case("graph.scenario.place-round-trip", place_round_trip),
        graph_case("graph.scenario.failed-calls", failed_calls),
        graph_case("graph.scenario.rejected-primitives", rejected_primitives),
        graph_case("graph.scenario.marks", marks),
        graph_case("graph.scenario.nested-marks", nested_marks),
        graph_case("graph.scenario.shared-rebuild", shared_rebuild),
        graph_case("graph.scenario.subtree-dispose", subtree_dispose),
        graph_case("graph.scenario.copies-and-planes", copies_and_planes),
        graph_case("graph.scenario.many-rotations", many_rotations),
    ]
}
