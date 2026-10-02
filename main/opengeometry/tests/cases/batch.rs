use crate::support::{case_files, read_fixture};
use opengeometry::brep::BrepEnvelope;
use opengeometry::operations::modifying::boolean::{
    multi_tool_boolean, subtract_planar_cutters_with_handlers, BooleanOp, MultiToolOutcome,
};
use opengeometry_test_support::stored;
use serde_json::json;
use std::collections::BTreeSet;

#[test]
fn batch_subtraction_matches_stored_brep_and_handlers() {
    let files = case_files("batch-matrix", |name| !name.contains(".fallback."));
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
                if stored::compared() {
                    assert_eq!(actual, fixture["result"], "{name}");
                }
                assert_eq!(json!(handlers), fixture["handlers"], "{name}");
            }
            Err(error) => assert_eq!(json!({"error": error}), fixture["result"], "{name}"),
        }
    }
}

#[test]
fn fallback_rows_parse() {
    let expected = BTreeSet::from(["cutters", "handlers", "host", "result"]);
    let files = case_files("batch-matrix", |name| name.contains(".fallback."));
    assert_eq!(files.len(), 3);
    for path in files {
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

#[test]
fn fallback_rows_match_stored_serial_results() {
    for path in case_files("batch-matrix", |name| name.contains(".fallback.")) {
        let stem = path.file_stem().unwrap().to_str().unwrap();
        let name = stem.strip_suffix(".fallback").unwrap();
        let fixture = read_fixture(&path);
        let host: BrepEnvelope = serde_json::from_value(fixture["host"].clone()).unwrap();
        let cutters: Vec<BrepEnvelope> =
            serde_json::from_value(fixture["cutters"].clone()).unwrap();
        let MultiToolOutcome {
            result, handlers, ..
        } = multi_tool_boolean(
            &host,
            &cutters,
            BooleanOp::Subtraction,
            format!("batch-{name}"),
        );
        let actual = match result {
            Ok(result) => {
                result.brep.validate().unwrap();
                json!({"brep": serde_json::to_value(result.brep).unwrap()})
            }
            Err(error) => json!({"error": error}),
        };
        if stored::compared() || fixture["result"].get("error").is_some() {
            assert_eq!(actual, fixture["result"], "{name}");
        }
        assert_eq!(json!(handlers), fixture["handlers"], "{name}");
    }
}
