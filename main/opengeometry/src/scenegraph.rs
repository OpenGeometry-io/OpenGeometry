use std::collections::{HashMap, HashSet, VecDeque};

use opengeometry_export_schema::{
    ExportEntity, ExportFeatureNode, ExportFeatureTree, ExportMesh, ExportScene,
    ExportSceneSnapshot,
};
use openmaths::Vector3;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use wasm_bindgen::prelude::*;

use crate::brep::Brep;
#[cfg(not(target_arch = "wasm32"))]
use crate::export::pdf::{export_scene_to_pdf_with_config, PdfExportConfig};
use crate::export::projection::{
    project_brep_to_scene, CameraParameters, HlrOptions, Scene2D, Scene2DLines,
};
use crate::operations::triangulate::triangulate_polygon_with_holes;
use crate::primitives::arc::OGArc;
use crate::primitives::cuboid::OGCuboid;
use crate::primitives::cylinder::OGCylinder;
use crate::primitives::line::OGLine;
use crate::primitives::polygon::OGPolygon;
use crate::primitives::polyline::OGPolyline;
use crate::primitives::rectangle::OGRectangle;
use crate::primitives::sphere::OGSphere;
use crate::primitives::wedge::OGWedge;

const MESH_EPSILON: f64 = 1.0e-12;

#[derive(Clone, Serialize, Deserialize)]
pub struct SceneEntity {
    pub id: String,
    pub kind: String,
    pub brep: Brep,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SceneFeatureNode {
    pub id: String,
    pub kind: String,
    pub entity_id: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub suppressed: bool,
    #[serde(default)]
    pub dirty: bool,
    pub payload_json: String,
}

impl SceneFeatureNode {
    fn to_export_node(&self) -> ExportFeatureNode {
        ExportFeatureNode {
            id: self.id.clone(),
            kind: self.kind.clone(),
            entity_id: self.entity_id.clone(),
            dependencies: self.dependencies.clone(),
            suppressed: self.suppressed,
            dirty: self.dirty,
            payload_json: Some(self.payload_json.clone()),
        }
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct SceneFeatureTree {
    #[serde(default)]
    pub nodes: Vec<SceneFeatureNode>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct OGScene {
    pub id: String,
    pub name: String,
    pub entities: Vec<SceneEntity>,
    #[serde(default)]
    pub feature_tree: SceneFeatureTree,
}

impl OGScene {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            entities: Vec::new(),
            feature_tree: SceneFeatureTree::default(),
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

    fn get_feature_node(&self, node_id: &str) -> Option<&SceneFeatureNode> {
        self.feature_tree
            .nodes
            .iter()
            .find(|node| node.id == node_id)
    }

    fn get_feature_node_mut(&mut self, node_id: &str) -> Option<&mut SceneFeatureNode> {
        self.feature_tree
            .nodes
            .iter_mut()
            .find(|node| node.id == node_id)
    }

    fn remove_feature_nodes_for_entity(&mut self, entity_id: &str) {
        self.feature_tree
            .nodes
            .retain(|node| node.entity_id != entity_id);
    }

    fn upsert_feature_node(&mut self, mut node: SceneFeatureNode) -> Result<(), String> {
        if node.id.trim().is_empty() {
            return Err("Feature node id cannot be empty".to_string());
        }
        if node.entity_id.trim().is_empty() {
            return Err(format!("Feature node '{}' has empty entity_id", node.id));
        }
        if node.payload_json.trim().is_empty() {
            return Err(format!("Feature node '{}' has empty payload_json", node.id));
        }

        node.dirty = true;
        let changed_id = node.id.clone();

        if let Some(existing) = self.get_feature_node_mut(&node.id) {
            *existing = node;
        } else {
            self.feature_tree.nodes.push(node);
        }

        self.validate_feature_tree()?;
        self.mark_dirty_with_descendants(&changed_id);

        self.recompute_dirty_features()
    }

    fn remove_feature_node(&mut self, node_id: &str) -> Result<bool, String> {
        let Some(position) = self
            .feature_tree
            .nodes
            .iter()
            .position(|node| node.id == node_id)
        else {
            return Ok(false);
        };

        let removed = self.feature_tree.nodes.remove(position);
        self.remove_entity(&removed.entity_id);

        let dependents: Vec<String> = self
            .feature_tree
            .nodes
            .iter()
            .filter(|node| node.dependencies.iter().any(|dep| dep == node_id))
            .map(|node| node.id.clone())
            .collect();

        for dep in dependents {
            if let Some(node) = self.get_feature_node_mut(&dep) {
                node.dependencies.retain(|candidate| candidate != node_id);
                node.dirty = true;
            }
        }

        self.recompute_dirty_features()?;
        Ok(true)
    }

    fn set_feature_node_suppressed(
        &mut self,
        node_id: &str,
        suppressed: bool,
    ) -> Result<(), String> {
        let Some(node) = self.get_feature_node_mut(node_id) else {
            return Err(format!("Feature node '{}' does not exist", node_id));
        };

        node.suppressed = suppressed;
        node.dirty = true;
        self.mark_dirty_with_descendants(node_id);
        self.recompute_dirty_features()
    }

    fn mark_feature_node_dirty(&mut self, node_id: &str) -> Result<(), String> {
        if self.get_feature_node(node_id).is_none() {
            return Err(format!("Feature node '{}' does not exist", node_id));
        }
        self.mark_dirty_with_descendants(node_id);
        Ok(())
    }

    fn mark_dirty_with_descendants(&mut self, node_id: &str) {
        let mut reverse = HashMap::<String, Vec<String>>::new();
        for node in &self.feature_tree.nodes {
            for dep in &node.dependencies {
                reverse
                    .entry(dep.clone())
                    .or_default()
                    .push(node.id.clone());
            }
        }

        let mut queue = VecDeque::<String>::new();
        let mut visited = HashSet::<String>::new();
        queue.push_back(node_id.to_string());

        while let Some(current) = queue.pop_front() {
            if !visited.insert(current.clone()) {
                continue;
            }

            if let Some(node) = self.get_feature_node_mut(&current) {
                node.dirty = true;
            }

            if let Some(children) = reverse.get(&current) {
                for child in children {
                    queue.push_back(child.clone());
                }
            }
        }
    }

    fn validate_feature_tree(&self) -> Result<(), String> {
        let mut node_ids = HashSet::<String>::new();
        for node in &self.feature_tree.nodes {
            if !node_ids.insert(node.id.clone()) {
                return Err(format!("Duplicate feature node id '{}'", node.id));
            }
        }

        for node in &self.feature_tree.nodes {
            for dep in &node.dependencies {
                if !node_ids.contains(dep) {
                    return Err(format!(
                        "Feature node '{}' references missing dependency '{}'",
                        node.id, dep
                    ));
                }
            }
        }

        self.topological_order().map(|_| ())
    }

    fn topological_order(&self) -> Result<Vec<String>, String> {
        let mut indegree = HashMap::<String, usize>::new();
        let mut outgoing = HashMap::<String, Vec<String>>::new();

        for node in &self.feature_tree.nodes {
            indegree.insert(node.id.clone(), node.dependencies.len());
            for dep in &node.dependencies {
                outgoing
                    .entry(dep.clone())
                    .or_default()
                    .push(node.id.clone());
            }
        }

        let mut queue = VecDeque::<String>::new();
        for (node_id, degree) in &indegree {
            if *degree == 0 {
                queue.push_back(node_id.clone());
            }
        }

        let mut ordered = Vec::<String>::new();
        while let Some(node_id) = queue.pop_front() {
            ordered.push(node_id.clone());
            if let Some(children) = outgoing.get(&node_id) {
                for child in children {
                    if let Some(entry) = indegree.get_mut(child) {
                        *entry -= 1;
                        if *entry == 0 {
                            queue.push_back(child.clone());
                        }
                    }
                }
            }
        }

        if ordered.len() != self.feature_tree.nodes.len() {
            return Err("Feature tree contains a cyclic dependency".to_string());
        }

        Ok(ordered)
    }

    fn recompute_dirty_features(&mut self) -> Result<(), String> {
        self.validate_feature_tree()?;

        let dirty_roots: Vec<String> = self
            .feature_tree
            .nodes
            .iter()
            .filter(|node| node.dirty)
            .map(|node| node.id.clone())
            .collect();

        if dirty_roots.is_empty() {
            return Ok(());
        }

        let mut impacted = HashSet::<String>::new();
        let mut reverse = HashMap::<String, Vec<String>>::new();
        for node in &self.feature_tree.nodes {
            for dep in &node.dependencies {
                reverse
                    .entry(dep.clone())
                    .or_default()
                    .push(node.id.clone());
            }
        }

        let mut queue = VecDeque::<String>::new();
        for node in dirty_roots {
            queue.push_back(node);
        }

        while let Some(current) = queue.pop_front() {
            if !impacted.insert(current.clone()) {
                continue;
            }
            if let Some(children) = reverse.get(&current) {
                for child in children {
                    queue.push_back(child.clone());
                }
            }
        }

        let topo = self.topological_order()?;
        for node_id in topo {
            if !impacted.contains(&node_id) {
                continue;
            }

            let (suppressed, entity_id, kind, payload_json) = {
                let Some(node) = self.get_feature_node(&node_id) else {
                    continue;
                };
                (
                    node.suppressed,
                    node.entity_id.clone(),
                    node.kind.clone(),
                    node.payload_json.clone(),
                )
            };

            if suppressed {
                self.remove_entity(&entity_id);
                if let Some(node) = self.get_feature_node_mut(&node_id) {
                    node.dirty = false;
                }
                continue;
            }

            let brep: Brep = serde_json::from_str(&payload_json).map_err(|err| {
                format!(
                    "Feature node '{}' payload_json is invalid BRep JSON: {}",
                    node_id, err
                )
            })?;

            brep.validate_topology().map_err(|err| {
                format!(
                    "Feature node '{}' has invalid BRep topology: {}",
                    node_id, err
                )
            })?;

            self.upsert_entity(SceneEntity {
                id: entity_id,
                kind,
                brep,
            });

            if let Some(node) = self.get_feature_node_mut(&node_id) {
                node.dirty = false;
            }
        }

        Ok(())
    }

    fn export_feature_tree(&self) -> ExportFeatureTree {
        ExportFeatureTree {
            nodes: self
                .feature_tree
                .nodes
                .iter()
                .map(|node| node.to_export_node())
                .collect(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SceneSummary {
    pub id: String,
    pub name: String,
    pub entity_count: usize,
}

#[derive(Clone, Serialize, Deserialize)]
struct FeatureNodeInput {
    pub id: String,
    pub kind: String,
    pub entity_id: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub suppressed: bool,
    pub payload_json: String,
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

    fn upsert_entity_feature(
        &mut self,
        scene_id: &str,
        entity_id: String,
        kind: String,
        brep: Brep,
    ) -> Result<(), String> {
        let scene = self.get_scene_mut(scene_id)?;

        let payload_json = serde_json::to_string(&brep)
            .map_err(|err| format!("Failed to serialize BRep payload: {}", err))?;

        let dependencies = scene
            .get_feature_node(&entity_id)
            .map(|node| node.dependencies.clone())
            .unwrap_or_default();

        let suppressed = scene
            .get_feature_node(&entity_id)
            .map(|node| node.suppressed)
            .unwrap_or(false);

        scene.upsert_feature_node(SceneFeatureNode {
            id: entity_id.clone(),
            kind: kind.clone(),
            entity_id,
            dependencies,
            suppressed,
            dirty: true,
            payload_json,
        })
    }

    fn scene_id_or_current(&self, scene_id: Option<String>) -> Result<String, String> {
        match scene_id {
            Some(id) => Ok(id),
            None => self.current_scene_id_result(),
        }
    }

    fn build_scene_snapshot(&self, scene_id: &str) -> Result<ExportSceneSnapshot, String> {
        let scene = self.get_scene(scene_id)?;

        let mut entities = Vec::with_capacity(scene.entities.len());
        for entity in &scene.entities {
            let mesh = tessellate_brep_to_mesh(&entity.brep);
            let brep_json = serde_json::to_string(&entity.brep).map_err(|err| {
                format!(
                    "Failed to serialize scene BRep for '{}': {}",
                    entity.id, err
                )
            })?;

            entities.push(ExportEntity {
                id: entity.id.clone(),
                kind: entity.kind.clone(),
                mesh,
                semantics: None,
                material_id: None,
                brep_json: Some(brep_json),
            });
        }

        Ok(ExportSceneSnapshot {
            scene: ExportScene {
                id: scene.id.clone(),
                name: scene.name.clone(),
            },
            entities,
            feature_tree: scene.export_feature_tree(),
            ..ExportSceneSnapshot::default()
        })
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
        self.upsert_entity_feature(scene_id, entity_id.into(), kind.into(), brep.clone())
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

    pub fn add_sphere_to_scene_internal(
        &mut self,
        scene_id: &str,
        entity_id: impl Into<String>,
        sphere: &OGSphere,
    ) -> Result<(), String> {
        self.add_brep_entity_to_scene_internal(scene_id, entity_id, "OGSphere", sphere.brep())
    }

    pub fn add_wedge_to_scene_internal(
        &mut self,
        scene_id: &str,
        entity_id: impl Into<String>,
        wedge: &OGWedge,
    ) -> Result<(), String> {
        self.add_brep_entity_to_scene_internal(scene_id, entity_id, "OGWedge", wedge.brep())
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
        let snapshot = self
            .build_scene_snapshot(&scene_id)
            .map_err(|err| JsValue::from_str(&err))?;

        serde_json::to_string(&snapshot).map_err(|err| {
            JsValue::from_str(&format!("Failed to serialize scene snapshot: {}", err))
        })
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
        let removed = scene.remove_entity(&entity_id);
        scene.remove_feature_nodes_for_entity(&entity_id);
        Ok(removed)
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

        self.upsert_entity_feature(&scene_id, entity_id, kind, brep)
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

    #[wasm_bindgen(js_name = upsertFeatureNode)]
    pub fn upsert_feature_node(
        &mut self,
        scene_id: String,
        feature_node_json: String,
    ) -> Result<(), JsValue> {
        let input: FeatureNodeInput = serde_json::from_str(&feature_node_json).map_err(|err| {
            JsValue::from_str(&format!(
                "Invalid feature node JSON payload for scene '{}': {}",
                scene_id, err
            ))
        })?;

        let scene = self
            .get_scene_mut(&scene_id)
            .map_err(|err| JsValue::from_str(&err))?;

        scene
            .upsert_feature_node(SceneFeatureNode {
                id: input.id,
                kind: input.kind,
                entity_id: input.entity_id,
                dependencies: input.dependencies,
                suppressed: input.suppressed,
                dirty: true,
                payload_json: input.payload_json,
            })
            .map_err(|err| JsValue::from_str(&err))
    }

    #[wasm_bindgen(js_name = removeFeatureNode)]
    pub fn remove_feature_node(
        &mut self,
        scene_id: String,
        node_id: String,
    ) -> Result<bool, JsValue> {
        let scene = self
            .get_scene_mut(&scene_id)
            .map_err(|err| JsValue::from_str(&err))?;

        scene
            .remove_feature_node(&node_id)
            .map_err(|err| JsValue::from_str(&err))
    }

    #[wasm_bindgen(js_name = setFeatureNodeSuppressed)]
    pub fn set_feature_node_suppressed(
        &mut self,
        scene_id: String,
        node_id: String,
        suppressed: bool,
    ) -> Result<(), JsValue> {
        let scene = self
            .get_scene_mut(&scene_id)
            .map_err(|err| JsValue::from_str(&err))?;

        scene
            .set_feature_node_suppressed(&node_id, suppressed)
            .map_err(|err| JsValue::from_str(&err))
    }

    #[wasm_bindgen(js_name = recomputeSceneFeatures)]
    pub fn recompute_scene_features(&mut self, scene_id: String) -> Result<(), JsValue> {
        let scene = self
            .get_scene_mut(&scene_id)
            .map_err(|err| JsValue::from_str(&err))?;
        scene
            .recompute_dirty_features()
            .map_err(|err| JsValue::from_str(&err))
    }

    #[wasm_bindgen(js_name = markFeatureNodeDirty)]
    pub fn mark_feature_node_dirty(
        &mut self,
        scene_id: String,
        node_id: String,
    ) -> Result<(), JsValue> {
        let scene = self
            .get_scene_mut(&scene_id)
            .map_err(|err| JsValue::from_str(&err))?;
        scene
            .mark_feature_node_dirty(&node_id)
            .map_err(|err| JsValue::from_str(&err))
    }

    #[wasm_bindgen(js_name = listFeatureNodes)]
    pub fn list_feature_nodes(&self, scene_id: String) -> Result<String, JsValue> {
        let scene = self
            .get_scene(&scene_id)
            .map_err(|err| JsValue::from_str(&err))?;

        serde_json::to_string(&scene.feature_tree)
            .map_err(|err| JsValue::from_str(&format!("Failed to serialize feature tree: {}", err)))
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

fn tessellate_brep_to_mesh(brep: &Brep) -> ExportMesh {
    let mut points = Vec::<[f64; 3]>::new();
    let mut triangles = Vec::<[usize; 3]>::new();
    let mut point_map = HashMap::<String, usize>::new();

    for face in &brep.faces {
        let (outer_vertices, holes_vertices) = brep.get_vertices_and_holes_by_face_id(face.id);

        if outer_vertices.len() < 3 {
            continue;
        }
        if holes_vertices.iter().any(|hole| hole.len() < 3) {
            continue;
        }

        let triangle_indices = triangulate_polygon_with_holes(&outer_vertices, &holes_vertices);
        if triangle_indices.is_empty() {
            continue;
        }

        let mut all_vertices = outer_vertices;
        for hole in holes_vertices {
            all_vertices.extend(hole);
        }

        for triangle in triangle_indices {
            let Some((&a, &b, &c)) = all_vertices
                .get(triangle[0])
                .zip(all_vertices.get(triangle[1]))
                .zip(all_vertices.get(triangle[2]))
                .map(|((a, b), c)| (a, b, c))
            else {
                continue;
            };

            if !is_finite_vec3(a) || !is_finite_vec3(b) || !is_finite_vec3(c) {
                continue;
            }

            if is_degenerate_triangle(a, b, c) {
                continue;
            }

            let i0 = get_or_create_mesh_point(&mut points, &mut point_map, a);
            let i1 = get_or_create_mesh_point(&mut points, &mut point_map, b);
            let i2 = get_or_create_mesh_point(&mut points, &mut point_map, c);

            triangles.push([i0, i1, i2]);
        }
    }

    ExportMesh { points, triangles }
}

fn get_or_create_mesh_point(
    points: &mut Vec<[f64; 3]>,
    point_map: &mut HashMap<String, usize>,
    point: Vector3,
) -> usize {
    let key = format!("{:.9}|{:.9}|{:.9}", point.x, point.y, point.z);
    if let Some(index) = point_map.get(&key) {
        return *index;
    }

    let index = points.len();
    points.push([point.x, point.y, point.z]);
    point_map.insert(key, index);
    index
}

fn is_finite_vec3(point: Vector3) -> bool {
    point.x.is_finite() && point.y.is_finite() && point.z.is_finite()
}

fn is_degenerate_triangle(a: Vector3, b: Vector3, c: Vector3) -> bool {
    let ab = [b.x - a.x, b.y - a.y, b.z - a.z];
    let ac = [c.x - a.x, c.y - a.y, c.z - a.z];

    let cross = [
        ab[1] * ac[2] - ab[2] * ac[1],
        ab[2] * ac[0] - ab[0] * ac[2],
        ab[0] * ac[1] - ab[1] * ac[0],
    ];

    let area_sq = cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2];
    !area_sq.is_finite() || area_sq <= MESH_EPSILON
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brep::BrepBuilder;

    fn tetrahedron_brep() -> Brep {
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

        builder.build().unwrap()
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
    fn rejects_feature_tree_cycles() {
        let mut scene = OGScene::new("feature-scene");
        let brep = serde_json::to_string(&tetrahedron_brep()).unwrap();

        scene
            .upsert_feature_node(SceneFeatureNode {
                id: "a".to_string(),
                kind: "OGCuboid".to_string(),
                entity_id: "entity-a".to_string(),
                dependencies: vec!["b".to_string()],
                suppressed: false,
                dirty: true,
                payload_json: brep.clone(),
            })
            .unwrap_err();

        scene
            .upsert_feature_node(SceneFeatureNode {
                id: "a".to_string(),
                kind: "OGCuboid".to_string(),
                entity_id: "entity-a".to_string(),
                dependencies: vec![],
                suppressed: false,
                dirty: true,
                payload_json: brep.clone(),
            })
            .unwrap();

        let result = scene.upsert_feature_node(SceneFeatureNode {
            id: "b".to_string(),
            kind: "OGCuboid".to_string(),
            entity_id: "entity-b".to_string(),
            dependencies: vec!["a".to_string(), "b".to_string()],
            suppressed: false,
            dirty: true,
            payload_json: brep,
        });

        assert!(result.is_err());
    }

    #[test]
    fn serializes_scene_snapshot_with_feature_tree() {
        let mut manager = OGSceneManager::new();
        let scene_id = manager.create_scene_internal("snapshot-scene");
        let brep = tetrahedron_brep();

        manager
            .add_brep_entity_to_scene_internal(&scene_id, "tetra-1", "Tetrahedron", &brep)
            .unwrap();

        let json = manager.get_scene_serialized(scene_id.clone()).unwrap();
        let snapshot: ExportSceneSnapshot = serde_json::from_str(&json).unwrap();

        assert_eq!(snapshot.scene.id, scene_id);
        assert_eq!(snapshot.entities.len(), 1);
        assert_eq!(snapshot.feature_tree.nodes.len(), 1);
        assert!(snapshot.entities[0].mesh.triangles.len() >= 4);
    }
}
