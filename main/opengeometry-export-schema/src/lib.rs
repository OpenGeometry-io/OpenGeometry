use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const EXPORT_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ExportSceneSnapshot {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub scene: ExportScene,
    #[serde(default)]
    pub entities: Vec<ExportEntity>,
    #[serde(default)]
    pub feature_tree: ExportFeatureTree,
    #[serde(default)]
    pub materials: Vec<ExportMaterial>,
}

impl Default for ExportSceneSnapshot {
    fn default() -> Self {
        Self {
            schema_version: default_schema_version(),
            scene: ExportScene::default(),
            entities: Vec::new(),
            feature_tree: ExportFeatureTree::default(),
            materials: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ExportScene {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ExportEntity {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub mesh: ExportMesh,
    #[serde(default)]
    pub semantics: Option<IfcEntitySemantics>,
    #[serde(default)]
    pub material_id: Option<String>,
    #[serde(default)]
    pub brep_json: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ExportMesh {
    #[serde(default)]
    pub points: Vec<[f64; 3]>,
    #[serde(default)]
    pub triangles: Vec<[usize; 3]>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct IfcEntitySemantics {
    #[serde(default)]
    pub ifc_class: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub object_type: Option<String>,
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub property_sets: HashMap<String, HashMap<String, String>>,
    #[serde(default)]
    pub quantity_sets: HashMap<String, HashMap<String, f64>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ExportMaterial {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub color: Option<ExportColor>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ExportColor {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
    #[serde(default = "default_alpha")]
    pub alpha: f64,
}

impl Default for ExportColor {
    fn default() -> Self {
        Self {
            red: 0.5,
            green: 0.5,
            blue: 0.5,
            alpha: 1.0,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ExportFeatureTree {
    #[serde(default)]
    pub nodes: Vec<ExportFeatureNode>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ExportFeatureNode {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub entity_id: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub suppressed: bool,
    #[serde(default)]
    pub dirty: bool,
    #[serde(default)]
    pub payload_json: Option<String>,
}

fn default_schema_version() -> u32 {
    EXPORT_SCHEMA_VERSION
}

fn default_alpha() -> f64 {
    1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_roundtrip_serde() {
        let snapshot = ExportSceneSnapshot {
            scene: ExportScene {
                id: "scene-1".to_string(),
                name: "Main Scene".to_string(),
            },
            entities: vec![ExportEntity {
                id: "entity-1".to_string(),
                kind: "OGCuboid".to_string(),
                mesh: ExportMesh {
                    points: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                    triangles: vec![[0, 1, 2]],
                },
                semantics: Some(IfcEntitySemantics {
                    ifc_class: Some("IFCWALL".to_string()),
                    ..IfcEntitySemantics::default()
                }),
                material_id: Some("mat-1".to_string()),
                brep_json: Some("{\"id\":\"brep-1\"}".to_string()),
            }],
            feature_tree: ExportFeatureTree {
                nodes: vec![ExportFeatureNode {
                    id: "feature-1".to_string(),
                    kind: "OGCuboid".to_string(),
                    entity_id: "entity-1".to_string(),
                    dependencies: vec![],
                    suppressed: false,
                    dirty: false,
                    payload_json: None,
                }],
            },
            materials: vec![ExportMaterial {
                id: "mat-1".to_string(),
                name: "Concrete".to_string(),
                description: Some("Structural concrete".to_string()),
                category: Some("Concrete".to_string()),
                color: Some(ExportColor {
                    red: 0.7,
                    green: 0.7,
                    blue: 0.7,
                    alpha: 1.0,
                }),
            }],
            ..ExportSceneSnapshot::default()
        };

        let json = serde_json::to_string(&snapshot).expect("serialize snapshot");
        let parsed: ExportSceneSnapshot = serde_json::from_str(&json).expect("parse snapshot");
        assert_eq!(parsed, snapshot);
    }
}
