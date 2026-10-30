mod tests;

use super::error::GeometryError;
use super::topology::BrepEnvelope;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct ValidationReport {
    valid: bool,
    validation_level: &'static str,
    error: Option<GeometryError>,
}

fn validate_json(serialized: &str) -> ValidationReport {
    let result = BrepEnvelope::from_json(serialized);
    ValidationReport {
        valid: result.is_ok(),
        validation_level: "structure and sampled geometry residuals",
        error: result.err(),
    }
}
