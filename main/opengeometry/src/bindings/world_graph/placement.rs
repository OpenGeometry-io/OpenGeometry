use super::{changed, serialise, OGWorldGraph};
use crate::bindings::{errors, params};
use crate::world_graph::{EditScope, ModifyingOperation, Transform};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsValue;

#[wasm_bindgen]
impl OGWorldGraph {
    pub fn operate(
        &mut self,
        og_id: &str,
        operation_json: &str,
        tools_json: &str,
        scope_json: &str,
    ) -> Result<String, JsValue> {
        params::id(og_id).map_err(errors::json)?;
        let operation: ModifyingOperation = params::parse(operation_json).map_err(errors::json)?;
        let tools: Vec<String> = params::parse(tools_json).map_err(errors::json)?;
        params::ids(&tools).map_err(errors::json)?;
        let scope: EditScope = params::parse(scope_json).map_err(errors::json)?;
        changed(
            self.inner
                .operate(og_id, operation, &tools, scope)
                .map_err(errors::json)?,
        )
    }

    pub fn transform(&mut self, og_id: &str, transform_json: &str) -> Result<String, JsValue> {
        params::id(og_id).map_err(errors::json)?;
        let transform: Transform = params::parse(transform_json).map_err(errors::json)?;
        changed(
            self.inner
                .transform(og_id, transform)
                .map_err(errors::json)?,
        )
    }

    pub fn placement(&self, og_id: &str) -> Result<String, JsValue> {
        serialise(&self.inner.placement(og_id).map_err(errors::json)?)
    }

    #[wasm_bindgen(js_name = worldPlacement)]
    pub fn world_placement(&self, og_id: &str) -> Result<String, JsValue> {
        serialise(
            &self
                .inner
                .world_placement_form(og_id)
                .map_err(errors::json)?,
        )
    }

    #[wasm_bindgen(js_name = worldMatrix)]
    pub fn world_matrix(&self, og_id: &str) -> Result<Vec<f64>, JsValue> {
        Ok(self
            .inner
            .world_matrix(og_id)
            .map_err(errors::json)?
            .to_vec())
    }

    pub fn bounds(&self, og_id: &str) -> Result<String, JsValue> {
        serialise(&self.inner.bounds(og_id).map_err(errors::json)?)
    }

    #[wasm_bindgen(js_name = localBounds)]
    pub fn local_bounds(&self, og_id: &str) -> Result<String, JsValue> {
        serialise(&self.inner.local_bounds(og_id).map_err(errors::json)?)
    }
}
