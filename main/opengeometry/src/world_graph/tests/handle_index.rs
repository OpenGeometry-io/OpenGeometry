use super::support::{child_of, copy_named, cuboid, found, graph, key, named, stale};
use crate::world_graph::{CopyOptions, EditScope, Primitive};

#[test]
fn lookup_finds_each_created_body_and_assembly() {
    let mut world = graph();
    world.create_system_assembly(named("level")).unwrap();
    world
        .create_primitive(cuboid(), child_of("wall", "level"))
        .unwrap();
    world.create_primitive(cuboid(), named("slab")).unwrap();
    for og_id in ["level", "wall", "slab"] {
        assert_eq!(found(&world, key(&world, og_id)), og_id);
    }
}

#[test]
fn lookup_refuses_a_handle_from_a_fresh_graph() {
    let mut world = graph();
    world.create_primitive(cuboid(), named("body")).unwrap();
    let issued = key(&world, "body");
    let fresh = graph();
    stale(&fresh, issued);
    assert_eq!(found(&world, issued), "body");
}

#[test]
fn lookup_refuses_a_stale_generation_and_an_unissued_handle() {
    let mut world = graph();
    world.create_primitive(cuboid(), named("body")).unwrap();
    let (handle, generation) = key(&world, "body");
    stale(&world, (handle, generation + 1));
    stale(&world, (handle + 1, generation));
    stale(&world, (u32::MAX, u32::MAX));
}

#[test]
fn lookup_refuses_a_disposed_node_and_its_descendants() {
    let mut world = graph();
    world.create_system_assembly(named("level")).unwrap();
    world
        .create_primitive(cuboid(), child_of("wall", "level"))
        .unwrap();
    world.create_primitive(cuboid(), named("slab")).unwrap();
    let level = key(&world, "level");
    let wall = key(&world, "wall");
    world.dispose("level").unwrap();
    stale(&world, level);
    stale(&world, wall);
    assert_eq!(found(&world, key(&world, "slab")), "slab");
}

#[test]
fn lookup_keeps_a_handle_across_reparenting() {
    let mut world = graph();
    world.create_system_assembly(named("first")).unwrap();
    world.create_system_assembly(named("second")).unwrap();
    world
        .create_primitive(cuboid(), child_of("wall", "first"))
        .unwrap();
    let wall = key(&world, "wall");
    world.add_child("second", &["wall".into()], true).unwrap();
    assert_eq!(found(&world, wall), "wall");
    world.remove_child("second", "wall", false).unwrap();
    assert_eq!(found(&world, wall), "wall");
    assert_eq!(key(&world, "wall"), wall);
}

#[test]
fn lookup_finds_instances_and_duplicates_under_their_own_handles() {
    let mut world = graph();
    world.create_primitive(cuboid(), named("body")).unwrap();
    world.instance("body", copy_named("instance")).unwrap();
    world.duplicate("body", copy_named("copy")).unwrap();
    let (unnamed, _) = world.instance("copy", CopyOptions::default()).unwrap();
    let keys: Vec<_> = ["body", "instance", "copy", unnamed.as_str()]
        .iter()
        .map(|og_id| key(&world, og_id))
        .collect();
    for (index, og_id) in ["body", "instance", "copy", unnamed.as_str()]
        .iter()
        .enumerate()
    {
        assert_eq!(found(&world, keys[index]), *og_id);
    }
    world.make_unique("instance").unwrap().unwrap();
    assert_eq!(found(&world, keys[1]), "instance");
    world.dispose("body").unwrap();
    stale(&world, keys[0]);
    assert_eq!(found(&world, keys[1]), "instance");
}

#[test]
fn lookup_keeps_a_handle_across_a_rebuild() {
    let mut world = graph();
    world.create_primitive(cuboid(), named("body")).unwrap();
    let body = key(&world, "body");
    let sphere = Primitive::Sphere { radius: 1.0 };
    world
        .rebuild_primitive("body", sphere, EditScope::Node)
        .unwrap();
    assert_eq!(found(&world, body), "body");
}

#[test]
fn lookup_after_rollback_restores_disposed_nodes_and_drops_created_ones() {
    let mut world = graph();
    world.create_system_assembly(named("level")).unwrap();
    world
        .create_primitive(cuboid(), child_of("wall", "level"))
        .unwrap();
    let level = key(&world, "level");
    let wall = key(&world, "wall");
    let mark = world.mark().unwrap();
    world.dispose("level").unwrap();
    world.create_primitive(cuboid(), named("wall")).unwrap();
    let recreated = key(&world, "wall");
    assert_ne!(recreated, wall);
    stale(&world, wall);
    assert_eq!(found(&world, recreated), "wall");
    world.rollback(mark).unwrap();
    assert_eq!(found(&world, level), "level");
    assert_eq!(found(&world, wall), "wall");
    stale(&world, recreated);
    world.release(mark).unwrap();
}
