use crate::graph_case::{
    child, circle, copy_options, create_options, cuboid, extrude, graph_case, inspect, on_plane,
    operation, polyline, primitive, rectangle, sweep,
};
use crate::graph_record::{changes, created, export, record_operation};
use crate::graph_state::{state, Tracked};
use crate::kernel::{
    ChangeSet, CreateOptions, EditScope, GraphError, ModifyingOperation, StepOptions, Transform,
    WorldGraph,
};
use crate::record::{Failure, Record};
use crate::runner::Case;

struct Scene {
    level: String,
    profile: String,
    wall: String,
    cutter: String,
    rail: String,
}

fn unique(record: &mut Record, label: &str, result: Result<Option<ChangeSet>, GraphError>) {
    match result {
        Ok(Some(changeset)) => {
            changes(record, label, Ok(changeset));
        }
        Ok(None) => record.block(&format!("step {label}"), "unchanged"),
        Err(error) => record.graph_error(&format!("step {label}"), &error),
    }
}

fn subtract(
    record: &mut Record,
    graph: &mut WorldGraph,
    label: &str,
    target: &str,
    tools: &[&str],
) {
    let subtract = ModifyingOperation::Subtract;
    record_operation(
        record,
        graph,
        label,
        target,
        subtract,
        tools,
        EditScope::Node,
    );
}

fn build_scene(record: &mut Record, graph: &mut WorldGraph, tracked: &mut Tracked) -> Scene {
    let level = tracked.add(
        created(
            record,
            "level",
            graph.create_system_assembly(create_options("level-1")),
        ),
        "level-1",
    );
    let profile = tracked.add(
        primitive(record, graph, rectangle(6.0, 0.2), CreateOptions::default()),
        "profile",
    );
    let wall = tracked.add(
        operation(
            record,
            graph,
            extrude(&profile, &[], 3.0),
            create_options("wall-1"),
        ),
        "wall-1",
    );
    let cutter = tracked.add(
        primitive(
            record,
            graph,
            cuboid(0.9, 2.1, 0.4),
            create_options("door-cutter"),
        ),
        "door-cutter",
    );
    changes(
        record,
        "translate cutter",
        graph.transform(
            &cutter,
            Transform::Translate {
                offset: [2.0, 0.0, 0.0],
            },
        ),
    );
    subtract(record, graph, "door cut", &wall, &[&cutter]);
    let path_points = [[0.0, 1.0, 1.0], [4.0, 1.0, 1.0], [4.0, 1.0, 4.0]];
    let path = tracked.add(
        primitive(
            record,
            graph,
            polyline(&path_points, false),
            CreateOptions::default(),
        ),
        "path",
    );
    let disc_options = on_plane(
        CreateOptions::default(),
        [0.0, 1.0, 1.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
    );
    let disc = tracked.add(primitive(record, graph, circle(0.05), disc_options), "disc");
    let rail = tracked.add(
        operation(record, graph, sweep(&disc, &path), create_options("rail-1")),
        "rail-1",
    );
    let members = [&profile, &wall, &cutter, &path, &disc, &rail].map(|id| id.to_string());
    changes(
        record,
        "add members",
        graph.add_child(&level, &members, false),
    );
    tracked.remember(graph);
    Scene {
        level,
        profile,
        wall,
        cutter,
        rail,
    }
}

fn edit_scene(record: &mut Record, graph: &mut WorldGraph, scene: &Scene) {
    let place = Transform::Place {
        origin: Some([0.0, 3.0, 0.0]),
        x_direction: None,
        normal: None,
        scale: None,
    };
    changes(record, "place level", graph.transform(&scene.level, place));
    let rotate = Transform::Rotate {
        axis: [0.0, 1.0, 0.0],
        degrees: 90.0,
        pivot: None,
    };
    changes(record, "rotate rail", graph.transform(&scene.rail, rotate));
    changes(
        record,
        "rebuild wall",
        graph.rebuild_operation(
            &scene.wall,
            extrude(&scene.profile, &[], 4.0),
            EditScope::Node,
        ),
    );
    subtract(record, graph, "re-cut", &scene.wall, &[&scene.cutter]);
    subtract(
        record,
        graph,
        "rail minus wall",
        &scene.rail,
        &[&scene.wall],
    );
}

fn trial_edit(record: &mut Record, graph: &mut WorldGraph, scene: &Scene) {
    let mark = graph.mark();
    record.section("step mark");
    record.graph_result("mark", &mark);
    record.debug("mark_stats", graph.mark_stats());
    changes(
        record,
        "trial rebuild",
        graph.rebuild_operation(
            &scene.wall,
            extrude(&scene.profile, &[], 5.0),
            EditScope::Node,
        ),
    );
    record.debug("mark_stats after trial", graph.mark_stats());
    if let Ok(mark) = mark {
        changes(record, "rollback trial", graph.rollback(mark));
        record.graph_result("release trial", &graph.release(mark));
        record.graph_result("release again", &graph.release(mark));
    }
    record.debug("mark_stats after release", graph.mark_stats());
}

fn instances(record: &mut Record, graph: &mut WorldGraph, tracked: &mut Tracked, scene: &Scene) {
    let rail_copy = tracked.add(
        created(
            record,
            "instance rail",
            graph.instance(&scene.rail, copy_options("rail-2", None)),
        ),
        "rail-2",
    );
    changes(
        record,
        "translate rail copy",
        graph.transform(
            &rail_copy,
            Transform::Translate {
                offset: [0.0, 0.0, 2.0],
            },
        ),
    );
    for index in 0..10 {
        let og_id = format!("wall-copy-{index}");
        let result = graph.instance(&scene.wall, copy_options(&og_id, Some(Some(&scene.level))));
        let og_id = tracked.add(
            created(record, &format!("instance wall {index}"), result),
            &og_id,
        );
        let offset = [index as f64 * 1.5, 0.0, 6.0];
        changes(
            record,
            &format!("translate wall copy {index}"),
            graph.transform(&og_id, Transform::Translate { offset }),
        );
    }
    unique(
        record,
        "make unique wall copy 1",
        graph.make_unique("wall-copy-1"),
    );
    unique(record, "make unique rail 1", graph.make_unique(&scene.rail));
    tracked.add(
        created(
            record,
            "duplicate rail",
            graph.duplicate(&scene.rail, copy_options("rail-3", None)),
        ),
        "rail-3",
    );
    tracked.remember(graph);
}

fn reparent(record: &mut Record, graph: &mut WorldGraph, tracked: &mut Tracked) {
    let level = tracked.add(
        created(
            record,
            "second level",
            graph.create_system_assembly(create_options("level-2")),
        ),
        "level-2",
    );
    let place = Transform::Place {
        origin: Some([10.0, 0.0, 0.0]),
        x_direction: Some([0.0, 0.0, -1.0]),
        normal: Some([0.0, 1.0, 0.0]),
        scale: None,
    };
    changes(record, "place level-2", graph.transform(&level, place));
    changes(
        record,
        "add rail copy keep world",
        graph.add_child(&level, &["rail-2".into()], true),
    );
    changes(
        record,
        "add wall copy keep local",
        graph.add_child(&level, &["wall-copy-0".into()], false),
    );
    changes(
        record,
        "remove rail copy keep world",
        graph.remove_child(&level, "rail-2", true),
    );
    changes(
        record,
        "remove rail copy again",
        graph.remove_child(&level, "rail-2", true),
    );
    changes(
        record,
        "cycle rejected",
        graph.add_child("wall-copy-0", &[level.clone()], false),
    );
    let nested = graph.create_primitive(cuboid(0.5, 0.5, 0.5), child("nested", &level));
    tracked.add(created(record, "nested", nested), "nested");
    tracked.remember(graph);
}

fn dispose_and_reuse(
    record: &mut Record,
    graph: &mut WorldGraph,
    tracked: &mut Tracked,
    scene: &Scene,
) {
    changes(record, "dispose cutter", graph.dispose(&scene.cutter));
    tracked.remember(graph);
    let mark = graph.mark();
    record.section("step reuse mark");
    record.graph_result("mark", &mark);
    changes(record, "dispose wall copy 9", graph.dispose("wall-copy-9"));
    created(
        record,
        "reuse wall copy 9",
        graph.create_primitive(cuboid(1.0, 1.0, 1.0), create_options("wall-copy-9")),
    );
    tracked.remember(graph);
    state(record, graph, tracked, "after reuse");
    if let Ok(mark) = mark {
        changes(record, "rollback reuse", graph.rollback(mark));
        record.graph_result("release reuse", &graph.release(mark));
    }
    tracked.remember(graph);
}

fn exports(record: &mut Record, graph: &WorldGraph, scene: &Scene) {
    let level = vec![scene.level.clone()];
    let metre_y = StepOptions {
        unit: "metre".into(),
        up_axis: "Y".into(),
        ..StepOptions::default()
    };
    export(
        record,
        graph,
        "level default",
        &level,
        &StepOptions::default(),
    );
    export(record, graph, "level metre y", &level, &metre_y);
    export(
        record,
        graph,
        "bodies",
        &[scene.wall.clone(), "rail-2".into(), "rail-3".into()],
        &metre_y,
    );
    export(
        record,
        graph,
        "wire rejected",
        &[scene.profile.clone()],
        &metre_y,
    );
    export(record, graph, "nothing selected", &[], &metre_y);
    let invalid = StepOptions {
        timestamp: "1970-02-30T00:00:00".into(),
        ..StepOptions::default()
    };
    export(record, graph, "invalid timestamp", &level, &invalid);
}

fn acceptance(graph: &mut WorldGraph, record: &mut Record) -> Result<(), Failure> {
    let mut tracked = Tracked::new();
    let scene = build_scene(record, graph, &mut tracked);
    state(record, graph, &tracked, "after door cut and rail");
    edit_scene(record, graph, &scene);
    tracked.remember(graph);
    state(record, graph, &tracked, "after placement and re-cut");
    trial_edit(record, graph, &scene);
    state(record, graph, &tracked, "after trial rollback");
    instances(record, graph, &mut tracked, &scene);
    reparent(record, graph, &mut tracked);
    state(record, graph, &tracked, "after instances and reparenting");
    dispose_and_reuse(record, graph, &mut tracked, &scene);
    state(record, graph, &tracked, "after dispose and rollback");
    exports(record, graph, &scene);
    for og_id in [scene.wall.as_str(), scene.rail.as_str(), "rail-2"] {
        inspect(record, graph, og_id)?;
    }
    Ok(())
}

pub(crate) fn cases() -> Vec<Case> {
    vec![graph_case("graph.acceptance-session", acceptance)]
}
