use crate::body::record_body;
use crate::fixtures::standard;
use crate::graph_record::{counters, created, node_state};
use crate::kernel::{
    CopyOptions, CreateOptions, CreatingOperation, Plane, Point3, Primitive, WorldGraph,
};
use crate::record::{Failure, Record};
use crate::runner::Case;

pub(crate) trait Script:
    Fn(&mut WorldGraph, &mut Record) -> Result<(), Failure> + Send + Sync + 'static
{
}

impl<T> Script for T where
    T: Fn(&mut WorldGraph, &mut Record) -> Result<(), Failure> + Send + Sync + 'static
{
}

pub(crate) fn graph_case(name: &str, script: impl Script) -> Case {
    Case::new(name, move |record| {
        let mut graph = WorldGraph::new(standard())?;
        script(&mut graph, record)?;
        record.section("final");
        counters(record, &graph);
        Ok(())
    })
}

pub(crate) fn create_options(og_id: &str) -> CreateOptions {
    CreateOptions {
        og_id: Some(og_id.into()),
        ..CreateOptions::default()
    }
}

pub(crate) fn child(og_id: &str, parent: &str) -> CreateOptions {
    CreateOptions {
        parent: Some(parent.into()),
        ..create_options(og_id)
    }
}

pub(crate) fn on_plane(
    options: CreateOptions,
    origin: Point3,
    normal: Point3,
    x_direction: Point3,
) -> CreateOptions {
    CreateOptions {
        plane: Some(Plane {
            origin: Some(origin),
            normal: Some(normal),
            x_direction: Some(x_direction),
        }),
        ..options
    }
}

pub(crate) fn copy_options(og_id: &str, parent: Option<Option<&str>>) -> CopyOptions {
    CopyOptions {
        og_id: Some(og_id.into()),
        parent: parent.map(|parent| parent.map(str::to_string)),
    }
}

pub(crate) fn rectangle(width: f64, breadth: f64) -> Primitive {
    Primitive::Rectangle { width, breadth }
}

pub(crate) fn circle(radius: f64) -> Primitive {
    Primitive::Circle { radius }
}

pub(crate) fn cuboid(width: f64, height: f64, depth: f64) -> Primitive {
    Primitive::Cuboid {
        width,
        height,
        depth,
    }
}

pub(crate) fn polyline(points: &[Point3], closed: bool) -> Primitive {
    Primitive::Polyline {
        points: points.to_vec(),
        closed,
    }
}

pub(crate) fn extrude(profile: &str, holes: &[&str], distance: f64) -> CreatingOperation {
    CreatingOperation::Extrude {
        profile: profile.into(),
        holes: holes.iter().map(|hole| hole.to_string()).collect(),
        distance,
    }
}

pub(crate) fn sweep(profile: &str, path: &str) -> CreatingOperation {
    CreatingOperation::Sweep {
        profile: profile.into(),
        path: path.into(),
    }
}

pub(crate) fn primitive(
    record: &mut Record,
    graph: &mut WorldGraph,
    primitive: Primitive,
    options: CreateOptions,
) -> Option<String> {
    let label = format!("create {:?} {primitive:?}", options.og_id);
    created(record, &label, graph.create_primitive(primitive, options))
}

pub(crate) fn operation(
    record: &mut Record,
    graph: &mut WorldGraph,
    operation: CreatingOperation,
    options: CreateOptions,
) -> Option<String> {
    let label = format!("operation {:?} {operation:?}", options.og_id);
    created(record, &label, graph.create_operation(operation, options))
}

pub(crate) fn inspect(record: &mut Record, graph: &WorldGraph, og_id: &str) -> Result<(), Failure> {
    node_state(record, graph, og_id);
    let shape = graph.brep(og_id)?;
    record_body(record, og_id, &shape);
    Ok(())
}
