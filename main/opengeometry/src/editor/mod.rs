//! Freeform/BRep editing and inspection that operates on `OGFreeformGeometry`.
//!
//! The editor module owns direct geometry edits, topology inspection, edit
//! capability queries, and edit result serialization. `freeform` remains the
//! standalone geometry entity that stores local BRep data plus placement.

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

use crate::freeform::{OGFreeformGeometry, ObjectTransformation};

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
pub struct OGFreeformEditor;

#[wasm_bindgen]
impl OGFreeformEditor {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self
    }

    #[wasm_bindgen(js_name = getTopologyRenderData)]
    pub fn get_topology_render_data(
        &self,
        geometry: &OGFreeformGeometry,
    ) -> Result<String, JsValue> {
        let payload = geometry.build_topology_display_data();
        serde_json::to_string(&payload).map_err(|error| {
            JsValue::from_str(&format!(
                "Failed to serialize topology display data: {}",
                error
            ))
        })
    }

    #[wasm_bindgen(js_name = getFaceInfo)]
    pub fn get_face_info(
        &self,
        geometry: &OGFreeformGeometry,
        face_id: u32,
    ) -> Result<String, JsValue> {
        let info = geometry.build_face_info(face_id)?;
        serde_json::to_string(&info).map_err(|error| {
            JsValue::from_str(&format!("Failed to serialize face info: {}", error))
        })
    }

    #[wasm_bindgen(js_name = getEdgeInfo)]
    pub fn get_edge_info(
        &self,
        geometry: &OGFreeformGeometry,
        edge_id: u32,
    ) -> Result<String, JsValue> {
        let info = geometry.build_edge_info(edge_id)?;
        serde_json::to_string(&info).map_err(|error| {
            JsValue::from_str(&format!("Failed to serialize edge info: {}", error))
        })
    }

    #[wasm_bindgen(js_name = getVertexInfo)]
    pub fn get_vertex_info(
        &self,
        geometry: &OGFreeformGeometry,
        vertex_id: u32,
    ) -> Result<String, JsValue> {
        let info = geometry.build_vertex_info(vertex_id)?;
        serde_json::to_string(&info).map_err(|error| {
            JsValue::from_str(&format!("Failed to serialize vertex info: {}", error))
        })
    }

    #[wasm_bindgen(js_name = getEditCapabilities)]
    pub fn get_edit_capabilities(&self, geometry: &OGFreeformGeometry) -> Result<String, JsValue> {
        let capabilities = geometry.build_entity_edit_capabilities();
        serde_json::to_string(&capabilities).map_err(|error| {
            JsValue::from_str(&format!("Failed to serialize edit capabilities: {}", error))
        })
    }

    #[wasm_bindgen(js_name = getFaceEditCapabilities)]
    pub fn get_face_edit_capabilities(
        &self,
        geometry: &OGFreeformGeometry,
        face_id: u32,
    ) -> Result<String, JsValue> {
        let capabilities = geometry.build_face_edit_capabilities(face_id)?;
        serde_json::to_string(&capabilities).map_err(|error| {
            JsValue::from_str(&format!(
                "Failed to serialize face edit capabilities: {}",
                error
            ))
        })
    }

    #[wasm_bindgen(js_name = getEdgeEditCapabilities)]
    pub fn get_edge_edit_capabilities(
        &self,
        geometry: &OGFreeformGeometry,
        edge_id: u32,
    ) -> Result<String, JsValue> {
        let capabilities = geometry.build_edge_edit_capabilities(edge_id)?;
        serde_json::to_string(&capabilities).map_err(|error| {
            JsValue::from_str(&format!(
                "Failed to serialize edge edit capabilities: {}",
                error
            ))
        })
    }

    #[wasm_bindgen(js_name = getVertexEditCapabilities)]
    pub fn get_vertex_edit_capabilities(
        &self,
        geometry: &OGFreeformGeometry,
        vertex_id: u32,
    ) -> Result<String, JsValue> {
        let capabilities = geometry.build_vertex_edit_capabilities(vertex_id)?;
        serde_json::to_string(&capabilities).map_err(|error| {
            JsValue::from_str(&format!(
                "Failed to serialize vertex edit capabilities: {}",
                error
            ))
        })
    }

    #[wasm_bindgen(js_name = pushPullFace)]
    pub fn push_pull_face(
        &self,
        geometry: &mut OGFreeformGeometry,
        face_id: u32,
        distance: f64,
        options_json: Option<String>,
    ) -> Result<String, JsValue> {
        let options = Self::parse_edit_operation_options(options_json)?;
        let constraints = ConstraintSettings::from(&options);
        self.apply_edit(geometry, &options, |entity| {
            entity.push_pull_face_internal(face_id, distance, &constraints)
        })
    }

    #[wasm_bindgen(js_name = moveFace)]
    pub fn move_face(
        &self,
        geometry: &mut OGFreeformGeometry,
        face_id: u32,
        translation: Vector3,
        options_json: Option<String>,
    ) -> Result<String, JsValue> {
        let options = Self::parse_edit_operation_options(options_json)?;
        let constraints = ConstraintSettings::from(&options);
        self.apply_edit(geometry, &options, |entity| {
            entity.translate_face_by_vector_internal(face_id, translation, &constraints)
        })
    }

    #[wasm_bindgen(js_name = moveEdge)]
    pub fn move_edge(
        &self,
        geometry: &mut OGFreeformGeometry,
        edge_id: u32,
        translation: Vector3,
        options_json: Option<String>,
    ) -> Result<String, JsValue> {
        let options = Self::parse_edit_operation_options(options_json)?;
        let constraints = ConstraintSettings::from(&options);
        self.apply_edit(geometry, &options, |entity| {
            entity.move_edge_internal(edge_id, translation, &constraints)
        })
    }

    #[wasm_bindgen(js_name = moveVertex)]
    pub fn move_vertex(
        &self,
        geometry: &mut OGFreeformGeometry,
        vertex_id: u32,
        translation: Vector3,
        options_json: Option<String>,
    ) -> Result<String, JsValue> {
        let options = Self::parse_edit_operation_options(options_json)?;
        let constraints = ConstraintSettings::from(&options);
        self.apply_edit(geometry, &options, |entity| {
            entity.move_vertex_internal(vertex_id, translation, &constraints)
        })
    }

    #[wasm_bindgen(js_name = extrudeFace)]
    pub fn extrude_face(
        &self,
        geometry: &mut OGFreeformGeometry,
        face_id: u32,
        distance: f64,
        options_json: Option<String>,
    ) -> Result<String, JsValue> {
        let options = Self::parse_edit_operation_options(options_json)?;
        self.apply_edit(geometry, &options, |entity| {
            entity.extrude_face_internal(face_id, distance, options.open_surface_mode)
        })
    }

    #[wasm_bindgen(js_name = cutFace)]
    pub fn cut_face(
        &self,
        geometry: &mut OGFreeformGeometry,
        face_id: u32,
        start_edge_id: u32,
        start_t: f64,
        end_edge_id: u32,
        end_t: f64,
        options_json: Option<String>,
    ) -> Result<String, JsValue> {
        let options = Self::parse_edit_operation_options(options_json)?;
        self.apply_edit(geometry, &options, |entity| {
            entity.cut_face_internal(face_id, start_edge_id, start_t, end_edge_id, end_t)
        })
    }

    #[wasm_bindgen(js_name = insertVertexOnEdge)]
    pub fn insert_vertex_on_edge(
        &self,
        geometry: &mut OGFreeformGeometry,
        edge_id: u32,
        t: f64,
        options_json: Option<String>,
    ) -> Result<String, JsValue> {
        let options = Self::parse_edit_operation_options(options_json)?;
        self.apply_edit(geometry, &options, |entity| {
            entity.insert_vertex_on_edge_internal(edge_id, t)
        })
    }

    #[wasm_bindgen(js_name = splitEdge)]
    pub fn split_edge(
        &self,
        geometry: &mut OGFreeformGeometry,
        edge_id: u32,
        t: f64,
        options_json: Option<String>,
    ) -> Result<String, JsValue> {
        let options = Self::parse_edit_operation_options(options_json)?;
        self.apply_edit(geometry, &options, |entity| {
            entity.split_edge_internal(edge_id, t)
        })
    }

    #[wasm_bindgen(js_name = loopCut)]
    pub fn loop_cut(
        &self,
        geometry: &mut OGFreeformGeometry,
        edge_id: u32,
        t: f64,
        options_json: Option<String>,
    ) -> Result<String, JsValue> {
        let options = Self::parse_edit_operation_options(options_json)?;
        self.apply_edit(geometry, &options, |entity| {
            entity.loop_cut_edge_ring_internal(edge_id, t)
        })
    }

    #[wasm_bindgen(js_name = removeVertex)]
    pub fn remove_vertex(
        &self,
        geometry: &mut OGFreeformGeometry,
        vertex_id: u32,
        options_json: Option<String>,
    ) -> Result<String, JsValue> {
        let options = Self::parse_edit_operation_options(options_json)?;
        self.apply_edit(geometry, &options, |entity| {
            entity.remove_vertex_internal(vertex_id)
        })
    }
}

impl OGFreeformEditor {
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

    fn apply_edit<F>(
        &self,
        geometry: &mut OGFreeformGeometry,
        options: &EditOperationOptions,
        mut edit: F,
    ) -> Result<String, JsValue>
    where
        F: FnMut(&mut OGFreeformGeometry) -> Result<EditEffect, BrepDiagnostic>,
    {
        let backup_brep = geometry.local_brep.clone();
        let topology_before = TopologySnapshot::from_brep(&geometry.local_brep);

        let mut diagnostics = Vec::new();
        let mut effect = EditEffect::default();

        match edit(geometry) {
            Ok(outcome) => {
                diagnostics.extend(outcome.diagnostics.clone());
                effect = outcome;
            }
            Err(error) => {
                diagnostics.push(error);
            }
        }

        if !has_error_diagnostics(&diagnostics) {
            if let Err(error) = geometry.local_brep.validate_topology() {
                diagnostics.push(BrepDiagnostic::error(
                    "invalid_topology",
                    format!("Edited BRep has invalid topology: {}", error),
                ));
            }

            diagnostics.extend(validate_geometry(&geometry.local_brep));
        }

        let failed = has_error_diagnostics(&diagnostics);
        if failed {
            geometry.local_brep = backup_brep;
        }

        let validity = BrepValidity {
            ok: !failed,
            healed: Some(false),
            diagnostics,
        };

        self.serialize_edit_result(
            geometry,
            validity,
            &topology_before,
            if failed { None } else { Some(&effect) },
            &options.result,
        )
    }

    fn serialize_edit_result(
        &self,
        geometry: &OGFreeformGeometry,
        validity: BrepValidity,
        topology_before: &TopologySnapshot,
        effect: Option<&EditEffect>,
        options: &EditResultOptions,
    ) -> Result<String, JsValue> {
        let world = geometry.world_brep();
        let topology_after = TopologySnapshot::from_brep(&geometry.local_brep);
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
            entity_id: geometry.id.clone(),
            brep_serialized: if options.include_brep_serialized {
                Some(serde_json::to_string(&world).map_err(|error| {
                    JsValue::from_str(&format!("Failed to serialize world BRep: {}", error))
                })?)
            } else {
                None
            },
            local_brep_serialized: if options.include_local_brep_serialized {
                Some(
                    serde_json::to_string(&geometry.local_brep).map_err(|error| {
                        JsValue::from_str(&format!("Failed to serialize local BRep: {}", error))
                    })?,
                )
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
            placement: ObjectTransformation::from_placement(&geometry.placement),
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
