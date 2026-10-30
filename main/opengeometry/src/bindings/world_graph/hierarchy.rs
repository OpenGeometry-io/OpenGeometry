use super::{changed, check_copy, created, serialise, OGWorldGraph};
use crate::bindings::{errors, params};
use crate::world_graph::CopyOptions;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsValue;

#[wasm_bindgen]
impl OGWorldGraph {
    pub fn node(&self, og_id: &str) -> Result<String, JsValue> {
        let node = self.inner.node(og_id).map_err(errors::json)?;
        let shape = if node.shape.is_some() {
            Some(self.inner.shape(og_id).map_err(errors::json)?)
        } else {
            None
        };
        let body_type = shape
            .map(|value| value.brep.body_type())
            .transpose()
            .map_err(|error| errors::json(error.into()))?
            .map(|kind| format!("{kind:?}"));
        serialise(&serde_json::json!({
            "ogId": node.og_id,
            "handle": node.handle,
            "generation": node.generation,
            "parent": node.parent,
            "children": node.children,
            "kind": node.kind,
            "shapeId": node.shape,
            "shapeRevision": shape.map(|value| value.revision),
            "bodyType": body_type,
        }))
    }

    #[wasm_bindgen(js_name = nodeByHandle)]
    pub fn node_by_handle(&self, handle: u32, generation: u32) -> Result<String, JsValue> {
        let node = self
            .inner
            .node_by_handle(handle, generation)
            .map_err(errors::json)?;
        self.node(&node.og_id)
    }

    pub fn children(&self, og_id: &str) -> Result<String, JsValue> {
        serialise(&self.inner.children(og_id).map_err(errors::json)?)
    }

    pub fn parent(&self, og_id: &str) -> Result<String, JsValue> {
        serialise(&self.inner.parent(og_id).map_err(errors::json)?)
    }

    #[wasm_bindgen(js_name = addChild)]
    pub fn add_child(
        &mut self,
        parent: &str,
        children_json: &str,
        keep_world: bool,
    ) -> Result<String, JsValue> {
        let children: Vec<String> = params::parse(children_json).map_err(errors::json)?;
        params::ids(&children).map_err(errors::json)?;
        changed(
            self.inner
                .add_child(parent, &children, keep_world)
                .map_err(errors::json)?,
        )
    }

    #[wasm_bindgen(js_name = removeChild)]
    pub fn remove_child(
        &mut self,
        parent: &str,
        child: &str,
        keep_world: bool,
    ) -> Result<String, JsValue> {
        changed(
            self.inner
                .remove_child(parent, child, keep_world)
                .map_err(errors::json)?,
        )
    }

    pub fn instance(&mut self, source: &str, options_json: &str) -> Result<String, JsValue> {
        let options: CopyOptions = params::parse(options_json).map_err(errors::json)?;
        check_copy(&options).map_err(errors::json)?;
        created(self.inner.instance(source, options).map_err(errors::json)?)
    }

    pub fn duplicate(&mut self, source: &str, options_json: &str) -> Result<String, JsValue> {
        let options: CopyOptions = params::parse(options_json).map_err(errors::json)?;
        check_copy(&options).map_err(errors::json)?;
        created(
            self.inner
                .duplicate(source, options)
                .map_err(errors::json)?,
        )
    }

    #[wasm_bindgen(js_name = makeUnique)]
    pub fn make_unique(&mut self, og_id: &str) -> Result<String, JsValue> {
        serialise(&self.inner.make_unique(og_id).map_err(errors::json)?)
    }

    #[wasm_bindgen(js_name = instanceCount)]
    pub fn instance_count(&self, og_id: &str) -> Result<u32, JsValue> {
        self.inner.instance_count(og_id).map_err(errors::json)
    }

    pub fn dispose(&mut self, og_id: &str) -> Result<String, JsValue> {
        changed(self.inner.dispose(og_id).map_err(errors::json)?)
    }
}
