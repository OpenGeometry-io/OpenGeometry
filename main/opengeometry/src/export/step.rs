use std::collections::{HashMap, HashSet};
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
    /// D9: when true, faces carrying analytic surface geometry (plane,
    /// cylinder) are exported as `ADVANCED_FACE`s on exact `PLANE` /
    /// `CYLINDRICAL_SURFACE` with `LINE` / `CIRCLE` edge curves, instead of a
    /// planar facet fan. Faces without analytic geometry still facet.
    #[serde(default = "default_true")]
    pub analytic_surfaces: bool,
    /// Length unit emitted in the STEP unit context (D8).
    #[serde(default)]
    pub length_unit: crate::units::LengthUnit,
}

fn default_true() -> bool {
    true
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
            analytic_surfaces: true,
            length_unit: crate::units::LengthUnit::default(),
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

        let mut point_map: HashMap<String, usize> = HashMap::new();
        let mut emitter = AnalyticEmitter::default();
        let mut face_ids = Vec::new();

        // D9 topological reduction: merge faces sharing one cylindrical surface
        // into a single ADVANCED_FACE bounded by its ring edges, so a cylinder
        // exports as one analytic face rather than a fan of tagged facets.
        let mut consumed: HashSet<u32> = HashSet::new();
        if config.analytic_surfaces {
            let merged = emit_merged_cylinder_faces(
                &mut writer,
                &mut point_map,
                &mut emitter,
                brep,
                scale,
                &mut consumed,
            );
            for face_ref in merged {
                face_ids.push(face_ref);
                report.exported_faces += 1;
            }
        }

        for face in &brep.faces {
            if consumed.contains(&face.id) {
                continue;
            }
            report.input_faces += 1;
            if config.analytic_surfaces && face.surface.is_some() {
                match emit_analytic_face(
                    &mut writer,
                    &mut point_map,
                    &mut emitter,
                    brep,
                    face,
                    scale,
                ) {
                    Some(face_ref) => {
                        face_ids.push(face_ref);
                        report.exported_faces += 1;
                        continue;
                    }
                    None => { /* fall through to faceting */ }
                }
            }

            let triangles = triangulate_single_face(
                brep,
                face,
                scale,
                config.error_policy,
                &mut report,
                format!("BREP index {} face {}", brep_index, face.id),
            )?;
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
                let face_ref = writer.add_entity(format!(
                    "FACE('',({}))",
                    Part21Writer::reference(outer_bound)
                ));
                face_ids.push(face_ref);
            }
            report.exported_triangles += triangles.len();
            report.exported_faces += triangles.len();
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

    let length_unit = emit_length_unit(&mut writer, config.length_unit);
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

fn triangulate_single_face(
    brep: &Brep,
    face: &crate::brep::Face,
    scale: f64,
    policy: StepErrorPolicy,
    report: &mut StepExportReport,
    label: String,
) -> Result<Vec<[Vector3; 3]>, StepExportError> {
    let mut triangles = Vec::new();
    let (outer_vertices, holes_vertices) = brep.get_vertices_and_holes_by_face_id(face.id);

    if outer_vertices.len() < 3 {
        if policy == StepErrorPolicy::Strict {
            return Err(StepExportError::MeshGeneration(format!(
                "{} has fewer than 3 vertices",
                label
            )));
        }
        report.skipped_faces += 1;
        return Ok(triangles);
    }

    if holes_vertices.iter().any(|hole| hole.len() < 3) {
        if policy == StepErrorPolicy::Strict {
            return Err(StepExportError::MeshGeneration(format!(
                "{} has an invalid hole loop",
                label
            )));
        }
        report.skipped_faces += 1;
        return Ok(triangles);
    }

    let triangle_indices = triangulate_polygon_with_holes(&outer_vertices, &holes_vertices);
    if triangle_indices.is_empty() {
        if policy == StepErrorPolicy::Strict {
            return Err(StepExportError::MeshGeneration(format!(
                "{} produced no triangles",
                label
            )));
        }
        report.skipped_faces += 1;
        return Ok(triangles);
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

        let scaled = [
            Vector3::new(a.x * scale, a.y * scale, a.z * scale),
            Vector3::new(b.x * scale, b.y * scale, b.z * scale),
            Vector3::new(c.x * scale, c.y * scale, c.z * scale),
        ];

        if is_degenerate_triangle(scaled[0], scaled[1], scaled[2]) {
            continue;
        }

        triangles.push(scaled);
    }

    Ok(triangles)
}

/// Caches reusable analytic entities (directions, surfaces, edge curves) so a
/// cylinder's lateral facets all reference a single `CYLINDRICAL_SURFACE` and
/// shared circle edges aren't re-emitted.
#[derive(Default)]
struct AnalyticEmitter {
    surfaces: HashMap<String, usize>,
    edge_curves: HashMap<u32, usize>,
    vertex_points: HashMap<u32, usize>,
}

/// Emits one face on its analytic surface as a STEP `ADVANCED_FACE`, with its
/// boundary edges expressed as exact `LINE` / `CIRCLE` `EDGE_CURVE`s. Returns
/// `None` (so the caller facets instead) if the boundary can't be traversed.
fn emit_analytic_face(
    writer: &mut Part21Writer,
    point_map: &mut HashMap<String, usize>,
    emitter: &mut AnalyticEmitter,
    brep: &Brep,
    face: &crate::brep::Face,
    scale: f64,
) -> Option<usize> {
    let surface = face.surface.as_ref()?;
    let surface_ref = emit_surface(writer, point_map, emitter, surface, scale);

    let outer = emit_edge_loop_bound(
        writer,
        point_map,
        emitter,
        brep,
        face.outer_loop,
        scale,
        true,
    )?;
    let mut bounds = vec![outer];
    for inner in &face.inner_loops {
        if let Some(b) =
            emit_edge_loop_bound(writer, point_map, emitter, brep, *inner, scale, false)
        {
            bounds.push(b);
        }
    }

    Some(writer.add_entity(format!(
        "ADVANCED_FACE('',({}),{},.T.)",
        join_refs(&bounds),
        Part21Writer::reference(surface_ref)
    )))
}

/// Merges every group of faces that share one cylindrical surface into a single
/// `ADVANCED_FACE`. The merged face's boundary is the set of edges used by only
/// one face of the group (the top/bottom circle rings); interior facet edges are
/// dropped. Records the consumed face ids so the caller skips them. Returns the
/// emitted `ADVANCED_FACE` references.
fn emit_merged_cylinder_faces(
    writer: &mut Part21Writer,
    point_map: &mut HashMap<String, usize>,
    emitter: &mut AnalyticEmitter,
    brep: &Brep,
    scale: f64,
    consumed: &mut HashSet<u32>,
) -> Vec<usize> {
    use crate::brep::SurfaceGeometry;

    // Group face ids by cylindrical-surface key.
    let mut groups: HashMap<String, Vec<u32>> = HashMap::new();
    for face in &brep.faces {
        if let Some(surface @ SurfaceGeometry::Cylinder { .. }) = &face.surface {
            groups
                .entry(surface_key(surface, scale))
                .or_default()
                .push(face.id);
        }
    }

    let mut emitted = Vec::new();
    for (_key, group) in groups {
        if group.len() < 2 {
            continue; // a lone cylindrical face is handled by the per-face path
        }

        // Boundary edges: those used by exactly one face in the group.
        let mut edge_use: HashMap<u32, usize> = HashMap::new();
        for &face_id in &group {
            for edge_id in face_boundary_edges(brep, face_id) {
                *edge_use.entry(edge_id).or_insert(0) += 1;
            }
        }
        let boundary: Vec<u32> = edge_use
            .iter()
            .filter(|(_, &count)| count == 1)
            .map(|(&edge_id, _)| edge_id)
            .collect();
        if boundary.len() < 3 {
            continue;
        }

        let loops = assemble_edge_loops(brep, &boundary);
        if loops.is_empty() {
            continue;
        }

        let surface = brep
            .faces
            .iter()
            .find(|f| f.id == group[0])
            .and_then(|f| f.surface.clone());
        let Some(surface) = surface else { continue };
        let surface_ref = emit_surface(writer, point_map, emitter, &surface, scale);

        let mut bounds = Vec::new();
        for (i, edge_loop) in loops.iter().enumerate() {
            let mut oriented = Vec::with_capacity(edge_loop.len());
            for &edge_id in edge_loop {
                if let Some(edge_curve) =
                    emit_edge_curve(writer, point_map, emitter, brep, edge_id, scale)
                {
                    oriented.push(writer.add_entity(format!(
                        "ORIENTED_EDGE('',*,*,{},.T.)",
                        Part21Writer::reference(edge_curve)
                    )));
                }
            }
            if oriented.is_empty() {
                continue;
            }
            let loop_ref = writer.add_entity(format!("EDGE_LOOP('',({}))", join_refs(&oriented)));
            let kind = if i == 0 {
                "FACE_OUTER_BOUND"
            } else {
                "FACE_BOUND"
            };
            bounds.push(writer.add_entity(format!(
                "{}('',{},.T.)",
                kind,
                Part21Writer::reference(loop_ref)
            )));
        }

        if bounds.is_empty() {
            continue;
        }

        emitted.push(writer.add_entity(format!(
            "ADVANCED_FACE('',({}),{},.T.)",
            join_refs(&bounds),
            Part21Writer::reference(surface_ref)
        )));
        for face_id in group {
            consumed.insert(face_id);
        }
    }

    emitted
}

/// All edge ids on a face's outer and inner loops.
fn face_boundary_edges(brep: &Brep, face_id: u32) -> Vec<u32> {
    let Some(face) = brep.faces.iter().find(|f| f.id == face_id) else {
        return Vec::new();
    };
    let mut edges = Vec::new();
    let mut loops = vec![face.outer_loop];
    loops.extend(face.inner_loops.iter().copied());
    for loop_id in loops {
        if let Ok(halfedges) = brep.get_loop_halfedges(loop_id) {
            for he_id in halfedges {
                if let Some(he) = brep.halfedges.get(he_id as usize) {
                    edges.push(he.edge);
                }
            }
        }
    }
    edges
}

/// Assembles a set of boundary edges into ordered closed loops by walking shared
/// vertices. Each returned loop is an ordered list of edge ids.
fn assemble_edge_loops(brep: &Brep, edges: &[u32]) -> Vec<Vec<u32>> {
    let mut endpoints: HashMap<u32, (u32, u32)> = HashMap::new();
    let mut vertex_edges: HashMap<u32, Vec<u32>> = HashMap::new();
    for &edge_id in edges {
        if let Some((a, b)) = brep.get_edge_endpoints(edge_id) {
            endpoints.insert(edge_id, (a, b));
            vertex_edges.entry(a).or_default().push(edge_id);
            vertex_edges.entry(b).or_default().push(edge_id);
        }
    }

    let mut remaining: HashSet<u32> = edges.iter().copied().collect();
    let mut loops = Vec::new();

    while let Some(&start_edge) = remaining.iter().next() {
        remaining.remove(&start_edge);
        let (start_v, mut cur_v) = endpoints[&start_edge];
        let mut loop_edges = vec![start_edge];

        while cur_v != start_v {
            let next = vertex_edges
                .get(&cur_v)
                .and_then(|candidates| candidates.iter().find(|e| remaining.contains(e)).copied());
            let Some(next_edge) = next else { break };
            remaining.remove(&next_edge);
            let (a, b) = endpoints[&next_edge];
            cur_v = if a == cur_v { b } else { a };
            loop_edges.push(next_edge);
        }

        if loop_edges.len() >= 3 {
            loops.push(loop_edges);
        }
    }

    // Largest loop first so it becomes the FACE_OUTER_BOUND.
    loops.sort_by(|a, b| b.len().cmp(&a.len()));
    loops
}

fn emit_edge_loop_bound(
    writer: &mut Part21Writer,
    point_map: &mut HashMap<String, usize>,
    emitter: &mut AnalyticEmitter,
    brep: &Brep,
    loop_id: u32,
    scale: f64,
    is_outer: bool,
) -> Option<usize> {
    let halfedges = brep.get_loop_halfedges(loop_id).ok()?;
    if halfedges.len() < 3 {
        return None;
    }
    let mut oriented = Vec::with_capacity(halfedges.len());
    for he_id in halfedges {
        let he = brep.halfedges.get(he_id as usize)?;
        let edge_curve = emit_edge_curve(writer, point_map, emitter, brep, he.edge, scale)?;
        oriented.push(writer.add_entity(format!(
            "ORIENTED_EDGE('',*,*,{},.T.)",
            Part21Writer::reference(edge_curve)
        )));
    }
    let edge_loop = writer.add_entity(format!("EDGE_LOOP('',({}))", join_refs(&oriented)));
    let kind = if is_outer {
        "FACE_OUTER_BOUND"
    } else {
        "FACE_BOUND"
    };
    Some(writer.add_entity(format!(
        "{}('',{},.T.)",
        kind,
        Part21Writer::reference(edge_loop)
    )))
}

fn emit_edge_curve(
    writer: &mut Part21Writer,
    point_map: &mut HashMap<String, usize>,
    emitter: &mut AnalyticEmitter,
    brep: &Brep,
    edge_id: u32,
    scale: f64,
) -> Option<usize> {
    if let Some(existing) = emitter.edge_curves.get(&edge_id) {
        return Some(*existing);
    }
    let (from_id, to_id) = brep.get_edge_endpoints(edge_id)?;
    let from_pos = scaled(brep.vertices.get(from_id as usize)?.position, scale);
    let to_pos = scaled(brep.vertices.get(to_id as usize)?.position, scale);
    let v_from = emit_vertex_point(writer, point_map, emitter, from_id, from_pos);
    let v_to = emit_vertex_point(writer, point_map, emitter, to_id, to_pos);

    let edge = brep.edges.iter().find(|e| e.id == edge_id);
    let curve_ref = match edge.and_then(|e| e.curve.as_ref()) {
        Some(crate::brep::CurveGeometry::Circle {
            center,
            normal,
            x_axis,
            radius,
            ..
        }) => {
            let placement =
                emit_axis_placement(writer, point_map, scaled(*center, scale), *normal, *x_axis);
            writer.add_entity(format!(
                "CIRCLE('',{},{})",
                Part21Writer::reference(placement),
                format_real(radius * scale)
            ))
        }
        _ => {
            let dir = direction(from_pos, to_pos);
            let d = emit_direction(writer, dir);
            let vector = writer.add_entity(format!(
                "VECTOR('',{},{})",
                Part21Writer::reference(d),
                format_real(distance(from_pos, to_pos).max(1.0))
            ));
            let line_point = get_or_create_point(writer, point_map, from_pos);
            writer.add_entity(format!(
                "LINE('',{},{})",
                Part21Writer::reference(line_point),
                Part21Writer::reference(vector)
            ))
        }
    };

    let edge_curve = writer.add_entity(format!(
        "EDGE_CURVE('',{},{},{},.T.)",
        Part21Writer::reference(v_from),
        Part21Writer::reference(v_to),
        Part21Writer::reference(curve_ref)
    ));
    emitter.edge_curves.insert(edge_id, edge_curve);
    Some(edge_curve)
}

fn emit_surface(
    writer: &mut Part21Writer,
    point_map: &mut HashMap<String, usize>,
    emitter: &mut AnalyticEmitter,
    surface: &crate::brep::SurfaceGeometry,
    scale: f64,
) -> usize {
    use crate::brep::SurfaceGeometry;
    let key = surface_key(surface, scale);
    if let Some(existing) = emitter.surfaces.get(&key) {
        return *existing;
    }
    let id = match surface {
        SurfaceGeometry::Plane { origin, normal } => {
            let placement = emit_axis_placement(
                writer,
                point_map,
                scaled(*origin, scale),
                *normal,
                any_perpendicular(*normal),
            );
            writer.add_entity(format!("PLANE('',{})", Part21Writer::reference(placement)))
        }
        SurfaceGeometry::Cylinder {
            origin,
            axis,
            ref_direction,
            radius,
            ..
        } => {
            let placement = emit_axis_placement(
                writer,
                point_map,
                scaled(*origin, scale),
                *axis,
                *ref_direction,
            );
            writer.add_entity(format!(
                "CYLINDRICAL_SURFACE('',{},{})",
                Part21Writer::reference(placement),
                format_real(radius * scale)
            ))
        }
    };
    emitter.surfaces.insert(key, id);
    id
}

fn emit_axis_placement(
    writer: &mut Part21Writer,
    point_map: &mut HashMap<String, usize>,
    location: Vector3,
    axis: Vector3,
    ref_direction: Vector3,
) -> usize {
    let point = get_or_create_point(writer, point_map, location);
    let axis_dir = emit_direction(writer, normalize(axis));
    let ref_dir = emit_direction(writer, normalize(ref_direction));
    writer.add_entity(format!(
        "AXIS2_PLACEMENT_3D('',{},{},{})",
        Part21Writer::reference(point),
        Part21Writer::reference(axis_dir),
        Part21Writer::reference(ref_dir)
    ))
}

fn emit_direction(writer: &mut Part21Writer, dir: Vector3) -> usize {
    writer.add_entity(format!(
        "DIRECTION('',({},{},{}))",
        format_real(dir.x),
        format_real(dir.y),
        format_real(dir.z)
    ))
}

fn emit_vertex_point(
    writer: &mut Part21Writer,
    point_map: &mut HashMap<String, usize>,
    emitter: &mut AnalyticEmitter,
    vertex_id: u32,
    position: Vector3,
) -> usize {
    if let Some(existing) = emitter.vertex_points.get(&vertex_id) {
        return *existing;
    }
    let point = get_or_create_point(writer, point_map, position);
    let vertex_point = writer.add_entity(format!(
        "VERTEX_POINT('',{})",
        Part21Writer::reference(point)
    ));
    emitter.vertex_points.insert(vertex_id, vertex_point);
    vertex_point
}

fn emit_length_unit(writer: &mut Part21Writer, unit: crate::units::LengthUnit) -> usize {
    match unit.step_si_prefix() {
        Some(prefix) => writer.add_entity(format!(
            "(LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT({},.METRE.))",
            prefix
        )),
        None => {
            // Non-SI (inch/foot): a conversion-based unit on top of the metre.
            let metre = writer.add_entity("(LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT($,.METRE.))");
            let measure = writer.add_entity(format!(
                "LENGTH_MEASURE_WITH_UNIT(LENGTH_MEASURE({}),{})",
                format_real(unit.metres_per_unit()),
                Part21Writer::reference(metre)
            ));
            let dimensions =
                writer.add_entity("DIMENSIONAL_EXPONENTS(1.,0.,0.,0.,0.,0.,0.)".to_string());
            writer.add_entity(format!(
                "( CONVERSION_BASED_UNIT('{}',{}) LENGTH_UNIT() NAMED_UNIT({}) )",
                unit.name(),
                Part21Writer::reference(measure),
                Part21Writer::reference(dimensions)
            ))
        }
    }
}

fn scaled(v: Vector3, scale: f64) -> Vector3 {
    Vector3::new(v.x * scale, v.y * scale, v.z * scale)
}

fn direction(from: Vector3, to: Vector3) -> Vector3 {
    normalize(Vector3::new(to.x - from.x, to.y - from.y, to.z - from.z))
}

fn distance(a: Vector3, b: Vector3) -> f64 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    let dz = a.z - b.z;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn normalize(v: Vector3) -> Vector3 {
    let len = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
    if len <= 1.0e-12 {
        Vector3::new(0.0, 0.0, 1.0)
    } else {
        Vector3::new(v.x / len, v.y / len, v.z / len)
    }
}

fn any_perpendicular(n: Vector3) -> Vector3 {
    let n = normalize(n);
    if n.x.abs() <= n.y.abs() && n.x.abs() <= n.z.abs() {
        normalize(Vector3::new(0.0, -n.z, n.y))
    } else if n.y.abs() <= n.z.abs() {
        normalize(Vector3::new(-n.z, 0.0, n.x))
    } else {
        normalize(Vector3::new(-n.y, n.x, 0.0))
    }
}

fn surface_key(surface: &crate::brep::SurfaceGeometry, scale: f64) -> String {
    use crate::brep::SurfaceGeometry;
    match surface {
        SurfaceGeometry::Plane { origin, normal } => format!(
            "P|{:.6}|{:.6}|{:.6}|{:.6}|{:.6}|{:.6}",
            origin.x * scale,
            origin.y * scale,
            origin.z * scale,
            normal.x,
            normal.y,
            normal.z
        ),
        SurfaceGeometry::Cylinder {
            origin,
            axis,
            radius,
            ..
        } => format!(
            "C|{:.6}|{:.6}|{:.6}|{:.6}|{:.6}|{:.6}|{:.6}",
            origin.x * scale,
            origin.y * scale,
            origin.z * scale,
            axis.x,
            axis.y,
            axis.z,
            radius * scale
        ),
    }
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
    fn cylinder_exports_analytic_cylindrical_surface() {
        // Acceptance criterion A1: circle profile → solid → STEP contains a
        // single CYLINDRICAL_SURFACE and circular edge curves, not a facet fan.
        use crate::primitives::cylinder::OGCylinder;

        let mut cylinder = OGCylinder::new("a1-cyl".to_string());
        cylinder
            .set_config(
                Vector3::new(0.0, 0.0, 0.0),
                1.0,
                2.0,
                2.0 * std::f64::consts::PI,
                32,
            )
            .unwrap();
        let brep = cylinder.world_brep();

        let (text, report) =
            export_brep_to_step_text(&brep, &StepExportConfig::default()).expect("step export");

        assert_eq!(
            text.matches("CYLINDRICAL_SURFACE").count(),
            1,
            "exactly one analytic cylindrical surface"
        );
        assert!(text.contains("CIRCLE("), "circular edge curves present");
        assert!(text.contains("ADVANCED_FACE"), "analytic faces emitted");
        assert!(text.contains("PLANE("), "planar caps as analytic planes");
        // Topological reduction (D9): the 32 lateral facets collapse into a
        // single cylindrical ADVANCED_FACE, so the whole solid is ~3 faces
        // (1 cylinder + 2 caps), not 34.
        let advanced_faces = text.matches("ADVANCED_FACE").count();
        assert!(
            advanced_faces <= 4,
            "lateral facets should merge into one cylinder face, got {} advanced faces",
            advanced_faces
        );
        assert!(report.exported_solids >= 1);
    }

    #[test]
    fn legacy_config_json_without_new_fields_still_deserializes() {
        // The SDK's exportSceneToStep sends JSON predating D8/D9 fields; serde
        // defaults must keep it working (analytic on, mm).
        let legacy = r#"{
            "schema":"AutomotiveDesign","product_name":"x","scale":1.0,
            "error_policy":"BestEffort","validate_topology":true,"require_closed_shell":true
        }"#;
        let config: StepExportConfig = serde_json::from_str(legacy).expect("legacy json");
        assert!(config.analytic_surfaces);
        assert_eq!(config.length_unit, crate::units::LengthUnit::Millimetre);
    }

    #[test]
    fn unit_context_reflects_configured_length_unit() {
        use crate::units::LengthUnit;
        let brep = tetrahedron_brep();

        let mm = StepExportConfig {
            length_unit: LengthUnit::Millimetre,
            ..StepExportConfig::default()
        };
        let (mm_text, _) = export_brep_to_step_text(&brep, &mm).unwrap();
        assert!(mm_text.contains("SI_UNIT(.MILLI.,.METRE.)"));

        let cm = StepExportConfig {
            length_unit: LengthUnit::Centimetre,
            ..StepExportConfig::default()
        };
        let (cm_text, _) = export_brep_to_step_text(&brep, &cm).unwrap();
        assert!(cm_text.contains("SI_UNIT(.CENTI.,.METRE.)"));
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
