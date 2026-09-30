use crate::brep::GeometryError;
use crate::world_graph::{ErrorCode, GraphError};
use wasm_bindgen::JsValue;

pub(super) fn json(error: GraphError) -> JsValue {
    let code = error.error_code();
    let (message, mut details) = match error {
        GraphError::Code {
            message, details, ..
        } => (message, details),
        GraphError::Geometry(source) => {
            let details = match &source {
                GeometryError::MissingReference { kind, index } => {
                    serde_json::json!({"kind": kind, "index": index})
                }
                GeometryError::CoverageGap { families } => {
                    serde_json::json!({"families": families})
                }
                GeometryError::Math(math) => {
                    serde_json::json!({"math": format!("{math:?}")})
                }
                _ => serde_json::Value::Null,
            };
            (source.to_string(), details)
        }
    };
    if details.is_null() {
        details = serde_json::json!({});
    }
    let code = serde_json::to_value(code).unwrap_or(serde_json::json!("InvalidGeometry"));
    JsValue::from_str(
        &serde_json::json!({"code": code, "message": message, "details": details}).to_string(),
    )
}

pub(super) fn serialization(error: serde_json::Error) -> JsValue {
    json(GraphError::code(
        ErrorCode::InvalidGeometry,
        error.to_string(),
    ))
}
