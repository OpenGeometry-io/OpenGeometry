use opengeometry::brep::BrepEnvelope;
use serde_json::{json, Value};

#[test]
fn i10_builders_and_operations_validate_against_source_verdicts() {
    macro_rules! check {
        ($name:literal) => {{
            let source = include_str!(concat!("fixtures/parity/validity/", $name, ".json"));
            let expected: Value = serde_json::from_str(include_str!(concat!(
                "fixtures/parity/validity/",
                $name,
                ".verdict.json"
            )))
            .expect($name);
            let actual = match BrepEnvelope::from_json(source) {
                Ok(_) => json!({ "valid": true }),
                Err(error) => json!({ "valid": false, "error": error }),
            };
            assert_eq!(actual, expected, "{}", $name);
        }};
    }
    check!("builder-cuboid");
    check!("builder-cylinder");
    check!("builder-sphere");
    check!("builder-cone");
    check!("builder-frustum");
    check!("builder-torus");
    check!("builder-annular-cylinder");
    check!("builder-cylinder-with-hole");
    check!("builder-circle");
    check!("builder-linear-extrusion");
    check!("builder-arc-edged-extrusion");
    check!("builder-arc-edged-extrusion-with-holes");
    check!("dense-ids");
    check!("twin-symmetry");
    check!("next-prev");
    check!("loop-membership");
    check!("face-pcurve");
    check!("wire-membership");
    check!("closed-wire-cycle");
    check!("open-wire-end");
}
