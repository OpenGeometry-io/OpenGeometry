//! Standalone freeform geometry built on top of OpenGeometry BReps.
//!
//! This module owns the public `OGFreeformGeometry` wasm surface for creating
//! freeform geometry, inspecting its serialized geometry payloads, and managing
//! placement. Direct topology and geometry editing lives in `crate::editor`.

#[cfg(test)]
mod tests;
mod types;

use openmaths::Vector3;
use wasm_bindgen::prelude::*;

use crate::brep::Brep;
use crate::spatial::placement::Placement3D;

pub use types::ObjectTransformation;

#[wasm_bindgen]
pub struct OGFreeformGeometry {
    pub(crate) id: String,
    pub(crate) local_brep: Brep,
    pub(crate) placement: Placement3D,
}

impl ObjectTransformation {
    pub(crate) fn from_placement(placement: &Placement3D) -> Self {
        Self {
            anchor: placement.anchor,
            translation: placement.translation(),
            rotation: placement.rotation(),
            scale: placement.scale(),
        }
    }

    fn apply_to_placement(&self, placement: &mut Placement3D) -> Result<(), String> {
        placement.set_anchor(self.anchor);
        placement.set_transform(self.translation, self.rotation, self.scale)
    }
}

#[wasm_bindgen]
impl OGFreeformGeometry {
    #[wasm_bindgen(constructor)]
    pub fn new(id: String, local_brep_serialized: String) -> Result<OGFreeformGeometry, JsValue> {
        let local_brep: Brep = serde_json::from_str(&local_brep_serialized).map_err(|error| {
            JsValue::from_str(&format!(
                "Failed to deserialize freeform BRep JSON payload: {}",
                error
            ))
        })?;

        local_brep.validate_topology().map_err(|error| {
            JsValue::from_str(&format!("Invalid freeform BRep topology: {}", error))
        })?;

        Ok(Self {
            id,
            local_brep,
            placement: Placement3D::new(),
        })
    }

    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.id.clone()
    }

    #[wasm_bindgen(js_name = getBrepSerialized)]
    pub fn get_brep_serialized(&self) -> String {
        serde_json::to_string(&self.world_brep()).unwrap_or_else(|_| "{}".to_string())
    }

    #[wasm_bindgen(js_name = getLocalBrepSerialized)]
    pub fn get_local_brep_serialized(&self) -> String {
        serde_json::to_string(&self.local_brep).unwrap_or_else(|_| "{}".to_string())
    }

    #[wasm_bindgen(js_name = getGeometrySerialized)]
    pub fn get_geometry_serialized(&self) -> String {
        let world = self.world_brep();
        serde_json::to_string(&world.get_triangle_vertex_buffer())
            .unwrap_or_else(|_| "[]".to_string())
    }

    #[wasm_bindgen(js_name = getGeometryBuffer)]
    pub fn get_geometry_buffer(&self) -> Vec<f64> {
        self.world_brep().get_triangle_vertex_buffer()
    }

    #[wasm_bindgen(js_name = getLocalGeometryBuffer)]
    pub fn get_local_geometry_buffer(&self) -> Vec<f64> {
        self.local_brep.get_triangle_vertex_buffer()
    }

    #[wasm_bindgen(js_name = getOutlineGeometrySerialized)]
    pub fn get_outline_geometry_serialized(&self) -> String {
        let world = self.world_brep();
        serde_json::to_string(&world.get_outline_vertex_buffer())
            .unwrap_or_else(|_| "[]".to_string())
    }

    #[wasm_bindgen(js_name = getOutlineGeometryBuffer)]
    pub fn get_outline_geometry_buffer(&self) -> Vec<f64> {
        self.world_brep().get_outline_vertex_buffer()
    }

    #[wasm_bindgen(js_name = getLocalOutlineGeometryBuffer)]
    pub fn get_local_outline_geometry_buffer(&self) -> Vec<f64> {
        self.local_brep.get_outline_vertex_buffer()
    }

    #[wasm_bindgen(js_name = getPlacementSerialized)]
    pub fn get_placement_serialized(&self) -> String {
        serde_json::to_string(&ObjectTransformation::from_placement(&self.placement))
            .unwrap_or_else(|_| "{}".to_string())
    }

    #[wasm_bindgen(js_name = setPlacementSerialized)]
    pub fn set_placement_serialized(
        &mut self,
        placement_serialized: String,
    ) -> Result<(), JsValue> {
        let transform: ObjectTransformation = serde_json::from_str(&placement_serialized)
            .map_err(|error| JsValue::from_str(&format!("Invalid placement payload: {}", error)))?;

        transform
            .apply_to_placement(&mut self.placement)
            .map_err(|error| JsValue::from_str(&error))
    }

    #[wasm_bindgen(js_name = setTransform)]
    pub fn set_transform(
        &mut self,
        translation: Vector3,
        rotation: Vector3,
        scale: Vector3,
    ) -> Result<(), JsValue> {
        self.placement
            .set_transform(translation, rotation, scale)
            .map_err(|error| JsValue::from_str(&error))
    }

    #[wasm_bindgen(js_name = setTranslation)]
    pub fn set_translation(&mut self, translation: Vector3) {
        self.placement.set_translation(translation);
    }

    #[wasm_bindgen(js_name = setRotation)]
    pub fn set_rotation(&mut self, rotation: Vector3) {
        self.placement.set_rotation(rotation);
    }

    #[wasm_bindgen(js_name = setScale)]
    pub fn set_scale(&mut self, scale: Vector3) -> Result<(), JsValue> {
        self.placement
            .set_scale(scale)
            .map_err(|error| JsValue::from_str(&error))
    }

    #[wasm_bindgen(js_name = setAnchor)]
    pub fn set_anchor(&mut self, anchor: Vector3) {
        self.placement.set_anchor(anchor);
    }
}

impl OGFreeformGeometry {
    pub(crate) fn world_brep(&self) -> Brep {
        self.local_brep.transformed(&self.placement)
    }
}
