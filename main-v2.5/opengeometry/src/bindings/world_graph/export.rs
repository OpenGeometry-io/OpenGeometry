use super::{serialise, OGWorldGraph};
use crate::bindings::{errors, params};
use crate::world_graph::StepOptions;
use js_sys::{Object, Reflect};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsValue;

#[wasm_bindgen]
impl OGWorldGraph {
    #[wasm_bindgen(js_name = exportStep)]
    pub fn export_step(&self, nodes_json: &str, options_json: &str) -> Result<JsValue, JsValue> {
        let nodes: Vec<String> = params::parse(nodes_json).map_err(errors::json)?;
        params::ids(&nodes).map_err(errors::json)?;
        let options: StepOptions = params::parse(options_json).map_err(errors::json)?;
        let (text, report) = self
            .inner
            .export_step(&nodes, &options)
            .map_err(errors::json)?;
        let result = Object::new();
        Reflect::set(
            &result,
            &JsValue::from_str("text"),
            &JsValue::from_str(&text),
        )?;
        Reflect::set(
            &result,
            &JsValue::from_str("reportJson"),
            &JsValue::from_str(&serialise(&report)?),
        )?;
        Ok(result.into())
    }
}
