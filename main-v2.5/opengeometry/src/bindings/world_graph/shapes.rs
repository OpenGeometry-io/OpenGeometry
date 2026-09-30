use super::{serialise, OGWorldGraph};
use crate::bindings::errors;
use crate::bindings::tessellator::buffers_to_js;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsValue;

#[wasm_bindgen]
impl OGWorldGraph {
    pub fn revision(&self) -> u64 {
        self.inner.revision()
    }

    pub fn brep(&self, og_id: &str) -> Result<String, JsValue> {
        self.inner
            .brep(og_id)
            .map_err(errors::json)?
            .to_json()
            .map_err(|error| errors::json(error.into()))
    }

    pub fn report(&self, og_id: &str) -> Result<String, JsValue> {
        serialise(&self.inner.report(og_id).map_err(errors::json)?)
    }

    #[wasm_bindgen(js_name = edgeKeys)]
    pub fn edge_keys(&self, og_id: &str) -> Result<String, JsValue> {
        serialise(&self.inner.edge_keys(og_id).map_err(errors::json)?)
    }

    pub fn snapshot(&self, shape_id: &str) -> Result<Vec<u8>, JsValue> {
        self.inner.snapshot(shape_id).map_err(errors::json)
    }

    pub fn buffers(
        &self,
        shape_id: &str,
        bucket: f64,
        max_triangles: u32,
    ) -> Result<JsValue, JsValue> {
        buffers_to_js(
            self.inner
                .buffers(shape_id, bucket, max_triangles as usize)
                .map_err(errors::json)?,
        )
    }
}
