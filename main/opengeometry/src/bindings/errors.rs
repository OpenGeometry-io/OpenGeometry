use crate::world_graph::{ErrorCode, ErrorDetails, GraphError};
use serde::Serialize;
use wasm_bindgen::JsValue;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ErrorDto {
    pub(crate) code: ErrorCode,
    pub(crate) message: String,
    pub(crate) details: serde_json::Value,
}

pub(crate) fn dto(error: GraphError) -> ErrorDto {
    let details = match error.details() {
        ErrorDetails::None => serde_json::json!({}),
        details => serde_json::to_value(details).unwrap_or(serde_json::json!({})),
    };
    ErrorDto {
        code: error.error_code(),
        message: error.message(),
        details,
    }
}

pub(super) fn json(error: GraphError) -> JsValue {
    let rendered = serde_json::to_value(dto(error))
        .map(|value| value.to_string())
        .unwrap_or_default();
    JsValue::from_str(&rendered)
}

pub(super) fn serialization(error: serde_json::Error) -> JsValue {
    json(GraphError::code(
        ErrorCode::InvalidGeometry,
        error.to_string(),
    ))
}
