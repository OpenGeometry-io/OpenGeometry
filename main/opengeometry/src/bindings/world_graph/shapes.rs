use super::{serialise, OGWorldGraph};
use crate::bindings::errors;
use crate::bindings::tessellator::buffers_to_js;
use crate::tessellation::display::{bucket_floor, static_bucket};
use crate::world_graph::{ErrorCode, GraphError};
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

    #[wasm_bindgen(js_name = worldBrep)]
    pub fn world_brep(&self, og_id: &str) -> Result<String, JsValue> {
        self.inner
            .placed_brep(og_id)
            .map_err(errors::json)?
            .to_json()
            .map_err(|error| errors::json(error.into()))
    }

    #[wasm_bindgen(js_name = faceNormalAt)]
    pub fn face_normal_at(
        &self,
        og_id: &str,
        face_id: u32,
        point: &[f64],
    ) -> Result<Vec<f64>, JsValue> {
        self.inner.node(og_id).map_err(errors::json)?;
        let point: [f64; 3] = point
            .try_into()
            .ok()
            .filter(|point: &[f64; 3]| point.iter().all(|value| value.is_finite()))
            .ok_or_else(|| {
                errors::json(GraphError::code(
                    ErrorCode::InvalidParameter,
                    "point must be three finite numbers",
                ))
            })?;
        Ok(self
            .inner
            .face_normal_at(og_id, face_id, point)
            .map_err(errors::json)?
            .to_vec())
    }

    pub fn report(&self, og_id: &str) -> Result<String, JsValue> {
        serialise(&self.inner.report(og_id).map_err(errors::json)?)
    }

    #[wasm_bindgen(js_name = edgeKeys)]
    pub fn edge_keys(&self, og_id: &str) -> Result<String, JsValue> {
        serialise(&self.inner.edge_keys(og_id).map_err(errors::json)?)
    }

    #[wasm_bindgen(js_name = displayBuckets)]
    pub fn display_buckets(&self, og_id: &str) -> Result<String, JsValue> {
        let brep = self.inner.brep(og_id).map_err(errors::json)?;
        let floor = bucket_floor(&brep).map_err(|error| errors::json(error.into()))?;
        let bucket = static_bucket(&brep).map_err(|error| errors::json(error.into()))?;
        serialise(&serde_json::json!({"floor": floor, "static": bucket}))
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
