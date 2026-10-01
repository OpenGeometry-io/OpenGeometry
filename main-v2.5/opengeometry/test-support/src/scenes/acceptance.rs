use crate::world_graph::{graph, named};
use opengeometry::exchange::StepExportReport;
use opengeometry::world_graph::{
    CopyOptions, CreateOptions, CreatingOperation, EditScope, ErrorCode, GraphError,
    ModifyingOperation, Plane, Primitive, StepOptions, Transform, WorldGraph,
};

pub struct AcceptanceScene {
    pub graph: WorldGraph,
    pub level: String,
    pub wall: String,
    pub rail: String,
    pub rails: Vec<String>,
}

pub fn recut_scene() -> AcceptanceScene {
    let mut graph = graph();
    let (level, _) = graph.create_system_assembly(named("level-1")).unwrap();
    let wall = build_wall(&mut graph);
    let rail = build_rail(&mut graph);
    let members = [
        "wall-profile",
        "wall-1",
        "door-cutter",
        "rail-path",
        "rail-disc",
        "rail-1",
    ]
    .map(String::from);
    graph.add_child(&level, &members, false).unwrap();
    let mut scene = AcceptanceScene {
        graph,
        level,
        wall,
        rails: vec![rail.clone()],
        rail,
    };
    place_and_recut(&mut scene);
    scene
}

pub fn acceptance_scene() -> AcceptanceScene {
    let mut scene = recut_scene();
    let _ = attempt_rail_minus_wall(&mut scene);
    trial_rebuild(&mut scene);
    add_rail_instances(&mut scene);
    reparent_test_child(&mut scene.graph);
    scene.graph.dispose("reparent-test").unwrap();
    scene.graph.dispose("door-cutter").unwrap();
    scene
}

pub fn level_export(
    scene: &AcceptanceScene,
    unit: &str,
    up_axis: &str,
) -> (String, StepExportReport) {
    let options = StepOptions {
        unit: unit.into(),
        up_axis: up_axis.into(),
        ..StepOptions::default()
    };
    scene
        .graph
        .export_step(&[scene.level.clone()], &options)
        .unwrap()
}

fn build_wall(graph: &mut WorldGraph) -> String {
    let wall = create_wall(graph, "wall-profile", "wall-1");
    let cutter = create_cutter(graph, "door-cutter", [0.9, 2.1, 0.4], 2.0);
    subtract(graph, &wall, &[cutter]);
    wall
}

fn build_rail(graph: &mut WorldGraph) -> String {
    let points = vec![[0.0, 1.0, 1.0], [4.0, 1.0, 1.0], [4.0, 1.0, 4.0]];
    let path = Primitive::Polyline {
        points,
        closed: false,
    };
    let (path, _) = graph.create_primitive(path, named("rail-path")).unwrap();
    let plane = Plane {
        origin: Some([0.0, 1.0, 1.0]),
        normal: Some([1.0, 0.0, 0.0]),
        x_direction: Some([0.0, 0.0, 1.0]),
    };
    let disc = CreateOptions {
        plane: Some(plane),
        ..named("rail-disc")
    };
    let (profile, _) = graph
        .create_primitive(Primitive::Circle { radius: 0.05 }, disc)
        .unwrap();
    let sweep = CreatingOperation::Sweep { profile, path };
    graph.create_operation(sweep, named("rail-1")).unwrap().0
}

fn place_and_recut(scene: &mut AcceptanceScene) {
    let graph = &mut scene.graph;
    let place = Transform::Place {
        origin: Some([0.0, 3.0, 0.0]),
        x_direction: None,
        normal: None,
        scale: None,
    };
    graph.transform(&scene.level, place).unwrap();
    let rotate = Transform::Rotate {
        axis: [0.0, 1.0, 0.0],
        degrees: 90.0,
        pivot: None,
    };
    graph.transform(&scene.rail, rotate).unwrap();
    let rebuilt = wall_extrusion("wall-profile", 4.0);
    graph
        .rebuild_operation(&scene.wall, rebuilt, EditScope::Node)
        .unwrap();
    subtract(graph, &scene.wall, &["door-cutter".into()]);
}

pub fn attempt_rail_minus_wall(scene: &mut AcceptanceScene) -> GraphError {
    let error = scene
        .graph
        .operate(
            &scene.rail,
            ModifyingOperation::Subtract,
            &[scene.wall.clone()],
            EditScope::Node,
        )
        .unwrap_err();
    assert_eq!(error.error_code(), ErrorCode::CoverageGap, "{error:?}");
    error
}

fn trial_rebuild(scene: &mut AcceptanceScene) {
    let graph = &mut scene.graph;
    let mark = graph.mark().unwrap();
    let trial = wall_extrusion("wall-profile", 5.0);
    graph
        .rebuild_operation(&scene.wall, trial, EditScope::Node)
        .unwrap();
    graph.rollback(mark).unwrap();
    graph.release(mark).unwrap();
}

fn add_rail_instances(scene: &mut AcceptanceScene) {
    for index in 2..=11 {
        let og_id = format!("rail-{index}");
        let offset = [0.0, 0.0, f64::from(index) * 2.0];
        let copy = instance_rail(&mut scene.graph, &scene.rail, &og_id, offset);
        scene.rails.push(copy);
    }
}

pub(super) fn instance_rail(
    graph: &mut WorldGraph,
    rail: &str,
    og_id: &str,
    offset: [f64; 3],
) -> String {
    let options = CopyOptions {
        og_id: Some(og_id.into()),
        parent: None,
    };
    let (copy, _) = graph.instance(rail, options).unwrap();
    graph
        .transform(&copy, Transform::Translate { offset })
        .unwrap();
    copy
}

fn reparent_test_child(graph: &mut WorldGraph) {
    let (reparent, _) = graph.create_system_assembly(named("reparent")).unwrap();
    let child = vec![create_cuboid(graph, "reparent-test", [0.1, 0.1, 0.1])];
    let offset = [10.0, 0.0, 0.0];
    graph
        .transform(&reparent, Transform::Translate { offset })
        .unwrap();
    graph.add_child(&reparent, &child, true).unwrap();
    graph.remove_child(&reparent, &child[0], true).unwrap();
    graph.add_child(&reparent, &child, false).unwrap();
}

pub(super) fn create_wall(graph: &mut WorldGraph, profile: &str, wall: &str) -> String {
    let rectangle = Primitive::Rectangle {
        width: 6.0,
        breadth: 0.2,
    };
    graph.create_primitive(rectangle, named(profile)).unwrap();
    let extrusion = wall_extrusion(profile, 3.0);
    graph.create_operation(extrusion, named(wall)).unwrap().0
}

pub(super) fn create_cutter(graph: &mut WorldGraph, og_id: &str, size: [f64; 3], x: f64) -> String {
    let cutter = create_cuboid(graph, og_id, size);
    let offset = [x, 0.0, 0.0];
    graph
        .transform(&cutter, Transform::Translate { offset })
        .unwrap();
    cutter
}

fn create_cuboid(graph: &mut WorldGraph, og_id: &str, size: [f64; 3]) -> String {
    let cuboid = Primitive::Cuboid {
        width: size[0],
        height: size[1],
        depth: size[2],
    };
    graph.create_primitive(cuboid, named(og_id)).unwrap().0
}

pub(super) fn subtract(graph: &mut WorldGraph, target: &str, tools: &[String]) {
    graph
        .operate(target, ModifyingOperation::Subtract, tools, EditScope::Node)
        .unwrap();
}

fn wall_extrusion(profile: &str, distance: f64) -> CreatingOperation {
    CreatingOperation::Extrude {
        profile: profile.into(),
        holes: Vec::new(),
        distance,
    }
}
