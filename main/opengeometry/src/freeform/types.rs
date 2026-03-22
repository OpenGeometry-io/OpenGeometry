use std::collections::HashMap;

use openmaths::Vector3;
use serde::{Deserialize, Serialize};

pub const GEOMETRY_EPSILON: f64 = 1.0e-9;
pub const FACE_AREA_EPSILON: f64 = 1.0e-12;

#[derive(Clone, Serialize, Deserialize)]
pub struct ObjectTransformation {
    pub anchor: Vector3,
    pub translation: Vector3,
    pub rotation: Vector3,
    pub scale: Vector3,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TopologyRemapStatus {
    Unchanged,
    Split,
    Merged,
    Deleted,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TopologyRemapEntry {
    pub old_id: u32,
    pub new_ids: Vec<u32>,
    pub primary_id: Option<u32>,
    pub status: TopologyRemapStatus,
}

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct TopologyCreatedIds {
    #[serde(default)]
    pub faces: Vec<u32>,
    #[serde(default)]
    pub edges: Vec<u32>,
    #[serde(default)]
    pub vertices: Vec<u32>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TopologyRemap {
    pub faces: Vec<TopologyRemapEntry>,
    pub edges: Vec<TopologyRemapEntry>,
    pub vertices: Vec<TopologyRemapEntry>,
    pub created_ids: TopologyCreatedIds,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BrepDiagnostic {
    pub code: String,
    pub severity: DiagnosticSeverity,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topology_id: Option<u32>,
}

impl BrepDiagnostic {
    pub fn info(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity: DiagnosticSeverity::Info,
            message: message.into(),
            domain: None,
            topology_id: None,
        }
    }

    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity: DiagnosticSeverity::Warning,
            message: message.into(),
            domain: None,
            topology_id: None,
        }
    }

    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity: DiagnosticSeverity::Error,
            message: message.into(),
            domain: None,
            topology_id: None,
        }
    }

    pub fn with_domain(mut self, domain: impl Into<String>, topology_id: Option<u32>) -> Self {
        self.domain = Some(domain.into());
        self.topology_id = topology_id;
        self
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BrepValidity {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub healed: Option<bool>,
    #[serde(default)]
    pub diagnostics: Vec<BrepDiagnostic>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct FaceInfo {
    pub face_id: u32,
    pub centroid: Vector3,
    pub normal: Vector3,
    pub surface_type: String,
    pub loop_ids: Vec<u32>,
    pub edge_ids: Vec<u32>,
    pub vertex_ids: Vec<u32>,
    pub adjacent_face_ids: Vec<u32>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct EdgeInfo {
    pub edge_id: u32,
    pub curve_type: String,
    pub start_vertex_id: u32,
    pub end_vertex_id: u32,
    pub start: Vector3,
    pub end: Vector3,
    pub incident_face_ids: Vec<u32>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct VertexInfo {
    pub vertex_id: u32,
    pub position: Vector3,
    pub edge_ids: Vec<u32>,
    pub face_ids: Vec<u32>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TopologyFaceRenderData {
    pub face_id: u32,
    pub positions: Vec<f64>,
    pub indices: Vec<u32>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TopologyEdgeRenderData {
    pub edge_id: u32,
    pub positions: Vec<f64>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TopologyVertexRenderData {
    pub vertex_id: u32,
    pub position: Vector3,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TopologyRenderData {
    pub faces: Vec<TopologyFaceRenderData>,
    pub edges: Vec<TopologyEdgeRenderData>,
    pub vertices: Vec<TopologyVertexRenderData>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct FreeformEditResult {
    pub entity_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brep_serialized: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_brep_serialized: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geometry_serialized: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outline_geometry_serialized: Option<String>,
    pub topology_changed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topology_remap: Option<TopologyRemap>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changed_faces: Option<Vec<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changed_edges: Option<Vec<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changed_vertices: Option<Vec<u32>>,
    pub validity: BrepValidity,
    pub placement: ObjectTransformation,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct EditCapabilities {
    pub can_push_pull_face: bool,
    pub can_move_face: bool,
    pub can_extrude_face: bool,
    pub can_cut_face: bool,
    pub can_move_edge: bool,
    pub can_move_vertex: bool,
    pub can_insert_vertex_on_edge: bool,
    pub can_remove_vertex: bool,
    pub can_split_edge: bool,
    pub can_loop_cut: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reasons: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct FeatureEditCapabilities {
    pub domain: String,
    pub topology_id: u32,
    pub can_push_pull_face: bool,
    pub can_move_face: bool,
    pub can_extrude_face: bool,
    pub can_cut_face: bool,
    pub can_move_edge: bool,
    pub can_move_vertex: bool,
    pub can_insert_vertex_on_edge: bool,
    pub can_remove_vertex: bool,
    pub can_split_edge: bool,
    pub can_loop_cut: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reasons: Vec<String>,
}

#[derive(Clone, Default)]
pub(super) struct TopologyDomainJournal {
    pub mapping: HashMap<u32, Vec<u32>>,
    pub created_ids: Vec<u32>,
}

impl TopologyDomainJournal {
    pub fn map(&mut self, old_id: u32, mut new_ids: Vec<u32>) {
        new_ids.sort_unstable();
        new_ids.dedup();
        self.mapping.insert(old_id, new_ids);
    }

    pub fn add_created(&mut self, id: u32) {
        self.created_ids.push(id);
    }
}

#[derive(Clone, Default)]
pub(super) struct TopologyChangeJournal {
    pub faces: TopologyDomainJournal,
    pub edges: TopologyDomainJournal,
    pub vertices: TopologyDomainJournal,
}

#[derive(Default)]
pub(super) struct EditEffect {
    pub diagnostics: Vec<BrepDiagnostic>,
    pub topology_journal: Option<TopologyChangeJournal>,
    pub changed_faces: Vec<u32>,
    pub changed_edges: Vec<u32>,
    pub changed_vertices: Vec<u32>,
}
