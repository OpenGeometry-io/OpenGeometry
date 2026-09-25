use crate::brep::Brep;
use crate::export::ifc::{export_brep_to_ifc_text, IfcExportConfig, IfcExportReport};
use crate::export::step::{export_brep_to_step_text, StepExportConfig, StepExportReport};
use crate::export::stl::{export_brep_to_stl_bytes, StlExportConfig, StlExportReport};
use serde::de::DeserializeOwned;
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct OGStlExportResult {
    bytes: Vec<u8>,
    report_json: String,
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

fn report_json(report: &impl Serialize) -> Result<String, String> {
    serde_json::to_string(report)
        .map_err(|error| format!("Failed to serialize export report: {error}"))
}

impl OGStlExportResult {
    pub(crate) fn from_parts(bytes: Vec<u8>, report: StlExportReport) -> Result<Self, String> {
        Ok(Self {
            bytes,
            report_json: report_json(&report)?,
        })
    }
}

impl OGStepExportResult {
    pub(crate) fn from_parts(text: String, report: StepExportReport) -> Result<Self, String> {
        Ok(Self {
            text,
            report_json: report_json(&report)?,
        })
    }
}

impl OGIfcExportResult {
    pub(crate) fn from_parts(text: String, report: IfcExportReport) -> Result<Self, String> {
        Ok(Self {
            text,
            report_json: report_json(&report)?,
        })
    }
}

pub(crate) fn optional_json<T: DeserializeOwned + Default>(
    payload: Option<String>,
    what: &str,
) -> Result<T, String> {
    match payload {
        Some(json) if !json.trim().is_empty() => serde_json::from_str(&json)
            .map_err(|error| format!("Invalid {what} JSON payload: {error}")),
        _ => Ok(T::default()),
    }
}

fn parse_brep(brep_serialized: &str) -> Result<Brep, JsValue> {
    serde_json::from_str(brep_serialized).map_err(|error| {
        JsValue::from_str(&format!("Failed to deserialize BRep JSON payload: {error}"))
    })
}

#[wasm_bindgen(js_name = exportBrepToStl)]
pub fn export_brep_to_stl(
    brep_serialized: &str,
    config_json: Option<String>,
) -> Result<OGStlExportResult, JsValue> {
    let config: StlExportConfig =
        optional_json(config_json, "STL config").map_err(|error| JsValue::from_str(&error))?;
    let (bytes, report) = export_brep_to_stl_bytes(&parse_brep(brep_serialized)?, &config)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    OGStlExportResult::from_parts(bytes, report).map_err(|error| JsValue::from_str(&error))
}

#[wasm_bindgen(js_name = exportBrepToStep)]
pub fn export_brep_to_step(
    brep_serialized: &str,
    config_json: Option<String>,
) -> Result<OGStepExportResult, JsValue> {
    let config: StepExportConfig =
        optional_json(config_json, "STEP config").map_err(|error| JsValue::from_str(&error))?;
    let (text, report) = export_brep_to_step_text(&parse_brep(brep_serialized)?, &config)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    OGStepExportResult::from_parts(text, report).map_err(|error| JsValue::from_str(&error))
}

#[wasm_bindgen(js_name = exportBrepToIfc)]
pub fn export_brep_to_ifc(
    brep_serialized: &str,
    config_json: Option<String>,
) -> Result<OGIfcExportResult, JsValue> {
    let config: IfcExportConfig =
        optional_json(config_json, "IFC config").map_err(|error| JsValue::from_str(&error))?;
    let (text, report) = export_brep_to_ifc_text(&parse_brep(brep_serialized)?, &config)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    OGIfcExportResult::from_parts(text, report).map_err(|error| JsValue::from_str(&error))
}
