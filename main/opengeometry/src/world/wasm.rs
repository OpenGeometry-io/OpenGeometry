use super::{NodeSpec, ProjectionView, Similarity3, WorldError, WorldGraph};
use crate::analytic::BrepEnvelope;
use crate::brep::Brep;
use crate::export::projection::{CameraParameters, HlrOptions};
use crate::export::wasm::{
    optional_json, OGIfcExportResult, OGStepExportResult, OGStlExportResult,
};
use crate::export::{IfcExportConfig, StepExportConfig, StlExportConfig};
use wasm_bindgen::prelude::*;

const MAX_NODE_JSON_BYTES: usize = 8 * 1024;
const MAX_ID_LIST_BYTES: usize = 4 * 1024 * 1024;
const MAX_LEGACY_BREP_BYTES: usize = 64 * 1024 * 1024;

fn js_error(error: WorldError) -> JsValue {
    JsValue::from_str(
        &serde_json::to_string(&error)
            .unwrap_or_else(|_| "world graph error could not be serialized".into()),
    )
}

fn parse<T: serde::de::DeserializeOwned>(json: &str, what: &str) -> Result<T, JsValue> {
    if json.len() > MAX_NODE_JSON_BYTES {
        return Err(js_error(WorldError::InvalidInput(format!(
            "{what} exceeds 8 KiB"
        ))));
    }
    serde_json::from_str(json)
        .map_err(|error| js_error(WorldError::InvalidInput(format!("{what}: {error}"))))
}

fn parse_options<T: serde::de::DeserializeOwned + Default>(
    json: Option<String>,
    what: &str,
) -> Result<T, JsValue> {
    optional_json(json, what).map_err(|error| js_error(WorldError::InvalidInput(error)))
}

fn export_failure(error: String) -> JsValue {
    js_error(WorldError::Export(error))
}

fn parse_ids(json: &str) -> Result<Vec<String>, JsValue> {
    if json.len() > MAX_ID_LIST_BYTES {
        return Err(js_error(WorldError::InvalidInput(
            "node id list exceeds 4 MiB".into(),
        )));
    }
    serde_json::from_str(json)
        .map_err(|error| js_error(WorldError::InvalidInput(format!("node id list: {error}"))))
}

#[wasm_bindgen]
pub struct OGWorldGraph {
    graph: WorldGraph,
}

impl Default for OGWorldGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl OGWorldGraph {
    #[wasm_bindgen(constructor)]
    pub fn new() -> OGWorldGraph {
        Self {
            graph: WorldGraph::new(),
        }
    }

    pub fn define(&mut self, id: &str, brep_json: &str) -> Result<(), JsValue> {
        let brep = BrepEnvelope::from_json(brep_json)
            .map_err(|error| js_error(WorldError::Geometry(error)))?;
        self.graph.define(id, brep).map_err(js_error)
    }

    pub fn define_legacy(&mut self, id: &str, brep_json: &str) -> Result<(), JsValue> {
        if brep_json.len() > MAX_LEGACY_BREP_BYTES {
            return Err(js_error(WorldError::InvalidInput(
                "legacy BRep JSON exceeds 64 MiB".into(),
            )));
        }
        let brep: Brep = serde_json::from_str(brep_json).map_err(|error| {
            js_error(WorldError::InvalidInput(format!(
                "legacy BRep {id}: {error}"
            )))
        })?;
        self.graph.define_legacy(id, brep).map_err(js_error)
    }

    pub fn undefine(&mut self, id: &str) -> Result<(), JsValue> {
        self.graph.undefine(id).map_err(js_error)
    }

    pub fn has_definition(&self, id: &str) -> bool {
        self.graph.has_definition(id)
    }

    pub fn add_node(&mut self, spec_json: &str) -> Result<(), JsValue> {
        let spec: NodeSpec = parse(spec_json, "node spec")?;
        self.graph.add_node(spec).map_err(js_error)
    }

    pub fn set_local(&mut self, id: &str, local_json: &str) -> Result<(), JsValue> {
        let local: Similarity3 = parse(local_json, "local transform")?;
        self.graph.set_local(id, local).map_err(js_error)
    }

    pub fn reparent(&mut self, id: &str, parent: Option<String>) -> Result<(), JsValue> {
        self.graph.reparent(id, parent.as_deref()).map_err(js_error)
    }

    pub fn remove_node(&mut self, id: &str) -> Result<(), JsValue> {
        self.graph.remove_node(id).map_err(js_error)
    }

    pub fn has_node(&self, id: &str) -> bool {
        self.graph.has_node(id)
    }

    pub fn get_world(&self, id: &str) -> Result<Vec<f64>, JsValue> {
        Ok(self
            .graph
            .world(id)
            .map_err(js_error)?
            .to_column_major()
            .to_vec())
    }

    pub fn get_world_bounds(&self, id: &str) -> Result<Vec<f64>, JsValue> {
        Ok(self
            .graph
            .world_bounds(id)
            .map_err(js_error)?
            .map(|bounds| bounds.to_vec())
            .unwrap_or_default())
    }

    pub fn clash(
        &self,
        items_a_json: &str,
        items_b_json: &str,
        depth: f64,
    ) -> Result<String, JsValue> {
        let items_a: Vec<String> = parse_ids(items_a_json)?;
        let items_b: Vec<String> = parse_ids(items_b_json)?;
        let clashes = self
            .graph
            .clash(&items_a, &items_b, depth)
            .map_err(js_error)?;
        serde_json::to_string(&clashes)
            .map_err(|error| js_error(WorldError::InvalidInput(error.to_string())))
    }

    pub fn clearance(
        &self,
        items_a_json: &str,
        items_b_json: &str,
        clearance: f64,
    ) -> Result<String, JsValue> {
        let items_a: Vec<String> = parse_ids(items_a_json)?;
        let items_b: Vec<String> = parse_ids(items_b_json)?;
        let found = self
            .graph
            .clearance(&items_a, &items_b, clearance)
            .map_err(js_error)?;
        serde_json::to_string(&found)
            .map_err(|error| js_error(WorldError::InvalidInput(error.to_string())))
    }

    pub fn distance(&self, item_a: &str, item_b: &str, tolerance: f64) -> Result<String, JsValue> {
        let found = self
            .graph
            .distance(item_a, item_b, tolerance)
            .map_err(js_error)?;
        serde_json::to_string(&found)
            .map_err(|error| js_error(WorldError::InvalidInput(error.to_string())))
    }

    pub fn project_lines(
        &self,
        items_json: &str,
        camera_json: &str,
        hlr_json: Option<String>,
        deflection: f64,
    ) -> Result<String, JsValue> {
        let items = parse_ids(items_json)?;
        let camera: CameraParameters = parse_options(Some(camera_json.to_string()), "camera")?;
        let hlr: HlrOptions = parse_options(hlr_json, "HLR")?;
        let scene = self
            .graph
            .project(&items, &camera, &hlr, deflection)
            .map_err(js_error)?;
        serde_json::to_string(&scene.to_lines())
            .map_err(|error| js_error(WorldError::InvalidInput(error.to_string())))
    }

    pub fn project_views(
        &self,
        items_json: &str,
        views_json: &str,
        deflection: f64,
    ) -> Result<String, JsValue> {
        let items = parse_ids(items_json)?;
        if views_json.len() > MAX_ID_LIST_BYTES {
            return Err(js_error(WorldError::InvalidInput(
                "projection views exceed 4 MiB".into(),
            )));
        }
        let views: Vec<ProjectionView> = serde_json::from_str(views_json)
            .map_err(|error| js_error(WorldError::InvalidInput(format!("views: {error}"))))?;
        let projected = self
            .graph
            .project_views(&items, &views, deflection)
            .map_err(js_error)?;
        serde_json::to_string(&projected)
            .map_err(|error| js_error(WorldError::InvalidInput(error.to_string())))
    }

    pub fn export_stl(
        &self,
        items_json: &str,
        config_json: Option<String>,
    ) -> Result<OGStlExportResult, JsValue> {
        let config: StlExportConfig = parse_options(config_json, "STL config")?;
        let (bytes, report) = self
            .graph
            .export_stl(&parse_ids(items_json)?, &config)
            .map_err(js_error)?;
        OGStlExportResult::from_parts(bytes, report).map_err(export_failure)
    }

    pub fn export_step(
        &self,
        items_json: &str,
        config_json: Option<String>,
    ) -> Result<OGStepExportResult, JsValue> {
        let config: StepExportConfig = parse_options(config_json, "STEP config")?;
        let (text, report) = self
            .graph
            .export_step(&parse_ids(items_json)?, &config)
            .map_err(js_error)?;
        OGStepExportResult::from_parts(text, report).map_err(export_failure)
    }

    pub fn export_ifc(
        &self,
        items_json: &str,
        config_json: Option<String>,
    ) -> Result<OGIfcExportResult, JsValue> {
        let config: IfcExportConfig = parse_options(config_json, "IFC config")?;
        let (text, report) = self
            .graph
            .export_ifc(&parse_ids(items_json)?, &config)
            .map_err(js_error)?;
        OGIfcExportResult::from_parts(text, report).map_err(export_failure)
    }

    pub fn node_count(&self) -> u32 {
        self.graph.node_count() as u32
    }

    pub fn definition_count(&self) -> u32 {
        self.graph.definition_count() as u32
    }
}
