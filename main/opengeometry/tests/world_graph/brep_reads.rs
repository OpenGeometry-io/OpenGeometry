use super::support::{box_primitive, near_within};
use opengeometry::brep::{BrepEnvelope, Surface, SurfaceGeometry};
use opengeometry::world_graph::{
    CreateOptions, EditScope, ErrorCode, ErrorDetails, ModifyingOperation, Primitive, StepOptions,
    Transform, WorldGraph,
};
use opengeometry_test_support::world_graph::{graph, named};
use std::sync::Arc;

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn unit(a: [f64; 3]) -> [f64; 3] {
    let length = dot(a, a).sqrt();
    a.map(|value| value / length)
}

fn axis_foot(frame_origin: [f64; 3], axis: [f64; 3], point: [f64; 3]) -> [f64; 3] {
    let along = dot(sub(point, frame_origin), axis);
    std::array::from_fn(|i| frame_origin[i] + axis[i] * along)
}

fn child_of(parent: &str, id: &str) -> CreateOptions {
    CreateOptions {
        parent: Some(parent.into()),
        ..named(id)
    }
}

fn assembly_at(world: &mut WorldGraph, id: &str, offset: [f64; 3]) {
    world.create_system_assembly(named(id)).unwrap();
    world
        .transform(id, Transform::Translate { offset })
        .unwrap();
}

fn cylinder_face(world: &WorldGraph, id: &str) -> (u32, [f64; 3], [f64; 3]) {
    let brep = world.brep(id).unwrap();
    let face = brep
        .topology
        .faces
        .iter()
        .find(|face| {
            matches!(
                brep.geometry.surfaces[face.surface as usize],
                SurfaceGeometry::Cylinder { .. }
            )
        })
        .unwrap();
    let surface = &brep.geometry.surfaces[face.surface as usize];
    let point = surface.point_at([0.7, 0.3]).unwrap();
    let frame = surface.frame();
    (face.id, point, axis_foot(frame.origin, frame.z, point))
}

#[test]
fn world_brep_of_a_translated_and_rotated_cuboid_holds_its_world_vertices() {
    let mut world = graph();
    assembly_at(&mut world, "site", [10.0, 0.0, 0.0]);
    world
        .create_primitive(box_primitive(), child_of("site", "body"))
        .unwrap();
    world
        .transform(
            "body",
            Transform::Translate {
                offset: [1.0, 2.0, -3.0],
            },
        )
        .unwrap();
    world
        .transform(
            "body",
            Transform::Rotate {
                axis: [0.3, 1.0, 0.2],
                degrees: 30.0,
                pivot: None,
            },
        )
        .unwrap();
    let local = world.brep("body").unwrap();
    let placed = world.placed_brep("body").unwrap();
    let placement = world.world_placement("body").unwrap();
    assert_eq!(placed.topology.faces.len(), local.topology.faces.len());
    for (moved, original) in placed
        .topology
        .vertices
        .iter()
        .zip(&local.topology.vertices)
    {
        near_within(
            &moved.position,
            &placement.apply_point(original.position),
            1e-12,
        );
    }
    assert!(placed
        .topology
        .vertices
        .iter()
        .any(|vertex| vertex.position[0] > 9.0));
    assert_eq!(placed.id, local.id);
    assert_eq!(placed.revision, local.revision);
}

#[test]
fn world_brep_at_identity_is_the_local_brep_byte_for_byte() {
    let mut world = graph();
    world
        .create_primitive(box_primitive(), named("body"))
        .unwrap();
    let placed = world.placed_brep("body").unwrap();
    let local = world.brep("body").unwrap();
    assert_eq!(placed.to_json().unwrap(), local.to_json().unwrap());
    assert!(Arc::ptr_eq(&placed, &local));
}

#[test]
fn world_brep_scales_tolerances_and_accuracy_by_the_world_scale() {
    let mut world = graph();
    world
        .create_primitive(box_primitive(), named("body"))
        .unwrap();
    world
        .transform(
            "body",
            Transform::Scale {
                factor: 2.5,
                pivot: None,
            },
        )
        .unwrap();
    let local = world.brep("body").unwrap();
    let placed = world.placed_brep("body").unwrap();
    let tolerances = |brep: &BrepEnvelope| -> Vec<f64> {
        let value: serde_json::Value = serde_json::from_str(&brep.to_json().unwrap()).unwrap();
        value["topology"]["vertices"]
            .as_array()
            .unwrap()
            .iter()
            .map(|vertex| vertex["tolerance"].as_f64().unwrap())
            .collect()
    };
    let scaled: Vec<f64> = tolerances(&local)
        .iter()
        .map(|tolerance| tolerance * 2.5)
        .collect();
    assert!(!scaled.is_empty());
    assert_eq!(tolerances(&placed), scaled);
    assert_eq!(
        placed.accuracy.geometric.to_bits(),
        (local.accuracy.geometric * 2.5).to_bits()
    );
    assert_eq!(placed.revision, local.revision);
}

#[test]
fn world_brep_beyond_the_precision_guard_is_limit_exceeded() {
    let mut world = graph();
    assembly_at(&mut world, "far", [1e8, 0.0, 0.0]);
    world
        .create_primitive(box_primitive(), child_of("far", "body"))
        .unwrap();
    assert_eq!(
        world.placed_brep("body").unwrap_err().error_code(),
        ErrorCode::LimitExceeded
    );
    world.brep("body").unwrap();
}

#[test]
fn face_normal_at_gives_each_cuboid_face_its_outward_normal() {
    let mut world = graph();
    world
        .create_primitive(box_primitive(), named("body"))
        .unwrap();
    let brep = world.brep("body").unwrap();
    let count = brep.topology.vertices.len() as f64;
    let centre: [f64; 3] = std::array::from_fn(|i| {
        brep.topology
            .vertices
            .iter()
            .map(|vertex| vertex.position[i])
            .sum::<f64>()
            / count
    });
    let mut normals = Vec::new();
    for face in &brep.topology.faces {
        let point = brep.geometry.surfaces[face.surface as usize].frame().origin;
        let normal = world.face_normal_at("body", face.id, point).unwrap();
        assert!(dot(normal, sub(point, centre)) > 0.0);
        assert_eq!(normal.iter().filter(|value| value.abs() == 1.0).count(), 1);
        normals.push(normal.map(|value| value.to_bits()));
    }
    normals.sort();
    normals.dedup();
    assert_eq!(normals.len(), 6);
}

#[test]
fn face_normal_at_on_a_cylinder_side_is_radial_at_the_point() {
    let mut world = graph();
    world
        .create_primitive(
            Primitive::Cylinder {
                radius: 0.5,
                height: 2.0,
            },
            named("body"),
        )
        .unwrap();
    let (face, point, foot) = cylinder_face(&world, "body");
    near_within(
        &world.face_normal_at("body", face, point).unwrap(),
        &unit(sub(point, foot)),
        1e-12,
    );
}

#[test]
fn face_normal_at_applies_a_reversed_face_sense() {
    let mut world = graph();
    world
        .create_primitive(box_primitive(), named("host"))
        .unwrap();
    world
        .create_primitive(
            Primitive::Cylinder {
                radius: 0.5,
                height: 4.0,
            },
            named("tool"),
        )
        .unwrap();
    world
        .transform(
            "tool",
            Transform::Translate {
                offset: [0.0, -1.0, 0.0],
            },
        )
        .unwrap();
    world
        .operate(
            "host",
            ModifyingOperation::Subtract,
            &["tool".into()],
            EditScope::Node,
        )
        .unwrap();
    let (face, point, foot) = cylinder_face(&world, "host");
    near_within(
        &world.face_normal_at("host", face, point).unwrap(),
        &unit(sub(foot, point)),
        1e-12,
    );
}

#[test]
fn face_normal_at_rejects_unknown_faces_axis_points_and_assemblies() {
    let mut world = graph();
    world
        .create_primitive(
            Primitive::Cylinder {
                radius: 0.5,
                height: 2.0,
            },
            named("body"),
        )
        .unwrap();
    world.create_system_assembly(named("group")).unwrap();
    let faces = world.brep("body").unwrap().topology.faces.len() as u32;
    let unknown = world.face_normal_at("body", faces, [0.0; 3]).unwrap_err();
    assert_eq!(unknown.error_code(), ErrorCode::InvalidParameter);
    assert_eq!(
        unknown.details(),
        ErrorDetails::MissingReference {
            kind: "face".into(),
            index: faces,
        }
    );
    let (face, _, foot) = cylinder_face(&world, "body");
    assert_eq!(
        world
            .face_normal_at("body", face, foot)
            .unwrap_err()
            .error_code(),
        ErrorCode::InvalidGeometry
    );
    assert_eq!(
        world
            .face_normal_at("group", 0, [0.0; 3])
            .unwrap_err()
            .error_code(),
        ErrorCode::InvalidOperand
    );
    assert_eq!(
        world
            .face_normal_at("missing", 0, [0.0; 3])
            .unwrap_err()
            .error_code(),
        ErrorCode::UnknownNode
    );
}

fn state(world: &WorldGraph, revision: u64) -> String {
    format!(
        "{} {} {:?} {} {} {:?} {} {} {}",
        world.revision(),
        world.brep("moved").unwrap().to_json().unwrap(),
        world.changes_since(revision).unwrap(),
        world.node_count(),
        world.shape_count(),
        world.mark_stats(),
        world.instance_count("still").unwrap(),
        world.brep("still").unwrap().to_json().unwrap(),
        world
            .export_step(&["moved".into(), "still".into()], &StepOptions::default())
            .unwrap()
            .0,
    )
}

#[test]
fn brep_reads_leave_graph_state_unchanged() {
    let mut world = graph();
    world
        .create_primitive(box_primitive(), named("still"))
        .unwrap();
    world
        .create_primitive(box_primitive(), named("moved"))
        .unwrap();
    world
        .transform(
            "moved",
            Transform::Translate {
                offset: [4.0, 0.0, 0.0],
            },
        )
        .unwrap();
    let revision = world.revision();
    let before = state(&world, revision);
    world.placed_brep("still").unwrap();
    world.placed_brep("moved").unwrap();
    world.face_normal_at("moved", 0, [0.0; 3]).unwrap();
    world.face_normal_at("moved", 99, [0.0; 3]).unwrap_err();
    assert_eq!(state(&world, revision), before);
}
