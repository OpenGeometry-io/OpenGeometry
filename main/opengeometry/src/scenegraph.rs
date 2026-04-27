use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use wasm_bindgen::prelude::*;

use crate::brep::Brep;
use crate::export::ifc::{
    export_brep_to_ifc_text, export_scene_entities_to_ifc_text, IfcEntityInput, IfcExportConfig,
    IfcExportReport,
};
use crate::export::projection::{
    project_brep_to_scene, CameraParameters, HlrOptions, Scene2D, Scene2DLines,
};
use crate::export::step::{
    export_brep_to_step_text, export_breps_to_step_text, StepExportConfig, StepExportReport,
};
use crate::export::stl::{
    export_brep_to_stl_bytes, export_breps_to_stl_bytes, StlExportConfig, StlExportReport,
};
use crate::primitives::arc::OGArc;
use crate::primitives::cuboid::OGCuboid;
use crate::primitives::cylinder::OGCylinder;
use crate::primitives::line::OGLine;
use crate::primitives::polygon::OGPolygon;
use crate::primitives::polyline::OGPolyline;
use crate::primitives::rectangle::OGRectangle;
use crate::primitives::sphere::OGSphere;
use crate::primitives::wedge::OGWedge;

#[cfg(not(target_arch = "wasm32"))]
use crate::export::ifc::export_scene_entities_to_ifc_file;
#[cfg(not(target_arch = "wasm32"))]
use crate::export::pdf::{export_scene_to_pdf_with_config, PdfExportConfig};
#[cfg(not(target_arch = "wasm32"))]
use crate::export::step::export_breps_to_step_file;
#[cfg(not(target_arch = "wasm32"))]
use crate::export::stl::export_breps_to_stl_file;

#[derive(Clone, Serialize, Deserialize)]
pub struct SceneEntity {
    pub id: String,
    pub kind: String,
    pub brep: Brep,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct OGScene {
    pub id: String,
    pub name: String,
    pub entities: Vec<SceneEntity>,
}

impl OGScene {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            entities: Vec::new(),
        }
    }

    pub fn upsert_entity(&mut self, entity: SceneEntity) {
        if let Some(existing) = self.entities.iter_mut().find(|item| item.id == entity.id) {
            *existing = entity;
            return;
        }

        self.entities.push(entity);
    }

    pub fn remove_entity(&mut self, entity_id: &str) -> bool {
        let before = self.entities.len();
        self.entities.retain(|entity| entity.id != entity_id);
        before != self.entities.len()
    }

    pub fn project_to_2d(&self, camera: &CameraParameters, hlr: &HlrOptions) -> Scene2D {
        let mut projected = Scene2D::with_name(self.name.clone());
        for entity in &self.entities {
            projected.extend(project_brep_to_scene(&entity.brep, camera, hlr));
        }
        projected
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SceneSummary {
    pub id: String,
    pub name: String,
    pub entity_count: usize,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct StlExportPayload {
    pub bytes: Vec<u8>,
    pub report: StlExportReport,
}

#[wasm_bindgen]
pub struct OGStlExportResult {
    bytes: Vec<u8>,
    report_json: String,
}

impl OGStlExportResult {
    fn from_parts(bytes: Vec<u8>, report: StlExportReport) -> Result<Self, String> {
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
    fn from_parts(text: String, report: IfcExportReport) -> Result<Self, String> {
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

#[wasm_bindgen]
pub struct OGSceneManager {
    scenes: HashMap<String, OGScene>,
    current_scene_id: Option<String>,
}

impl Default for OGSceneManager {
    fn default() -> Self {
        Self {
            scenes: HashMap::new(),
            current_scene_id: None,
        }
    }
}

impl OGSceneManager {
    fn get_scene(&self, scene_id: &str) -> Result<&OGScene, String> {
        self.scenes
            .get(scene_id)
            .ok_or_else(|| format!("Scene '{}' does not exist", scene_id))
    }

    fn get_scene_mut(&mut self, scene_id: &str) -> Result<&mut OGScene, String> {
        self.scenes
            .get_mut(scene_id)
            .ok_or_else(|| format!("Scene '{}' does not exist", scene_id))
    }

    fn current_scene_id_result(&self) -> Result<String, String> {
        self.current_scene_id
            .clone()
            .ok_or_else(|| "No current scene selected".to_string())
    }

    fn parse_camera_json(camera_json: &str) -> Result<CameraParameters, String> {
        if camera_json.trim().is_empty() {
            return Ok(CameraParameters::default());
        }

        serde_json::from_str(camera_json)
            .map_err(|err| format!("Invalid camera JSON payload: {}", err))
    }

    fn parse_hlr_json(hlr_json: Option<String>) -> Result<HlrOptions, String> {
        match hlr_json {
            Some(payload) if !payload.trim().is_empty() => serde_json::from_str(&payload)
                .map_err(|err| format!("Invalid HLR JSON payload: {}", err)),
            _ => Ok(HlrOptions::default()),
        }
    }

    fn parse_stl_config_json(config_json: Option<String>) -> Result<StlExportConfig, String> {
        match config_json {
            Some(payload) if !payload.trim().is_empty() => serde_json::from_str(&payload)
                .map_err(|err| format!("Invalid STL config JSON payload: {}", err)),
            _ => Ok(StlExportConfig::default()),
        }
    }

    fn parse_step_config_json(config_json: Option<String>) -> Result<StepExportConfig, String> {
        match config_json {
            Some(payload) if !payload.trim().is_empty() => serde_json::from_str(&payload)
                .map_err(|err| format!("Invalid STEP config JSON payload: {}", err)),
            _ => Ok(StepExportConfig::default()),
        }
    }

    fn parse_ifc_config_json(config_json: Option<String>) -> Result<IfcExportConfig, String> {
        match config_json {
            Some(payload) if !payload.trim().is_empty() => serde_json::from_str(&payload)
                .map_err(|err| format!("Invalid IFC config JSON payload: {}", err)),
            _ => Ok(IfcExportConfig::default()),
        }
    }

    fn upsert_entity_brep(
        &mut self,
        scene_id: &str,
        entity_id: String,
        kind: String,
        brep: Brep,
    ) -> Result<(), String> {
        let scene = self.get_scene_mut(scene_id)?;
        scene.upsert_entity(SceneEntity {
            id: entity_id,
            kind,
            brep,
        });
        Ok(())
    }

    fn scene_id_or_current(&self, scene_id: Option<String>) -> Result<String, String> {
        match scene_id {
            Some(id) => Ok(id),
            None => self.current_scene_id_result(),
        }
    }

    pub fn create_scene_internal(&mut self, name: impl Into<String>) -> String {
        let scene = OGScene::new(name);
        let id = scene.id.clone();
        self.scenes.insert(id.clone(), scene);
        self.current_scene_id = Some(id.clone());
        id
    }

    pub fn add_brep_entity_to_scene_internal(
        &mut self,
        scene_id: &str,
        entity_id: impl Into<String>,
        kind: impl Into<String>,
        brep: &Brep,
    ) -> Result<(), String> {
        self.upsert_entity_brep(scene_id, entity_id.into(), kind.into(), brep.clone())
    }

    pub fn add_line_to_scene_internal(
        &mut self,
        scene_id: &str,
        entity_id: impl Into<String>,
        line: &OGLine,
    ) -> Result<(), String> {
        let world_brep = line.world_brep();
        self.add_brep_entity_to_scene_internal(scene_id, entity_id, "OGLine", &world_brep)
    }

    pub fn add_polyline_to_scene_internal(
        &mut self,
        scene_id: &str,
        entity_id: impl Into<String>,
        polyline: &OGPolyline,
    ) -> Result<(), String> {
        let world_brep = polyline.world_brep();
        self.add_brep_entity_to_scene_internal(scene_id, entity_id, "OGPolyline", &world_brep)
    }

    pub fn add_arc_to_scene_internal(
        &mut self,
        scene_id: &str,
        entity_id: impl Into<String>,
        arc: &OGArc,
    ) -> Result<(), String> {
        let world_brep = arc.world_brep();
        self.add_brep_entity_to_scene_internal(scene_id, entity_id, "OGArc", &world_brep)
    }

    pub fn add_rectangle_to_scene_internal(
        &mut self,
        scene_id: &str,
        entity_id: impl Into<String>,
        rectangle: &OGRectangle,
    ) -> Result<(), String> {
        let world_brep = rectangle.world_brep();
        self.add_brep_entity_to_scene_internal(scene_id, entity_id, "OGRectangle", &world_brep)
    }

    pub fn add_polygon_to_scene_internal(
        &mut self,
        scene_id: &str,
        entity_id: impl Into<String>,
        polygon: &OGPolygon,
    ) -> Result<(), String> {
        let world_brep = polygon.world_brep();
        self.add_brep_entity_to_scene_internal(scene_id, entity_id, "OGPolygon", &world_brep)
    }

    pub fn add_cuboid_to_scene_internal(
        &mut self,
        scene_id: &str,
        entity_id: impl Into<String>,
        cuboid: &OGCuboid,
    ) -> Result<(), String> {
        let world_brep = cuboid.world_brep();
        self.add_brep_entity_to_scene_internal(scene_id, entity_id, "OGCuboid", &world_brep)
    }

    pub fn add_cylinder_to_scene_internal(
        &mut self,
        scene_id: &str,
        entity_id: impl Into<String>,
        cylinder: &OGCylinder,
    ) -> Result<(), String> {
        let world_brep = cylinder.world_brep();
        self.add_brep_entity_to_scene_internal(scene_id, entity_id, "OGCylinder", &world_brep)
    }

    pub fn add_sphere_to_scene_internal(
        &mut self,
        scene_id: &str,
        entity_id: impl Into<String>,
        sphere: &OGSphere,
    ) -> Result<(), String> {
        let world_brep = sphere.world_brep();
        self.add_brep_entity_to_scene_internal(scene_id, entity_id, "OGSphere", &world_brep)
    }

    pub fn add_wedge_to_scene_internal(
        &mut self,
        scene_id: &str,
        entity_id: impl Into<String>,
        wedge: &OGWedge,
    ) -> Result<(), String> {
        let world_brep = wedge.world_brep();
        self.add_brep_entity_to_scene_internal(scene_id, entity_id, "OGWedge", &world_brep)
    }

    pub fn project_scene_to_2d(
        &self,
        scene_id: &str,
        camera: &CameraParameters,
        hlr: &HlrOptions,
    ) -> Result<Scene2D, String> {
        let scene = self.get_scene(scene_id)?;
        Ok(scene.project_to_2d(camera, hlr))
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn project_scene_to_pdf_with_camera(
        &self,
        scene_id: &str,
        camera: &CameraParameters,
        hlr: &HlrOptions,
        file_path: &str,
        config: &PdfExportConfig,
    ) -> Result<(), String> {
        let projected = self.project_scene_to_2d(scene_id, camera, hlr)?;
        export_scene_to_pdf_with_config(&projected, file_path, config)
            .map_err(|err| err.to_string())
    }

    pub fn project_scene_to_2d_json(
        &self,
        scene_id: &str,
        camera: &CameraParameters,
        hlr: &HlrOptions,
    ) -> Result<String, String> {
        let projected = self.project_scene_to_2d(scene_id, camera, hlr)?;
        serde_json::to_string(&projected)
            .map_err(|err| format!("Failed to serialize projected scene: {}", err))
    }

    pub fn project_scene_to_2d_json_pretty(
        &self,
        scene_id: &str,
        camera: &CameraParameters,
        hlr: &HlrOptions,
    ) -> Result<String, String> {
        let projected = self.project_scene_to_2d(scene_id, camera, hlr)?;
        serde_json::to_string_pretty(&projected)
            .map_err(|err| format!("Failed to serialize projected scene: {}", err))
    }

    pub fn project_scene_to_2d_lines(
        &self,
        scene_id: &str,
        camera: &CameraParameters,
        hlr: &HlrOptions,
    ) -> Result<Scene2DLines, String> {
        let projected = self.project_scene_to_2d(scene_id, camera, hlr)?;
        Ok(projected.to_lines())
    }

    pub fn project_scene_to_2d_lines_json(
        &self,
        scene_id: &str,
        camera: &CameraParameters,
        hlr: &HlrOptions,
    ) -> Result<String, String> {
        let projected_lines = self.project_scene_to_2d_lines(scene_id, camera, hlr)?;
        serde_json::to_string(&projected_lines)
            .map_err(|err| format!("Failed to serialize projected line scene: {}", err))
    }

    pub fn project_scene_to_2d_lines_json_pretty(
        &self,
        scene_id: &str,
        camera: &CameraParameters,
        hlr: &HlrOptions,
    ) -> Result<String, String> {
        let projected_lines = self.project_scene_to_2d_lines(scene_id, camera, hlr)?;
        serde_json::to_string_pretty(&projected_lines)
            .map_err(|err| format!("Failed to serialize projected line scene: {}", err))
    }

    pub fn export_scene_to_stl_bytes_internal(
        &self,
        scene_id: &str,
        config: &StlExportConfig,
    ) -> Result<(Vec<u8>, StlExportReport), String> {
        let scene = self.get_scene(scene_id)?;
        let breps: Vec<&Brep> = scene.entities.iter().map(|entity| &entity.brep).collect();
        export_breps_to_stl_bytes(breps, config).map_err(|err| err.to_string())
    }

    pub fn export_brep_serialized_to_stl_bytes_internal(
        &self,
        brep_serialized: &str,
        config: &StlExportConfig,
    ) -> Result<(Vec<u8>, StlExportReport), String> {
        let brep: Brep = serde_json::from_str(brep_serialized)
            .map_err(|err| format!("Failed to deserialize BRep JSON payload: {}", err))?;
        export_brep_to_stl_bytes(&brep, config).map_err(|err| err.to_string())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn export_scene_to_stl_file_internal(
        &self,
        scene_id: &str,
        file_path: &str,
        config: &StlExportConfig,
    ) -> Result<StlExportReport, String> {
        let scene = self.get_scene(scene_id)?;
        let breps: Vec<&Brep> = scene.entities.iter().map(|entity| &entity.brep).collect();
        export_breps_to_stl_file(breps, file_path, config).map_err(|err| err.to_string())
    }

    pub fn export_scene_to_step_text_internal(
        &self,
        scene_id: &str,
        config: &StepExportConfig,
    ) -> Result<(String, StepExportReport), String> {
        let scene = self.get_scene(scene_id)?;
        let breps: Vec<&Brep> = scene.entities.iter().map(|entity| &entity.brep).collect();
        export_breps_to_step_text(breps, config).map_err(|err| err.to_string())
    }

    pub fn export_brep_serialized_to_step_text_internal(
        &self,
        brep_serialized: &str,
        config: &StepExportConfig,
    ) -> Result<(String, StepExportReport), String> {
        let brep: Brep = serde_json::from_str(brep_serialized)
            .map_err(|err| format!("Failed to deserialize BRep JSON payload: {}", err))?;
        export_brep_to_step_text(&brep, config).map_err(|err| err.to_string())
    }

    pub fn export_scene_to_ifc_text_internal(
        &self,
        scene_id: &str,
        config: &IfcExportConfig,
    ) -> Result<(String, IfcExportReport), String> {
        let scene = self.get_scene(scene_id)?;
        let entities = scene.entities.iter().map(|entity| IfcEntityInput {
            entity_id: entity.id.as_str(),
            kind: entity.kind.as_str(),
            brep: &entity.brep,
        });
        export_scene_entities_to_ifc_text(entities, config).map_err(|err| err.to_string())
    }

    pub fn export_brep_serialized_to_ifc_text_internal(
        &self,
        brep_serialized: &str,
        config: &IfcExportConfig,
    ) -> Result<(String, IfcExportReport), String> {
        let brep: Brep = serde_json::from_str(brep_serialized)
            .map_err(|err| format!("Failed to deserialize BRep JSON payload: {}", err))?;
        export_brep_to_ifc_text(&brep, config).map_err(|err| err.to_string())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn export_scene_to_step_file_internal(
        &self,
        scene_id: &str,
        file_path: &str,
        config: &StepExportConfig,
    ) -> Result<StepExportReport, String> {
        let scene = self.get_scene(scene_id)?;
        let breps: Vec<&Brep> = scene.entities.iter().map(|entity| &entity.brep).collect();
        export_breps_to_step_file(breps, file_path, config).map_err(|err| err.to_string())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn export_scene_to_ifc_file_internal(
        &self,
        scene_id: &str,
        file_path: &str,
        config: &IfcExportConfig,
    ) -> Result<IfcExportReport, String> {
        let scene = self.get_scene(scene_id)?;
        let entities = scene.entities.iter().map(|entity| IfcEntityInput {
            entity_id: entity.id.as_str(),
            kind: entity.kind.as_str(),
            brep: &entity.brep,
        });
        export_scene_entities_to_ifc_file(entities, file_path, config)
            .map_err(|err| err.to_string())
    }
}

#[wasm_bindgen]
impl OGSceneManager {
    #[wasm_bindgen(constructor)]
    pub fn new() -> OGSceneManager {
        OGSceneManager::default()
    }

    #[wasm_bindgen(js_name = createScene)]
    pub fn create_scene(&mut self, name: String) -> String {
        self.create_scene_internal(name)
    }

    #[wasm_bindgen(js_name = removeScene)]
    pub fn remove_scene(&mut self, scene_id: String) -> bool {
        let removed = self.scenes.remove(&scene_id).is_some();
        if removed && self.current_scene_id.as_deref() == Some(scene_id.as_str()) {
            self.current_scene_id = self.scenes.keys().next().cloned();
        }
        removed
    }

    #[wasm_bindgen(js_name = setCurrentScene)]
    pub fn set_current_scene(&mut self, scene_id: String) -> Result<(), JsValue> {
        if !self.scenes.contains_key(&scene_id) {
            return Err(JsValue::from_str(&format!(
                "Scene '{}' does not exist",
                scene_id
            )));
        }
        self.current_scene_id = Some(scene_id);
        Ok(())
    }

    #[wasm_bindgen(js_name = getCurrentSceneId)]
    pub fn get_current_scene_id(&self) -> Option<String> {
        self.current_scene_id.clone()
    }

    #[wasm_bindgen(js_name = listScenes)]
    pub fn list_scenes(&self) -> Result<String, JsValue> {
        let mut summaries = Vec::new();
        for scene in self.scenes.values() {
            summaries.push(SceneSummary {
                id: scene.id.clone(),
                name: scene.name.clone(),
                entity_count: scene.entities.len(),
            });
        }

        serde_json::to_string(&summaries)
            .map_err(|err| JsValue::from_str(&format!("Failed to serialize scenes: {}", err)))
    }

    #[wasm_bindgen(js_name = getSceneSerialized)]
    pub fn get_scene_serialized(&self, scene_id: String) -> Result<String, JsValue> {
        let scene = self
            .get_scene(&scene_id)
            .map_err(|err| JsValue::from_str(&err))?;

        serde_json::to_string(scene)
            .map_err(|err| JsValue::from_str(&format!("Failed to serialize scene: {}", err)))
    }

    #[wasm_bindgen(js_name = removeEntityFromScene)]
    pub fn remove_entity_from_scene(
        &mut self,
        scene_id: String,
        entity_id: String,
    ) -> Result<bool, JsValue> {
        let scene = self
            .get_scene_mut(&scene_id)
            .map_err(|err| JsValue::from_str(&err))?;
        Ok(scene.remove_entity(&entity_id))
    }

    #[wasm_bindgen(js_name = addBrepEntityToScene)]
    pub fn add_brep_entity_to_scene(
        &mut self,
        scene_id: String,
        entity_id: String,
        kind: String,
        brep_serialized: String,
    ) -> Result<(), JsValue> {
        let brep: Brep = serde_json::from_str(&brep_serialized).map_err(|err| {
            JsValue::from_str(&format!("Failed to deserialize BRep JSON payload: {}", err))
        })?;

        brep.validate_topology().map_err(|err| {
            JsValue::from_str(&format!(
                "Invalid BRep topology for '{}': {}",
                entity_id, err
            ))
        })?;

        self.upsert_entity_brep(&scene_id, entity_id, kind, brep)
            .map_err(|err| JsValue::from_str(&err))
    }

    #[wasm_bindgen(js_name = addBrepEntityToCurrentScene)]
    pub fn add_brep_entity_to_current_scene(
        &mut self,
        entity_id: String,
        kind: String,
        brep_serialized: String,
    ) -> Result<(), JsValue> {
        let scene_id = self
            .scene_id_or_current(None)
            .map_err(|err| JsValue::from_str(&err))?;
        self.add_brep_entity_to_scene(scene_id, entity_id, kind, brep_serialized)
    }

    #[wasm_bindgen(js_name = replaceBrepEntityInScene)]
    pub fn replace_brep_entity_in_scene(
        &mut self,
        scene_id: String,
        entity_id: String,
        kind: String,
        brep_serialized: String,
    ) -> Result<(), JsValue> {
        self.add_brep_entity_to_scene(scene_id, entity_id, kind, brep_serialized)
    }

    #[wasm_bindgen(js_name = replaceBrepEntityInCurrentScene)]
    pub fn replace_brep_entity_in_current_scene(
        &mut self,
        entity_id: String,
        kind: String,
        brep_serialized: String,
    ) -> Result<(), JsValue> {
        self.add_brep_entity_to_current_scene(entity_id, kind, brep_serialized)
    }

    #[wasm_bindgen(js_name = refreshBrepEntityInScene)]
    pub fn refresh_brep_entity_in_scene(
        &mut self,
        scene_id: String,
        entity_id: String,
        kind: String,
        brep_serialized: String,
    ) -> Result<(), JsValue> {
        self.replace_brep_entity_in_scene(scene_id, entity_id, kind, brep_serialized)
    }

    #[wasm_bindgen(js_name = refreshBrepEntityInCurrentScene)]
    pub fn refresh_brep_entity_in_current_scene(
        &mut self,
        entity_id: String,
        kind: String,
        brep_serialized: String,
    ) -> Result<(), JsValue> {
        self.replace_brep_entity_in_current_scene(entity_id, kind, brep_serialized)
    }

    #[wasm_bindgen(js_name = addLineToScene)]
    pub fn add_line_to_scene(
        &mut self,
        scene_id: String,
        entity_id: String,
        line: &OGLine,
    ) -> Result<(), JsValue> {
        self.add_line_to_scene_internal(&scene_id, entity_id, line)
            .map_err(|err| JsValue::from_str(&err))
    }

    #[wasm_bindgen(js_name = addLineToCurrentScene)]
    pub fn add_line_to_current_scene(
        &mut self,
        entity_id: String,
        line: &OGLine,
    ) -> Result<(), JsValue> {
        let scene_id = self
            .scene_id_or_current(None)
            .map_err(|err| JsValue::from_str(&err))?;
        self.add_line_to_scene(scene_id, entity_id, line)
    }

    #[wasm_bindgen(js_name = addPolylineToScene)]
    pub fn add_polyline_to_scene(
        &mut self,
        scene_id: String,
        entity_id: String,
        polyline: &OGPolyline,
    ) -> Result<(), JsValue> {
        self.add_polyline_to_scene_internal(&scene_id, entity_id, polyline)
            .map_err(|err| JsValue::from_str(&err))
    }

    #[wasm_bindgen(js_name = addPolylineToCurrentScene)]
    pub fn add_polyline_to_current_scene(
        &mut self,
        entity_id: String,
        polyline: &OGPolyline,
    ) -> Result<(), JsValue> {
        let scene_id = self
            .scene_id_or_current(None)
            .map_err(|err| JsValue::from_str(&err))?;
        self.add_polyline_to_scene(scene_id, entity_id, polyline)
    }

    #[wasm_bindgen(js_name = addArcToScene)]
    pub fn add_arc_to_scene(
        &mut self,
        scene_id: String,
        entity_id: String,
        arc: &OGArc,
    ) -> Result<(), JsValue> {
        self.add_arc_to_scene_internal(&scene_id, entity_id, arc)
            .map_err(|err| JsValue::from_str(&err))
    }

    #[wasm_bindgen(js_name = addArcToCurrentScene)]
    pub fn add_arc_to_current_scene(
        &mut self,
        entity_id: String,
        arc: &OGArc,
    ) -> Result<(), JsValue> {
        let scene_id = self
            .scene_id_or_current(None)
            .map_err(|err| JsValue::from_str(&err))?;
        self.add_arc_to_scene(scene_id, entity_id, arc)
    }

    #[wasm_bindgen(js_name = addRectangleToScene)]
    pub fn add_rectangle_to_scene(
        &mut self,
        scene_id: String,
        entity_id: String,
        rectangle: &OGRectangle,
    ) -> Result<(), JsValue> {
        self.add_rectangle_to_scene_internal(&scene_id, entity_id, rectangle)
            .map_err(|err| JsValue::from_str(&err))
    }

    #[wasm_bindgen(js_name = addRectangleToCurrentScene)]
    pub fn add_rectangle_to_current_scene(
        &mut self,
        entity_id: String,
        rectangle: &OGRectangle,
    ) -> Result<(), JsValue> {
        let scene_id = self
            .scene_id_or_current(None)
            .map_err(|err| JsValue::from_str(&err))?;
        self.add_rectangle_to_scene(scene_id, entity_id, rectangle)
    }

    #[wasm_bindgen(js_name = addPolygonToScene)]
    pub fn add_polygon_to_scene(
        &mut self,
        scene_id: String,
        entity_id: String,
        polygon: &OGPolygon,
    ) -> Result<(), JsValue> {
        self.add_polygon_to_scene_internal(&scene_id, entity_id, polygon)
            .map_err(|err| JsValue::from_str(&err))
    }

    #[wasm_bindgen(js_name = addPolygonToCurrentScene)]
    pub fn add_polygon_to_current_scene(
        &mut self,
        entity_id: String,
        polygon: &OGPolygon,
    ) -> Result<(), JsValue> {
        let scene_id = self
            .scene_id_or_current(None)
            .map_err(|err| JsValue::from_str(&err))?;
        self.add_polygon_to_scene(scene_id, entity_id, polygon)
    }

    #[wasm_bindgen(js_name = addCuboidToScene)]
    pub fn add_cuboid_to_scene(
        &mut self,
        scene_id: String,
        entity_id: String,
        cuboid: &OGCuboid,
    ) -> Result<(), JsValue> {
        self.add_cuboid_to_scene_internal(&scene_id, entity_id, cuboid)
            .map_err(|err| JsValue::from_str(&err))
    }

    #[wasm_bindgen(js_name = addCuboidToCurrentScene)]
    pub fn add_cuboid_to_current_scene(
        &mut self,
        entity_id: String,
        cuboid: &OGCuboid,
    ) -> Result<(), JsValue> {
        let scene_id = self
            .scene_id_or_current(None)
            .map_err(|err| JsValue::from_str(&err))?;
        self.add_cuboid_to_scene(scene_id, entity_id, cuboid)
    }

    #[wasm_bindgen(js_name = addCylinderToScene)]
    pub fn add_cylinder_to_scene(
        &mut self,
        scene_id: String,
        entity_id: String,
        cylinder: &OGCylinder,
    ) -> Result<(), JsValue> {
        self.add_cylinder_to_scene_internal(&scene_id, entity_id, cylinder)
            .map_err(|err| JsValue::from_str(&err))
    }

    #[wasm_bindgen(js_name = addCylinderToCurrentScene)]
    pub fn add_cylinder_to_current_scene(
        &mut self,
        entity_id: String,
        cylinder: &OGCylinder,
    ) -> Result<(), JsValue> {
        let scene_id = self
            .scene_id_or_current(None)
            .map_err(|err| JsValue::from_str(&err))?;
        self.add_cylinder_to_scene(scene_id, entity_id, cylinder)
    }

    #[wasm_bindgen(js_name = addSphereToScene)]
    pub fn add_sphere_to_scene(
        &mut self,
        scene_id: String,
        entity_id: String,
        sphere: &OGSphere,
    ) -> Result<(), JsValue> {
        self.add_sphere_to_scene_internal(&scene_id, entity_id, sphere)
            .map_err(|err| JsValue::from_str(&err))
    }

    #[wasm_bindgen(js_name = addSphereToCurrentScene)]
    pub fn add_sphere_to_current_scene(
        &mut self,
        entity_id: String,
        sphere: &OGSphere,
    ) -> Result<(), JsValue> {
        let scene_id = self
            .scene_id_or_current(None)
            .map_err(|err| JsValue::from_str(&err))?;
        self.add_sphere_to_scene(scene_id, entity_id, sphere)
    }

    #[wasm_bindgen(js_name = addWedgeToScene)]
    pub fn add_wedge_to_scene(
        &mut self,
        scene_id: String,
        entity_id: String,
        wedge: &OGWedge,
    ) -> Result<(), JsValue> {
        self.add_wedge_to_scene_internal(&scene_id, entity_id, wedge)
            .map_err(|err| JsValue::from_str(&err))
    }

    #[wasm_bindgen(js_name = addWedgeToCurrentScene)]
    pub fn add_wedge_to_current_scene(
        &mut self,
        entity_id: String,
        wedge: &OGWedge,
    ) -> Result<(), JsValue> {
        let scene_id = self
            .scene_id_or_current(None)
            .map_err(|err| JsValue::from_str(&err))?;
        self.add_wedge_to_scene(scene_id, entity_id, wedge)
    }

    #[wasm_bindgen(js_name = projectTo2DCamera)]
    pub fn project_to_2d_camera(
        &self,
        scene_id: String,
        camera_json: String,
        hlr_json: Option<String>,
    ) -> Result<String, JsValue> {
        let camera =
            Self::parse_camera_json(&camera_json).map_err(|err| JsValue::from_str(&err))?;
        let hlr = Self::parse_hlr_json(hlr_json).map_err(|err| JsValue::from_str(&err))?;
        self.project_scene_to_2d_json(&scene_id, &camera, &hlr)
            .map_err(|err| JsValue::from_str(&err))
    }

    #[wasm_bindgen(js_name = projectTo2DCameraPretty)]
    pub fn project_to_2d_camera_pretty(
        &self,
        scene_id: String,
        camera_json: String,
        hlr_json: Option<String>,
    ) -> Result<String, JsValue> {
        let camera =
            Self::parse_camera_json(&camera_json).map_err(|err| JsValue::from_str(&err))?;
        let hlr = Self::parse_hlr_json(hlr_json).map_err(|err| JsValue::from_str(&err))?;
        self.project_scene_to_2d_json_pretty(&scene_id, &camera, &hlr)
            .map_err(|err| JsValue::from_str(&err))
    }

    #[wasm_bindgen(js_name = projectTo2DLines)]
    pub fn project_to_2d_lines(
        &self,
        scene_id: String,
        camera_json: String,
        hlr_json: Option<String>,
    ) -> Result<String, JsValue> {
        let camera =
            Self::parse_camera_json(&camera_json).map_err(|err| JsValue::from_str(&err))?;
        let hlr = Self::parse_hlr_json(hlr_json).map_err(|err| JsValue::from_str(&err))?;
        self.project_scene_to_2d_lines_json(&scene_id, &camera, &hlr)
            .map_err(|err| JsValue::from_str(&err))
    }

    #[wasm_bindgen(js_name = projectTo2DLinesPretty)]
    pub fn project_to_2d_lines_pretty(
        &self,
        scene_id: String,
        camera_json: String,
        hlr_json: Option<String>,
    ) -> Result<String, JsValue> {
        let camera =
            Self::parse_camera_json(&camera_json).map_err(|err| JsValue::from_str(&err))?;
        let hlr = Self::parse_hlr_json(hlr_json).map_err(|err| JsValue::from_str(&err))?;
        self.project_scene_to_2d_lines_json_pretty(&scene_id, &camera, &hlr)
            .map_err(|err| JsValue::from_str(&err))
    }

    #[wasm_bindgen(js_name = projectCurrentTo2DCamera)]
    pub fn project_current_to_2d_camera(
        &self,
        camera_json: String,
        hlr_json: Option<String>,
    ) -> Result<String, JsValue> {
        let scene_id = self
            .scene_id_or_current(None)
            .map_err(|err| JsValue::from_str(&err))?;
        self.project_to_2d_camera(scene_id, camera_json, hlr_json)
    }

    #[wasm_bindgen(js_name = projectCurrentTo2DLines)]
    pub fn project_current_to_2d_lines(
        &self,
        camera_json: String,
        hlr_json: Option<String>,
    ) -> Result<String, JsValue> {
        let scene_id = self
            .scene_id_or_current(None)
            .map_err(|err| JsValue::from_str(&err))?;
        self.project_to_2d_lines(scene_id, camera_json, hlr_json)
    }

    #[wasm_bindgen(js_name = exportBrepToStl)]
    pub fn export_brep_to_stl(
        &self,
        brep_serialized: String,
        config_json: Option<String>,
    ) -> Result<OGStlExportResult, JsValue> {
        let config =
            Self::parse_stl_config_json(config_json).map_err(|err| JsValue::from_str(&err))?;
        let (bytes, report) = self
            .export_brep_serialized_to_stl_bytes_internal(&brep_serialized, &config)
            .map_err(|err| JsValue::from_str(&err))?;

        OGStlExportResult::from_parts(bytes, report).map_err(|err| JsValue::from_str(&err))
    }

    #[wasm_bindgen(js_name = exportSceneToStl)]
    pub fn export_scene_to_stl(
        &self,
        scene_id: String,
        config_json: Option<String>,
    ) -> Result<OGStlExportResult, JsValue> {
        let config =
            Self::parse_stl_config_json(config_json).map_err(|err| JsValue::from_str(&err))?;
        let (bytes, report) = self
            .export_scene_to_stl_bytes_internal(&scene_id, &config)
            .map_err(|err| JsValue::from_str(&err))?;

        OGStlExportResult::from_parts(bytes, report).map_err(|err| JsValue::from_str(&err))
    }

    #[wasm_bindgen(js_name = exportCurrentSceneToStl)]
    pub fn export_current_scene_to_stl(
        &self,
        config_json: Option<String>,
    ) -> Result<OGStlExportResult, JsValue> {
        let scene_id = self
            .scene_id_or_current(None)
            .map_err(|err| JsValue::from_str(&err))?;
        self.export_scene_to_stl(scene_id, config_json)
    }

    #[wasm_bindgen(js_name = exportBrepToStep)]
    pub fn export_brep_to_step(
        &self,
        brep_serialized: String,
        config_json: Option<String>,
    ) -> Result<OGStepExportResult, JsValue> {
        let config =
            Self::parse_step_config_json(config_json).map_err(|err| JsValue::from_str(&err))?;
        let (text, report) = self
            .export_brep_serialized_to_step_text_internal(&brep_serialized, &config)
            .map_err(|err| JsValue::from_str(&err))?;

        OGStepExportResult::from_parts(text, report).map_err(|err| JsValue::from_str(&err))
    }

    #[wasm_bindgen(js_name = exportSceneToStep)]
    pub fn export_scene_to_step(
        &self,
        scene_id: String,
        config_json: Option<String>,
    ) -> Result<OGStepExportResult, JsValue> {
        let config =
            Self::parse_step_config_json(config_json).map_err(|err| JsValue::from_str(&err))?;
        let (text, report) = self
            .export_scene_to_step_text_internal(&scene_id, &config)
            .map_err(|err| JsValue::from_str(&err))?;

        OGStepExportResult::from_parts(text, report).map_err(|err| JsValue::from_str(&err))
    }

    #[wasm_bindgen(js_name = exportCurrentSceneToStep)]
    pub fn export_current_scene_to_step(
        &self,
        config_json: Option<String>,
    ) -> Result<OGStepExportResult, JsValue> {
        let scene_id = self
            .scene_id_or_current(None)
            .map_err(|err| JsValue::from_str(&err))?;
        self.export_scene_to_step(scene_id, config_json)
    }

    #[wasm_bindgen(js_name = exportBrepToIfc)]
    pub fn export_brep_to_ifc(
        &self,
        brep_serialized: String,
        config_json: Option<String>,
    ) -> Result<OGIfcExportResult, JsValue> {
        let config =
            Self::parse_ifc_config_json(config_json).map_err(|err| JsValue::from_str(&err))?;
        let (text, report) = self
            .export_brep_serialized_to_ifc_text_internal(&brep_serialized, &config)
            .map_err(|err| JsValue::from_str(&err))?;

        OGIfcExportResult::from_parts(text, report).map_err(|err| JsValue::from_str(&err))
    }

    #[wasm_bindgen(js_name = exportSceneToIfc)]
    pub fn export_scene_to_ifc(
        &self,
        scene_id: String,
        config_json: Option<String>,
    ) -> Result<OGIfcExportResult, JsValue> {
        let config =
            Self::parse_ifc_config_json(config_json).map_err(|err| JsValue::from_str(&err))?;
        let (text, report) = self
            .export_scene_to_ifc_text_internal(&scene_id, &config)
            .map_err(|err| JsValue::from_str(&err))?;

        OGIfcExportResult::from_parts(text, report).map_err(|err| JsValue::from_str(&err))
    }

    #[wasm_bindgen(js_name = exportCurrentSceneToIfc)]
    pub fn export_current_scene_to_ifc(
        &self,
        config_json: Option<String>,
    ) -> Result<OGIfcExportResult, JsValue> {
        let scene_id = self
            .scene_id_or_current(None)
            .map_err(|err| JsValue::from_str(&err))?;
        self.export_scene_to_ifc(scene_id, config_json)
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[wasm_bindgen(js_name = exportSceneToStlFile)]
    pub fn export_scene_to_stl_file(
        &self,
        scene_id: String,
        file_path: String,
        config_json: Option<String>,
    ) -> Result<String, JsValue> {
        let config =
            Self::parse_stl_config_json(config_json).map_err(|err| JsValue::from_str(&err))?;
        let report = self
            .export_scene_to_stl_file_internal(&scene_id, &file_path, &config)
            .map_err(|err| JsValue::from_str(&err))?;
        serde_json::to_string(&report).map_err(|err| {
            JsValue::from_str(&format!("Failed to serialize STL export report: {}", err))
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[wasm_bindgen(js_name = exportSceneToStepFile)]
    pub fn export_scene_to_step_file(
        &self,
        scene_id: String,
        file_path: String,
        config_json: Option<String>,
    ) -> Result<String, JsValue> {
        let config =
            Self::parse_step_config_json(config_json).map_err(|err| JsValue::from_str(&err))?;
        let report = self
            .export_scene_to_step_file_internal(&scene_id, &file_path, &config)
            .map_err(|err| JsValue::from_str(&err))?;
        serde_json::to_string(&report).map_err(|err| {
            JsValue::from_str(&format!("Failed to serialize STEP export report: {}", err))
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[wasm_bindgen(js_name = exportSceneToIfcFile)]
    pub fn export_scene_to_ifc_file(
        &self,
        scene_id: String,
        file_path: String,
        config_json: Option<String>,
    ) -> Result<String, JsValue> {
        let config =
            Self::parse_ifc_config_json(config_json).map_err(|err| JsValue::from_str(&err))?;
        let report = self
            .export_scene_to_ifc_file_internal(&scene_id, &file_path, &config)
            .map_err(|err| JsValue::from_str(&err))?;
        serde_json::to_string(&report).map_err(|err| {
            JsValue::from_str(&format!("Failed to serialize IFC export report: {}", err))
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[wasm_bindgen(js_name = projectToPDF)]
    pub fn project_to_pdf(
        &self,
        scene_id: String,
        camera_json: String,
        hlr_json: Option<String>,
        file_path: String,
    ) -> Result<(), JsValue> {
        let camera =
            Self::parse_camera_json(&camera_json).map_err(|err| JsValue::from_str(&err))?;
        let hlr = Self::parse_hlr_json(hlr_json).map_err(|err| JsValue::from_str(&err))?;
        self.project_scene_to_pdf_with_camera(
            &scene_id,
            &camera,
            &hlr,
            &file_path,
            &PdfExportConfig::default(),
        )
        .map_err(|err| JsValue::from_str(&err))
    }
}

// ---------------------------------------------------------------------------
// OGEntityRegistry — flat entity store with batched multi-view projection
// ---------------------------------------------------------------------------

/// Optional section plane for cutting views.
#[derive(Clone, Serialize, Deserialize)]
pub struct SectionPlane {
    pub origin: [f64; 3],
    pub normal: [f64; 3],
}

/// One viewport description passed to `projectCurrentToViews`.
#[derive(Deserialize)]
struct ViewRequest {
    id: String,
    camera: CameraParameters,
    #[serde(default)]
    hlr: HlrOptions,
    section_plane: Option<SectionPlane>,
}

/// Map `OGEntityKind` strings to AIA/NCS layer codes (AIA CAD Layer Guidelines).
fn aia_layer(kind: &str) -> Option<&'static str> {
    match kind.to_lowercase().trim() {
        "wall" => Some("A-WALL"),
        "door" => Some("A-DOOR"),
        "window" | "glazing" | "glaz" => Some("A-GLAZ"),
        "slab" | "floor" | "ceiling" => Some("A-FLOR"),
        "stair" | "stairs" => Some("A-FLOR-STRS"),
        "column" | "col" => Some("A-COLS"),
        "beam" => Some("S-BEAM"),
        "roof" => Some("A-ROOF"),
        _ => None,
    }
}

/// Flat entity registry with a batched multi-view projection API.
/// Replaces the scene-centric OGSceneManager pattern for export use cases.
#[wasm_bindgen]
pub struct OGEntityRegistry {
    entities: HashMap<String, SceneEntity>,
}

impl Default for OGEntityRegistry {
    fn default() -> Self {
        Self {
            entities: HashMap::new(),
        }
    }
}

impl OGEntityRegistry {
    fn project_view(&self, view: &ViewRequest) -> Scene2D {
        let mut out = Scene2D::with_name(view.id.clone());
        for entity in self.entities.values() {
            let mut entity_scene = project_brep_to_scene(&entity.brep, &view.camera, &view.hlr);
            let layer = aia_layer(&entity.kind).map(str::to_string);
            for seg in &mut entity_scene.segments {
                seg.layer = layer.clone();
                seg.source_entity_id = Some(entity.id.clone());
            }
            out.extend(entity_scene);
        }
        // Section-plane intersection is wired up here in Phase 3+.
        // `view.section_plane` field is available for future use.
        let _ = &view.section_plane;
        out
    }
}

#[wasm_bindgen]
impl OGEntityRegistry {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register (or replace) an entity. `kind` must be an OGEntityKind string.
    #[wasm_bindgen(js_name = registerEntity)]
    pub fn register_entity(
        &mut self,
        id: String,
        kind: String,
        brep_json: String,
    ) -> Result<(), JsValue> {
        let brep: Brep = serde_json::from_str(&brep_json)
            .map_err(|e| JsValue::from_str(&format!("Failed to deserialize BRep: {}", e)))?;

        brep.validate_topology().map_err(|e| {
            JsValue::from_str(&format!("Invalid BRep topology for '{}': {}", id, e))
        })?;

        self.entities
            .insert(id.clone(), SceneEntity { id, kind, brep });
        Ok(())
    }

    /// Remove a previously registered entity.
    #[wasm_bindgen(js_name = unregisterEntity)]
    pub fn unregister_entity(&mut self, id: String) -> bool {
        self.entities.remove(&id).is_some()
    }

    /// Remove all registered entities.
    #[wasm_bindgen(js_name = clearEntities)]
    pub fn clear_entities(&mut self) {
        self.entities.clear();
    }

    /// Batched multi-view projection.
    ///
    /// Input:  JSON array of `{ id, camera, hlr?, section_plane? }` (ViewRequest).
    /// Output: JSON map of `{ viewportId: Scene2D }`.
    ///
    /// All viewports are projected in a single WASM call, amortising
    /// serialisation overhead and keeping BRep data in WASM memory.
    #[wasm_bindgen(js_name = projectCurrentToViews)]
    pub fn project_current_to_views(&self, views_json: String) -> Result<String, JsValue> {
        let views: Vec<ViewRequest> = serde_json::from_str(&views_json)
            .map_err(|e| JsValue::from_str(&format!("Invalid views JSON: {}", e)))?;

        let mut result: HashMap<String, Scene2D> = HashMap::with_capacity(views.len());
        for view in &views {
            result.insert(view.id.clone(), self.project_view(view));
        }

        serde_json::to_string(&result).map_err(|e| {
            JsValue::from_str(&format!("Failed to serialize projection result: {}", e))
        })
    }
}

// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brep::{Brep, BrepBuilder};
    use crate::primitives::cuboid::OGCuboid;
    use openmaths::Vector3;
    use uuid::Uuid;

    fn assert_close(actual: f64, expected: f64) {
        let delta = (actual - expected).abs();
        assert!(
            delta <= 1.0e-9,
            "expected {expected}, got {actual}, delta {delta}"
        );
    }

    fn assert_vec_close(actual: Vector3, expected: Vector3) {
        assert_close(actual.x, expected.x);
        assert_close(actual.y, expected.y);
        assert_close(actual.z, expected.z);
    }

    #[test]
    fn test_scene_projection_from_edge_entity() {
        let mut manager = OGSceneManager::new();
        let scene_id = manager.create_scene_internal("test-scene");

        let mut builder = BrepBuilder::new(Uuid::new_v4());
        builder.add_vertices(&[Vector3::new(-1.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0)]);
        builder.add_wire(&[0, 1], false).unwrap();
        let brep: Brep = builder.build().unwrap();

        manager
            .add_brep_entity_to_scene_internal(&scene_id, "edge-1", "Edge", &brep)
            .unwrap();

        let scene_2d = manager
            .project_scene_to_2d(
                &scene_id,
                &CameraParameters::default(),
                &HlrOptions::default(),
            )
            .unwrap();

        assert!(!scene_2d.is_empty());
    }

    #[test]
    fn test_scene_projection_lines_json_payload() {
        let mut manager = OGSceneManager::new();
        let scene_id = manager.create_scene_internal("test-scene");

        let mut builder = BrepBuilder::new(Uuid::new_v4());
        builder.add_vertices(&[Vector3::new(-1.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0)]);
        builder.add_wire(&[0, 1], false).unwrap();
        let brep: Brep = builder.build().unwrap();

        manager
            .add_brep_entity_to_scene_internal(&scene_id, "edge-1", "Edge", &brep)
            .unwrap();

        let payload = manager
            .project_scene_to_2d_lines_json(
                &scene_id,
                &CameraParameters::default(),
                &HlrOptions::default(),
            )
            .unwrap();
        let projected: Scene2DLines = serde_json::from_str(&payload).unwrap();

        assert_eq!(projected.name.as_deref(), Some("test-scene"));
        assert_eq!(projected.lines.len(), 1);
        assert!(projected.lines[0].start.x.is_finite());
        assert!(projected.lines[0].start.y.is_finite());
        assert!(projected.lines[0].end.x.is_finite());
        assert!(projected.lines[0].end.y.is_finite());
    }

    #[test]
    fn test_scene_stl_export_binary_payload() {
        let mut manager = OGSceneManager::new();
        let scene_id = manager.create_scene_internal("stl-scene");

        let mut builder = BrepBuilder::new(Uuid::new_v4());
        builder.add_vertices(&[
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
        ]);
        builder.add_face(&[0, 1, 2], &[]).unwrap();
        let brep: Brep = builder.build().unwrap();

        manager
            .add_brep_entity_to_scene_internal(&scene_id, "tri-1", "Triangle", &brep)
            .unwrap();

        let (bytes, report) = manager
            .export_scene_to_stl_bytes_internal(&scene_id, &StlExportConfig::default())
            .unwrap();

        assert!(bytes.len() >= 84);
        let triangle_count = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]);
        assert_eq!(triangle_count as usize, report.exported_triangles);
        assert_eq!(report.exported_triangles, 1);
    }

    #[test]
    fn adding_placed_cuboid_to_scene_snapshots_world_space_brep() {
        let mut manager = OGSceneManager::new();
        let scene_id = manager.create_scene_internal("placed-scene");

        let mut cuboid = OGCuboid::new("placed-cuboid".to_string());
        cuboid
            .set_config(Vector3::new(0.0, 0.0, 0.0), 2.0, 2.0, 2.0)
            .expect("cuboid config");
        cuboid
            .set_transform(
                Vector3::new(5.0, 1.0, -2.0),
                Vector3::new(0.0, 0.4, 0.0),
                Vector3::new(1.25, 1.25, 1.25),
            )
            .expect("placement transform");
        let expected_brep = cuboid.world_brep();

        manager
            .add_cuboid_to_scene_internal(&scene_id, "placed-cuboid", &cuboid)
            .expect("scene insert");

        let scene = manager.get_scene(&scene_id).expect("scene");
        let entity = scene
            .entities
            .iter()
            .find(|item| item.id == "placed-cuboid")
            .expect("entity");

        assert_eq!(entity.kind, "OGCuboid");
        assert_eq!(entity.brep.vertices.len(), expected_brep.vertices.len());
        for (actual, expected) in entity
            .brep
            .vertices
            .iter()
            .zip(expected_brep.vertices.iter())
        {
            assert_vec_close(actual.position, expected.position);
        }

        let scene_center = entity.brep.bounds_center().expect("scene bounds");
        let world_center = expected_brep.bounds_center().expect("world bounds");
        assert_vec_close(scene_center, world_center);

        let projected = manager
            .project_scene_to_2d(
                &scene_id,
                &CameraParameters::default(),
                &HlrOptions::default(),
            )
            .expect("projection");
        assert!(!projected.is_empty());
    }

    #[test]
    fn scene_snapshot_stays_stale_until_explicit_replace_refresh() {
        let mut manager = OGSceneManager::new();
        let scene_id = manager.create_scene_internal("refresh-scene");

        let mut cuboid = OGCuboid::new("refresh-cuboid".to_string());
        cuboid
            .set_config(Vector3::new(0.0, 0.0, 0.0), 2.0, 2.0, 2.0)
            .expect("cuboid config");
        cuboid
            .set_transform(
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(0.0, 0.0, 0.0),
                Vector3::new(1.0, 1.0, 1.0),
            )
            .expect("initial placement");

        manager
            .add_cuboid_to_scene_internal(&scene_id, "refresh-cuboid", &cuboid)
            .expect("scene insert");

        cuboid
            .set_transform(
                Vector3::new(6.0, 0.0, 0.0),
                Vector3::new(0.0, 0.0, 0.0),
                Vector3::new(1.0, 1.0, 1.0),
            )
            .expect("updated placement");

        let scene_before_refresh = manager.get_scene(&scene_id).expect("scene");
        let entity_before_refresh = scene_before_refresh
            .entities
            .iter()
            .find(|item| item.id == "refresh-cuboid")
            .expect("entity");
        let center_before_refresh = entity_before_refresh
            .brep
            .bounds_center()
            .expect("bounds before refresh");
        assert_close(center_before_refresh.x, 1.0);

        manager
            .replace_brep_entity_in_scene(
                scene_id.clone(),
                "refresh-cuboid".to_string(),
                "OGCuboid".to_string(),
                serde_json::to_string(&cuboid.world_brep()).expect("serialize world brep"),
            )
            .expect("replace scene entity");

        let scene_after_refresh = manager.get_scene(&scene_id).expect("scene");
        let entity_after_refresh = scene_after_refresh
            .entities
            .iter()
            .find(|item| item.id == "refresh-cuboid")
            .expect("entity");
        let center_after_refresh = entity_after_refresh
            .brep
            .bounds_center()
            .expect("bounds after refresh");
        assert_close(center_after_refresh.x, 6.0);
    }

    #[test]
    fn test_scene_step_export_text_payload() {
        let mut manager = OGSceneManager::new();
        let scene_id = manager.create_scene_internal("step-scene");

        let mut builder = BrepBuilder::new(Uuid::new_v4());
        builder.add_vertices(&[
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.5, 0.8660254, 0.0),
            Vector3::new(0.5, 0.2886751, 0.8164966),
        ]);
        builder.add_face(&[0, 2, 1], &[]).unwrap();
        builder.add_face(&[0, 1, 3], &[]).unwrap();
        builder.add_face(&[1, 2, 3], &[]).unwrap();
        builder.add_face(&[2, 0, 3], &[]).unwrap();
        let brep: Brep = builder.build().unwrap();

        manager
            .add_brep_entity_to_scene_internal(&scene_id, "tetra-1", "Tetrahedron", &brep)
            .unwrap();

        let (text, report) = manager
            .export_scene_to_step_text_internal(&scene_id, &StepExportConfig::default())
            .unwrap();

        assert!(text.starts_with("ISO-10303-21;"));
        assert!(text.contains("FILE_SCHEMA(('AUTOMOTIVE_DESIGN'));"));
        assert!(text.contains("MANIFOLD_SOLID_BREP"));
        assert_eq!(report.exported_solids, 1);
    }

    #[test]
    fn test_scene_ifc_export_text_payload() {
        let mut manager = OGSceneManager::new();
        let scene_id = manager.create_scene_internal("ifc-scene");

        let mut builder = BrepBuilder::new(Uuid::new_v4());
        builder.add_vertices(&[
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.5, 0.8660254, 0.0),
            Vector3::new(0.5, 0.2886751, 0.8164966),
        ]);
        builder.add_face(&[0, 2, 1], &[]).unwrap();
        builder.add_face(&[0, 1, 3], &[]).unwrap();
        builder.add_face(&[1, 2, 3], &[]).unwrap();
        builder.add_face(&[2, 0, 3], &[]).unwrap();
        let brep: Brep = builder.build().unwrap();

        manager
            .add_brep_entity_to_scene_internal(&scene_id, "tetra-1", "Tetrahedron", &brep)
            .unwrap();

        let (text, report) = manager
            .export_scene_to_ifc_text_internal(&scene_id, &IfcExportConfig::default())
            .unwrap();

        assert!(text.starts_with("ISO-10303-21;"));
        assert!(text.contains("FILE_SCHEMA(('IFC4'));"));
        assert!(text.contains("IFCPROJECT("));
        assert!(text.contains("IFCTRIANGULATEDFACESET("));
        assert_eq!(report.exported_elements, 1);
    }

    // -----------------------------------------------------------------------
    // OGEntityRegistry tests
    // -----------------------------------------------------------------------

    fn wire_brep() -> Brep {
        let mut builder = BrepBuilder::new(Uuid::new_v4());
        builder.add_vertices(&[Vector3::new(-1.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0)]);
        builder.add_wire(&[0, 1], false).unwrap();
        builder.build().unwrap()
    }

    #[test]
    fn registry_register_and_project_assigns_aia_layer() {
        let mut registry = OGEntityRegistry::new();
        let brep_json = serde_json::to_string(&wire_brep()).unwrap();
        registry
            .register_entity("wall-1".to_string(), "wall".to_string(), brep_json)
            .unwrap();

        let camera_json = serde_json::to_string(&CameraParameters::default()).unwrap();
        let views_json = format!(r#"[{{"id":"plan","camera":{}}}]"#, camera_json);

        let result_json = registry.project_current_to_views(views_json).unwrap();
        let result: HashMap<String, Scene2D> = serde_json::from_str(&result_json).unwrap();

        let plan = result.get("plan").expect("plan viewport missing");
        assert!(!plan.segments.is_empty(), "expected non-empty projection");
        for seg in &plan.segments {
            assert_eq!(
                seg.layer.as_deref(),
                Some("A-WALL"),
                "layer should be A-WALL"
            );
            assert_eq!(seg.source_entity_id.as_deref(), Some("wall-1"));
        }
    }

    #[test]
    fn registry_multi_view_returns_all_viewports() {
        let mut registry = OGEntityRegistry::new();
        let brep_json = serde_json::to_string(&wire_brep()).unwrap();
        registry
            .register_entity("e1".to_string(), "generic".to_string(), brep_json)
            .unwrap();

        let camera_json = serde_json::to_string(&CameraParameters::default()).unwrap();
        let views_json = format!(
            r#"[{{"id":"plan","camera":{cam}}},{{"id":"elev","camera":{cam}}}]"#,
            cam = camera_json
        );

        let result_json = registry.project_current_to_views(views_json).unwrap();
        let result: HashMap<String, Scene2D> = serde_json::from_str(&result_json).unwrap();

        assert_eq!(result.len(), 2, "expected 2 viewports");
        assert!(result.contains_key("plan"));
        assert!(result.contains_key("elev"));
    }

    #[test]
    fn registry_unregister_removes_entity_from_projection() {
        let mut registry = OGEntityRegistry::new();
        let brep_json = serde_json::to_string(&wire_brep()).unwrap();
        registry
            .register_entity("e1".to_string(), "wall".to_string(), brep_json)
            .unwrap();

        let removed = registry.unregister_entity("e1".to_string());
        assert!(removed);

        let camera_json = serde_json::to_string(&CameraParameters::default()).unwrap();
        let views_json = format!(r#"[{{"id":"plan","camera":{}}}]"#, camera_json);
        let result_json = registry.project_current_to_views(views_json).unwrap();
        let result: HashMap<String, Scene2D> = serde_json::from_str(&result_json).unwrap();

        let plan = result.get("plan").unwrap();
        assert!(
            plan.segments.is_empty(),
            "segments should be empty after unregister"
        );
    }

    #[test]
    fn aia_layer_maps_known_kinds() {
        assert_eq!(aia_layer("wall"), Some("A-WALL"));
        assert_eq!(aia_layer("door"), Some("A-DOOR"));
        assert_eq!(aia_layer("window"), Some("A-GLAZ"));
        assert_eq!(aia_layer("slab"), Some("A-FLOR"));
        assert_eq!(aia_layer("stair"), Some("A-FLOR-STRS"));
        assert_eq!(aia_layer("column"), Some("A-COLS"));
        assert_eq!(aia_layer("unknown"), None);
    }
}
