use super::{serialise, OGWorldGraph};
use crate::bindings::errors;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsValue;

#[wasm_bindgen]
impl OGWorldGraph {
    #[wasm_bindgen(js_name = exportIfcBody)]
    pub fn export_ifc_body(&self, og_id: &str, model_unit: &str) -> Result<String, JsValue> {
        serialise(
            &self
                .inner
                .export_ifc_body(og_id, model_unit)
                .map_err(errors::json)?,
        )
    }
}
