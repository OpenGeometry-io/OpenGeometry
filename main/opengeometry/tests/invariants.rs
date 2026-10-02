use opengeometry::brep::{Accuracy, BrepEnvelope, Frame3, GeometryError, PatchBounds};
use opengeometry::primitives;
use opengeometry::tessellation::display::static_bucket;
use opengeometry::world_graph::{
    CopyOptions, CreateOptions, CreatingOperation, ErrorCode, Primitive, StepOptions, Transform,
    WorldGraph,
};

const CUBOID: &str = include_str!("fixtures/cases/cuboid.brep.json");

#[test]
fn i5_graph_accepts_only_the_standard_accuracy_and_the_display_bucket_ignores_the_budget() {
    let code = |accuracy| {
        WorldGraph::new(accuracy)
            .err()
            .map(|error| error.error_code())
    };
    assert_eq!(
        code(Accuracy {
            geometric: 0.0,
            ..Accuracy::STANDARD
        }),
        Some(ErrorCode::InvalidGeometry)
    );
    assert_eq!(
        code(Accuracy {
            intersection: 1e-7,
            ..Accuracy::STANDARD
        }),
        Some(ErrorCode::InvalidGeometry)
    );
    let coarse = Accuracy {
        tessellation: 0.1,
        ..Accuracy::STANDARD
    };
    assert_eq!(code(coarse), Some(ErrorCode::InvalidParameter));
    assert_eq!(code(Accuracy::STANDARD), None);
    let cylinder = |accuracy| {
        primitives::cylinder("cylinder".into(), Frame3::IDENTITY, 1.0, 2.0, accuracy).unwrap()
    };
    assert_eq!(
        static_bucket(&cylinder(Accuracy::STANDARD)).unwrap(),
        static_bucket(&cylinder(coarse)).unwrap()
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
    assert!(serde_json::from_str::<CreateOptions>(r#"{"ogId":"body","extra":1}"#).is_err());
    assert!(BrepEnvelope::from_json(CUBOID).is_ok());
    for key in [r#""range":{"#, r#""uv_bounds":[{"#] {
        assert!(CUBOID.contains(key), "{key}");
        let injected = CUBOID.replacen(key, &format!(r#"{key}"extra":1,"#), 1);
        assert!(BrepEnvelope::from_json(&injected).is_err(), "{key}");
    }
    assert!(serde_json::from_str::<PatchBounds>(
        r#"{"axes":[{"lo":0,"hi":1},{"lo":0,"hi":1},{"lo":0,"hi":1}],"extra":1}"#
    )
    .is_err());
    assert!(serde_json::from_str::<GeometryError>(
        r#"{"CoverageGap":{"families":["a","b"],"extra":1}}"#
    )
    .is_err());
    assert!(serde_json::from_str::<Transform>(
        r#"{"kind":"Translate","offset":[0,0,0],"extra":1}"#
    )
    .is_err());
    assert!(serde_json::from_str::<CopyOptions>(r#"{"ogId":"copy","extra":1}"#).is_err());
    assert!(serde_json::from_str::<CreatingOperation>(
        r#"{"kind":"Sweep","profile":"p","path":"q","extra":1}"#
    )
    .is_err());
    assert!(serde_json::from_str::<StepOptions>(r#"{"unit":"metre","extra":1}"#).is_err());
}

#[test]
fn i11_features_at_or_below_four_geometric_are_invalid_parameters() {
    let mut graph = WorldGraph::new(Accuracy::STANDARD).unwrap();
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
