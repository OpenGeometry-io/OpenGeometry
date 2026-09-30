use opengeometry::world_graph::{
    CopyOptions, CreateOptions, CreatingOperation, ErrorCode, Primitive, StepOptions, Transform,
};
use opengeometry_test_support::part21;
use opengeometry_test_support::world_graph::{graph, named};

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
    let parsed = part21::Document::parse(&first).unwrap();
    assert_eq!(parsed.entity_count(), report.entities);
    assert_eq!(parsed.count("ADVANCED_FACE"), report.faces);
    assert_eq!(parsed.count("EDGE_CURVE"), report.edges);
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
    let (text, report) = world.export_step(&["body".into()], &options).unwrap();
    assert!(text.contains("quote''\\\\#12\\X2\\0009\\X0\\\\X2\\00F8\\X0\\"));
    assert!(text.contains(
        "CARTESIAN_POINT('',(1.00000000000000000E0,-3.00000000000000000E0,2.00000000000000000E0))"
    ));
    assert_eq!(report.up_axis, "Z");
    assert_eq!(report.products, 1);
    part21::Document::parse(&text).unwrap();
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
    assert!(world
        .export_step(&["far".into()], &StepOptions::default())
        .is_ok());
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
    let (text, report) = world.export_step(&["parent".into()], &options).unwrap();
    assert_eq!(report.products, 2);
    assert_eq!(
        report
            .bodies
            .iter()
            .map(|body| body.og_id.as_str())
            .collect::<Vec<_>>(),
        ["parent", "child"]
    );
    assert_eq!(
        serde_json::to_value(&report.skipped).unwrap(),
        serde_json::json!([{"ogId": "rail", "reason": "Wire"}])
    );
    assert!(text.contains(
        "CARTESIAN_POINT('',(9.00000000000000000E0,0.00000000000000000E0,1.90000000000000000E1))"
    ));
    part21::Document::parse(&text).unwrap();
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
    let (text, report) = world
        .export_step(&["rail".into()], &StepOptions::default())
        .unwrap();
    assert!(report.pcurveless_edges > 0);
    assert_eq!(report.bodies[0].pcurveless_edges, report.pcurveless_edges);
    assert_eq!(text.matches("=EDGE_CURVE(").count(), report.edges);
    let parsed = part21::Document::parse(&text).unwrap();
    assert_eq!(parsed.pcurveless_edges().unwrap(), report.pcurveless_edges);
}

#[test]
fn cylinders_and_circle_extrusions_export_one_exact_cylindrical_surface() {
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
    let options = StepOptions {
        unit: "metre".into(),
        up_axis: "Y".into(),
        ..StepOptions::default()
    };
    for name in ["extruded", "primitive"] {
        let (text, report) = world.export_step(&[name.into()], &options).unwrap();
        let parsed = part21::Document::parse(&text).unwrap();
        assert_eq!(parsed.count("CYLINDRICAL_SURFACE"), 1);
        assert!(parsed.count("CIRCLE") >= 2);
        assert_eq!(parsed.count("POLY_LOOP"), 0);
        assert_eq!(report.solids, 1);
        assert!(text.contains("1.75000000000000000E0"));
    }
}
