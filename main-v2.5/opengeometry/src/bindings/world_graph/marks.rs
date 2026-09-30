use super::{changed, serialise, OGWorldGraph};
use crate::bindings::errors;
use crate::world_graph::{ErrorCode, GraphError};
use js_sys::{Float64Array, Object, Reflect};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsValue;

#[wasm_bindgen]
impl OGWorldGraph {
    pub fn mark(&mut self) -> Result<u32, JsValue> {
        let slot = self.next_mark;
        self.next_mark = self.next_mark.checked_add(1).ok_or_else(|| {
            errors::json(GraphError::code(
                ErrorCode::LimitExceeded,
                "mark handle limit",
            ))
        })?;
        let mark = self.inner.mark().map_err(errors::json)?;
        self.marks.insert(slot, mark);
        Ok(slot)
    }

    pub fn rollback(&mut self, slot: u32) -> Result<String, JsValue> {
        let mark = *self.marks.get(&slot).ok_or_else(|| {
            errors::json(GraphError::code(
                ErrorCode::InvalidMark,
                "mark handle is stale",
            ))
        })?;
        changed(self.inner.rollback(mark).map_err(errors::json)?)
    }

    pub fn release(&mut self, slot: u32) -> Result<(), JsValue> {
        let mark = *self.marks.get(&slot).ok_or_else(|| {
            errors::json(GraphError::code(
                ErrorCode::InvalidMark,
                "mark handle is stale",
            ))
        })?;
        self.inner.release(mark).map_err(errors::json)?;
        self.marks.remove(&slot);
        Ok(())
    }

    #[wasm_bindgen(js_name = markStats)]
    pub fn mark_stats(&self) -> Result<String, JsValue> {
        let stats = self.inner.mark_stats();
        serialise(
            &serde_json::json!({"liveMarks": stats.live_marks, "retainedRevisions": stats.retained_revisions}),
        )
    }

    #[wasm_bindgen(js_name = changesSince)]
    pub fn changes_since(&self, revision: u64) -> Result<JsValue, JsValue> {
        let changes = self.inner.changes_since(revision).map_err(errors::json)?;
        let mut matrices = Vec::with_capacity((changes.added.len() + changes.changed.len()) * 16);
        for node in changes.added.iter().chain(&changes.changed) {
            matrices.extend(self.inner.world_matrix(&node.og_id).map_err(errors::json)?);
        }
        let result = Object::new();
        Reflect::set(
            &result,
            &JsValue::from_str("changesJson"),
            &JsValue::from_str(&serialise(&changes)?),
        )?;
        Reflect::set(
            &result,
            &JsValue::from_str("matrices"),
            &Float64Array::from(matrices.as_slice()),
        )?;
        Ok(result.into())
    }
}
