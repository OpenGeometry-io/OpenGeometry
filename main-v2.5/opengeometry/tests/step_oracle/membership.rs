use crate::support::{add_cuboid, export_matched};
use opengeometry::world_graph::{
    CopyOptions, EditScope, ModifyingOperation, Primitive, StepOptions, Transform, WorldGraph,
};
use opengeometry_test_support::part21::SolidEntry;
use opengeometry_test_support::world_graph::{graph, named};

fn both_conventions() -> [StepOptions; 2] {
    [
        StepOptions {
            unit: "metre".into(),
            up_axis: "Y".into(),
            ..StepOptions::default()
        },
        StepOptions::default(),
    ]
}

fn cylinder(world: &mut WorldGraph, og_id: &str) {
    world
        .create_primitive(
            Primitive::Cylinder {
                radius: 1.0,
                height: 2.0,
            },
            named(og_id),
        )
        .unwrap();
}

#[test]
fn z_up_export_values_equal_the_placed_brep_bit_for_bit() {
    let mut world = graph();
    add_cuboid(&mut world, "box", [2.0, 2.0, 2.0], [2.0, 0.0, 2.0]);
    cylinder(&mut world, "cylinder");
    for options in both_conventions() {
        export_matched(&world, &["box", "cylinder"], &options);
    }
}

#[test]
fn rotated_and_scaled_export_values_equal_the_placed_brep_bit_for_bit() {
    let mut world = graph();
    add_cuboid(&mut world, "box", [2.0, 3.0, 1.0], [0.0; 3]);
    cylinder(&mut world, "cylinder");
    for og_id in ["box", "cylinder"] {
        for transform in [
            Transform::Rotate {
                axis: [1.0, 2.0, 3.0],
                degrees: 30.0,
                pivot: None,
            },
            Transform::Scale {
                factor: 2.5,
                pivot: None,
            },
        ] {
            world.transform(og_id, transform).unwrap();
        }
    }
    for options in both_conventions() {
        export_matched(&world, &["box", "cylinder"], &options);
    }
}

#[test]
fn instance_export_values_equal_each_instances_placed_brep_bit_for_bit() {
    let mut world = graph();
    add_cuboid(&mut world, "wall", [2.0, 2.0, 2.0], [3.0, 0.0, 0.0]);
    world
        .instance(
            "wall",
            CopyOptions {
                og_id: Some("wall-copy".into()),
                ..CopyOptions::default()
            },
        )
        .unwrap();
    for transform in [
        Transform::Rotate {
            axis: [0.0, 1.0, 0.0],
            degrees: 90.0,
            pivot: None,
        },
        Transform::Translate {
            offset: [0.0, 0.0, 5.0],
        },
    ] {
        world.transform("wall-copy", transform).unwrap();
    }
    for options in both_conventions() {
        let exported = export_matched(&world, &["wall", "wall-copy"], &options);
        assert_eq!(exported.report["products"], 2);
    }
}

#[test]
fn far_body_export_values_equal_the_placed_brep_bit_for_bit() {
    let mut world = graph();
    cylinder(&mut world, "far");
    world
        .transform(
            "far",
            Transform::Translate {
                offset: [10_000.0, 0.0, 0.0],
            },
        )
        .unwrap();
    for options in both_conventions() {
        export_matched(&world, &["far"], &options);
    }
}

#[test]
fn cavity_solid_exports_brep_with_voids_and_plain_solid_exports_manifold() {
    let mut world = graph();
    add_cuboid(&mut world, "host", [4.0, 4.0, 4.0], [0.0; 3]);
    add_cuboid(&mut world, "inner", [1.0, 1.0, 1.0], [0.0, 1.5, 0.0]);
    add_cuboid(&mut world, "plain", [1.0, 1.0, 1.0], [10.0, 0.0, 0.0]);
    world
        .operate(
            "host",
            ModifyingOperation::Subtract,
            &["inner".into()],
            EditScope::Node,
        )
        .unwrap();
    for options in both_conventions() {
        let exported = export_matched(&world, &["host", "plain"], &options);
        assert_eq!(
            exported.document.solids().unwrap(),
            [
                SolidEntry {
                    voids: 1,
                    with_voids: true
                },
                SolidEntry {
                    voids: 0,
                    with_voids: false
                }
            ]
        );
    }
}
