use std::collections::HashMap;

use opengeometry_export_io::ifc::{
    export_snapshot_to_ifc_text, IfcExportConfig as IoIfcExportConfig,
    IfcExportReport as IoIfcExportReport,
};
use opengeometry_export_io::stl::{
    export_snapshot_to_stl_bytes, StlExportConfig as IoStlExportConfig,
    StlExportReport as IoStlExportReport,
};
use opengeometry_export_schema::{ExportMaterial, ExportSceneSnapshot, IfcEntitySemantics};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::brep::Brep;
use crate::export::step::{export_breps_to_step_text, StepExportConfig, StepExportReport};

#[wasm_bindgen]
pub struct OGStlExportResult {
    bytes: Vec<u8>,
    report_json: String,
}

impl OGStlExportResult {
    fn from_parts(bytes: Vec<u8>, report: IoStlExportReport) -> Result<Self, String> {
        let report_json = serde_json::to_string(&report)
            .map_err(|err| format!("Failed to serialize STL export report: {}", err))?;
        Ok(Self { bytes, report_json })
    }
}

#[wasm_bindgen]
impl OGStlExportResult {
    #[wasm_bindgen(getter)]
    pub fn bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    #[wasm_bindgen(getter, js_name = reportJson)]
    pub fn report_json(&self) -> String {
        self.report_json.clone()
    }
}

#[wasm_bindgen]
pub struct OGStepExportResult {
    text: String,
    report_json: String,
}

impl OGStepExportResult {
    fn from_parts(text: String, report: StepExportReport) -> Result<Self, String> {
        let report_json = serde_json::to_string(&report)
            .map_err(|err| format!("Failed to serialize STEP export report: {}", err))?;
        Ok(Self { text, report_json })
    }
}

#[wasm_bindgen]
impl OGStepExportResult {
    #[wasm_bindgen(getter)]
    pub fn text(&self) -> String {
        self.text.clone()
    }

    #[wasm_bindgen(getter, js_name = reportJson)]
    pub fn report_json(&self) -> String {
        self.report_json.clone()
    }
}

#[wasm_bindgen]
pub struct OGIfcExportResult {
    text: String,
    report_json: String,
}

impl OGIfcExportResult {
    fn from_parts(text: String, report: IoIfcExportReport) -> Result<Self, String> {
        let report_json = serde_json::to_string(&report)
            .map_err(|err| format!("Failed to serialize IFC export report: {}", err))?;
        Ok(Self { text, report_json })
    }
}

#[wasm_bindgen]
impl OGIfcExportResult {
    #[wasm_bindgen(getter)]
    pub fn text(&self) -> String {
        self.text.clone()
    }

    #[wasm_bindgen(getter, js_name = reportJson)]
    pub fn report_json(&self) -> String {
        self.report_json.clone()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct IfcFileExportConfig {
    #[serde(flatten)]
    export: IoIfcExportConfig,
    #[serde(default)]
    semantics: Option<HashMap<String, IfcEntitySemantics>>,
    #[serde(default)]
    materials: Option<Vec<ExportMaterial>>,
    #[serde(default)]
    entity_materials: Option<HashMap<String, String>>,
}

#[wasm_bindgen]
pub struct OGFileExporter;

#[wasm_bindgen]
impl OGFileExporter {
    #[wasm_bindgen(constructor)]
    pub fn new() -> OGFileExporter {
        OGFileExporter
    }

    #[wasm_bindgen(js_name = exportSceneSnapshotToIfc)]
    pub fn export_scene_snapshot_to_ifc(
        &self,
        snapshot_json: String,
        config_json: Option<String>,
    ) -> Result<OGIfcExportResult, JsValue> {
        let mut snapshot: ExportSceneSnapshot = serde_json::from_str(&snapshot_json)
            .map_err(|err| JsValue::from_str(&format!("Invalid snapshot JSON payload: {}", err)))?;
        let config = parse_ifc_config_json(config_json).map_err(|err| JsValue::from_str(&err))?;

        if let Some(semantics) = &config.semantics {
            for entity in &mut snapshot.entities {
                if let Some(entity_semantics) = semantics.get(&entity.id) {
                    entity.semantics = Some(entity_semantics.clone());
                }
            }
        }

        if let Some(materials) = &config.materials {
            snapshot.materials = materials.clone();
        }

        if let Some(entity_materials) = &config.entity_materials {
            for entity in &mut snapshot.entities {
                if let Some(material_id) = entity_materials.get(&entity.id) {
                    entity.material_id = Some(material_id.clone());
                }
            }
        }

        let (text, report) = export_snapshot_to_ifc_text(&snapshot, &config.export)
            .map_err(|err| JsValue::from_str(&err.to_string()))?;

        OGIfcExportResult::from_parts(text, report).map_err(|err| JsValue::from_str(&err))
    }

    #[wasm_bindgen(js_name = exportSceneSnapshotToStl)]
    pub fn export_scene_snapshot_to_stl(
        &self,
        snapshot_json: String,
        config_json: Option<String>,
    ) -> Result<OGStlExportResult, JsValue> {
        let snapshot: ExportSceneSnapshot = serde_json::from_str(&snapshot_json)
            .map_err(|err| JsValue::from_str(&format!("Invalid snapshot JSON payload: {}", err)))?;
        let config = parse_stl_config_json(config_json).map_err(|err| JsValue::from_str(&err))?;

        let (bytes, report) = export_snapshot_to_stl_bytes(&snapshot, &config)
            .map_err(|err| JsValue::from_str(&err.to_string()))?;

        OGStlExportResult::from_parts(bytes, report).map_err(|err| JsValue::from_str(&err))
    }

    #[wasm_bindgen(js_name = exportSceneSnapshotToStep)]
    pub fn export_scene_snapshot_to_step(
        &self,
        snapshot_json: String,
        config_json: Option<String>,
    ) -> Result<OGStepExportResult, JsValue> {
        let snapshot: ExportSceneSnapshot = serde_json::from_str(&snapshot_json)
            .map_err(|err| JsValue::from_str(&format!("Invalid snapshot JSON payload: {}", err)))?;
        let config = parse_step_config_json(config_json).map_err(|err| JsValue::from_str(&err))?;

        let mut owned_breps = Vec::<Brep>::new();
        for entity in &snapshot.entities {
            let Some(brep_json) = entity.brep_json.as_deref() else {
                continue;
            };
            let brep: Brep = serde_json::from_str(brep_json).map_err(|err| {
                JsValue::from_str(&format!(
                    "Entity '{}' has invalid brep_json payload: {}",
                    entity.id, err
                ))
            })?;
            owned_breps.push(brep);
        }

        if owned_breps.is_empty() {
            return Err(JsValue::from_str(
                "Snapshot does not contain any entity brep_json payloads for STEP export",
            ));
        }

        let brep_refs: Vec<&Brep> = owned_breps.iter().collect();
        let (text, report) = export_breps_to_step_text(brep_refs, &config)
            .map_err(|err| JsValue::from_str(&err.to_string()))?;

        OGStepExportResult::from_parts(text, report).map_err(|err| JsValue::from_str(&err))
    }
}

fn parse_ifc_config_json(config_json: Option<String>) -> Result<IfcFileExportConfig, String> {
    match config_json {
        Some(payload) if !payload.trim().is_empty() => serde_json::from_str(&payload)
            .map_err(|err| format!("Invalid IFC config JSON payload: {}", err)),
        _ => Ok(IfcFileExportConfig::default()),
    }
}

fn parse_stl_config_json(config_json: Option<String>) -> Result<IoStlExportConfig, String> {
    match config_json {
        Some(payload) if !payload.trim().is_empty() => serde_json::from_str(&payload)
            .map_err(|err| format!("Invalid STL config JSON payload: {}", err)),
        _ => Ok(IoStlExportConfig::default()),
    }
}

fn parse_step_config_json(config_json: Option<String>) -> Result<StepExportConfig, String> {
    match config_json {
        Some(payload) if !payload.trim().is_empty() => serde_json::from_str(&payload)
            .map_err(|err| format!("Invalid STEP config JSON payload: {}", err)),
        _ => Ok(StepExportConfig::default()),
    }
}
