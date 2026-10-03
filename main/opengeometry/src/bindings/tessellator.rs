use super::{errors, panic};
use crate::tessellation::{display::DisplayBuffers, SnapshotStore};
use js_sys::{Float32Array, Float64Array, Object, Reflect, Uint32Array};
use wasm_bindgen::{prelude::wasm_bindgen, JsValue};

fn field(object: &Object, name: &str, value: &JsValue) -> Result<(), JsValue> {
    Reflect::set(object, &JsValue::from_str(name), value).map(|_| ())
}

pub(super) fn buffers_to_js(buffers: DisplayBuffers) -> Result<JsValue, JsValue> {
    let result = Object::new();
    field(
        &result,
        "positions",
        &Float32Array::from(buffers.positions.as_slice()),
    )?;
    field(
        &result,
        "normals",
        &Float32Array::from(buffers.normals.as_slice()),
    )?;
    field(
        &result,
        "indices",
        &Uint32Array::from(buffers.indices.as_slice()),
    )?;
    field(
        &result,
        "faceRanges",
        &Uint32Array::from(buffers.face_ranges.as_slice()),
    )?;
    field(
        &result,
        "outline",
        &Float32Array::from(buffers.outline.as_slice()),
    )?;
    field(
        &result,
        "edgeIds",
        &Uint32Array::from(buffers.edge_ids.as_slice()),
    )?;
    field(
        &result,
        "origin",
        &Float64Array::from(buffers.origin.as_slice()),
    )?;
    field(
        &result,
        "revision",
        &JsValue::from_f64(buffers.revision as f64),
    )?;
    field(&result, "bucket", &JsValue::from_f64(buffers.bucket))?;
    field(
        &result,
        "achievedDeflection",
        &JsValue::from_f64(buffers.achieved_deflection),
    )?;
    field(
        &result,
        "triangles",
        &JsValue::from_f64(f64::from(buffers.triangles)),
    )?;
    Ok(result.into())
}

#[wasm_bindgen]
pub struct OGTessellator {
    inner: SnapshotStore,
}

#[wasm_bindgen]
impl OGTessellator {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        panic::install_panic_hook();
        Self {
            inner: SnapshotStore::new(),
        }
    }

    pub fn load(&mut self, bytes: &[u8]) -> Result<u32, JsValue> {
        self.inner
            .load(bytes)
            .map_err(|error| errors::json(error.into()))
    }

    pub fn buffers(&self, slot: u32, bucket: f64, max_triangles: u32) -> Result<JsValue, JsValue> {
        buffers_to_js(
            self.inner
                .buffers(slot, bucket, max_triangles as usize)
                .map_err(|error| errors::json(error.into()))?,
        )
    }

    #[wasm_bindgen(js_name = drop)]
    pub fn release(&mut self, slot: u32) -> bool {
        self.inner.release(slot)
    }
}

impl Default for OGTessellator {
    fn default() -> Self {
        Self::new()
    }
}
