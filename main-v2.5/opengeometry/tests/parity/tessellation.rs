use crate::support::{parity_fixtures, read_fixture};
use opengeometry::brep::BrepEnvelope;
use opengeometry::tessellation::tessellate;
use opengeometry_test_support::canonical::canonical_tessellation;
use serde_json::json;
use std::collections::BTreeMap;
use std::path::Path;

#[test]
fn source_tessellation_matches_for_supported_primitive_fixtures() {
    macro_rules! check {
        ($name:literal) => {{
            let body = BrepEnvelope::from_json(include_str!(concat!("../fixtures/parity/", $name, ".brep.json"))).unwrap();
            let mesh = tessellate(&body, 0.01, 2_000_000).unwrap();
            let actual = json!({
                "positions": mesh.positions,
                "normals": mesh.normals,
                "indices": mesh.indices,
                "triangleFaceIds": mesh.triangle_face_ids,
                "outlinePositions": mesh.outline_positions,
                "outlineEdgeIds": mesh.outline_edge_ids,
                "revision": mesh.revision,
                "achievedDeflection": mesh.achieved_deflection,
            });
            let expected: serde_json::Value = serde_json::from_str(include_str!(concat!("../fixtures/parity/", $name, ".tess.json"))).unwrap();
            assert_eq!(actual, expected, "{}", $name);
        }};
    }
    check!("cuboid");
    check!("cylinder");
    check!("sphere");
    check!("cone");
    check!("frustum");
    check!("torus");
    check!("annular-cylinder");
    check!("cylinder-with-hole");
    check!("circle");
    check!("linear-extrusion");
    check!("arc-edged-extrusion");
    check!("arc-edged-extrusion-with-holes");
}

#[test]
fn boolean_fixture_tessellation_matches_source_canonically() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/parity");
    let mut sources = BTreeMap::new();
    for name in [
        "box-union",
        "box-intersection",
        "box-cut",
        "box-cavity",
        "sphere-cut",
    ] {
        let source = std::fs::read_to_string(root.join(format!("{name}.brep.json"))).unwrap();
        sources.insert(name.to_string(), BrepEnvelope::from_json(&source).unwrap());
    }
    for (folder, prefix) in [("boolean-matrix", "matrix"), ("batch-matrix", "batch")] {
        for path in parity_fixtures(folder, |name| {
            !name.contains(".step.") && !name.contains(".fallback.")
        }) {
            let fixture = read_fixture(&path);
            if let Some(brep) = fixture["result"].get("brep") {
                let name = path.file_stem().unwrap().to_str().unwrap();
                sources.insert(
                    format!("{prefix}-{name}"),
                    serde_json::from_value(brep.clone()).unwrap(),
                );
            }
        }
    }
    let mut listed = parity_fixtures("tessellation", |_| true)
        .iter()
        .map(|path| path.file_stem().unwrap().to_str().unwrap().to_string())
        .collect::<Vec<_>>();
    listed.sort();
    assert_eq!(listed, sources.keys().cloned().collect::<Vec<_>>());
    for (name, body) in &sources {
        let actual = match tessellate(body, 0.01, 2_000_000) {
            Ok(mesh) => canonical_tessellation(&mesh),
            Err(error) => json!({"error": error}),
        };
        let expected = read_fixture(&root.join("tessellation").join(format!("{name}.json")));
        assert_eq!(actual, expected, "{name}");
    }
}

#[test]
fn i7_repeated_tessellation_is_bitwise_identical() {
    for source in [
        include_str!("../fixtures/parity/cylinder.brep.json"),
        include_str!("../fixtures/parity/torus.brep.json"),
        include_str!("../fixtures/parity/arc-edged-extrusion.brep.json"),
    ] {
        let body = BrepEnvelope::from_json(source).unwrap();
        let first = tessellate(&body, 0.01, 2_000_000).unwrap();
        let second = tessellate(&body, 0.01, 2_000_000).unwrap();
        assert_eq!(
            first
                .positions
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
            second
                .positions
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            first
                .normals
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
            second
                .normals
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>()
        );
        assert_eq!(first.indices, second.indices);
        assert_eq!(first.triangle_face_ids, second.triangle_face_ids);
        assert_eq!(
            first
                .outline_positions
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
            second
                .outline_positions
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>()
        );
        assert_eq!(first.outline_edge_ids, second.outline_edge_ids);
    }
}
