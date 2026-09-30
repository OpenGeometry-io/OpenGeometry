use super::{BrepEnvelope, GeometryError};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ValidationReport {
    pub valid: bool,
    pub validation_level: &'static str,
    pub error: Option<GeometryError>,
}

pub fn validate_json(serialized: &str) -> ValidationReport {
    let result = BrepEnvelope::from_json(serialized);
    ValidationReport {
        valid: result.is_ok(),
        validation_level: "v2 structure and sampled geometry residuals",
        error: result.err(),
    }
}
