use super::support::box_primitive;
use opengeometry::brep::BodyType;
use opengeometry::world_graph::{
    CopyOptions, CreateOptions, CreatingOperation, ErrorCode, Plane, Primitive, WorldGraph,
};
use opengeometry_test_support::world_graph::{graph, named};

fn rectangle() -> Primitive {
    Primitive::Rectangle {
        width: 2.0,
        breadth: 3.0,
    }
}

fn extrude() -> CreatingOperation {
    CreatingOperation::Extrude {
        profile: "profile".into(),
        holes: Vec::new(),
        distance: 1.0,
    }
}

fn profiled_graph() -> WorldGraph {
    let mut world = graph();
    world
        .create_primitive(rectangle(), named("profile"))
        .unwrap();
    world
}

fn unnamed_box(world: &mut WorldGraph) -> String {
    world
        .create_primitive(box_primitive(), CreateOptions::default())
        .unwrap()
        .0
}

#[test]
fn mismatched_expected_body_type_is_rejected_before_commit() {
    let mut world = profiled_graph();
    let revision = world.revision();
    let nodes = world.node_count();
    let shape_ids = world.reserve(1, &[]).unwrap().shape_ids;
    let wire_as_solid = world
        .create_primitive(
            rectangle(),
            CreateOptions {
                body_type: Some(BodyType::Solid),
                ..CreateOptions::default()
            },
        )
        .unwrap_err();
    assert_eq!(wire_as_solid.error_code(), ErrorCode::BodyTypeMismatch);
    let solid_as_wire = world
        .create_operation(
            extrude(),
            CreateOptions {
                body_type: Some(BodyType::Wire),
                ..CreateOptions::default()
            },
        )
        .unwrap_err();
    assert_eq!(solid_as_wire.error_code(), ErrorCode::BodyTypeMismatch);
    assert_eq!(world.revision(), revision);
    assert_eq!(world.node_count(), nodes);
    assert_eq!(world.reserve(1, &[]).unwrap().shape_ids, shape_ids);
    assert_eq!(unnamed_box(&mut world), "solid-1");
}

#[test]
fn generated_ids_are_kind_prefixed_from_one() {
    let mut world = graph();
    let cuboid = unnamed_box(&mut world);
    let (wire, _) = world
        .create_primitive(rectangle(), CreateOptions::default())
        .unwrap();
    let (assembly, _) = world
        .create_system_assembly(CreateOptions::default())
        .unwrap();
    let (instance, _) = world.instance(&cuboid, CopyOptions::default()).unwrap();
    let (duplicate, _) = world.duplicate(&wire, CopyOptions::default()).unwrap();
    assert_eq!(
        [cuboid, wire, assembly, instance, duplicate],
        ["solid-1", "wire-1", "assembly-1", "solid-2", "wire-2"]
    );
    world
        .create_primitive(box_primitive(), named("solid-3"))
        .unwrap();
    assert_eq!(unnamed_box(&mut world), "solid-4");
}

#[test]
fn json_null_parent_moves_a_copy_to_the_top_level() {
    let to_top: CopyOptions = serde_json::from_str(r#"{"parent":null}"#).unwrap();
    assert_eq!(to_top.parent, Some(None));
    let keep: CopyOptions = serde_json::from_str("{}").unwrap();
    assert_eq!(keep.parent, None);
    let mut world = graph();
    world.create_system_assembly(named("group")).unwrap();
    world
        .create_primitive(
            box_primitive(),
            CreateOptions {
                og_id: Some("body".into()),
                parent: Some("group".into()),
                ..CreateOptions::default()
            },
        )
        .unwrap();
    let (copy, _) = world.instance("body", to_top).unwrap();
    assert_eq!(world.parent(&copy).unwrap(), None);
}

#[test]
fn plane_is_rejected_on_operations_and_assemblies() {
    let mut world = profiled_graph();
    let revision = world.revision();
    let nodes = world.node_count();
    let planed = || CreateOptions {
        plane: Some(Plane::default()),
        ..CreateOptions::default()
    };
    let errors = [
        world.create_operation(extrude(), planed()).unwrap_err(),
        world.create_system_assembly(planed()).unwrap_err(),
        world
            .create_system_assembly(CreateOptions {
                body_type: Some(BodyType::Solid),
                ..CreateOptions::default()
            })
            .unwrap_err(),
    ];
    for error in errors {
        assert_eq!(error.error_code(), ErrorCode::InvalidParameter);
    }
    assert_eq!(world.revision(), revision);
    assert_eq!(world.node_count(), nodes);
}
