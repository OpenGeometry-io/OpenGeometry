use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use wasm_bindgen::prelude::*;

use crate::brep::Brep;
use crate::export::projection::{
    project_brep_to_scene, CameraParameters, HlrOptions, Scene2D, Scene2DLines,
};
use crate::primitives::arc::OGArc;
use crate::primitives::cuboid::OGCuboid;
use crate::primitives::cylinder::OGCylinder;
use crate::primitives::line::OGLine;
use crate::primitives::polygon::OGPolygon;
use crate::primitives::polyline::OGPolyline;
use crate::primitives::rectangle::OGRectangle;

#[cfg(not(target_arch = "wasm32"))]
use crate::export::pdf::{export_scene_to_pdf_with_config, PdfExportConfig};

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
        self.add_brep_entity_to_scene_internal(scene_id, entity_id, "OGLine", line.brep())
    }

    pub fn add_polyline_to_scene_internal(
        &mut self,
        scene_id: &str,
        entity_id: impl Into<String>,
        polyline: &OGPolyline,
    ) -> Result<(), String> {
        self.add_brep_entity_to_scene_internal(scene_id, entity_id, "OGPolyline", polyline.brep())
    }

    pub fn add_arc_to_scene_internal(
        &mut self,
        scene_id: &str,
        entity_id: impl Into<String>,
        arc: &OGArc,
    ) -> Result<(), String> {
        self.add_brep_entity_to_scene_internal(scene_id, entity_id, "OGArc", arc.brep())
    }

    pub fn add_rectangle_to_scene_internal(
        &mut self,
        scene_id: &str,
        entity_id: impl Into<String>,
        rectangle: &OGRectangle,
    ) -> Result<(), String> {
        self.add_brep_entity_to_scene_internal(scene_id, entity_id, "OGRectangle", rectangle.brep())
    }

    pub fn add_polygon_to_scene_internal(
        &mut self,
        scene_id: &str,
        entity_id: impl Into<String>,
        polygon: &OGPolygon,
    ) -> Result<(), String> {
        self.add_brep_entity_to_scene_internal(scene_id, entity_id, "OGPolygon", polygon.brep())
    }

    pub fn add_cuboid_to_scene_internal(
        &mut self,
        scene_id: &str,
        entity_id: impl Into<String>,
        cuboid: &OGCuboid,
    ) -> Result<(), String> {
        self.add_brep_entity_to_scene_internal(scene_id, entity_id, "OGCuboid", cuboid.brep())
    }

    pub fn add_cylinder_to_scene_internal(
        &mut self,
        scene_id: &str,
        entity_id: impl Into<String>,
        cylinder: &OGCylinder,
    ) -> Result<(), String> {
        self.add_brep_entity_to_scene_internal(scene_id, entity_id, "OGCylinder", cylinder.brep())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brep::{Edge, Vertex};
    use openmaths::Vector3;

    #[test]
    fn test_scene_projection_from_edge_entity() {
        let mut manager = OGSceneManager::new();
        let scene_id = manager.create_scene_internal("test-scene");

        let mut brep = Brep::new(Uuid::new_v4());
        brep.vertices
            .push(Vertex::new(0, Vector3::new(-1.0, 0.0, 0.0)));
        brep.vertices
            .push(Vertex::new(1, Vector3::new(1.0, 0.0, 0.0)));
        brep.edges.push(Edge::new(0, 0, 1));

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
}
