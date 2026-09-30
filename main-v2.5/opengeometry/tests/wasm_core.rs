use opengeometry::brep::BrepEnvelope;
use opengeometry::tessellation::display::display_buffers;
use opengeometry::tessellation::{tessellate, SnapshotStore};
use opengeometry_test_support::tolerance::{ordered_bits, step_parts, within_four_ulp};
use serde_json::Value;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn embedded_source_corpus_roundtrips_with_cross_target_float_tolerance() {
    let corpus = [
        ("cuboid", include_str!("fixtures/parity/cuboid.brep.json")),
        (
            "cylinder",
            include_str!("fixtures/parity/cylinder.brep.json"),
        ),
        ("sphere", include_str!("fixtures/parity/sphere.brep.json")),
        ("cone", include_str!("fixtures/parity/cone.brep.json")),
        ("frustum", include_str!("fixtures/parity/frustum.brep.json")),
        ("torus", include_str!("fixtures/parity/torus.brep.json")),
        (
            "annular-cylinder",
            include_str!("fixtures/parity/annular-cylinder.brep.json"),
        ),
        (
            "cylinder-with-hole",
            include_str!("fixtures/parity/cylinder-with-hole.brep.json"),
        ),
        ("circle", include_str!("fixtures/parity/circle.brep.json")),
        (
            "linear-extrusion",
            include_str!("fixtures/parity/linear-extrusion.brep.json"),
        ),
        (
            "arc-edged-extrusion",
            include_str!("fixtures/parity/arc-edged-extrusion.brep.json"),
        ),
        (
            "arc-edged-extrusion-with-holes",
            include_str!("fixtures/parity/arc-edged-extrusion-with-holes.brep.json"),
        ),
        (
            "box-union",
            include_str!("fixtures/parity/box-union.brep.json"),
        ),
        (
            "box-intersection",
            include_str!("fixtures/parity/box-intersection.brep.json"),
        ),
        ("box-cut", include_str!("fixtures/parity/box-cut.brep.json")),
        (
            "box-cavity",
            include_str!("fixtures/parity/box-cavity.brep.json"),
        ),
        (
            "sphere-cut",
            include_str!("fixtures/parity/sphere-cut.brep.json"),
        ),
    ];
    for (name, source) in corpus {
        let body = BrepEnvelope::from_json(source).unwrap();
        body.validate().unwrap();
        let output = body.to_json().unwrap();
        let actual: Value = serde_json::from_str(&output).unwrap();
        let expected: Value = serde_json::from_str(source).unwrap();
        assert!(within_four_ulp(&actual, &expected), "{name}");
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn embedded_source_tessellation_corpus_matches_within_four_ulp() {
    macro_rules! check {
        ($name:literal) => {{
            let body = BrepEnvelope::from_json(include_str!(concat!("fixtures/parity/", $name, ".brep.json"))).unwrap();
            let mesh = tessellate(&body, 0.01, 2_000_000).unwrap();
            let actual = serde_json::json!({
                "positions": mesh.positions,
                "normals": mesh.normals,
                "indices": mesh.indices,
                "triangleFaceIds": mesh.triangle_face_ids,
                "outlinePositions": mesh.outline_positions,
                "outlineEdgeIds": mesh.outline_edge_ids,
                "revision": mesh.revision,
                "achievedDeflection": mesh.achieved_deflection,
            });
            let expected: Value = serde_json::from_str(include_str!(concat!("fixtures/parity/", $name, ".tess.json"))).unwrap();
            assert!(within_four_ulp(&actual, &expected), "{}", $name);
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

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn embedded_source_step_corpus_keeps_entity_structure_and_four_ulp_reals() {
    macro_rules! check {
        ($name:literal) => {{
            let body = BrepEnvelope::from_json(include_str!(concat!(
                "fixtures/parity/",
                $name,
                ".brep.json"
            )))
            .unwrap();
            let actual = opengeometry::exchange::export_step(&body, "metre")
                .unwrap()
                .0;
            let expected = include_str!(concat!("fixtures/parity/", $name, ".step.m"));
            let (actual_structure, actual_reals) = step_parts(&actual);
            let (expected_structure, expected_reals) = step_parts(expected);
            assert_eq!(actual_structure, expected_structure, "{} structure", $name);
            assert_eq!(
                actual_reals.len(),
                expected_reals.len(),
                "{} real count",
                $name
            );
            for (a, b) in actual_reals.into_iter().zip(expected_reals) {
                assert!(
                    a == b
                        || (a == 0.0 && b == 0.0)
                        || ordered_bits(a).abs_diff(ordered_bits(b)) <= 4,
                    "{} real {} differs from {}",
                    $name,
                    a,
                    b
                );
            }
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
    check!("linear-extrusion");
    check!("arc-edged-extrusion");
    check!("arc-edged-extrusion-with-holes");
    check!("box-union");
    check!("box-intersection");
    check!("box-cut");
    check!("box-cavity");
    check!("sphere-cut");
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn embedded_brep_parity_and_snapshot_buffers() {
    let bytes = include_bytes!("fixtures/parity/cuboid.brep.json");
    let json = std::str::from_utf8(bytes).unwrap();
    let brep = BrepEnvelope::from_json(json).unwrap();
    let roundtrip = brep.to_json().unwrap();
    assert_eq!(roundtrip, json);
    let mesh = tessellate(&brep, 0.01, 2_000_000).unwrap();
    let tessellation = serde_json::json!({
        "positions": mesh.positions,
        "normals": mesh.normals,
        "indices": mesh.indices,
        "triangleFaceIds": mesh.triangle_face_ids,
        "outlinePositions": mesh.outline_positions,
        "outlineEdgeIds": mesh.outline_edge_ids,
        "revision": mesh.revision,
        "achievedDeflection": mesh.achieved_deflection,
    });
    let fixture: serde_json::Value =
        serde_json::from_slice(include_bytes!("fixtures/parity/cuboid.tess.json")).unwrap();
    assert_eq!(tessellation, fixture);
    let direct = display_buffers(&brep, 0.01, 2_000_000).unwrap();
    let mut tessellator = SnapshotStore::new();
    let slot = tessellator.load(roundtrip.as_bytes()).unwrap();
    assert_eq!(direct, tessellator.buffers(slot, 0.01, 2_000_000).unwrap());
}
