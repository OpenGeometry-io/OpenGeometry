use crate::accuracy::accuracy;
use opengeometry::world_graph::{CreateOptions, WorldGraph};

pub fn graph() -> WorldGraph {
    WorldGraph::new(accuracy(1e-6)).unwrap()
}

pub fn named(id: &str) -> CreateOptions {
    CreateOptions {
        og_id: Some(id.into()),
        ..CreateOptions::default()
    }
}
