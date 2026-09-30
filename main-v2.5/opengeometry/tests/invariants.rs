use opengeometry::brep::Accuracy;
use opengeometry::tessellation::display::static_bucket;
use opengeometry::world_graph::{CreateOptions, ErrorCode, Primitive, WorldGraph};

fn tessellation_accuracy(tessellation: f64) -> Accuracy {
    Accuracy {
        geometric: 1e-8,
        intersection: 1e-9,
        tessellation,
        exchange: 1e-6,
    }
}

#[test]
fn i5_graph_accuracy_is_validated_and_display_bucket_ignores_tessellation_budget() {
    assert!(WorldGraph::new(Accuracy {
        geometric: 0.0,
        ..tessellation_accuracy(0.01)
    })
    .is_err());
    assert!(WorldGraph::new(Accuracy {
        intersection: 1e-7,
        ..tessellation_accuracy(0.01)
    })
    .is_err());
    let mut fine = WorldGraph::new(tessellation_accuracy(0.01)).unwrap();
    let mut coarse = WorldGraph::new(tessellation_accuracy(0.1)).unwrap();
    for graph in [&mut fine, &mut coarse] {
        graph
            .create_primitive(
                Primitive::Cylinder {
                    radius: 1.0,
                    height: 2.0,
                },
                CreateOptions {
                    og_id: Some("cylinder".into()),
                    ..CreateOptions::default()
                },
            )
            .unwrap();
    }
    assert_eq!(
        static_bucket(&fine.brep("cylinder").unwrap()).unwrap(),
        static_bucket(&coarse.brep("cylinder").unwrap()).unwrap()
    );
}

#[test]
fn i8_library_enables_clippy_panics_and_unwrap_denials() {
    let root = include_str!("../src/lib.rs");
    for rule in [
        "clippy::unwrap_used",
        "clippy::expect_used",
        "clippy::panic",
        "clippy::unreachable",
    ] {
        assert!(root.contains(rule), "{rule}");
    }
}

#[test]
fn i9_public_input_types_reject_unknown_fields() {
    assert!(serde_json::from_str::<Accuracy>(
        r#"{"geometric":1e-8,"intersection":1e-9,"tessellation":0.01,"exchange":1e-6,"extra":1}"#
    )
    .is_err());
    assert!(serde_json::from_str::<Primitive>(
        r#"{"kind":"Cuboid","width":1,"height":1,"depth":1,"extra":1}"#
    )
    .is_err());
    assert!(serde_json::from_str::<CreateOptions>(r#"{"og_id":"body","extra":1}"#).is_err());
}

#[test]
fn i11_features_at_or_below_four_geometric_are_invalid_parameters() {
    let mut graph = WorldGraph::new(tessellation_accuracy(0.01)).unwrap();
    for primitive in [
        Primitive::Cuboid {
            width: 4e-8,
            height: 1.0,
            depth: 1.0,
        },
        Primitive::Cylinder {
            radius: 4e-8,
            height: 1.0,
        },
        Primitive::Rectangle {
            width: 1.0,
            breadth: 4e-8,
        },
        Primitive::Circle { radius: 4e-8 },
    ] {
        let error = graph
            .create_primitive(primitive, CreateOptions::default())
            .unwrap_err();
        assert_eq!(error.error_code(), ErrorCode::InvalidParameter);
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn i12_crate_has_no_cargo_features_or_feature_gated_source() {
    let manifest = include_str!("../Cargo.toml");
    assert!(!manifest.lines().any(|line| line.trim() == "[features]"));
    fn inspect(directory: &std::path::Path) {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                inspect(&path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let source = std::fs::read_to_string(&path).unwrap();
                assert!(!source.contains("cfg(feature"), "{}", path.display());
            }
        }
    }
    inspect(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"));
}
