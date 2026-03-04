use std::fmt;

use crate::brep::{Brep, Face};
use crate::operations::triangulate::triangulate_polygon_with_holes;
use openmaths::Vector3;
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
    pub validate_topology: bool,
}

impl Default for StlExportConfig {
    fn default() -> Self {
        Self {
            header: Some(DEFAULT_STL_HEADER.to_string()),
            scale: 1.0,
            error_policy: StlErrorPolicy::BestEffort,
            validate_topology: true,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StlExportReport {
    pub input_breps: usize,
    pub input_faces: usize,
    pub exported_triangles: usize,
    pub skipped_faces: usize,
    pub skipped_triangles: usize,
    pub topology_errors: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum StlExportError {
    EmptyInput,
    InvalidTopology(String),
    MeshGeneration(String),
    Io(String),
}

impl fmt::Display for StlExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StlExportError::EmptyInput => write!(f, "No BREP input provided for STL export"),
            StlExportError::InvalidTopology(message) => write!(f, "Invalid topology: {}", message),
            StlExportError::MeshGeneration(message) => {
                write!(f, "Failed to generate STL mesh: {}", message)
            }
            StlExportError::Io(message) => write!(f, "STL I/O error: {}", message),
        }
    }
}

impl std::error::Error for StlExportError {}

pub fn export_brep_to_stl_bytes(
    brep: &Brep,
    config: &StlExportConfig,
) -> Result<(Vec<u8>, StlExportReport), StlExportError> {
    export_breps_to_stl_bytes([brep], config)
}

pub fn export_breps_to_stl_bytes<'a, I>(
    breps: I,
    config: &StlExportConfig,
) -> Result<(Vec<u8>, StlExportReport), StlExportError>
where
    I: IntoIterator<Item = &'a Brep>,
{
    let scale = validate_config(config)?;
    let breps: Vec<&Brep> = breps.into_iter().collect();
    if breps.is_empty() {
        return Err(StlExportError::EmptyInput);
    }

    let mut triangles: Vec<stl_io::Triangle> = Vec::new();
    let mut report = StlExportReport {
        input_breps: breps.len(),
        ..StlExportReport::default()
    };

    for brep in breps {
        if config.validate_topology {
            if let Err(error) = brep.validate_topology() {
                if config.error_policy == StlErrorPolicy::Strict {
                    return Err(StlExportError::InvalidTopology(format!(
                        "BREP {} failed validation: {}",
                        brep.id, error
                    )));
                }
                report.topology_errors += 1;
                continue;
            }
        }

        for face in &brep.faces {
            report.input_faces += 1;
            triangulate_face(
                brep,
                face,
                scale,
                config.error_policy,
                &mut triangles,
                &mut report,
            )?;
        }
    }

    if triangles.is_empty() {
        return Err(StlExportError::MeshGeneration(
            "No triangles were exported from the provided BREP inputs".to_string(),
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
pub fn export_brep_to_stl_file(
    brep: &Brep,
    file_path: &str,
    config: &StlExportConfig,
) -> Result<StlExportReport, StlExportError> {
    let (bytes, report) = export_brep_to_stl_bytes(brep, config)?;
    std::fs::write(file_path, bytes).map_err(|error| StlExportError::Io(error.to_string()))?;
    Ok(report)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn export_breps_to_stl_file<'a, I>(
    breps: I,
    file_path: &str,
    config: &StlExportConfig,
) -> Result<StlExportReport, StlExportError>
where
    I: IntoIterator<Item = &'a Brep>,
{
    let (bytes, report) = export_breps_to_stl_bytes(breps, config)?;
    std::fs::write(file_path, bytes).map_err(|error| StlExportError::Io(error.to_string()))?;
    Ok(report)
}

fn validate_config(config: &StlExportConfig) -> Result<f64, StlExportError> {
    if !config.scale.is_finite() || config.scale <= 0.0 {
        return Err(StlExportError::MeshGeneration(
            "STL scale must be a finite positive number".to_string(),
        ));
    }
    Ok(config.scale)
}

fn triangulate_face(
    brep: &Brep,
    face: &Face,
    scale: f64,
    policy: StlErrorPolicy,
    triangles: &mut Vec<stl_io::Triangle>,
    report: &mut StlExportReport,
) -> Result<(), StlExportError> {
    let (outer_vertices, holes_vertices) = brep.get_vertices_and_holes_by_face_id(face.id);
    if outer_vertices.len() < 3 {
        return handle_face_issue(
            policy,
            report,
            format!("Face {} has fewer than 3 outer vertices", face.id),
        );
    }

    if holes_vertices.iter().any(|hole| hole.len() < 3) {
        return handle_face_issue(
            policy,
            report,
            format!("Face {} has a hole with fewer than 3 vertices", face.id),
        );
    }

    let triangle_indices = triangulate_polygon_with_holes(&outer_vertices, &holes_vertices);
    if triangle_indices.is_empty() {
        return handle_face_issue(
            policy,
            report,
            format!("Face {} triangulation returned no triangles", face.id),
        );
    }

    let mut all_vertices = outer_vertices;
    for hole in holes_vertices {
        all_vertices.extend(hole);
    }

    let target_normal = face_normal_hint(face, &all_vertices);
    let mut face_has_exported_triangle = false;

    for triangle in triangle_indices {
        let Some((&a, &b, &c)) = all_vertices
            .get(triangle[0])
            .zip(all_vertices.get(triangle[1]))
            .zip(all_vertices.get(triangle[2]))
            .map(|((a, b), c)| (a, b, c))
        else {
            if policy == StlErrorPolicy::Strict {
                return Err(StlExportError::MeshGeneration(format!(
                    "Face {} produced out-of-range triangle index",
                    face.id
                )));
            }
            report.skipped_triangles += 1;
            continue;
        };

        if !is_finite_vec3(a) || !is_finite_vec3(b) || !is_finite_vec3(c) {
            if policy == StlErrorPolicy::Strict {
                return Err(StlExportError::MeshGeneration(format!(
                    "Face {} contains non-finite coordinates",
                    face.id
                )));
            }
            report.skipped_triangles += 1;
            continue;
        }

        let mut oriented = [a, b, c];
        let Some(mut normal) = compute_triangle_normal(&a, &b, &c) else {
            if policy == StlErrorPolicy::Strict {
                return Err(StlExportError::MeshGeneration(format!(
                    "Face {} contains a degenerate triangle",
                    face.id
                )));
            }
            report.skipped_triangles += 1;
            continue;
        };

        if let Some(target) = target_normal {
            if dot(normal, target) < 0.0 {
                oriented.swap(1, 2);
                normal = [-normal[0], -normal[1], -normal[2]];
            }
        }

        let Some(v0) = scaled_vertex(oriented[0], scale) else {
            if policy == StlErrorPolicy::Strict {
                return Err(StlExportError::MeshGeneration(format!(
                    "Face {} failed to scale triangle coordinates",
                    face.id
                )));
            }
            report.skipped_triangles += 1;
            continue;
        };
        let Some(v1) = scaled_vertex(oriented[1], scale) else {
            if policy == StlErrorPolicy::Strict {
                return Err(StlExportError::MeshGeneration(format!(
                    "Face {} failed to scale triangle coordinates",
                    face.id
                )));
            }
            report.skipped_triangles += 1;
            continue;
        };
        let Some(v2) = scaled_vertex(oriented[2], scale) else {
            if policy == StlErrorPolicy::Strict {
                return Err(StlExportError::MeshGeneration(format!(
                    "Face {} failed to scale triangle coordinates",
                    face.id
                )));
            }
            report.skipped_triangles += 1;
            continue;
        };

        triangles.push(stl_io::Triangle {
            normal: stl_io::Normal::new([normal[0] as f32, normal[1] as f32, normal[2] as f32]),
            vertices: [
                stl_io::Vertex::new(v0),
                stl_io::Vertex::new(v1),
                stl_io::Vertex::new(v2),
            ],
        });
        report.exported_triangles += 1;
        face_has_exported_triangle = true;
    }

    if !face_has_exported_triangle {
        return handle_face_issue(
            policy,
            report,
            format!("Face {} generated no valid triangles", face.id),
        );
    }

    Ok(())
}

fn handle_face_issue(
    policy: StlErrorPolicy,
    report: &mut StlExportReport,
    message: String,
) -> Result<(), StlExportError> {
    if policy == StlErrorPolicy::Strict {
        return Err(StlExportError::MeshGeneration(message));
    }
    report.skipped_faces += 1;
    Ok(())
}

fn face_normal_hint(face: &Face, all_vertices: &[Vector3]) -> Option<[f64; 3]> {
    let face_normal = [face.normal.x, face.normal.y, face.normal.z];
    normalize(face_normal).or_else(|| compute_polygon_normal(all_vertices))
}

fn compute_polygon_normal(vertices: &[Vector3]) -> Option<[f64; 3]> {
    if vertices.len() < 3 {
        return None;
    }

    let mut nx = 0.0;
    let mut ny = 0.0;
    let mut nz = 0.0;

    for index in 0..vertices.len() {
        let current = vertices[index];
        let next = vertices[(index + 1) % vertices.len()];
        nx += (current.y - next.y) * (current.z + next.z);
        ny += (current.z - next.z) * (current.x + next.x);
        nz += (current.x - next.x) * (current.y + next.y);
    }

    normalize([nx, ny, nz])
}

fn compute_triangle_normal(a: &Vector3, b: &Vector3, c: &Vector3) -> Option<[f64; 3]> {
    let ab = [b.x - a.x, b.y - a.y, b.z - a.z];
    let ac = [c.x - a.x, c.y - a.y, c.z - a.z];
    normalize(cross(ab, ac))
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn normalize(vector: [f64; 3]) -> Option<[f64; 3]> {
    let length_sq = dot(vector, vector);
    if !length_sq.is_finite() || length_sq <= TRIANGLE_EPSILON {
        return None;
    }
    let inv_len = length_sq.sqrt().recip();
    Some([
        vector[0] * inv_len,
        vector[1] * inv_len,
        vector[2] * inv_len,
    ])
}

fn is_finite_vec3(point: Vector3) -> bool {
    point.x.is_finite() && point.y.is_finite() && point.z.is_finite()
}

fn scaled_vertex(point: Vector3, scale: f64) -> Option<[f32; 3]> {
    let x = point.x * scale;
    let y = point.y * scale;
    let z = point.z * scale;
    if !(x.is_finite() && y.is_finite() && z.is_finite()) {
        return None;
    }
    Some([x as f32, y as f32, z as f32])
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
    use crate::brep::BrepBuilder;
    use uuid::Uuid;

    fn triangle_brep() -> Brep {
        let mut builder = BrepBuilder::new(Uuid::new_v4());
        builder.add_vertices(&[
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
        ]);
        builder
            .add_face(&[0, 1, 2], &[])
            .expect("triangle face should be valid");
        builder.build().expect("triangle brep should be valid")
    }

    #[test]
    fn exports_binary_stl_with_expected_size_and_count() {
        let brep = triangle_brep();
        let config = StlExportConfig::default();

        let (bytes, report) = export_brep_to_stl_bytes(&brep, &config).expect("export should pass");

        assert_eq!(report.exported_triangles, 1);
        assert_eq!(bytes.len(), 84 + 50);

        let count = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]);
        assert_eq!(count, 1);
    }

    #[test]
    fn writes_custom_binary_header() {
        let brep = triangle_brep();
        let config = StlExportConfig {
            header: Some("OpenGeometry Test STL".to_string()),
            ..StlExportConfig::default()
        };

        let (bytes, _) = export_brep_to_stl_bytes(&brep, &config).expect("export should pass");
        assert_eq!(
            &bytes[..b"OpenGeometry Test STL".len()],
            b"OpenGeometry Test STL"
        );
    }

    #[test]
    fn best_effort_skips_degenerate_triangles() {
        let valid = triangle_brep();
        let mut degenerate = triangle_brep();
        degenerate.vertices[2].position = degenerate.vertices[1].position;

        let (bytes, report) =
            export_breps_to_stl_bytes([&valid, &degenerate], &StlExportConfig::default())
                .expect("best-effort export should still succeed");

        assert!(!bytes.is_empty());
        assert_eq!(report.exported_triangles, 1);
        assert!(report.skipped_triangles >= 1 || report.skipped_faces >= 1);
    }

    #[test]
    fn strict_policy_fails_on_degenerate_triangles() {
        let mut degenerate = triangle_brep();
        degenerate.vertices[2].position = degenerate.vertices[1].position;

        let config = StlExportConfig {
            error_policy: StlErrorPolicy::Strict,
            ..StlExportConfig::default()
        };
        let result = export_brep_to_stl_bytes(&degenerate, &config);
        assert!(result.is_err());
    }
}
