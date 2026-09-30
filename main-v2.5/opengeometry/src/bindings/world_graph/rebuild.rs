use super::{changed, OGWorldGraph};
use crate::bindings::{errors, params};
use crate::operations::CreatingOperation;
use crate::world_graph::{EditScope, ErrorCode, GraphError, Primitive};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsValue;

#[wasm_bindgen]
impl OGWorldGraph {
    #[wasm_bindgen(js_name = rebuildPrimitive)]
    pub fn rebuild_primitive(
        &mut self,
        og_id: &str,
        primitive_json: &str,
        scope_json: &str,
    ) -> Result<String, JsValue> {
        params::id(og_id).map_err(errors::json)?;
        let primitive: Primitive = params::parse(primitive_json).map_err(errors::json)?;
        if matches!(primitive, Primitive::Polyline { .. }) {
            return Err(errors::json(GraphError::code(
                ErrorCode::InvalidParameter,
                "polyline points must use rebuildPolyline",
            )));
        }
        let scope: EditScope = params::parse(scope_json).map_err(errors::json)?;
        changed(
            self.inner
                .rebuild_primitive(og_id, primitive, scope)
                .map_err(errors::json)?,
        )
    }

    #[wasm_bindgen(js_name = rebuildPolyline)]
    pub fn rebuild_polyline(
        &mut self,
        og_id: &str,
        points: &[f64],
        closed: bool,
        scope_json: &str,
    ) -> Result<String, JsValue> {
        params::id(og_id).map_err(errors::json)?;
        let scope: EditScope = params::parse(scope_json).map_err(errors::json)?;
        let points = params::points(points).map_err(errors::json)?;
        changed(
            self.inner
                .rebuild_primitive(og_id, Primitive::Polyline { points, closed }, scope)
                .map_err(errors::json)?,
        )
    }

    #[wasm_bindgen(js_name = rebuildOperation)]
    pub fn rebuild_operation(
        &mut self,
        og_id: &str,
        operation_json: &str,
        scope_json: &str,
    ) -> Result<String, JsValue> {
        params::id(og_id).map_err(errors::json)?;
        let operation: CreatingOperation = params::parse(operation_json).map_err(errors::json)?;
        if matches!(&operation, CreatingOperation::Extrude { holes, .. } if holes.len() > 1_000) {
            return Err(errors::json(GraphError::code(
                ErrorCode::LimitExceeded,
                "profile hole limit",
            )));
        }
        let scope: EditScope = params::parse(scope_json).map_err(errors::json)?;
        changed(
            self.inner
                .rebuild_operation(og_id, operation, scope)
                .map_err(errors::json)?,
        )
    }
}
