use opengeometry::brep::{BrepEnvelope, GeometryError};
use opengeometry::operations::modifying::boolean::{boolean_brep_outcome_with_handlers, BooleanOp};
use serde_json::Value;
use std::error::Error;
use std::time::{Duration, Instant};

const ROTATED_BOX_UNION: &str =
    include_str!("../tests/fixtures/cases/boolean-matrix/rotated-box-union.json");
const COVERAGE_GAP_BOUND: Duration = Duration::from_secs(2);

fn main() -> Result<(), Box<dyn Error>> {
    let fixture: Value = serde_json::from_str(ROTATED_BOX_UNION)?;
    let a: BrepEnvelope = serde_json::from_value(fixture["a"].clone())?;
    let b: BrepEnvelope = serde_json::from_value(fixture["b"].clone())?;
    let started = Instant::now();
    let (result, _) = boolean_brep_outcome_with_handlers(
        &a,
        &b,
        BooleanOp::Union,
        "budgets-rotated-box-union".into(),
    );
    let elapsed = started.elapsed();
    println!(
        "rotated-box union: {} ms against a bound of {} ms",
        elapsed.as_secs_f64() * 1000.0,
        COVERAGE_GAP_BOUND.as_millis()
    );
    if !matches!(result, Err(GeometryError::CoverageGap { .. })) {
        return Err("the rotated-box union no longer returns CoverageGap".into());
    }
    if elapsed >= COVERAGE_GAP_BOUND {
        return Err("the rotated-box union CoverageGap exceeded its bound".into());
    }
    Ok(())
}
