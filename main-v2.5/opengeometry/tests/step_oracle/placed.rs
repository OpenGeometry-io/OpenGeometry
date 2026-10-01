use crate::support::{export_matched, parse_and_match, placed_export_bodies, unit_scale};
use opengeometry::world_graph::{
    CopyOptions, CreateOptions, CreatingOperation, ErrorCode, Primitive, StepOptions, Transform,
    WorldGraph,
};
use opengeometry_test_support::world_graph::{graph, named};
use serde_json::{json, Value};

#[test]
fn world_export_expands_assemblies_deduplicates_and_preserves_state() {
    let mut world = graph();
    world.create_system_assembly(named("level")).unwrap();
    world
        .create_primitive(
            Primitive::Cuboid {
                width: 2.0,
                height: 2.0,
                depth: 2.0,
            },
            CreateOptions {
                parent: Some("level".into()),
                ..named("wall")
            },
        )
        .unwrap();
    world
        .create_primitive(
            Primitive::Circle { radius: 0.5 },
            CreateOptions {
                parent: Some("level".into()),
                ..named("profile")
            },
        )
        .unwrap();
    world
        .instance(
            "wall",
            CopyOptions {
                og_id: Some("wall-copy".into()),
                ..CopyOptions::default()
            },
        )
        .unwrap();
    let before = world.brep("wall").unwrap().to_json().unwrap();
    let revision = world.revision();
    let options = StepOptions::default();
    let (first, report) = world
        .export_step(&["level".into(), "wall".into()], &options)
        .unwrap();
    let (second, _) = world
        .export_step(&["level".into(), "wall".into()], &options)
        .unwrap();
    assert_eq!(first, second);
    assert_eq!(report.products, 2);
    assert_eq!(report.solids, 2);
    assert_eq!(
        report
            .bodies
            .iter()
            .map(|body| body.og_id.as_str())
            .collect::<Vec<_>>(),
        ["wall", "wall-copy"]
    );
    assert_eq!(report.skipped.len(), 1);
    assert_eq!(report.skipped[0].og_id, "profile");
    assert!(first.contains("PRODUCT('wall','wall'"));
    assert!(first.contains("PRODUCT('wall-copy','wall-copy'"));
    let bodies = placed_export_bodies(&world, &report, &options);
    let report = serde_json::to_value(&report).unwrap();
    let parsed = parse_and_match(&first, &report, &options.unit, &bodies);
    assert_eq!(parsed.product_names().unwrap(), ["'wall'", "'wall-copy'"]);
    assert_eq!(world.revision(), revision);
    assert_eq!(world.brep("wall").unwrap().to_json().unwrap(), before);
    assert_eq!(
        world
            .export_step(&["profile".into()], &options)
            .unwrap_err()
            .error_code(),
        ErrorCode::InvalidOperand
    );
    assert_eq!(
        world
            .export_step(&["level".into(), "profile".into()], &options)
            .unwrap_err()
            .error_code(),
        ErrorCode::InvalidOperand,
    );
    assert_eq!(world.revision(), revision);
}

#[test]
fn world_export_applies_z_up_and_escapes_file_name() {
    let mut world = graph();
    world
        .create_primitive(
            Primitive::Cuboid {
                width: 2.0,
                height: 2.0,
                depth: 2.0,
            },
            named("body"),
        )
        .unwrap();
    world
        .transform(
            "body",
            Transform::Translate {
                offset: [2.0, 0.0, 2.0],
            },
        )
        .unwrap();
    let options = StepOptions {
        unit: "metre".into(),
        name: "quote'\\#12\tø".into(),
        ..StepOptions::default()
    };
    let exported = export_matched(&world, &["body"], &options);
    assert!(exported
        .text
        .contains("quote''\\\\#12\\X2\\0009\\X0\\\\X2\\00F8\\X0\\"));
    assert!(exported.text.contains(
        "CARTESIAN_POINT('',(1.00000000000000000E0,-3.00000000000000000E0,2.00000000000000000E0))"
    ));
    assert_eq!(exported.report["upAxis"], "Z");
    assert_eq!(exported.report["products"], 1);
}

#[test]
fn world_export_handles_far_body_and_rejects_options_without_mutation() {
    let defaults: StepOptions = serde_json::from_str("{}").unwrap();
    assert_eq!(defaults.unit, "millimetre");
    assert_eq!(defaults.up_axis, "Z");
    let mut world = graph();
    world
        .create_primitive(
            Primitive::Cuboid {
                width: 1.0,
                height: 1.0,
                depth: 1.0,
            },
            named("far"),
        )
        .unwrap();
    world
        .transform(
            "far",
            Transform::Translate {
                offset: [10_000.0, 0.0, 0.0],
            },
        )
        .unwrap();
    let revision = world.revision();
    export_matched(&world, &["far"], &StepOptions::default());
    for options in [
        StepOptions {
            unit: "inch".into(),
            ..StepOptions::default()
        },
        StepOptions {
            up_axis: "X".into(),
            ..StepOptions::default()
        },
        StepOptions {
            timestamp: "2026-02-30T00:00:00".into(),
            ..StepOptions::default()
        },
        StepOptions {
            name: String::new(),
            ..StepOptions::default()
        },
    ] {
        assert_eq!(
            world
                .export_step(&["far".into()], &options)
                .unwrap_err()
                .error_code(),
            ErrorCode::InvalidParameter
        );
    }
    assert_eq!(
        world
            .export_step(
                &["far".into()],
                &StepOptions {
                    name: "x".repeat(4097),
                    ..StepOptions::default()
                },
            )
            .unwrap_err()
            .error_code(),
        ErrorCode::LimitExceeded
    );
    assert!(world
        .export_step(
            &["far".into()],
            &StepOptions {
                name: "x".repeat(4096),
                ..StepOptions::default()
            },
        )
        .is_ok());
    assert_eq!(world.revision(), revision);
}

#[test]
fn body_children_are_exported_or_skipped_in_stored_order() {
    let mut world = graph();
    let cube = Primitive::Cuboid {
        width: 2.0,
        height: 2.0,
        depth: 2.0,
    };
    world
        .create_primitive(cube.clone(), named("parent"))
        .unwrap();
    world
        .transform(
            "parent",
            Transform::Translate {
                offset: [10.0, 0.0, 0.0],
            },
        )
        .unwrap();
    world
        .create_primitive(
            cube,
            CreateOptions {
                parent: Some("parent".into()),
                ..named("child")
            },
        )
        .unwrap();
    world
        .transform(
            "child",
            Transform::Translate {
                offset: [0.0, 0.0, 20.0],
            },
        )
        .unwrap();
    world
        .create_primitive(
            Primitive::Polyline {
                points: vec![[0.0; 3], [1.0, 0.0, 0.0]],
                closed: false,
            },
            CreateOptions {
                parent: Some("parent".into()),
                ..named("rail")
            },
        )
        .unwrap();
    let revision = world.revision();
    let options = StepOptions {
        unit: "metre".into(),
        up_axis: "Y".into(),
        ..StepOptions::default()
    };
    let exported = export_matched(&world, &["parent"], &options);
    assert_eq!(exported.report["products"], 2);
    assert_eq!(
        exported.report["bodies"]
            .as_array()
            .unwrap()
            .iter()
            .map(|body| &body["ogId"])
            .collect::<Vec<_>>(),
        ["parent", "child"]
    );
    assert_eq!(
        exported.report["skipped"],
        json!([{"ogId": "rail", "reason": "Wire"}])
    );
    assert!(exported.text.contains(
        "CARTESIAN_POINT('',(9.00000000000000000E0,0.00000000000000000E0,1.90000000000000000E1))"
    ));
    assert_eq!(world.revision(), revision);
}

#[test]
fn sweep_rail_exports_projected_edges_without_pcurves() {
    let mut world = graph();
    world
        .create_primitive(Primitive::Circle { radius: 0.5 }, named("profile"))
        .unwrap();
    world
        .create_primitive(
            Primitive::Polyline {
                points: vec![[0.0; 3], [0.0, 3.0, 0.0], [3.0, 3.0, 0.0]],
                closed: false,
            },
            named("path"),
        )
        .unwrap();
    world
        .create_operation(
            CreatingOperation::Sweep {
                profile: "profile".into(),
                path: "path".into(),
            },
            named("rail"),
        )
        .unwrap();
    let exported = export_matched(&world, &["rail"], &StepOptions::default());
    assert_eq!(exported.report["pcurvelessEdges"], 2);
    assert_eq!(exported.report["bodies"][0]["pcurvelessEdges"], 2);
    assert_eq!(exported.document.pcurveless_edges().unwrap(), 2);
}

fn bits(values: &[f64]) -> Vec<u64> {
    values.iter().map(|value| value.to_bits()).collect()
}

fn circle_edges(world: &WorldGraph, og_id: &str) -> usize {
    let brep: Value = serde_json::from_str(&world.brep(og_id).unwrap().to_json().unwrap()).unwrap();
    let curves = &brep["geometry"]["curves"];
    brep["topology"]["edges"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|edge| {
            edge["geometry"]["kind"] == "Curve"
                && curves[edge["geometry"]["curve"].as_u64().unwrap() as usize]["kind"] == "Circle"
        })
        .count()
}

#[test]
fn circle_extrusion_and_cylinder_edges_are_circles_of_the_source_radius() {
    let mut world = graph();
    world
        .create_primitive(Primitive::Circle { radius: 1.75 }, named("circle"))
        .unwrap();
    world
        .create_operation(
            CreatingOperation::Extrude {
                profile: "circle".into(),
                holes: Vec::new(),
                distance: 3.0,
            },
            named("extruded"),
        )
        .unwrap();
    world
        .create_primitive(
            Primitive::Cylinder {
                radius: 1.75,
                height: 3.0,
            },
            named("primitive"),
        )
        .unwrap();
    let metre_y = StepOptions {
        unit: "metre".into(),
        up_axis: "Y".into(),
        ..StepOptions::default()
    };
    for options in [metre_y, StepOptions::default()] {
        let radius = (1.75 * unit_scale(&options.unit)).to_bits();
        for name in ["extruded", "primitive"] {
            let exported = export_matched(&world, &[name], &options);
            let circles = exported
                .document
                .edge_supports()
                .unwrap()
                .into_iter()
                .filter(|support| support.kind == "CIRCLE")
                .collect::<Vec<_>>();
            assert_eq!(circles.len(), circle_edges(&world, name), "{name}");
            for circle in circles {
                assert!(circle.with_pcurves, "{name}");
                assert_eq!(bits(&circle.radii), [radius], "{name}");
            }
            let surfaces = exported.document.surface_radii().unwrap();
            assert_eq!(surfaces.len(), 1, "{name}");
            assert_eq!(surfaces[0].0, "cylinder", "{name}");
            assert_eq!(bits(&surfaces[0].1), [radius], "{name}");
            assert_eq!(exported.report["solids"], 1);
            assert_eq!(exported.document.count("POLY_LOOP"), 0);
        }
    }
}
