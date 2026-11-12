use opengeometry::world_graph::WorldGraph;
use opengeometry_test_support::scenes::acceptance::recut_scene;
use opengeometry_test_support::scenes::prismatic::pan_and_inset;
use opengeometry_test_support::scenes::storey::{build_storey, StoreySize};
use opengeometry_test_support::stored;
use sha2::{Digest, Sha256};

const STOREY_DIGEST: &str = "e2fe0b4ac3dbcc18462394bb9ced445c57ec5d83070880801d73e50e0bcc7ac7";
const PAN_DIGEST: &str = "feddbe397d2629efbe359d3405f2aa2e00e4282e52b3c5f245971f6e57e1c065";

fn add_subtree(world: &WorldGraph, og_id: &str, ids: &mut Vec<String>) {
    ids.push(og_id.to_string());
    for child in world.children(og_id).unwrap() {
        add_subtree(world, child, ids);
    }
}

fn every_node(world: &WorldGraph, roots: &[&str]) -> Vec<String> {
    let mut ids = Vec::new();
    for root in roots {
        add_subtree(world, root, &mut ids);
    }
    assert_eq!(ids.len(), world.node_count());
    ids
}

fn assert_encloses_validated_brep(world: &WorldGraph, og_id: &str) {
    let Some(local) = world.local_bounds(og_id).unwrap() else {
        assert!(world.node(og_id).unwrap().shape.is_none(), "{og_id}");
        return;
    };
    let brep = world.brep(og_id).unwrap();
    brep.validate().unwrap();
    for vertex in &brep.topology.vertices {
        for axis in 0..3 {
            assert!(local[axis] <= vertex.position[axis], "{og_id}");
            assert!(vertex.position[axis] <= local[axis + 3], "{og_id}");
        }
    }
    assert!(world.bounds(og_id).unwrap().is_some(), "{og_id}");
}

fn bounds_digest(world: &WorldGraph, ids: &[String]) -> String {
    let mut hasher = Sha256::new();
    for og_id in ids {
        hasher.update(og_id.as_bytes());
        for bounds in [world.bounds(og_id), world.local_bounds(og_id)] {
            for value in bounds.unwrap().into_iter().flatten() {
                hasher.update(value.to_bits().to_le_bytes());
            }
        }
    }
    format!("{:x}", hasher.finalize())
}

fn assert_bounds_are_the_validated_enclosure(world: &WorldGraph, roots: &[&str], digest: &str) {
    let ids = every_node(world, roots);
    for og_id in &ids {
        assert_encloses_validated_brep(world, og_id);
    }
    if stored::compared() {
        assert_eq!(bounds_digest(world, &ids), digest);
    }
}

#[test]
fn storey_bounds_equal_the_enclosure_of_each_validated_stored_brep() {
    let mut scene = recut_scene();
    let size = StoreySize {
        walls: 2,
        rail_instances: 3,
    };
    let storey = build_storey(&mut scene, size);
    let roots = [scene.level.as_str(), storey.root.as_str()];
    assert_bounds_are_the_validated_enclosure(&scene.graph, &roots, STOREY_DIGEST);
}

#[test]
fn pan_and_inset_bounds_equal_the_enclosure_of_each_validated_stored_brep() {
    let world = pan_and_inset(0.35);
    let roots = ["pan-profile", "pan", "inset-profile", "inset"];
    assert_bounds_are_the_validated_enclosure(&world, &roots, PAN_DIGEST);
}
