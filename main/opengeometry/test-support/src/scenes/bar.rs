use crate::world_graph::{graph, named};
use opengeometry::world_graph::{EditScope, ModifyingOperation, Primitive, Transform, WorldGraph};

type Bounds = [f64; 6];

struct Placement {
    name: &'static str,
    at: [f64; 3],
    height: f64,
}

const OPERATIONS: [(&str, ModifyingOperation); 3] = [
    ("S", ModifyingOperation::Subtract),
    ("U", ModifyingOperation::Union),
    ("I", ModifyingOperation::Intersect),
];

pub fn bar_handlers(target: &Primitive) -> Vec<String> {
    let bounds = created(target).bounds("target").unwrap().unwrap();
    let mut rows = Vec::new();
    for placement in placements(bounds) {
        for (name, tool) in tools(bounds, placement.height) {
            let cells = OPERATIONS.map(|(label, operation)| {
                format!("{label}={}", cell(target, &tool, placement.at, operation))
            });
            rows.push(format!("{}/{name} {}", placement.name, cells.join(" ")));
        }
    }
    rows
}

pub fn shell_is_valid(target: &Primitive, thickness: f64) -> bool {
    let mut world = created(target);
    world.shell("target", thickness, EditScope::Node).unwrap();
    let validity = world.validity("target").unwrap();
    validity.valid && validity.closed
}

fn created(target: &Primitive) -> WorldGraph {
    let mut world = graph();
    world
        .create_primitive(target.clone(), named("target"))
        .unwrap();
    world
}

fn placements(b: Bounds) -> [Placement; 3] {
    let size = [b[3] - b[0], b[4] - b[1], b[5] - b[2]];
    let mid = [(b[0] + b[3]) / 2.0, (b[2] + b[5]) / 2.0];
    let top = b[1] + 0.75 * size[1];
    let inner = mid[0] + 0.07 * size[0];
    let back = mid[1] + 0.05 * size[2];
    [
        Placement {
            name: "A",
            at: [b[3] - 0.02 * size[0], top, mid[1] + 0.03 * size[2]],
            height: 0.5 * size[1],
        },
        Placement {
            name: "B",
            at: [inner, top, back],
            height: 0.5 * size[1],
        },
        Placement {
            name: "T",
            at: [inner, b[1] - 0.25 * size[1], back],
            height: 1.5 * size[1],
        },
    ]
}

fn tools(b: Bounds, height: f64) -> [(&'static str, Primitive); 2] {
    let width = 0.3 * (b[3] - b[0]).min(b[5] - b[2]);
    [
        (
            "box",
            Primitive::Cuboid {
                width,
                height,
                depth: width,
            },
        ),
        (
            "cyl",
            Primitive::Cylinder {
                radius: width / 2.0,
                height,
            },
        ),
    ]
}

fn cell(
    target: &Primitive,
    tool: &Primitive,
    at: [f64; 3],
    operation: ModifyingOperation,
) -> String {
    let mut world = created(target);
    world.create_primitive(tool.clone(), named("tool")).unwrap();
    world
        .transform("tool", Transform::Translate { offset: at })
        .unwrap();
    world
        .operate("target", operation, &["tool".into()], EditScope::Node)
        .unwrap_or_else(|error| panic!("{operation:?} {tool:?} at {at:?}: {error:?}"));
    let validity = world.validity("target").unwrap();
    assert!(
        validity.valid && validity.closed,
        "{operation:?} {tool:?} at {at:?}"
    );
    let report = world.report("target").unwrap().unwrap();
    report["handlers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|handler| handler.as_str().unwrap())
        .collect::<Vec<_>>()
        .join("+")
}
