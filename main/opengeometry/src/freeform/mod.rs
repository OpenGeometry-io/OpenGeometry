//! Freeform geometry editing built on top of OpenGeometry BReps.
//!
//! This module owns the public `OGFreeformGeometry` wasm surface. The direct
//! editing kernels live here, while higher-level GUI packages are expected to
//! decide when a parametric object should be converted into freeform mode.

mod capabilities;
mod edits;
mod inspection;
mod remap;
#[cfg(test)]
mod tests;
mod topology_display;
mod types;
mod validation;

use openmaths::Vector3;
use serde::Deserialize;
use wasm_bindgen::prelude::*;

use crate::brep::Brep;
use crate::spatial::placement::Placement3D;

use remap::{build_topology_remap, topology_changed, TopologySnapshot};
use validation::validate_geometry;

pub use types::*;

#[derive(Clone, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct EditResultOptions {
    include_brep_serialized: bool,
    include_local_brep_serialized: bool,
    include_geometry_serialized: bool,
    include_outline_geometry_serialized: bool,
    include_topology_remap: bool,
    include_deltas: bool,
}

impl Default for EditResultOptions {
    fn default() -> Self {
        Self {
            include_brep_serialized: false,
            include_local_brep_serialized: false,
            include_geometry_serialized: false,
            include_outline_geometry_serialized: false,
            include_topology_remap: true,
            include_deltas: true,
        }
    }
}

#[derive(Clone, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct EditOperationOptions {
    #[serde(flatten)]
    result: EditResultOptions,
    constraint_axis: Option<Vector3>,
    constraint_plane_normal: Option<Vector3>,
    preserve_coplanarity: bool,
    constraint_frame: String,
    open_surface_mode: bool,
}

impl Default for EditOperationOptions {
    fn default() -> Self {
        Self {
            result: EditResultOptions::default(),
            constraint_axis: None,
            constraint_plane_normal: None,
            preserve_coplanarity: false,
            constraint_frame: "local".to_string(),
            open_surface_mode: false,
        }
    }
}

#[derive(Clone, Default)]
pub(super) struct ConstraintSettings {
    axis: Option<Vector3>,
    plane_normal: Option<Vector3>,
    preserve_coplanarity: bool,
    constraint_frame: String,
}

impl From<&EditOperationOptions> for ConstraintSettings {
    fn from(options: &EditOperationOptions) -> Self {
        Self {
            axis: options.constraint_axis,
            plane_normal: options.constraint_plane_normal,
            preserve_coplanarity: options.preserve_coplanarity,
            constraint_frame: options.constraint_frame.clone(),
        }
    }
}

#[wasm_bindgen]
pub struct OGFreeformGeometry {
    id: String,
    local_brep: Brep,
    placement: Placement3D,
}

impl ObjectTransformation {
    fn from_placement(placement: &Placement3D) -> Self {
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

    #[wasm_bindgen(js_name = getOutlineGeometrySerialized)]
    pub fn get_outline_geometry_serialized(&self) -> String {
        let world = self.world_brep();
        serde_json::to_string(&world.get_outline_vertex_buffer())
            .unwrap_or_else(|_| "[]".to_string())
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

    #[wasm_bindgen(js_name = getTopologyRenderData)]
    pub fn get_topology_render_data(&self) -> Result<String, JsValue> {
        let payload = self.build_topology_display_data();
        serde_json::to_string(&payload).map_err(|error| {
            JsValue::from_str(&format!(
                "Failed to serialize topology display data: {}",
                error
            ))
        })
    }

    #[wasm_bindgen(js_name = getFaceInfo)]
    pub fn get_face_info(&self, face_id: u32) -> Result<String, JsValue> {
        let info = self.build_face_info(face_id)?;
        serde_json::to_string(&info).map_err(|error| {
            JsValue::from_str(&format!("Failed to serialize face info: {}", error))
        })
    }

    #[wasm_bindgen(js_name = getEdgeInfo)]
    pub fn get_edge_info(&self, edge_id: u32) -> Result<String, JsValue> {
        let info = self.build_edge_info(edge_id)?;
        serde_json::to_string(&info).map_err(|error| {
            JsValue::from_str(&format!("Failed to serialize edge info: {}", error))
        })
    }

    #[wasm_bindgen(js_name = getVertexInfo)]
    pub fn get_vertex_info(&self, vertex_id: u32) -> Result<String, JsValue> {
        let info = self.build_vertex_info(vertex_id)?;
        serde_json::to_string(&info).map_err(|error| {
            JsValue::from_str(&format!("Failed to serialize vertex info: {}", error))
        })
    }

    #[wasm_bindgen(js_name = getEditCapabilities)]
    pub fn get_edit_capabilities(&self) -> Result<String, JsValue> {
        let capabilities = self.build_entity_edit_capabilities();
        serde_json::to_string(&capabilities).map_err(|error| {
            JsValue::from_str(&format!("Failed to serialize edit capabilities: {}", error))
        })
    }

    #[wasm_bindgen(js_name = getFaceEditCapabilities)]
    pub fn get_face_edit_capabilities(&self, face_id: u32) -> Result<String, JsValue> {
        let capabilities = self.build_face_edit_capabilities(face_id)?;
        serde_json::to_string(&capabilities).map_err(|error| {
            JsValue::from_str(&format!(
                "Failed to serialize face edit capabilities: {}",
                error
            ))
        })
    }

    #[wasm_bindgen(js_name = getEdgeEditCapabilities)]
    pub fn get_edge_edit_capabilities(&self, edge_id: u32) -> Result<String, JsValue> {
        let capabilities = self.build_edge_edit_capabilities(edge_id)?;
        serde_json::to_string(&capabilities).map_err(|error| {
            JsValue::from_str(&format!(
                "Failed to serialize edge edit capabilities: {}",
                error
            ))
        })
    }

    #[wasm_bindgen(js_name = getVertexEditCapabilities)]
    pub fn get_vertex_edit_capabilities(&self, vertex_id: u32) -> Result<String, JsValue> {
        let capabilities = self.build_vertex_edit_capabilities(vertex_id)?;
        serde_json::to_string(&capabilities).map_err(|error| {
            JsValue::from_str(&format!(
                "Failed to serialize vertex edit capabilities: {}",
                error
            ))
        })
    }

    #[wasm_bindgen(js_name = pushPullFace)]
    pub fn push_pull_face(
        &mut self,
        face_id: u32,
        distance: f64,
        options_json: Option<String>,
    ) -> Result<String, JsValue> {
        let options = Self::parse_edit_operation_options(options_json)?;
        let constraints = ConstraintSettings::from(&options);
        self.apply_edit(&options, |entity| {
            entity.push_pull_face_internal(face_id, distance, &constraints)
        })
    }

    #[wasm_bindgen(js_name = moveFace)]
    pub fn move_face(
        &mut self,
        face_id: u32,
        translation: Vector3,
        options_json: Option<String>,
    ) -> Result<String, JsValue> {
        let options = Self::parse_edit_operation_options(options_json)?;
        let constraints = ConstraintSettings::from(&options);
        self.apply_edit(&options, |entity| {
            entity.translate_face_by_vector_internal(face_id, translation, &constraints)
        })
    }

    #[wasm_bindgen(js_name = moveEdge)]
    pub fn move_edge(
        &mut self,
        edge_id: u32,
        translation: Vector3,
        options_json: Option<String>,
    ) -> Result<String, JsValue> {
        let options = Self::parse_edit_operation_options(options_json)?;
        let constraints = ConstraintSettings::from(&options);
        self.apply_edit(&options, |entity| {
            entity.move_edge_internal(edge_id, translation, &constraints)
        })
    }

    #[wasm_bindgen(js_name = moveVertex)]
    pub fn move_vertex(
        &mut self,
        vertex_id: u32,
        translation: Vector3,
        options_json: Option<String>,
    ) -> Result<String, JsValue> {
        let options = Self::parse_edit_operation_options(options_json)?;
        let constraints = ConstraintSettings::from(&options);
        self.apply_edit(&options, |entity| {
            entity.move_vertex_internal(vertex_id, translation, &constraints)
        })
    }

    #[wasm_bindgen(js_name = extrudeFace)]
    pub fn extrude_face(
        &mut self,
        face_id: u32,
        distance: f64,
        options_json: Option<String>,
    ) -> Result<String, JsValue> {
        let options = Self::parse_edit_operation_options(options_json)?;
        self.apply_edit(&options, |entity| {
            entity.extrude_face_internal(face_id, distance, options.open_surface_mode)
        })
    }

    #[wasm_bindgen(js_name = cutFace)]
    pub fn cut_face(
        &mut self,
        face_id: u32,
        start_edge_id: u32,
        start_t: f64,
        end_edge_id: u32,
        end_t: f64,
        options_json: Option<String>,
    ) -> Result<String, JsValue> {
        let options = Self::parse_edit_operation_options(options_json)?;
        self.apply_edit(&options, |entity| {
            entity.cut_face_internal(face_id, start_edge_id, start_t, end_edge_id, end_t)
        })
    }

    #[wasm_bindgen(js_name = insertVertexOnEdge)]
    pub fn insert_vertex_on_edge(
        &mut self,
        edge_id: u32,
        t: f64,
        options_json: Option<String>,
    ) -> Result<String, JsValue> {
        let options = Self::parse_edit_operation_options(options_json)?;
        self.apply_edit(&options, |entity| {
            entity.insert_vertex_on_edge_internal(edge_id, t)
        })
    }

    #[wasm_bindgen(js_name = splitEdge)]
    pub fn split_edge(
        &mut self,
        edge_id: u32,
        t: f64,
        options_json: Option<String>,
    ) -> Result<String, JsValue> {
        let options = Self::parse_edit_operation_options(options_json)?;
        self.apply_edit(&options, |entity| entity.split_edge_internal(edge_id, t))
    }

    #[wasm_bindgen(js_name = loopCut)]
    pub fn loop_cut(
        &mut self,
        edge_id: u32,
        t: f64,
        options_json: Option<String>,
    ) -> Result<String, JsValue> {
        let options = Self::parse_edit_operation_options(options_json)?;
        self.apply_edit(&options, |entity| {
            entity.loop_cut_edge_ring_internal(edge_id, t)
        })
    }

    #[wasm_bindgen(js_name = removeVertex)]
    pub fn remove_vertex(
        &mut self,
        vertex_id: u32,
        options_json: Option<String>,
    ) -> Result<String, JsValue> {
        let options = Self::parse_edit_operation_options(options_json)?;
        self.apply_edit(&options, |entity| entity.remove_vertex_internal(vertex_id))
    }
}

impl OGFreeformGeometry {
    fn parse_edit_operation_options(
        options_json: Option<String>,
    ) -> Result<EditOperationOptions, JsValue> {
        match options_json {
            Some(raw) => serde_json::from_str(&raw).map_err(|error| {
                JsValue::from_str(&format!("Invalid edit options payload: {}", error))
            }),
            None => Ok(EditOperationOptions::default()),
        }
    }

    fn world_brep(&self) -> Brep {
        self.local_brep.transformed(&self.placement)
    }

    fn apply_edit<F>(
        &mut self,
        options: &EditOperationOptions,
        mut edit: F,
    ) -> Result<String, JsValue>
    where
        F: FnMut(&mut Self) -> Result<EditEffect, BrepDiagnostic>,
    {
        let backup_brep = self.local_brep.clone();
        let topology_before = TopologySnapshot::from_brep(&self.local_brep);

        let mut diagnostics = Vec::new();
        let mut effect = EditEffect::default();

        match edit(self) {
            Ok(outcome) => {
                diagnostics.extend(outcome.diagnostics.clone());
                effect = outcome;
            }
            Err(error) => {
                diagnostics.push(error);
            }
        }

        if !has_error_diagnostics(&diagnostics) {
            if let Err(error) = self.local_brep.validate_topology() {
                diagnostics.push(BrepDiagnostic::error(
                    "invalid_topology",
                    format!("Edited BRep has invalid topology: {}", error),
                ));
            }

            diagnostics.extend(validate_geometry(&self.local_brep));
        }

        let failed = has_error_diagnostics(&diagnostics);
        if failed {
            self.local_brep = backup_brep;
        }

        let validity = BrepValidity {
            ok: !failed,
            healed: Some(false),
            diagnostics,
        };

        self.serialize_edit_result(
            validity,
            &topology_before,
            if failed { None } else { Some(&effect) },
            &options.result,
        )
    }

    fn serialize_edit_result(
        &self,
        validity: BrepValidity,
        topology_before: &TopologySnapshot,
        effect: Option<&EditEffect>,
        options: &EditResultOptions,
    ) -> Result<String, JsValue> {
        let world = self.world_brep();
        let topology_after = TopologySnapshot::from_brep(&self.local_brep);
        let topology_changed_flag = topology_changed(topology_before, &topology_after);

        let fallback_changed_faces = if topology_changed_flag {
            Some(topology_after.face_ids().to_vec())
        } else {
            None
        };
        let fallback_changed_edges = if topology_changed_flag {
            Some(topology_after.edge_ids().to_vec())
        } else {
            None
        };
        let fallback_changed_vertices = if topology_changed_flag {
            Some(topology_after.vertex_ids().to_vec())
        } else {
            None
        };

        let result = FreeformEditResult {
            entity_id: self.id.clone(),
            brep_serialized: if options.include_brep_serialized {
                Some(serde_json::to_string(&world).map_err(|error| {
                    JsValue::from_str(&format!("Failed to serialize world BRep: {}", error))
                })?)
            } else {
                None
            },
            local_brep_serialized: if options.include_local_brep_serialized {
                Some(serde_json::to_string(&self.local_brep).map_err(|error| {
                    JsValue::from_str(&format!("Failed to serialize local BRep: {}", error))
                })?)
            } else {
                None
            },
            geometry_serialized: if options.include_geometry_serialized {
                Some(
                    serde_json::to_string(&world.get_triangle_vertex_buffer()).map_err(
                        |error| {
                            JsValue::from_str(&format!(
                                "Failed to serialize geometry buffer: {}",
                                error
                            ))
                        },
                    )?,
                )
            } else {
                None
            },
            outline_geometry_serialized: if options.include_outline_geometry_serialized {
                Some(
                    serde_json::to_string(&world.get_outline_vertex_buffer()).map_err(|error| {
                        JsValue::from_str(&format!("Failed to serialize outline buffer: {}", error))
                    })?,
                )
            } else {
                None
            },
            topology_changed: topology_changed_flag,
            topology_remap: if options.include_topology_remap {
                Some(build_topology_remap(
                    topology_before,
                    &topology_after,
                    effect.and_then(|entry| entry.topology_journal.as_ref()),
                ))
            } else {
                None
            },
            changed_faces: if options.include_deltas {
                effect
                    .map(|entry| normalize_ids(&entry.changed_faces))
                    .or(fallback_changed_faces)
            } else {
                None
            },
            changed_edges: if options.include_deltas {
                effect
                    .map(|entry| normalize_ids(&entry.changed_edges))
                    .or(fallback_changed_edges)
            } else {
                None
            },
            changed_vertices: if options.include_deltas {
                effect
                    .map(|entry| normalize_ids(&entry.changed_vertices))
                    .or(fallback_changed_vertices)
            } else {
                None
            },
            validity,
            placement: ObjectTransformation::from_placement(&self.placement),
        };

        serde_json::to_string(&result).map_err(|error| {
            JsValue::from_str(&format!("Failed to serialize edit result: {}", error))
        })
    }
}

fn has_error_diagnostics(diagnostics: &[BrepDiagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
}

fn normalize_ids(ids: &[u32]) -> Vec<u32> {
    let mut normalized = ids.to_vec();
    normalized.sort_unstable();
    normalized.dedup();
    normalized
}
