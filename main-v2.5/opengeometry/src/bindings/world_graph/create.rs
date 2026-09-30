use super::{check_options, created, OGWorldGraph};
use crate::bindings::{errors, params};
use crate::world_graph::{CreateOptions, CreatingOperation, ErrorCode, GraphError, Primitive};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsValue;

#[wasm_bindgen]
impl OGWorldGraph {
    #[wasm_bindgen(js_name = createPrimitive)]
    pub fn create_primitive(
        &mut self,
        primitive_json: &str,
        options_json: &str,
    ) -> Result<String, JsValue> {
        let primitive: Primitive = params::parse(primitive_json).map_err(errors::json)?;
        let options: CreateOptions = params::parse(options_json).map_err(errors::json)?;
        check_options(&options).map_err(errors::json)?;
        if matches!(primitive, Primitive::Polyline { .. }) {
            return Err(errors::json(GraphError::code(
                ErrorCode::InvalidParameter,
                "polyline points must use createPolyline",
            )));
        }
        created(
            self.inner
                .create_primitive(primitive, options)
                .map_err(errors::json)?,
        )
    }

    #[wasm_bindgen(js_name = createPolyline)]
    pub fn create_polyline(
        &mut self,
        points: &[f64],
        closed: bool,
        options_json: &str,
    ) -> Result<String, JsValue> {
        let options: CreateOptions = params::parse(options_json).map_err(errors::json)?;
        check_options(&options).map_err(errors::json)?;
        let points = params::points(points).map_err(errors::json)?;
        created(
            self.inner
                .create_primitive(Primitive::Polyline { points, closed }, options)
                .map_err(errors::json)?,
        )
    }

    #[wasm_bindgen(js_name = createOperation)]
    pub fn create_operation(
        &mut self,
        operation_json: &str,
        options_json: &str,
    ) -> Result<String, JsValue> {
        let operation: CreatingOperation = params::parse(operation_json).map_err(errors::json)?;
        let options: CreateOptions = params::parse(options_json).map_err(errors::json)?;
        check_options(&options).map_err(errors::json)?;
        if matches!(&operation, CreatingOperation::Extrude { holes, .. } if holes.len() > 1_000) {
            return Err(errors::json(GraphError::code(
                ErrorCode::LimitExceeded,
                "profile hole limit",
            )));
        }
        created(
            self.inner
                .create_operation(operation, options)
                .map_err(errors::json)?,
        )
    }

    #[wasm_bindgen(js_name = createSystemAssembly)]
    pub fn create_system_assembly(&mut self, options_json: &str) -> Result<String, JsValue> {
        let options: CreateOptions = params::parse(options_json).map_err(errors::json)?;
        check_options(&options).map_err(errors::json)?;
        created(
            self.inner
                .create_system_assembly(options)
                .map_err(errors::json)?,
        )
    }
}
