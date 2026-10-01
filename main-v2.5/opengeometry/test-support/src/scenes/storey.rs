use super::acceptance::{create_cutter, create_wall, subtract, AcceptanceScene};
use crate::world_graph::named;
use opengeometry::brep::{BrepEnvelope, FaceRole};
use opengeometry::world_graph::{CopyOptions, Transform, WorldGraph};
use std::collections::BTreeSet;

pub struct StoreySize {
    pub walls: usize,
    pub rail_instances: usize,
}

pub struct Storey {
    pub root: String,
    pub walls: Vec<String>,
    pub rails: Vec<String>,
}

pub fn build_storey(scene: &mut AcceptanceScene, size: StoreySize) -> Storey {
    let graph = &mut scene.graph;
    let (root, _) = graph.create_system_assembly(named("storey")).unwrap();
    let mut members = Vec::new();
    let walls: Vec<String> = (0..size.walls)
        .map(|index| add_storey_wall(graph, index, &mut members))
        .collect();
    let rails: Vec<String> = (0..size.rail_instances)
        .map(|index| add_rail_instance(graph, &scene.rail, index))
        .collect();
    members.extend(rails.iter().cloned());
    graph.add_child(&root, &members, false).unwrap();
    Storey { root, walls, rails }
}

pub fn openings(brep: &BrepEnvelope) -> usize {
    brep.topology
        .faces
        .iter()
        .filter(|face| face.provenance.role == FaceRole::Cut)
        .flat_map(|face| face.provenance.sources.iter().map(|source| &source.body))
        .collect::<BTreeSet<_>>()
        .len()
}

fn add_storey_wall(graph: &mut WorldGraph, index: usize, members: &mut Vec<String>) -> String {
    let profile = format!("storey-profile-{index}");
    let wall = create_wall(graph, &profile, &format!("storey-wall-{index}"));
    let cutters: Vec<String> = (0..4)
        .map(|opening| {
            let og_id = format!("storey-opening-{index}-{opening}");
            let x = -2.1 + 1.4 * f64::from(opening);
            create_cutter(graph, &og_id, [0.6, 2.0, 0.4], x)
        })
        .collect();
    subtract(graph, &wall, &cutters);
    for cutter in &cutters {
        graph.dispose(cutter).unwrap();
    }
    graph
        .transform(
            &wall,
            Transform::Translate {
                offset: grid(index, 20),
            },
        )
        .unwrap();
    members.extend([profile, wall.clone()]);
    wall
}

fn add_rail_instance(graph: &mut WorldGraph, rail: &str, index: usize) -> String {
    let options = CopyOptions {
        og_id: Some(format!("storey-rail-{index}")),
        parent: None,
    };
    let (copy, _) = graph.instance(rail, options).unwrap();
    graph
        .transform(
            &copy,
            Transform::Translate {
                offset: grid(index, 40),
            },
        )
        .unwrap();
    copy
}

fn grid(index: usize, columns: usize) -> [f64; 3] {
    let column = (index % columns) as f64;
    let row = (index / columns) as f64;
    [column * 8.0, 0.0, row * 8.0]
}
