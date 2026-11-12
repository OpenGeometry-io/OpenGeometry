use crate::brep::Accuracy;
use crate::world_graph::{CopyOptions, CreateOptions, ErrorCode, Primitive, WorldGraph};

pub(super) fn graph() -> WorldGraph {
    WorldGraph::new(Accuracy::STANDARD).unwrap()
}

pub(super) fn named(id: &str) -> CreateOptions {
    CreateOptions {
        og_id: Some(id.into()),
        ..CreateOptions::default()
    }
}

pub(super) fn child_of(id: &str, parent: &str) -> CreateOptions {
    CreateOptions {
        og_id: Some(id.into()),
        parent: Some(parent.into()),
        ..CreateOptions::default()
    }
}

pub(super) fn copy_named(id: &str) -> CopyOptions {
    CopyOptions {
        og_id: Some(id.into()),
        ..CopyOptions::default()
    }
}

pub(super) fn cuboid() -> Primitive {
    Primitive::Cuboid {
        width: 1.0,
        height: 2.0,
        depth: 3.0,
    }
}

pub(super) fn key(world: &WorldGraph, og_id: &str) -> (u32, u32) {
    let node = world.node(og_id).unwrap();
    (node.handle, node.generation)
}

pub(super) fn found(world: &WorldGraph, key: (u32, u32)) -> String {
    world.node_by_handle(key.0, key.1).unwrap().og_id.clone()
}

pub(super) fn stale(world: &WorldGraph, key: (u32, u32)) {
    let error = world.node_by_handle(key.0, key.1).unwrap_err();
    assert_eq!(error.error_code(), ErrorCode::Disposed);
    assert_eq!(error.message(), "node handle is stale");
}

pub(super) fn scan(world: &WorldGraph, handle: u32, generation: u32) -> Option<String> {
    world
        .nodes
        .values()
        .find(|node| node.handle == handle && node.generation == generation)
        .map(|node| node.og_id.clone())
}

pub(super) fn agrees(world: &WorldGraph, handle: u32, generation: u32) {
    let indexed = world.node_by_handle(handle, generation);
    match scan(world, handle, generation) {
        Some(og_id) => assert_eq!(indexed.unwrap().og_id, og_id),
        None => stale(world, (handle, generation)),
    }
}
