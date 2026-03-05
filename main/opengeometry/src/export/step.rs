use std::collections::HashMap;
use std::fmt;

use openmaths::Vector3;
use serde::{Deserialize, Serialize};

use crate::brep::Brep;
use crate::operations::triangulate::triangulate_polygon_with_holes;

use super::part21::{sanitize_string_literal, Part21Writer};

const STEP_LENGTH_EPSILON: f64 = 1.0e-12;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum StepErrorPolicy {
    Strict,
    BestEffort,
}

impl Default for StepErrorPolicy {
    fn default() -> Self {
        Self::BestEffort
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum StepSchema {
    AutomotiveDesign,
}

impl Default for StepSchema {
    fn default() -> Self {
        Self::AutomotiveDesign
    }
}

impl StepSchema {
    fn as_file_schema(self) -> &'static str {
        match self {
            Self::AutomotiveDesign => "AUTOMOTIVE_DESIGN",
        }
    }

    fn as_application_context(self) -> &'static str {
        match self {
            Self::AutomotiveDesign => "automotive_design",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StepExportConfig {
    pub schema: StepSchema,
    pub product_name: Option<String>,
    pub scale: f64,
    pub error_policy: StepErrorPolicy,
    pub validate_topology: bool,
    pub require_closed_shell: bool,
}

impl Default for StepExportConfig {
    fn default() -> Self {
        Self {
            schema: StepSchema::default(),
            product_name: Some("OpenGeometry STEP Export".to_string()),
            scale: 1.0,
            error_policy: StepErrorPolicy::BestEffort,
            validate_topology: true,
            require_closed_shell: true,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StepExportReport {
    pub input_breps: usize,
    pub input_faces: usize,
    pub exported_solids: usize,
    pub exported_faces: usize,
    pub exported_triangles: usize,
    pub skipped_entities: usize,
    pub skipped_faces: usize,
    pub topology_errors: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum StepExportError {
    EmptyInput,
    InvalidTopology(String),
    UnsupportedEntity(String),
    MeshGeneration(String),
    Serialization(String),
    Io(String),
}

impl fmt::Display for StepExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StepExportError::EmptyInput => write!(f, "No BREP input provided for STEP export"),
            StepExportError::InvalidTopology(msg) => write!(f, "Invalid topology: {}", msg),
            StepExportError::UnsupportedEntity(msg) => write!(f, "Unsupported BREP: {}", msg),
            StepExportError::MeshGeneration(msg) => write!(f, "Mesh generation failed: {}", msg),
            StepExportError::Serialization(msg) => {
                write!(f, "STEP Part-21 serialization failed: {}", msg)
            }
            StepExportError::Io(msg) => write!(f, "STEP I/O failed: {}", msg),
        }
    }
}

impl std::error::Error for StepExportError {}

pub fn export_brep_to_step_text(
    brep: &Brep,
    config: &StepExportConfig,
) -> Result<(String, StepExportReport), StepExportError> {
    export_breps_to_step_text([brep], config)
}

pub fn export_breps_to_step_text<'a, I>(
    breps: I,
    config: &StepExportConfig,
) -> Result<(String, StepExportReport), StepExportError>
where
    I: IntoIterator<Item = &'a Brep>,
{
    let scale = validate_config(config)?;
    let breps: Vec<&Brep> = breps.into_iter().collect();
    if breps.is_empty() {
        return Err(StepExportError::EmptyInput);
    }

    let mut report = StepExportReport {
        input_breps: breps.len(),
        ..StepExportReport::default()
    };

    let file_name = config
        .product_name
        .clone()
        .unwrap_or_else(|| "opengeometry-export".to_string());
    let mut writer = Part21Writer::new(config.schema.as_file_schema());
    writer.set_description("OpenGeometry STEP Export");
    writer.set_file_name(file_name.clone());

    let mut solid_ids = Vec::new();

    for (brep_index, brep) in breps.iter().enumerate() {
        if config.validate_topology {
            if let Err(error) = brep.validate_topology() {
                if config.error_policy == StepErrorPolicy::Strict {
                    return Err(StepExportError::InvalidTopology(format!(
                        "BREP {} failed validation: {}",
                        brep.id, error
                    )));
                }
                report.topology_errors += 1;
                report.skipped_entities += 1;
                continue;
            }
        }

        if config.require_closed_shell && !is_closed_solid(brep) {
            let message = format!("BREP {} is not a closed shell solid", brep.id);
            if config.error_policy == StepErrorPolicy::Strict {
                return Err(StepExportError::UnsupportedEntity(message));
            }
            report.skipped_entities += 1;
            continue;
        }

        let triangles = triangulate_brep_faces(
            brep,
            scale,
            config.error_policy,
            &mut report,
            format!("BREP index {}", brep_index),
        )?;

        if triangles.is_empty() {
            if config.error_policy == StepErrorPolicy::Strict {
                return Err(StepExportError::MeshGeneration(format!(
                    "BREP {} generated no valid triangles",
                    brep.id
                )));
            }
            report.skipped_entities += 1;
            continue;
        }

        let mut point_map: HashMap<String, usize> = HashMap::new();
        let mut face_ids = Vec::new();

        for triangle in &triangles {
            let p0 = get_or_create_point(&mut writer, &mut point_map, triangle[0]);
            let p1 = get_or_create_point(&mut writer, &mut point_map, triangle[1]);
            let p2 = get_or_create_point(&mut writer, &mut point_map, triangle[2]);

            let poly_loop = writer.add_entity(format!(
                "POLY_LOOP('',({},{},{}))",
                Part21Writer::reference(p0),
                Part21Writer::reference(p1),
                Part21Writer::reference(p2)
            ));
            let outer_bound = writer.add_entity(format!(
                "FACE_OUTER_BOUND('',{},.T.)",
                Part21Writer::reference(poly_loop)
            ));
            let face = writer.add_entity(format!(
                "FACE('',({}))",
                Part21Writer::reference(outer_bound)
            ));
            face_ids.push(face);
        }

        if face_ids.is_empty() {
            if config.error_policy == StepErrorPolicy::Strict {
                return Err(StepExportError::MeshGeneration(format!(
                    "BREP {} has no exportable faces",
                    brep.id
                )));
            }
            report.skipped_entities += 1;
            continue;
        }

        let shell = writer.add_entity(format!("CLOSED_SHELL('',({}))", join_refs(&face_ids)));

        let solid = writer.add_entity(format!(
            "MANIFOLD_SOLID_BREP('{}',{})",
            sanitize_string_literal(&format!("solid-{}", brep_index)),
            Part21Writer::reference(shell)
        ));

        report.exported_faces += face_ids.len();
        report.exported_triangles += triangles.len();
        report.exported_solids += 1;
        solid_ids.push(solid);
    }

    if solid_ids.is_empty() {
        return Err(StepExportError::MeshGeneration(
            "No solids were exported from the provided BREP inputs".to_string(),
        ));
    }

    let app_context = writer.add_entity(format!(
        "APPLICATION_CONTEXT('{}')",
        sanitize_string_literal(config.schema.as_application_context())
    ));

    writer.add_entity(format!(
        "APPLICATION_PROTOCOL_DEFINITION('international standard','{}',2000,{})",
        sanitize_string_literal(config.schema.as_application_context()),
        Part21Writer::reference(app_context)
    ));

    let product_context = writer.add_entity(format!(
        "PRODUCT_CONTEXT('',{},'mechanical')",
        Part21Writer::reference(app_context)
    ));

    let product = writer.add_entity(format!(
        "PRODUCT('{}','{}','',({}))",
        sanitize_string_literal("opengeometry-product"),
        sanitize_string_literal(&file_name),
        Part21Writer::reference(product_context)
    ));

    let product_formation = writer.add_entity(format!(
        "PRODUCT_DEFINITION_FORMATION_WITH_SPECIFIED_SOURCE('1','',{},.NOT_KNOWN.)",
        Part21Writer::reference(product)
    ));

    writer.add_entity(format!(
        "PRODUCT_RELATED_PRODUCT_CATEGORY('part','',({}))",
        Part21Writer::reference(product)
    ));

    let product_definition_context = writer.add_entity(format!(
        "PRODUCT_DEFINITION_CONTEXT('part definition',{},'design')",
        Part21Writer::reference(app_context)
    ));

    let product_definition = writer.add_entity(format!(
        "PRODUCT_DEFINITION('','',{}, {})",
        Part21Writer::reference(product_formation),
        Part21Writer::reference(product_definition_context)
    ));

    let length_unit = writer.add_entity("(LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT($,.METRE.))");
    let angle_unit = writer.add_entity("(NAMED_UNIT(*) PLANE_ANGLE_UNIT() SI_UNIT($,.RADIAN.))");
    let solid_angle_unit =
        writer.add_entity("(NAMED_UNIT(*) SOLID_ANGLE_UNIT() SI_UNIT($,.STERADIAN.))");

    let uncertainty = writer.add_entity(format!(
        "UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.E-6),{},'distance_accuracy_value','confusion accuracy')",
        Part21Writer::reference(length_unit)
    ));

    let geometric_context = writer.add_entity(format!(
        "( GEOMETRIC_REPRESENTATION_CONTEXT(3) GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT(({})) GLOBAL_UNIT_ASSIGNED_CONTEXT(({}, {}, {})) REPRESENTATION_CONTEXT('','') )",
        Part21Writer::reference(uncertainty),
        Part21Writer::reference(length_unit),
        Part21Writer::reference(angle_unit),
        Part21Writer::reference(solid_angle_unit)
    ));

    let shape_representation = writer.add_entity(format!(
        "ADVANCED_BREP_SHAPE_REPRESENTATION('',({}),{})",
        join_refs(&solid_ids),
        Part21Writer::reference(geometric_context)
    ));

    let product_shape = writer.add_entity(format!(
        "PRODUCT_DEFINITION_SHAPE('','',{})",
        Part21Writer::reference(product_definition)
    ));

    writer.add_entity(format!(
        "SHAPE_DEFINITION_REPRESENTATION({}, {})",
        Part21Writer::reference(product_shape),
        Part21Writer::reference(shape_representation)
    ));

    let text = writer.build().map_err(StepExportError::Serialization)?;
    Ok((text, report))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn export_brep_to_step_file(
    brep: &Brep,
    file_path: &str,
    config: &StepExportConfig,
) -> Result<StepExportReport, StepExportError> {
    let (text, report) = export_brep_to_step_text(brep, config)?;
    std::fs::write(file_path, text).map_err(|err| StepExportError::Io(err.to_string()))?;
    Ok(report)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn export_breps_to_step_file<'a, I>(
    breps: I,
    file_path: &str,
    config: &StepExportConfig,
) -> Result<StepExportReport, StepExportError>
where
    I: IntoIterator<Item = &'a Brep>,
{
    let (text, report) = export_breps_to_step_text(breps, config)?;
    std::fs::write(file_path, text).map_err(|err| StepExportError::Io(err.to_string()))?;
    Ok(report)
}

fn validate_config(config: &StepExportConfig) -> Result<f64, StepExportError> {
    if !config.scale.is_finite() || config.scale <= 0.0 {
        return Err(StepExportError::MeshGeneration(
            "STEP scale must be a finite positive value".to_string(),
        ));
    }
    Ok(config.scale)
}

fn is_closed_solid(brep: &Brep) -> bool {
    if brep.faces.is_empty() || brep.edges.is_empty() {
        return false;
    }

    if !brep.shells.is_empty() && brep.shells.iter().all(|shell| !shell.is_closed) {
        return false;
    }

    brep.edges.iter().all(|edge| edge.twin_halfedge.is_some())
}

fn triangulate_brep_faces(
    brep: &Brep,
    scale: f64,
    policy: StepErrorPolicy,
    report: &mut StepExportReport,
    label: String,
) -> Result<Vec<[Vector3; 3]>, StepExportError> {
    let mut triangles = Vec::new();

    for face in &brep.faces {
        report.input_faces += 1;
        let (outer_vertices, holes_vertices) = brep.get_vertices_and_holes_by_face_id(face.id);

        if outer_vertices.len() < 3 {
            if policy == StepErrorPolicy::Strict {
                return Err(StepExportError::MeshGeneration(format!(
                    "{} face {} has fewer than 3 vertices",
                    label, face.id
                )));
            }
            report.skipped_faces += 1;
            continue;
        }

        if holes_vertices.iter().any(|hole| hole.len() < 3) {
            if policy == StepErrorPolicy::Strict {
                return Err(StepExportError::MeshGeneration(format!(
                    "{} face {} has an invalid hole loop",
                    label, face.id
                )));
            }
            report.skipped_faces += 1;
            continue;
        }

        let triangle_indices = triangulate_polygon_with_holes(&outer_vertices, &holes_vertices);
        if triangle_indices.is_empty() {
            if policy == StepErrorPolicy::Strict {
                return Err(StepExportError::MeshGeneration(format!(
                    "{} face {} produced no triangles",
                    label, face.id
                )));
            }
            report.skipped_faces += 1;
            continue;
        }

        let mut all_vertices = outer_vertices;
        for hole in holes_vertices {
            all_vertices.extend(hole);
        }

        let mut face_has_triangle = false;

        for triangle in triangle_indices {
            let Some((&a, &b, &c)) = all_vertices
                .get(triangle[0])
                .zip(all_vertices.get(triangle[1]))
                .zip(all_vertices.get(triangle[2]))
                .map(|((a, b), c)| (a, b, c))
            else {
                if policy == StepErrorPolicy::Strict {
                    return Err(StepExportError::MeshGeneration(format!(
                        "{} face {} emitted out-of-range triangle indices",
                        label, face.id
                    )));
                }
                continue;
            };

            if !is_finite_vec3(a) || !is_finite_vec3(b) || !is_finite_vec3(c) {
                if policy == StepErrorPolicy::Strict {
                    return Err(StepExportError::MeshGeneration(format!(
                        "{} face {} has non-finite coordinates",
                        label, face.id
                    )));
                }
                continue;
            }

            let scaled = [
                Vector3::new(a.x * scale, a.y * scale, a.z * scale),
                Vector3::new(b.x * scale, b.y * scale, b.z * scale),
                Vector3::new(c.x * scale, c.y * scale, c.z * scale),
            ];

            if is_degenerate_triangle(scaled[0], scaled[1], scaled[2]) {
                if policy == StepErrorPolicy::Strict {
                    return Err(StepExportError::MeshGeneration(format!(
                        "{} face {} contains degenerate triangle",
                        label, face.id
                    )));
                }
                continue;
            }

            triangles.push(scaled);
            face_has_triangle = true;
        }

        if !face_has_triangle {
            if policy == StepErrorPolicy::Strict {
                return Err(StepExportError::MeshGeneration(format!(
                    "{} face {} yielded no valid triangles",
                    label, face.id
                )));
            }
            report.skipped_faces += 1;
        }
    }

    Ok(triangles)
}

fn get_or_create_point(
    writer: &mut Part21Writer,
    point_map: &mut HashMap<String, usize>,
    point: Vector3,
) -> usize {
    let key = format!("{:.9}|{:.9}|{:.9}", point.x, point.y, point.z);
    if let Some(existing) = point_map.get(&key) {
        return *existing;
    }

    let id = writer.add_entity(format!(
        "CARTESIAN_POINT('',({},{},{}))",
        format_real(point.x),
        format_real(point.y),
        format_real(point.z)
    ));

    point_map.insert(key, id);
    id
}

fn join_refs(ids: &[usize]) -> String {
    ids.iter()
        .map(|id| Part21Writer::reference(*id))
        .collect::<Vec<_>>()
        .join(",")
}

fn format_real(value: f64) -> String {
    let mut out = format!("{:.9}", value);
    while out.contains('.') && out.ends_with('0') {
        out.pop();
    }
    if out.ends_with('.') {
        out.push('0');
    }
    out
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
    !area_sq.is_finite() || area_sq <= STEP_LENGTH_EPSILON
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brep::BrepBuilder;
    use uuid::Uuid;

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
    fn exports_step_part21_document() {
        let brep = tetrahedron_brep();
        let (text, report) =
            export_brep_to_step_text(&brep, &StepExportConfig::default()).expect("step export");

        assert!(text.starts_with("ISO-10303-21;"));
        assert!(text.contains("FILE_SCHEMA(('AUTOMOTIVE_DESIGN'));"));
        assert!(text.contains("MANIFOLD_SOLID_BREP"));
        assert!(text.contains("ADVANCED_BREP_SHAPE_REPRESENTATION"));
        assert!(report.exported_solids >= 1);
        assert!(report.exported_triangles >= 4);
    }

    #[test]
    fn best_effort_skips_non_solid_brep() {
        let solid = tetrahedron_brep();

        let mut wire_builder = BrepBuilder::new(Uuid::new_v4());
        wire_builder.add_vertices(&[Vector3::new(0.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0)]);
        wire_builder.add_wire(&[0, 1], false).unwrap();
        let wire = wire_builder.build().unwrap();

        let (text, report) =
            export_breps_to_step_text([&solid, &wire], &StepExportConfig::default())
                .expect("best effort should succeed");

        assert!(text.contains("MANIFOLD_SOLID_BREP"));
        assert_eq!(report.exported_solids, 1);
        assert!(report.skipped_entities >= 1);
    }

    #[test]
    fn strict_fails_on_non_solid_brep() {
        let mut wire_builder = BrepBuilder::new(Uuid::new_v4());
        wire_builder.add_vertices(&[Vector3::new(0.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0)]);
        wire_builder.add_wire(&[0, 1], false).unwrap();
        let wire = wire_builder.build().unwrap();

        let config = StepExportConfig {
            error_policy: StepErrorPolicy::Strict,
            ..StepExportConfig::default()
        };

        let result = export_brep_to_step_text(&wire, &config);
        assert!(result.is_err());
    }
}
