use std::fmt;

use opengeometry_export_schema::{ExportMesh, ExportSceneSnapshot};
use serde::{Deserialize, Serialize};

const DEFAULT_STL_HEADER: &str = "OpenGeometry STL Export";
const STL_HEADER_BYTES: usize = 80;
const TRIANGLE_EPSILON: f64 = 1.0e-12;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum StlErrorPolicy {
    Strict,
    BestEffort,
}

impl Default for StlErrorPolicy {
    fn default() -> Self {
        Self::BestEffort
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StlExportConfig {
    pub header: Option<String>,
    pub scale: f64,
    pub error_policy: StlErrorPolicy,
}

impl Default for StlExportConfig {
    fn default() -> Self {
        Self {
            header: Some(DEFAULT_STL_HEADER.to_string()),
            scale: 1.0,
            error_policy: StlErrorPolicy::BestEffort,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StlExportReport {
    pub input_entities: usize,
    pub input_triangles: usize,
    pub exported_triangles: usize,
    pub skipped_entities: usize,
    pub skipped_triangles: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum StlExportError {
    EmptyInput,
    InvalidMesh(String),
    Io(String),
}

impl fmt::Display for StlExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StlExportError::EmptyInput => write!(f, "No input entities provided for STL export"),
            StlExportError::InvalidMesh(message) => {
                write!(f, "Failed to generate STL mesh: {}", message)
            }
            StlExportError::Io(message) => write!(f, "STL I/O error: {}", message),
        }
    }
}

impl std::error::Error for StlExportError {}

pub fn export_snapshot_to_stl_bytes(
    snapshot: &ExportSceneSnapshot,
    config: &StlExportConfig,
) -> Result<(Vec<u8>, StlExportReport), StlExportError> {
    let scale = validate_config(config)?;

    if snapshot.entities.is_empty() {
        return Err(StlExportError::EmptyInput);
    }

    let mut triangles: Vec<stl_io::Triangle> = Vec::new();
    let mut report = StlExportReport {
        input_entities: snapshot.entities.len(),
        ..StlExportReport::default()
    };

    for entity in &snapshot.entities {
        let before_count = triangles.len();
        export_entity_mesh(
            &entity.id,
            &entity.mesh,
            scale,
            config.error_policy,
            &mut triangles,
            &mut report,
        )?;

        if triangles.len() == before_count {
            report.skipped_entities += 1;
        }
    }

    if triangles.is_empty() {
        return Err(StlExportError::InvalidMesh(
            "No triangles were exported from the provided snapshot".to_string(),
        ));
    }

    let mut bytes = Vec::with_capacity(STL_HEADER_BYTES + 4 + triangles.len() * 50);
    stl_io::write_stl(&mut bytes, triangles.iter())
        .map_err(|error| StlExportError::Io(error.to_string()))?;

    apply_header(
        &mut bytes,
        config.header.as_deref().unwrap_or(DEFAULT_STL_HEADER),
    );

    Ok((bytes, report))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn export_snapshot_to_stl_file(
    snapshot: &ExportSceneSnapshot,
    file_path: &str,
    config: &StlExportConfig,
) -> Result<StlExportReport, StlExportError> {
    let (bytes, report) = export_snapshot_to_stl_bytes(snapshot, config)?;
    std::fs::write(file_path, bytes).map_err(|error| StlExportError::Io(error.to_string()))?;
    Ok(report)
}

fn validate_config(config: &StlExportConfig) -> Result<f64, StlExportError> {
    if !config.scale.is_finite() || config.scale <= 0.0 {
        return Err(StlExportError::InvalidMesh(
            "STL scale must be a finite positive number".to_string(),
        ));
    }
    Ok(config.scale)
}

fn export_entity_mesh(
    entity_id: &str,
    mesh: &ExportMesh,
    scale: f64,
    policy: StlErrorPolicy,
    triangles: &mut Vec<stl_io::Triangle>,
    report: &mut StlExportReport,
) -> Result<(), StlExportError> {
    if mesh.points.is_empty() {
        if policy == StlErrorPolicy::Strict {
            return Err(StlExportError::InvalidMesh(format!(
                "Entity '{}' has empty points array",
                entity_id
            )));
        }
        return Ok(());
    }

    for tri in &mesh.triangles {
        report.input_triangles += 1;

        if tri[0] >= mesh.points.len() || tri[1] >= mesh.points.len() || tri[2] >= mesh.points.len()
        {
            if policy == StlErrorPolicy::Strict {
                return Err(StlExportError::InvalidMesh(format!(
                    "Entity '{}' has out-of-range triangle index",
                    entity_id
                )));
            }
            report.skipped_triangles += 1;
            continue;
        }

        let Some(v0) = scaled_vertex(mesh.points[tri[0]], scale) else {
            if policy == StlErrorPolicy::Strict {
                return Err(StlExportError::InvalidMesh(format!(
                    "Entity '{}' has non-finite point coordinates",
                    entity_id
                )));
            }
            report.skipped_triangles += 1;
            continue;
        };
        let Some(v1) = scaled_vertex(mesh.points[tri[1]], scale) else {
            if policy == StlErrorPolicy::Strict {
                return Err(StlExportError::InvalidMesh(format!(
                    "Entity '{}' has non-finite point coordinates",
                    entity_id
                )));
            }
            report.skipped_triangles += 1;
            continue;
        };
        let Some(v2) = scaled_vertex(mesh.points[tri[2]], scale) else {
            if policy == StlErrorPolicy::Strict {
                return Err(StlExportError::InvalidMesh(format!(
                    "Entity '{}' has non-finite point coordinates",
                    entity_id
                )));
            }
            report.skipped_triangles += 1;
            continue;
        };

        let Some(normal) = compute_triangle_normal(v0, v1, v2) else {
            if policy == StlErrorPolicy::Strict {
                return Err(StlExportError::InvalidMesh(format!(
                    "Entity '{}' contains degenerate triangle",
                    entity_id
                )));
            }
            report.skipped_triangles += 1;
            continue;
        };

        triangles.push(stl_io::Triangle {
            normal: stl_io::Normal::new([normal[0], normal[1], normal[2]]),
            vertices: [
                stl_io::Vertex::new(v0),
                stl_io::Vertex::new(v1),
                stl_io::Vertex::new(v2),
            ],
        });
        report.exported_triangles += 1;
    }

    Ok(())
}

fn scaled_vertex(point: [f64; 3], scale: f64) -> Option<[f32; 3]> {
    let x = point[0] * scale;
    let y = point[1] * scale;
    let z = point[2] * scale;

    if !(x.is_finite() && y.is_finite() && z.is_finite()) {
        return None;
    }

    Some([x as f32, y as f32, z as f32])
}

fn compute_triangle_normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> Option<[f32; 3]> {
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];

    let cross = [
        ab[1] * ac[2] - ab[2] * ac[1],
        ab[2] * ac[0] - ab[0] * ac[2],
        ab[0] * ac[1] - ab[1] * ac[0],
    ];

    let length_sq = cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2];
    if !length_sq.is_finite() || length_sq <= TRIANGLE_EPSILON as f32 {
        return None;
    }

    let inv = length_sq.sqrt().recip();
    Some([cross[0] * inv, cross[1] * inv, cross[2] * inv])
}

fn apply_header(bytes: &mut [u8], header: &str) {
    if bytes.len() < STL_HEADER_BYTES {
        return;
    }

    bytes[..STL_HEADER_BYTES].fill(0);
    let header_bytes = header.as_bytes();
    let copy_len = header_bytes.len().min(STL_HEADER_BYTES);
    bytes[..copy_len].copy_from_slice(&header_bytes[..copy_len]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use opengeometry_export_schema::{
        ExportEntity, ExportFeatureTree, ExportMesh, ExportScene, ExportSceneSnapshot,
    };

    #[test]
    fn exports_binary_stl_from_snapshot() {
        let snapshot = ExportSceneSnapshot {
            scene: ExportScene {
                id: "scene-1".to_string(),
                name: "Sample".to_string(),
            },
            entities: vec![ExportEntity {
                id: "entity-1".to_string(),
                kind: "Triangle".to_string(),
                mesh: ExportMesh {
                    points: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                    triangles: vec![[0, 1, 2]],
                },
                ..ExportEntity::default()
            }],
            feature_tree: ExportFeatureTree::default(),
            ..ExportSceneSnapshot::default()
        };

        let (bytes, report) = export_snapshot_to_stl_bytes(&snapshot, &StlExportConfig::default())
            .expect("stl export");

        assert!(bytes.len() >= 84);
        assert_eq!(report.exported_triangles, 1);
        let count = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]);
        assert_eq!(count, 1);
    }
}
