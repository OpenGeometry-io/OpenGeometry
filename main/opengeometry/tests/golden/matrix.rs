use crate::boolean_case::{record_boolean_handlers, result_body};
use crate::kernel::{BooleanOp, BooleanResult, BrepEnvelope, GeometryError};
use crate::record::{Failure, Record};
use crate::runner::Case;
use serde_json::{json, to_value, Value};
use std::fs;
use std::path::{Path, PathBuf};

const FIXTURE_DIRECTORY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/parity/boolean-matrix"
);

const MATRIX_FIXTURES: usize = 18;

const VERDICTS: [&str; 2] = ["result", "handlers"];

fn fixture_paths() -> Result<Vec<PathBuf>, Failure> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(FIXTURE_DIRECTORY)? {
        let path = entry?.path();
        let json = path
            .extension()
            .is_some_and(|extension| extension == "json");
        let step = path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().contains(".step."));
        if json && !step {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

fn record_verdict(record: &mut Record, subject: &str, equal: bool) {
    record.field(subject, if equal { "equal" } else { "different" });
    record.note_verdict(subject);
    if !equal {
        record.disagree(subject);
    }
}

fn record_fixture_agreement(
    record: &mut Record,
    fixture: &Value,
    result: &Result<BooleanResult, GeometryError>,
    handlers: &[String],
) -> Result<(), Failure> {
    let actual = match result {
        Ok(result) => json!({"brep": to_value(&result.brep)?}),
        Err(error) => json!({"error": to_value(error)?}),
    };
    record.section("fixture");
    record_verdict(record, "result", actual == fixture["result"]);
    record_verdict(record, "handlers", json!(handlers) == fixture["handlers"]);
    Ok(())
}

struct MatrixFixture {
    expected: Value,
    a: BrepEnvelope,
    b: BrepEnvelope,
    operation: BooleanOp,
}

fn load_fixture(path: &Path) -> Result<MatrixFixture, Failure> {
    let expected: Value = serde_json::from_slice(&fs::read(path)?)?;
    Ok(MatrixFixture {
        a: serde_json::from_value(expected["a"].clone())?,
        b: serde_json::from_value(expected["b"].clone())?,
        operation: serde_json::from_value(expected["operation"].clone())?,
        expected,
    })
}

fn matrix_case(name: &str, path: PathBuf) -> Case {
    let id = format!("matrix-{name}");
    Case::new(format!("booleans.matrix.{name}"), move |record| {
        let MatrixFixture {
            expected,
            a,
            b,
            operation,
        } = load_fixture(&path).inspect_err(|_| record.disagree("fixture-unreadable"))?;
        let (result, handlers) = record_boolean_handlers(record, &a, &b, operation, &id);
        record_fixture_agreement(record, &expected, &result, &handlers)?;
        result_body(record, result);
        Ok(())
    })
    .requiring_verdicts(&VERDICTS)
}

pub(crate) fn cases() -> Result<Vec<Case>, Failure> {
    let paths = fixture_paths()?;
    if paths.len() != MATRIX_FIXTURES {
        return Err(format!(
            "matrix: exactly {MATRIX_FIXTURES} matrix fixtures are required; found {} in {FIXTURE_DIRECTORY}",
            paths.len()
        )
        .into());
    }
    paths
        .into_iter()
        .map(|path| {
            let name = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .ok_or("matrix fixture name is not UTF-8")?
                .to_string();
            Ok(matrix_case(&name, path))
        })
        .collect()
}
