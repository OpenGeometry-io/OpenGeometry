use crate::support::parity_fixtures;
use opengeometry::brep::BrepEnvelope;
use opengeometry::operations::modifying::boolean::subtract_planar_cutters_with_handlers;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::Path;

fn read_fixture(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

#[test]
fn batch_subtraction_matches_source_brep_and_handlers() {
    let files = parity_fixtures("batch-matrix", |name| {
        !name.contains(".step.") && !name.contains(".fallback.")
    });
    assert_eq!(files.len(), 23);
    for path in files {
        let name = path.file_stem().unwrap().to_str().unwrap();
        let fixture = read_fixture(&path);
        let host: BrepEnvelope = serde_json::from_value(fixture["host"].clone()).unwrap();
        let cutters: Vec<BrepEnvelope> =
            serde_json::from_value(fixture["cutters"].clone()).unwrap();
        match subtract_planar_cutters_with_handlers(&host, &cutters, format!("batch-{name}")) {
            Ok((result, handlers)) => {
                result.brep.validate().unwrap();
                let actual = json!({"brep": serde_json::to_value(result.brep).unwrap()});
                assert_eq!(actual, fixture["result"], "{name}");
                assert_eq!(json!(handlers), fixture["handlers"], "{name}");
            }
            Err(error) => assert_eq!(json!({"error": error}), fixture["result"], "{name}"),
        }
    }
}

#[test]
fn fallback_rows_parse() {
    let expected = BTreeSet::from(["cutters", "handlers", "host", "result"]);
    for path in parity_fixtures("batch-matrix", |name| name.contains(".fallback.")) {
        let fixture = read_fixture(&path);
        let keys = fixture
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        assert_eq!(keys, expected, "{}", path.display());
    }
}
