use opengeometry::brep::BrepEnvelope;
use opengeometry::tessellation::tessellate;
use serde_json::json;

#[test]
fn primitive_tessellation_matches_stored_fixtures() {
    macro_rules! check {
        ($name:literal) => {{
            let body = BrepEnvelope::from_json(include_str!(concat!("../fixtures/cases/", $name, ".brep.json"))).unwrap();
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
            let expected: serde_json::Value = serde_json::from_str(include_str!(concat!("../fixtures/cases/", $name, ".tess.json"))).unwrap();
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
fn i7_repeated_tessellation_is_bitwise_identical() {
    for source in [
        include_str!("../fixtures/cases/cylinder.brep.json"),
        include_str!("../fixtures/cases/torus.brep.json"),
        include_str!("../fixtures/cases/arc-edged-extrusion.brep.json"),
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
