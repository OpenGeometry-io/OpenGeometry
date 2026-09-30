use opengeometry::brep::BrepEnvelope;
use serde_json::{json, Value};
use std::path::Path;

const VERDICT_SUFFIX: &str = ".verdict.json";

#[test]
fn i10_every_validity_fixture_reproduces_its_source_verdict() {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/parity/validity");
    let mut names: Vec<String> = std::fs::read_dir(&folder)
        .unwrap()
        .filter_map(|entry| {
            let file = entry.unwrap().file_name().to_string_lossy().into_owned();
            file.strip_suffix(VERDICT_SUFFIX).map(str::to_owned)
        })
        .collect();
    names.sort();
    assert_eq!(names.len(), 28);
    for name in names {
        let source = std::fs::read_to_string(folder.join(format!("{name}.json"))).unwrap();
        let verdict = std::fs::read(folder.join(format!("{name}{VERDICT_SUFFIX}"))).unwrap();
        let expected: Value = serde_json::from_slice(&verdict).expect(&name);
        let actual = match BrepEnvelope::from_json(&source) {
            Ok(_) => json!({ "valid": true }),
            Err(error) => json!({ "valid": false, "error": error }),
        };
        assert_eq!(actual, expected, "{name}");
    }
}
