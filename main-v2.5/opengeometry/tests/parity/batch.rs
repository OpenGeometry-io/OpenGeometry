use opengeometry::brep::BrepEnvelope;
use opengeometry::operations::modifying::boolean::subtract_planar_cutters_with_handlers;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn batch_fixtures(keep: impl Fn(&str) -> bool) -> Vec<PathBuf> {
    let directory =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/parity/batch-matrix");
    let mut files = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            let name = path.file_name().unwrap().to_string_lossy();
            name.ends_with(".json") && keep(&name)
        })
        .collect::<Vec<_>>();
    files.sort();
    files
}

fn read_fixture(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

#[test]
fn batch_subtraction_matches_source_brep_and_handlers() {
    let files = batch_fixtures(|name| !name.contains(".step.") && !name.contains(".fallback."));
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
    for path in batch_fixtures(|name| name.contains(".fallback.")) {
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
