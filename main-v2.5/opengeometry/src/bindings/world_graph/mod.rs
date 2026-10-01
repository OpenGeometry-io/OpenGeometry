mod create;
mod export;
mod hierarchy;
mod marks;
mod placement;
mod rebuild;
mod shapes;

use crate::bindings::{errors, panic, params};
use crate::brep::Accuracy;
use crate::world_graph::{ChangeSet, CopyOptions, CreateOptions, GraphError, MarkId, WorldGraph};
use serde::Serialize;
use std::collections::BTreeMap;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsValue;

fn serialise<T: Serialize>(value: &T) -> Result<String, JsValue> {
    serde_json::to_string(value).map_err(errors::serialization)
}

fn changed(changes: ChangeSet) -> Result<String, JsValue> {
    serialise(&changes)
}

fn created(value: (String, ChangeSet)) -> Result<String, JsValue> {
    serialise(&serde_json::json!({"ogId": value.0, "changes": value.1}))
}

fn check_options(options: &CreateOptions) -> Result<(), GraphError> {
    if let Some(id) = &options.og_id {
        params::id(id)?;
    }
    if let Some(parent) = &options.parent {
        params::id(parent)?;
    }
    Ok(())
}

fn check_copy(options: &CopyOptions) -> Result<(), GraphError> {
    if let Some(id) = &options.og_id {
        params::id(id)?;
    }
    Ok(())
}

#[wasm_bindgen]
pub struct OGWorldGraph {
    inner: WorldGraph,
    marks: BTreeMap<u32, MarkId>,
    next_mark: u32,
}

#[wasm_bindgen]
impl OGWorldGraph {
    #[wasm_bindgen(constructor)]
    pub fn new(accuracy_json: &str) -> Result<Self, JsValue> {
        panic::install_panic_hook();
        let accuracy: Accuracy = params::parse(accuracy_json).map_err(errors::json)?;
        let inner = WorldGraph::new(accuracy).map_err(errors::json)?;
        Ok(Self {
            inner,
            marks: BTreeMap::new(),
            next_mark: 0,
        })
    }
}
