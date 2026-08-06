use std::collections::HashMap;
use std::fmt;

use openmaths::Vector3;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::brep::Brep;
use crate::operations::triangulate::triangulate_polygon_with_holes;

use super::part21::{sanitize_string_literal, Part21Writer};

const IFC_LENGTH_EPSILON: f64 = 1.0e-12;
const IFC_CLASS_PROXY: &str = "IFCBUILDINGELEMENTPROXY";
const IFC_CLASS_SPACE: &str = "IFCSPACE";
const IFC_CLASS_SITE: &str = "IFCSITE";
const IFC_ALLOWED_CLASSES: [&str; 14] = [
    IFC_CLASS_PROXY,
    "IFCWALL",
    "IFCSLAB",
    "IFCCOLUMN",
    "IFCBEAM",
    "IFCMEMBER",
    "IFCDOOR",
    "IFCWINDOW",
    "IFCROOF",
    "IFCSTAIR",
    "IFCRAILING",
    "IFCFOOTING",
    IFC_CLASS_SPACE,
    IFC_CLASS_SITE,
];

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum IfcErrorPolicy {
    Strict,
    BestEffort,
}

impl Default for IfcErrorPolicy {
    fn default() -> Self {
        Self::BestEffort
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum IfcSchemaVersion {
    Ifc4Add2,
}

impl Default for IfcSchemaVersion {
    fn default() -> Self {
        Self::Ifc4Add2
    }
}

impl IfcSchemaVersion {
    fn as_file_schema(self) -> &'static str {
        match self {
            Self::Ifc4Add2 => "IFC4",
        }
    }
}

/// A typed IfcPropertySingleValue payload. `#[serde(untagged)]` keeps the
/// config JSON natural (`true` / `4500` / `"office"`) and remains backward
/// compatible with the previous string-only property sets.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum IfcPropertyValue {
    Bool(bool),
    Number(f64),
    Text(String),
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct IfcEntitySemantics {
    pub ifc_class: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub object_type: Option<String>,
    pub tag: Option<String>,
    #[serde(default)]
    pub property_sets: HashMap<String, HashMap<String, IfcPropertyValue>>,
    #[serde(default)]
    pub quantity_sets: HashMap<String, HashMap<String, f64>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IfcExportConfig {
    pub schema: IfcSchemaVersion,
    pub project_name: Option<String>,
    pub site_name: Option<String>,
    pub building_name: Option<String>,
    pub storey_name: Option<String>,
    pub scale: f64,
    pub error_policy: IfcErrorPolicy,
    pub validate_topology: bool,
    pub require_closed_shell: bool,
    pub semantics: Option<HashMap<String, IfcEntitySemantics>>,
    /// Length unit emitted in the IFC unit assignment (D8).
    #[serde(default)]
    pub length_unit: crate::units::LengthUnit,
    /// D9: emit analytic `IFCADVANCEDBREP` (IFCPLANE / IFCCYLINDRICALSURFACE with
    /// IFCLINE / IFCCIRCLE edges) for entities whose faces carry analytic
    /// surfaces, instead of an `IFCTRIANGULATEDFACESET`. Falls back to
    /// tessellation when a brep has no analytic geometry.
    #[serde(default = "default_true_ifc")]
    pub analytic_surfaces: bool,
    /// Convert source Y-up geometry (Three.js convention) to IFC's Z-up
    /// world by mapping `(x, y, z) -> (x, -z, y)` once, before any
    /// emission. Default on, since kernel BReps are authored Y-up.
    #[serde(default = "default_true_ifc")]
    pub up_axis_conversion: bool,
}

fn default_true_ifc() -> bool {
    true
}

impl Default for IfcExportConfig {
    fn default() -> Self {
        Self {
            schema: IfcSchemaVersion::default(),
            project_name: Some("OpenGeometry Project".to_string()),
            site_name: Some("OpenGeometry Site".to_string()),
            building_name: Some("OpenGeometry Building".to_string()),
            storey_name: Some("OpenGeometry Storey".to_string()),
            scale: 1.0,
            error_policy: IfcErrorPolicy::BestEffort,
            validate_topology: true,
            require_closed_shell: true,
            semantics: None,
            length_unit: crate::units::LengthUnit::default(),
            analytic_surfaces: true,
            up_axis_conversion: true,
        }
    }
}

/// Map a single source Y-up coordinate/direction to IFC Z-up:
/// `(x, y, z) -> (x, -z, y)`. A proper rotation (+90° about X,
/// determinant +1) so handedness is preserved and nothing mirrors.
fn to_z_up(v: Vector3) -> Vector3 {
    Vector3::new(v.x, -v.z, v.y)
}

/// Return a Z-up copy of `brep`, transforming vertices, analytic edge
/// curves and face surfaces consistently, then recomputing normals.
/// Applied exactly once at export entry so the rest of the pipeline is
/// untouched and double-application is impossible.
fn brep_to_z_up(brep: &Brep) -> Brep {
    let mut converted = brep.clone();
    for vertex in &mut converted.vertices {
        vertex.position = to_z_up(vertex.position);
    }
    for edge in &mut converted.edges {
        if let Some(curve) = &edge.curve {
            // scale = 1.0: the conversion carries no scale (radii unchanged).
            edge.curve = Some(curve.transformed_with(&to_z_up, 1.0));
        }
    }
    for face in &mut converted.faces {
        if let Some(surface) = &face.surface {
            face.surface = Some(surface.transformed_with(&to_z_up, 1.0));
        }
    }
    if !converted.faces.is_empty() {
        converted.recompute_face_normals();
    }
    converted
}

/// IFC SI prefix token (e.g. `.MILLI.`) for the length unit, or `$` for the
/// base metre. Non-SI units fall back to metre (IFC conversion-based units are
/// a follow-on).
fn ifc_length_unit_entity(unit: crate::units::LengthUnit) -> String {
    let prefix = unit.step_si_prefix().unwrap_or("$");
    format!("IFCSIUNIT(*,.LENGTHUNIT.,{},.METRE.)", prefix)
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct IfcExportReport {
    pub input_breps: usize,
    pub input_faces: usize,
    pub exported_elements: usize,
    pub exported_faces: usize,
    pub exported_triangles: usize,
    pub skipped_entities: usize,
    pub skipped_faces: usize,
    pub topology_errors: usize,
    pub semantics_applied: usize,
    pub proxy_fallbacks: usize,
    pub property_sets_written: usize,
    pub quantity_sets_written: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum IfcExportError {
    EmptyInput,
    InvalidTopology(String),
    UnsupportedEntity(String),
    InvalidSemantics(String),
    MeshGeneration(String),
    Serialization(String),
    Io(String),
}

impl fmt::Display for IfcExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IfcExportError::EmptyInput => write!(f, "No BREP input provided for IFC export"),
            IfcExportError::InvalidTopology(msg) => write!(f, "Invalid topology: {}", msg),
            IfcExportError::UnsupportedEntity(msg) => write!(f, "Unsupported BREP: {}", msg),
            IfcExportError::InvalidSemantics(msg) => write!(f, "Invalid IFC semantics: {}", msg),
            IfcExportError::MeshGeneration(msg) => write!(f, "Mesh generation failed: {}", msg),
            IfcExportError::Serialization(msg) => write!(f, "IFC serialization failed: {}", msg),
            IfcExportError::Io(msg) => write!(f, "IFC I/O failed: {}", msg),
        }
    }
}

impl std::error::Error for IfcExportError {}

#[derive(Clone, Copy)]
pub struct IfcEntityInput<'a> {
    pub entity_id: &'a str,
    pub kind: &'a str,
    pub brep: &'a Brep,
}

#[derive(Clone)]
struct IfcOwnedEntity<'a> {
    entity_id: String,
    kind: String,
    brep: &'a Brep,
}

#[derive(Clone)]
struct TessellatedMesh {
    points: Vec<Vector3>,
    faces: Vec<[usize; 3]>,
}

pub fn export_brep_to_ifc_text(
    brep: &Brep,
    config: &IfcExportConfig,
) -> Result<(String, IfcExportReport), IfcExportError> {
    let owned = vec![IfcOwnedEntity {
        entity_id: "brep-0".to_string(),
        kind: "BREP".to_string(),
        brep,
    }];
    export_owned_entities_to_ifc_text(&owned, config)
}

pub fn export_breps_to_ifc_text<'a, I>(
    breps: I,
    config: &IfcExportConfig,
) -> Result<(String, IfcExportReport), IfcExportError>
where
    I: IntoIterator<Item = &'a Brep>,
{
    let mut owned = Vec::new();
    for (index, brep) in breps.into_iter().enumerate() {
        owned.push(IfcOwnedEntity {
            entity_id: format!("brep-{}", index),
            kind: "BREP".to_string(),
            brep,
        });
    }

    export_owned_entities_to_ifc_text(&owned, config)
}

pub fn export_scene_entities_to_ifc_text<'a, I>(
    entities: I,
    config: &IfcExportConfig,
) -> Result<(String, IfcExportReport), IfcExportError>
where
    I: IntoIterator<Item = IfcEntityInput<'a>>,
{
    let owned: Vec<IfcOwnedEntity<'a>> = entities
        .into_iter()
        .map(|entity| IfcOwnedEntity {
            entity_id: entity.entity_id.to_string(),
            kind: entity.kind.to_string(),
            brep: entity.brep,
        })
        .collect();

    export_owned_entities_to_ifc_text(&owned, config)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn export_brep_to_ifc_file(
    brep: &Brep,
    file_path: &str,
    config: &IfcExportConfig,
) -> Result<IfcExportReport, IfcExportError> {
    let (text, report) = export_brep_to_ifc_text(brep, config)?;
    std::fs::write(file_path, text).map_err(|err| IfcExportError::Io(err.to_string()))?;
    Ok(report)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn export_breps_to_ifc_file<'a, I>(
    breps: I,
    file_path: &str,
    config: &IfcExportConfig,
) -> Result<IfcExportReport, IfcExportError>
where
    I: IntoIterator<Item = &'a Brep>,
{
    let (text, report) = export_breps_to_ifc_text(breps, config)?;
    std::fs::write(file_path, text).map_err(|err| IfcExportError::Io(err.to_string()))?;
    Ok(report)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn export_scene_entities_to_ifc_file<'a, I>(
    entities: I,
    file_path: &str,
    config: &IfcExportConfig,
) -> Result<IfcExportReport, IfcExportError>
where
    I: IntoIterator<Item = IfcEntityInput<'a>>,
{
    let (text, report) = export_scene_entities_to_ifc_text(entities, config)?;
    std::fs::write(file_path, text).map_err(|err| IfcExportError::Io(err.to_string()))?;
    Ok(report)
}

fn export_owned_entities_to_ifc_text<'a>(
    entities: &[IfcOwnedEntity<'a>],
    config: &IfcExportConfig,
) -> Result<(String, IfcExportReport), IfcExportError> {
    let scale = validate_config(config)?;

    if entities.is_empty() {
        return Err(IfcExportError::EmptyInput);
    }

    // Y-up -> Z-up once, up front: convert each BREP and re-bind the
    // working entities to the converted geometry. Everything downstream
    // (analytic surfaces, tessellation, placements) then operates on
    // Z-up data unchanged.
    let converted_breps: Vec<Brep>;
    let converted_entities: Vec<IfcOwnedEntity<'_>>;
    let entities: &[IfcOwnedEntity<'_>] = if config.up_axis_conversion {
        converted_breps = entities.iter().map(|e| brep_to_z_up(e.brep)).collect();
        converted_entities = entities
            .iter()
            .zip(converted_breps.iter())
            .map(|(e, brep)| IfcOwnedEntity {
                entity_id: e.entity_id.clone(),
                kind: e.kind.clone(),
                brep,
            })
            .collect();
        &converted_entities
    } else {
        entities
    };

    let mut report = IfcExportReport {
        input_breps: entities.len(),
        ..IfcExportReport::default()
    };

    let project_name = config
        .project_name
        .clone()
        .unwrap_or_else(|| "OpenGeometry Project".to_string());

    let mut writer = Part21Writer::new(config.schema.as_file_schema());
    writer.set_description("ViewDefinition [CoordinationView]");
    writer.set_file_name(project_name.clone());

    let origin = writer.add_entity("IFCCARTESIANPOINT((0.,0.,0.))");
    let axis_z = writer.add_entity("IFCDIRECTION((0.,0.,1.))");
    let axis_x = writer.add_entity("IFCDIRECTION((1.,0.,0.))");
    let world_axis = writer.add_entity(format!(
        "IFCAXIS2PLACEMENT3D({},{},{})",
        Part21Writer::reference(origin),
        Part21Writer::reference(axis_z),
        Part21Writer::reference(axis_x)
    ));

    let geom_context = writer.add_entity(format!(
        "IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.E-5,{},$)",
        Part21Writer::reference(world_axis)
    ));

    let length_unit = writer.add_entity(ifc_length_unit_entity(config.length_unit));
    let area_unit = writer.add_entity("IFCSIUNIT(*,.AREAUNIT.,$,.SQUARE_METRE.)");
    let volume_unit = writer.add_entity("IFCSIUNIT(*,.VOLUMEUNIT.,$,.CUBIC_METRE.)");
    let angle_unit = writer.add_entity("IFCSIUNIT(*,.PLANEANGLEUNIT.,$,.RADIAN.)");
    let unit_assignment = writer.add_entity(format!(
        "IFCUNITASSIGNMENT(({}, {}, {}, {}))",
        Part21Writer::reference(length_unit),
        Part21Writer::reference(area_unit),
        Part21Writer::reference(volume_unit),
        Part21Writer::reference(angle_unit)
    ));

    let project = writer.add_entity(format!(
        "IFCPROJECT('{}',$,'{}',$,$,$,$,({}),{})",
        ifc_guid("project"),
        sanitize_string_literal(&project_name),
        Part21Writer::reference(geom_context),
        Part21Writer::reference(unit_assignment)
    ));

    let site_axis = writer.add_entity(format!(
        "IFCLOCALPLACEMENT($,{})",
        Part21Writer::reference(world_axis)
    ));
    let building_axis = writer.add_entity(format!(
        "IFCLOCALPLACEMENT({},{})",
        Part21Writer::reference(site_axis),
        Part21Writer::reference(world_axis)
    ));
    let storey_axis = writer.add_entity(format!(
        "IFCLOCALPLACEMENT({},{})",
        Part21Writer::reference(building_axis),
        Part21Writer::reference(world_axis)
    ));

    let site = writer.add_entity(format!(
        "IFCSITE('{}',$,'{}',$,$,{},$,$,.ELEMENT.,$,$,$,$,$)",
        ifc_guid("site"),
        sanitize_string_literal(
            &config
                .site_name
                .clone()
                .unwrap_or_else(|| "OpenGeometry Site".to_string())
        ),
        Part21Writer::reference(site_axis)
    ));

    let building = writer.add_entity(format!(
        "IFCBUILDING('{}',$,'{}',$,$,{},$,$,.ELEMENT.,$,$,$)",
        ifc_guid("building"),
        sanitize_string_literal(
            &config
                .building_name
                .clone()
                .unwrap_or_else(|| "OpenGeometry Building".to_string())
        ),
        Part21Writer::reference(building_axis)
    ));

    let storey = writer.add_entity(format!(
        "IFCBUILDINGSTOREY('{}',$,'{}',$,$,{},$,$,.ELEMENT.,0.)",
        ifc_guid("storey"),
        sanitize_string_literal(
            &config
                .storey_name
                .clone()
                .unwrap_or_else(|| "OpenGeometry Storey".to_string())
        ),
        Part21Writer::reference(storey_axis)
    ));

    writer.add_entity(format!(
        "IFCRELAGGREGATES('{}',$,$,$,{},({}))",
        ifc_guid("rel-project-site"),
        Part21Writer::reference(project),
        Part21Writer::reference(site)
    ));

    writer.add_entity(format!(
        "IFCRELAGGREGATES('{}',$,$,$,{},({}))",
        ifc_guid("rel-site-building"),
        Part21Writer::reference(site),
        Part21Writer::reference(building)
    ));

    writer.add_entity(format!(
        "IFCRELAGGREGATES('{}',$,$,$,{},({}))",
        ifc_guid("rel-building-storey"),
        Part21Writer::reference(building),
        Part21Writer::reference(storey)
    ));

    let mut element_ids = Vec::new();
    let mut space_ids = Vec::new();
    let mut site_ids = Vec::new();

    for entity in entities {
        let brep = entity.brep;

        if config.validate_topology {
            if let Err(error) = brep.validate_topology() {
                if config.error_policy == IfcErrorPolicy::Strict {
                    return Err(IfcExportError::InvalidTopology(format!(
                        "Entity '{}' failed topology validation: {}",
                        entity.entity_id, error
                    )));
                }
                report.topology_errors += 1;
                report.skipped_entities += 1;
                continue;
            }
        }

        if config.require_closed_shell && !is_closed_solid(brep) {
            if config.error_policy == IfcErrorPolicy::Strict {
                return Err(IfcExportError::UnsupportedEntity(format!(
                    "Entity '{}' is not a closed-shell solid",
                    entity.entity_id
                )));
            }
            report.skipped_entities += 1;
            continue;
        }

        let mesh = triangulate_entity_mesh(
            entity,
            scale,
            config.error_policy,
            &mut report,
            format!("entity '{}'", entity.entity_id),
        )?;

        if mesh.faces.is_empty() || mesh.points.is_empty() {
            if config.error_policy == IfcErrorPolicy::Strict {
                return Err(IfcExportError::MeshGeneration(format!(
                    "Entity '{}' generated no exportable mesh",
                    entity.entity_id
                )));
            }
            report.skipped_entities += 1;
            continue;
        }

        let semantics = config
            .semantics
            .as_ref()
            .and_then(|map| map.get(&entity.entity_id));

        let class_name = resolve_ifc_class(&entity.entity_id, semantics, config, &mut report)?;

        // D9: prefer an analytic IFCADVANCEDBREP when the brep carries analytic
        // surfaces; otherwise fall back to the tessellated face set.
        let analytic_rep =
            if config.analytic_surfaces && brep.faces.iter().any(|f| f.surface.is_some()) {
                emit_ifc_advanced_brep(&mut writer, brep, scale, geom_context)
            } else {
                None
            };

        let shape_representation = if let Some((rep, face_count)) = analytic_rep {
            report.exported_faces += face_count;
            rep
        } else {
            let mesh_point_list = writer.add_entity(format!(
                "IFCCARTESIANPOINTLIST3D({})",
                format_ifc_coord_list(&mesh.points)
            ));

            let mesh_faceset = writer.add_entity(format!(
                "IFCTRIANGULATEDFACESET({},$,.T.,{},$)",
                Part21Writer::reference(mesh_point_list),
                format_ifc_face_index_list(&mesh.faces)
            ));

            report.exported_triangles += mesh.faces.len();
            report.exported_faces += mesh.faces.len();

            writer.add_entity(format!(
                "IFCSHAPEREPRESENTATION({},'Body','Tessellation',({}))",
                Part21Writer::reference(geom_context),
                Part21Writer::reference(mesh_faceset)
            ))
        };

        let definition_shape = writer.add_entity(format!(
            "IFCPRODUCTDEFINITIONSHAPE($,$,({}))",
            Part21Writer::reference(shape_representation)
        ));

        let placement = writer.add_entity(format!(
            "IFCLOCALPLACEMENT({},{})",
            Part21Writer::reference(storey_axis),
            Part21Writer::reference(world_axis)
        ));

        let default_name = format!("{}-{}", entity.kind, entity.entity_id);
        let name = semantics
            .and_then(|sem| sem.name.clone())
            .unwrap_or(default_name);
        let description = semantics
            .and_then(|sem| sem.description.clone())
            .unwrap_or_default();
        let object_type = semantics
            .and_then(|sem| sem.object_type.clone())
            .unwrap_or_else(|| entity.kind.clone());
        let tag = semantics
            .and_then(|sem| sem.tag.clone())
            .unwrap_or_else(|| entity.entity_id.clone());

        let guid = ifc_guid(&format!("element-{}", entity.entity_id));
        let name_literal = sanitize_string_literal(&name);
        let description_expr = if description.is_empty() {
            "$".to_string()
        } else {
            format!("'{}'", sanitize_string_literal(&description))
        };
        let object_type_literal = sanitize_string_literal(&object_type);
        let placement_ref = Part21Writer::reference(placement);
        let shape_ref = Part21Writer::reference(definition_shape);
        let tag_literal = sanitize_string_literal(&tag);

        // IfcSpace and IfcSite are spatial elements with their own attribute
        // signatures (LongName + CompositionType instead of Tag); everything
        // else uses the shared building-element signature.
        let element_expr = match class_name {
            IFC_CLASS_SPACE => format!(
                "IFCSPACE('{}',$,'{}',{},'{}',{},{},'{}',.ELEMENT.,.NOTDEFINED.,$)",
                guid,
                name_literal,
                description_expr,
                object_type_literal,
                placement_ref,
                shape_ref,
                tag_literal
            ),
            IFC_CLASS_SITE => format!(
                "IFCSITE('{}',$,'{}',{},'{}',{},{},'{}',.ELEMENT.,$,$,$,$,$)",
                guid,
                name_literal,
                description_expr,
                object_type_literal,
                placement_ref,
                shape_ref,
                tag_literal
            ),
            _ => format!(
                "{}('{}',$,'{}',{},'{}',{},{},'{}',.NOTDEFINED.)",
                class_name,
                guid,
                name_literal,
                description_expr,
                object_type_literal,
                placement_ref,
                shape_ref,
                tag_literal
            ),
        };

        let element_id = writer.add_entity(element_expr);
        // Spatial elements may not ride IfcRelContainedInSpatialStructure:
        // spaces AGGREGATE under the storey, extra sites under the project.
        match class_name {
            IFC_CLASS_SPACE => space_ids.push(element_id),
            IFC_CLASS_SITE => site_ids.push(element_id),
            _ => element_ids.push(element_id),
        }

        if let Some(semantics) = semantics {
            write_property_sets(
                &mut writer,
                element_id,
                &entity.entity_id,
                semantics,
                &mut report,
            );
            write_quantity_sets(
                &mut writer,
                element_id,
                &entity.entity_id,
                semantics,
                &mut report,
            );
        }

        report.exported_elements += 1;
    }

    if element_ids.is_empty() && space_ids.is_empty() && site_ids.is_empty() {
        return Err(IfcExportError::MeshGeneration(
            "No elements were exported from the provided BREP inputs".to_string(),
        ));
    }

    if !element_ids.is_empty() {
        writer.add_entity(format!(
            "IFCRELCONTAINEDINSPATIALSTRUCTURE('{}',$,'ContainedInStorey',$,({}),{})",
            ifc_guid("rel-contained-storey"),
            join_refs(&element_ids),
            Part21Writer::reference(storey)
        ));
    }
    if !space_ids.is_empty() {
        writer.add_entity(format!(
            "IFCRELAGGREGATES('{}',$,$,$,{},({}))",
            ifc_guid("rel-storey-spaces"),
            Part21Writer::reference(storey),
            join_refs(&space_ids)
        ));
    }
    if !site_ids.is_empty() {
        writer.add_entity(format!(
            "IFCRELAGGREGATES('{}',$,$,$,{},({}))",
            ifc_guid("rel-project-element-sites"),
            Part21Writer::reference(project),
            join_refs(&site_ids)
        ));
    }

    let text = writer.build().map_err(IfcExportError::Serialization)?;
    Ok((text, report))
}

/// D9: emits an analytic `IFCADVANCEDBREP` shape representation for a brep whose
/// faces carry analytic surfaces (D1) — IFCPLANE / IFCCYLINDRICALSURFACE faces
/// with IFCLINE / IFCCIRCLE edge curves. Returns the IFCSHAPEREPRESENTATION ref
/// and the analytic face count, or `None` if no advanced face could be built.
fn emit_ifc_advanced_brep(
    writer: &mut Part21Writer,
    brep: &Brep,
    scale: f64,
    geom_context: usize,
) -> Option<(usize, usize)> {
    use crate::brep::SurfaceGeometry;

    let mut edge_curves: HashMap<u32, usize> = HashMap::new();
    let mut vertex_points: HashMap<u32, usize> = HashMap::new();
    let mut surfaces: HashMap<String, usize> = HashMap::new();
    let mut face_ids = Vec::new();

    let scaled = |v: Vector3| Vector3::new(v.x * scale, v.y * scale, v.z * scale);

    for face in &brep.faces {
        let Some(surface) = face.surface.as_ref() else {
            return None; // mixed analytic/non-analytic — fall back to tessellation
        };

        let surface_ref = {
            let key = ifc_surface_key(surface, scale);
            if let Some(existing) = surfaces.get(&key) {
                *existing
            } else {
                let id = match surface {
                    SurfaceGeometry::Plane { origin, normal } => {
                        let placement = ifc_axis_placement(
                            writer,
                            scaled(*origin),
                            *normal,
                            ifc_any_perpendicular(*normal),
                        );
                        writer
                            .add_entity(format!("IFCPLANE({})", Part21Writer::reference(placement)))
                    }
                    SurfaceGeometry::Cylinder {
                        origin,
                        axis,
                        ref_direction,
                        radius,
                        ..
                    } => {
                        let placement =
                            ifc_axis_placement(writer, scaled(*origin), *axis, *ref_direction);
                        writer.add_entity(format!(
                            "IFCCYLINDRICALSURFACE({},{})",
                            Part21Writer::reference(placement),
                            format_ifc_real(radius * scale)
                        ))
                    }
                };
                surfaces.insert(key, id);
                id
            }
        };

        let mut bounds = Vec::new();
        let outer = ifc_edge_loop_bound(
            writer,
            brep,
            face.outer_loop,
            scale,
            true,
            &mut edge_curves,
            &mut vertex_points,
        )?;
        bounds.push(outer);
        for inner in &face.inner_loops {
            if let Some(b) = ifc_edge_loop_bound(
                writer,
                brep,
                *inner,
                scale,
                false,
                &mut edge_curves,
                &mut vertex_points,
            ) {
                bounds.push(b);
            }
        }

        face_ids.push(writer.add_entity(format!(
            "IFCADVANCEDFACE({},{},.T.)",
            format_ifc_ref_list(&bounds),
            Part21Writer::reference(surface_ref)
        )));
    }

    if face_ids.is_empty() {
        return None;
    }

    let shell = writer.add_entity(format!(
        "IFCCLOSEDSHELL({})",
        format_ifc_ref_list(&face_ids)
    ));
    let advanced_brep = writer.add_entity(format!(
        "IFCADVANCEDBREP({})",
        Part21Writer::reference(shell)
    ));
    let rep = writer.add_entity(format!(
        "IFCSHAPEREPRESENTATION({},'Body','AdvancedBrep',({}))",
        Part21Writer::reference(geom_context),
        Part21Writer::reference(advanced_brep)
    ));
    Some((rep, face_ids.len()))
}

fn ifc_edge_loop_bound(
    writer: &mut Part21Writer,
    brep: &Brep,
    loop_id: u32,
    scale: f64,
    is_outer: bool,
    edge_curves: &mut HashMap<u32, usize>,
    vertex_points: &mut HashMap<u32, usize>,
) -> Option<usize> {
    let halfedges = brep.get_loop_halfedges(loop_id).ok()?;
    if halfedges.len() < 3 {
        return None;
    }
    let mut oriented = Vec::with_capacity(halfedges.len());
    for he_id in halfedges {
        let he = brep.halfedges.get(he_id as usize)?;
        let edge_curve = ifc_edge_curve(writer, brep, he.edge, scale, edge_curves, vertex_points)?;
        oriented.push(writer.add_entity(format!(
            "IFCORIENTEDEDGE(*,*,{},.T.)",
            Part21Writer::reference(edge_curve)
        )));
    }
    let edge_loop = writer.add_entity(format!("IFCEDGELOOP({})", format_ifc_ref_list(&oriented)));
    let kind = if is_outer {
        "IFCFACEOUTERBOUND"
    } else {
        "IFCFACEBOUND"
    };
    Some(writer.add_entity(format!(
        "{}({},.T.)",
        kind,
        Part21Writer::reference(edge_loop)
    )))
}

fn ifc_edge_curve(
    writer: &mut Part21Writer,
    brep: &Brep,
    edge_id: u32,
    scale: f64,
    edge_curves: &mut HashMap<u32, usize>,
    vertex_points: &mut HashMap<u32, usize>,
) -> Option<usize> {
    use crate::brep::CurveGeometry;
    if let Some(existing) = edge_curves.get(&edge_id) {
        return Some(*existing);
    }
    let (from_id, to_id) = brep.get_edge_endpoints(edge_id)?;
    let from_pos = scaled_v(brep.vertices.get(from_id as usize)?.position, scale);
    let to_pos = scaled_v(brep.vertices.get(to_id as usize)?.position, scale);
    let v_from = ifc_vertex_point(writer, from_id, from_pos, vertex_points);
    let v_to = ifc_vertex_point(writer, to_id, to_pos, vertex_points);

    let edge = brep.edges.iter().find(|e| e.id == edge_id);
    let curve_ref = match edge.and_then(|e| e.curve.as_ref()) {
        Some(CurveGeometry::Circle {
            center,
            normal,
            x_axis,
            radius,
            ..
        }) => {
            let placement = ifc_axis_placement(writer, scaled_v(*center, scale), *normal, *x_axis);
            writer.add_entity(format!(
                "IFCCIRCLE({},{})",
                Part21Writer::reference(placement),
                format_ifc_real(radius * scale)
            ))
        }
        _ => {
            let dir = ifc_direction_between(from_pos, to_pos);
            let d = writer.add_entity(format!(
                "IFCDIRECTION(({},{},{}))",
                format_ifc_real(dir.x),
                format_ifc_real(dir.y),
                format_ifc_real(dir.z)
            ));
            let vector = writer.add_entity(format!(
                "IFCVECTOR({},{})",
                Part21Writer::reference(d),
                format_ifc_real(ifc_distance(from_pos, to_pos).max(1.0))
            ));
            let point = writer.add_entity(format!(
                "IFCCARTESIANPOINT(({},{},{}))",
                format_ifc_real(from_pos.x),
                format_ifc_real(from_pos.y),
                format_ifc_real(from_pos.z)
            ));
            writer.add_entity(format!(
                "IFCLINE({},{})",
                Part21Writer::reference(point),
                Part21Writer::reference(vector)
            ))
        }
    };

    let edge_curve = writer.add_entity(format!(
        "IFCEDGECURVE({},{},{},.T.)",
        Part21Writer::reference(v_from),
        Part21Writer::reference(v_to),
        Part21Writer::reference(curve_ref)
    ));
    edge_curves.insert(edge_id, edge_curve);
    Some(edge_curve)
}

fn ifc_vertex_point(
    writer: &mut Part21Writer,
    vertex_id: u32,
    position: Vector3,
    cache: &mut HashMap<u32, usize>,
) -> usize {
    if let Some(existing) = cache.get(&vertex_id) {
        return *existing;
    }
    let point = writer.add_entity(format!(
        "IFCCARTESIANPOINT(({},{},{}))",
        format_ifc_real(position.x),
        format_ifc_real(position.y),
        format_ifc_real(position.z)
    ));
    let vp = writer.add_entity(format!(
        "IFCVERTEXPOINT({})",
        Part21Writer::reference(point)
    ));
    cache.insert(vertex_id, vp);
    vp
}

fn ifc_axis_placement(
    writer: &mut Part21Writer,
    location: Vector3,
    axis: Vector3,
    ref_direction: Vector3,
) -> usize {
    let point = writer.add_entity(format!(
        "IFCCARTESIANPOINT(({},{},{}))",
        format_ifc_real(location.x),
        format_ifc_real(location.y),
        format_ifc_real(location.z)
    ));
    let axis = ifc_normalize(axis);
    let refd = ifc_normalize(ref_direction);
    let axis_dir = writer.add_entity(format!(
        "IFCDIRECTION(({},{},{}))",
        format_ifc_real(axis.x),
        format_ifc_real(axis.y),
        format_ifc_real(axis.z)
    ));
    let ref_dir = writer.add_entity(format!(
        "IFCDIRECTION(({},{},{}))",
        format_ifc_real(refd.x),
        format_ifc_real(refd.y),
        format_ifc_real(refd.z)
    ));
    writer.add_entity(format!(
        "IFCAXIS2PLACEMENT3D({},{},{})",
        Part21Writer::reference(point),
        Part21Writer::reference(axis_dir),
        Part21Writer::reference(ref_dir)
    ))
}

fn scaled_v(v: Vector3, scale: f64) -> Vector3 {
    Vector3::new(v.x * scale, v.y * scale, v.z * scale)
}

fn ifc_direction_between(from: Vector3, to: Vector3) -> Vector3 {
    ifc_normalize(Vector3::new(to.x - from.x, to.y - from.y, to.z - from.z))
}

fn ifc_distance(a: Vector3, b: Vector3) -> f64 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    let dz = a.z - b.z;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn ifc_normalize(v: Vector3) -> Vector3 {
    let len = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
    if len <= IFC_LENGTH_EPSILON {
        Vector3::new(0.0, 0.0, 1.0)
    } else {
        Vector3::new(v.x / len, v.y / len, v.z / len)
    }
}

fn ifc_any_perpendicular(n: Vector3) -> Vector3 {
    let n = ifc_normalize(n);
    if n.x.abs() <= n.y.abs() && n.x.abs() <= n.z.abs() {
        ifc_normalize(Vector3::new(0.0, -n.z, n.y))
    } else if n.y.abs() <= n.z.abs() {
        ifc_normalize(Vector3::new(-n.z, 0.0, n.x))
    } else {
        ifc_normalize(Vector3::new(-n.y, n.x, 0.0))
    }
}

fn ifc_surface_key(surface: &crate::brep::SurfaceGeometry, scale: f64) -> String {
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

fn format_ifc_ref_list(ids: &[usize]) -> String {
    format!(
        "({})",
        ids.iter()
            .map(|id| Part21Writer::reference(*id))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn format_ifc_real(value: f64) -> String {
    let mut out = format!("{:.9}", value);
    while out.contains('.') && out.ends_with('0') {
        out.pop();
    }
    if out.ends_with('.') {
        out.push('0');
    }
    out
}

fn validate_config(config: &IfcExportConfig) -> Result<f64, IfcExportError> {
    if !config.scale.is_finite() || config.scale <= 0.0 {
        return Err(IfcExportError::MeshGeneration(
            "IFC scale must be a finite positive value".to_string(),
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

fn triangulate_entity_mesh(
    entity: &IfcOwnedEntity<'_>,
    scale: f64,
    policy: IfcErrorPolicy,
    report: &mut IfcExportReport,
    label: String,
) -> Result<TessellatedMesh, IfcExportError> {
    let mut points = Vec::<Vector3>::new();
    let mut point_map = HashMap::<String, usize>::new();
    let mut faces = Vec::<[usize; 3]>::new();

    for face in &entity.brep.faces {
        report.input_faces += 1;

        let (outer_vertices, holes_vertices) =
            entity.brep.get_vertices_and_holes_by_face_id(face.id);

        if outer_vertices.len() < 3 {
            if policy == IfcErrorPolicy::Strict {
                return Err(IfcExportError::MeshGeneration(format!(
                    "{} face {} has fewer than 3 vertices",
                    label, face.id
                )));
            }
            report.skipped_faces += 1;
            continue;
        }

        if holes_vertices.iter().any(|hole| hole.len() < 3) {
            if policy == IfcErrorPolicy::Strict {
                return Err(IfcExportError::MeshGeneration(format!(
                    "{} face {} has invalid hole loops",
                    label, face.id
                )));
            }
            report.skipped_faces += 1;
            continue;
        }

        let triangle_indices = triangulate_polygon_with_holes(&outer_vertices, &holes_vertices);
        if triangle_indices.is_empty() {
            if policy == IfcErrorPolicy::Strict {
                return Err(IfcExportError::MeshGeneration(format!(
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
                if policy == IfcErrorPolicy::Strict {
                    return Err(IfcExportError::MeshGeneration(format!(
                        "{} face {} emitted out-of-range triangle indices",
                        label, face.id
                    )));
                }
                continue;
            };

            if !is_finite_vec3(a) || !is_finite_vec3(b) || !is_finite_vec3(c) {
                if policy == IfcErrorPolicy::Strict {
                    return Err(IfcExportError::MeshGeneration(format!(
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
                if policy == IfcErrorPolicy::Strict {
                    return Err(IfcExportError::MeshGeneration(format!(
                        "{} face {} contains degenerate triangle",
                        label, face.id
                    )));
                }
                continue;
            }

            let i0 = get_or_create_mesh_point(&mut points, &mut point_map, scaled[0]);
            let i1 = get_or_create_mesh_point(&mut points, &mut point_map, scaled[1]);
            let i2 = get_or_create_mesh_point(&mut points, &mut point_map, scaled[2]);

            faces.push([i0 + 1, i1 + 1, i2 + 1]);
            face_has_triangle = true;
        }

        if !face_has_triangle {
            if policy == IfcErrorPolicy::Strict {
                return Err(IfcExportError::MeshGeneration(format!(
                    "{} face {} yielded no valid triangles",
                    label, face.id
                )));
            }
            report.skipped_faces += 1;
        }
    }

    Ok(TessellatedMesh { points, faces })
}

fn get_or_create_mesh_point(
    points: &mut Vec<Vector3>,
    point_map: &mut HashMap<String, usize>,
    point: Vector3,
) -> usize {
    let key = format!("{:.9}|{:.9}|{:.9}", point.x, point.y, point.z);
    if let Some(index) = point_map.get(&key) {
        return *index;
    }

    let index = points.len();
    points.push(point);
    point_map.insert(key, index);
    index
}

fn resolve_ifc_class(
    entity_id: &str,
    semantics: Option<&IfcEntitySemantics>,
    config: &IfcExportConfig,
    report: &mut IfcExportReport,
) -> Result<&'static str, IfcExportError> {
    let Some(semantics) = semantics else {
        return Ok(IFC_CLASS_PROXY);
    };

    let Some(raw_class) = semantics.ifc_class.as_ref() else {
        return Ok(IFC_CLASS_PROXY);
    };

    let normalized = raw_class.trim().to_ascii_uppercase();
    if let Some(class_name) = IFC_ALLOWED_CLASSES
        .iter()
        .find(|candidate| **candidate == normalized)
        .copied()
    {
        report.semantics_applied += 1;
        return Ok(class_name);
    }

    if config.error_policy == IfcErrorPolicy::Strict {
        return Err(IfcExportError::InvalidSemantics(format!(
            "Entity '{}' requested unsupported ifc_class '{}'. Allowed classes: {}",
            entity_id,
            raw_class,
            IFC_ALLOWED_CLASSES.join(", ")
        )));
    }

    report.proxy_fallbacks += 1;
    Ok(IFC_CLASS_PROXY)
}

fn write_property_sets(
    writer: &mut Part21Writer,
    element_id: usize,
    entity_id: &str,
    semantics: &IfcEntitySemantics,
    report: &mut IfcExportReport,
) {
    // Sorted iteration: HashMap order is arbitrary and the SPF output must be
    // deterministic across runs.
    let mut set_names: Vec<&String> = semantics.property_sets.keys().collect();
    set_names.sort();
    for set_name in set_names {
        let properties = &semantics.property_sets[set_name];
        if properties.is_empty() {
            continue;
        }

        let mut property_names: Vec<&String> = properties.keys().collect();
        property_names.sort();
        let mut property_ids = Vec::new();
        for property_name in property_names {
            let property_value = &properties[property_name];
            let literal = match property_value {
                IfcPropertyValue::Bool(value) => {
                    format!("IFCBOOLEAN({})", if *value { ".T." } else { ".F." })
                }
                IfcPropertyValue::Number(value) => format!("IFCREAL({})", format_real(*value)),
                IfcPropertyValue::Text(value) => {
                    format!("IFCTEXT('{}')", sanitize_string_literal(value))
                }
            };
            let property = writer.add_entity(format!(
                "IFCPROPERTYSINGLEVALUE('{}',$,{},$)",
                sanitize_string_literal(property_name),
                literal
            ));
            property_ids.push(property);
        }

        let property_set = writer.add_entity(format!(
            "IFCPROPERTYSET('{}',$,'{}',$,({}))",
            ifc_guid(&format!("pset-{}-{}", entity_id, set_name)),
            sanitize_string_literal(set_name),
            join_refs(&property_ids)
        ));

        writer.add_entity(format!(
            "IFCRELDEFINESBYPROPERTIES('{}',$,$,$,({}),{})",
            ifc_guid(&format!("pset-rel-{}-{}", entity_id, set_name)),
            Part21Writer::reference(element_id),
            Part21Writer::reference(property_set)
        ));

        report.property_sets_written += 1;
    }
}

fn write_quantity_sets(
    writer: &mut Part21Writer,
    element_id: usize,
    entity_id: &str,
    semantics: &IfcEntitySemantics,
    report: &mut IfcExportReport,
) {
    let mut set_names: Vec<&String> = semantics.quantity_sets.keys().collect();
    set_names.sort();
    for set_name in set_names {
        let quantities = &semantics.quantity_sets[set_name];
        if quantities.is_empty() {
            continue;
        }

        let mut quantity_names: Vec<&String> = quantities.keys().collect();
        quantity_names.sort();
        let mut quantity_ids = Vec::new();
        for quantity_name in quantity_names {
            let quantity = writer.add_entity(format!(
                "IFCQUANTITYLENGTH('{}',$,$,{},$)",
                sanitize_string_literal(quantity_name),
                format_real(quantities[quantity_name])
            ));
            quantity_ids.push(quantity);
        }

        let quantity_set = writer.add_entity(format!(
            "IFCELEMENTQUANTITY('{}',$,'{}',$,$,({}))",
            ifc_guid(&format!("qset-{}-{}", entity_id, set_name)),
            sanitize_string_literal(set_name),
            join_refs(&quantity_ids)
        ));

        writer.add_entity(format!(
            "IFCRELDEFINESBYPROPERTIES('{}',$,$,$,({}),{})",
            ifc_guid(&format!("qset-rel-{}-{}", entity_id, set_name)),
            Part21Writer::reference(element_id),
            Part21Writer::reference(quantity_set)
        ));

        report.quantity_sets_written += 1;
    }
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

fn format_ifc_coord_list(points: &[Vector3]) -> String {
    let coords = points
        .iter()
        .map(|point| {
            format!(
                "({},{},{})",
                format_real(point.x),
                format_real(point.y),
                format_real(point.z)
            )
        })
        .collect::<Vec<_>>()
        .join(",");

    format!("({})", coords)
}

fn format_ifc_face_index_list(faces: &[[usize; 3]]) -> String {
    let entries = faces
        .iter()
        .map(|face| format!("({},{},{})", face[0], face[1], face[2]))
        .collect::<Vec<_>>()
        .join(",");

    format!("({})", entries)
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
    !area_sq.is_finite() || area_sq <= IFC_LENGTH_EPSILON
}

fn ifc_guid(seed: &str) -> String {
    const IFC_CHARS: &[u8; 64] =
        b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_$";

    let uuid = Uuid::new_v5(&Uuid::NAMESPACE_URL, seed.as_bytes());
    let mut number = u128::from_be_bytes(*uuid.as_bytes());
    let mut out = [b'0'; 22];

    for index in (0..22).rev() {
        out[index] = IFC_CHARS[(number & 63) as usize];
        number >>= 6;
    }

    String::from_utf8(out.to_vec()).unwrap_or_else(|_| "0000000000000000000000".to_string())
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
    fn exports_ifc_spf_document() {
        let brep = tetrahedron_brep();
        let (text, report) =
            export_brep_to_ifc_text(&brep, &IfcExportConfig::default()).expect("ifc export");

        assert!(text.starts_with("ISO-10303-21;"));
        assert!(text.contains("FILE_SCHEMA(('IFC4'));"));
        assert!(text.contains("IFCPROJECT("));
        assert!(text.contains("IFCTRIANGULATEDFACESET("));
        assert!(report.exported_elements >= 1);
        assert!(report.exported_triangles >= 4);
    }

    #[test]
    fn cylinder_exports_analytic_ifc_advanced_brep() {
        // D9: a cylinder (analytic surfaces present) exports as an
        // IFCADVANCEDBREP with one IFCCYLINDRICALSURFACE + circle edges, not a
        // triangulated face set.
        use crate::primitives::cylinder::OGCylinder;
        let mut cyl = OGCylinder::new("ifc-cyl".into());
        cyl.set_config(
            Vector3::new(0.0, 0.0, 0.0),
            1.0,
            2.0,
            2.0 * std::f64::consts::PI,
            24,
        )
        .unwrap();
        let brep = cyl.world_brep();

        let (text, _) =
            export_brep_to_ifc_text(&brep, &IfcExportConfig::default()).expect("ifc export");

        assert!(text.contains("IFCADVANCEDBREP("));
        assert_eq!(text.matches("IFCCYLINDRICALSURFACE(").count(), 1);
        assert!(text.contains("IFCCIRCLE("));
        assert!(text.contains("IFCPLANE("));
        assert!(!text.contains("IFCTRIANGULATEDFACESET("));
    }

    /// A tetrahedron whose apex is at source (1, 3, 2) so the Y-up/Z-up
    /// distinction is unambiguous (no zero components on the apex).
    fn oriented_tetrahedron() -> Brep {
        let mut builder = BrepBuilder::new(Uuid::new_v4());
        builder.add_vertices(&[
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(4.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 4.0),
            Vector3::new(1.0, 3.0, 2.0),
        ]);
        builder.add_face(&[0, 2, 1], &[]).unwrap();
        builder.add_face(&[0, 1, 3], &[]).unwrap();
        builder.add_face(&[1, 2, 3], &[]).unwrap();
        builder.add_face(&[2, 0, 3], &[]).unwrap();
        builder.build().unwrap()
    }

    #[test]
    fn up_axis_conversion_default_maps_y_to_z() {
        // Default config has up_axis_conversion = true: source apex
        // (1, 3, 2) -> IFC (1, -2, 3).
        let brep = oriented_tetrahedron();
        let (text, _) =
            export_brep_to_ifc_text(&brep, &IfcExportConfig::default()).expect("ifc export");
        assert!(
            text.contains("(1.0,-2.0,3.0)"),
            "apex should be Z-up (1,-2,3); got:\n{}",
            text
        );
        assert!(
            !text.contains("(1.0,3.0,2.0)"),
            "source Y-up apex must not appear when conversion is on"
        );
    }

    #[test]
    fn up_axis_conversion_off_preserves_source_coordinates() {
        // Flag off: geometry is emitted exactly as authored.
        let brep = oriented_tetrahedron();
        let config = IfcExportConfig {
            up_axis_conversion: false,
            ..IfcExportConfig::default()
        };
        let (text, _) = export_brep_to_ifc_text(&brep, &config).expect("ifc export");
        assert!(
            text.contains("(1.0,3.0,2.0)"),
            "apex should be unchanged (1,3,2) when conversion is off; got:\n{}",
            text
        );
        assert!(!text.contains("(1.0,-2.0,3.0)"));
    }

    #[test]
    fn up_axis_conversion_config_defaults_true() {
        assert!(IfcExportConfig::default().up_axis_conversion);
    }

    #[test]
    fn applies_semantics_class_when_supported() {
        let brep = tetrahedron_brep();

        let mut semantics = HashMap::new();
        semantics.insert(
            "brep-0".to_string(),
            IfcEntitySemantics {
                ifc_class: Some("IFCWALL".to_string()),
                name: Some("Wall A".to_string()),
                ..IfcEntitySemantics::default()
            },
        );

        let config = IfcExportConfig {
            semantics: Some(semantics),
            ..IfcExportConfig::default()
        };

        let (text, report) = export_brep_to_ifc_text(&brep, &config).expect("ifc export");
        assert!(text.contains("IFCWALL("));
        assert_eq!(report.semantics_applied, 1);
    }

    #[test]
    fn strict_rejects_invalid_ifc_class() {
        let brep = tetrahedron_brep();

        let mut semantics = HashMap::new();
        semantics.insert(
            "brep-0".to_string(),
            IfcEntitySemantics {
                ifc_class: Some("IFCUNKNOWN".to_string()),
                ..IfcEntitySemantics::default()
            },
        );

        let config = IfcExportConfig {
            semantics: Some(semantics),
            error_policy: IfcErrorPolicy::Strict,
            ..IfcExportConfig::default()
        };

        let result = export_brep_to_ifc_text(&brep, &config);
        assert!(result.is_err());
    }

    /// IFCSPACE exports as a real space — its own attribute signature,
    /// AGGREGATED under the storey rather than contained in it — instead of
    /// degrading to a building element proxy.
    #[test]
    fn exports_space_as_aggregated_spatial_element() {
        let brep = tetrahedron_brep();

        let mut semantics = HashMap::new();
        semantics.insert(
            "brep-0".to_string(),
            IfcEntitySemantics {
                ifc_class: Some("IfcSpace".to_string()),
                name: Some("Living Room".to_string()),
                object_type: Some("residential".to_string()),
                ..IfcEntitySemantics::default()
            },
        );

        let config = IfcExportConfig {
            semantics: Some(semantics),
            error_policy: IfcErrorPolicy::Strict,
            ..IfcExportConfig::default()
        };

        let (text, report) = export_brep_to_ifc_text(&brep, &config).expect("ifc export");
        assert!(text.contains("IFCSPACE("), "space class applied");
        assert!(
            !text.contains("IFCBUILDINGELEMENTPROXY("),
            "no proxy fallback"
        );
        assert!(
            !text.contains("IFCRELCONTAINEDINSPATIALSTRUCTURE("),
            "a lone space is not contained-in-storey"
        );
        assert!(
            text.contains("'residential'"),
            "object type (program/usage) survives"
        );
        // Storey → space aggregation exists (beyond the 3 scaffold aggregates).
        assert_eq!(text.matches("IFCRELAGGREGATES(").count(), 4);
        assert_eq!(report.semantics_applied, 1);
        assert_eq!(report.proxy_fallbacks, 0);
    }

    /// IFCSITE exports with the site attribute signature, aggregated under the
    /// project (a project may hold several sites).
    #[test]
    fn exports_site_aggregated_under_project() {
        let brep = tetrahedron_brep();

        let mut semantics = HashMap::new();
        semantics.insert(
            "brep-0".to_string(),
            IfcEntitySemantics {
                ifc_class: Some("IFCSITE".to_string()),
                name: Some("Parcel 12".to_string()),
                ..IfcEntitySemantics::default()
            },
        );

        let config = IfcExportConfig {
            semantics: Some(semantics),
            error_policy: IfcErrorPolicy::Strict,
            ..IfcExportConfig::default()
        };

        let (text, _) = export_brep_to_ifc_text(&brep, &config).expect("ifc export");
        // Scaffold site + the exported parcel site.
        assert_eq!(text.matches("IFCSITE(").count(), 2);
        assert!(text.contains("'Parcel 12'"));
        assert!(!text.contains("IFCRELCONTAINEDINSPATIALSTRUCTURE("));
    }

    /// Property sets carry TYPED values (IFCREAL / IFCBOOLEAN / IFCTEXT) and
    /// the writer's output is deterministic (sorted set / property names).
    #[test]
    fn writes_typed_property_sets() {
        let brep = tetrahedron_brep();

        let mut properties = HashMap::new();
        properties.insert("gfaM2".to_string(), IfcPropertyValue::Number(4500.25));
        properties.insert(
            "program".to_string(),
            IfcPropertyValue::Text("office".to_string()),
        );
        properties.insert("compliant".to_string(), IfcPropertyValue::Bool(true));
        let mut property_sets = HashMap::new();
        property_sets.insert("OpenPlans_Yield".to_string(), properties);

        let mut quantities = HashMap::new();
        quantities.insert("NetSideArea".to_string(), 3735.5);
        let mut quantity_sets = HashMap::new();
        quantity_sets.insert("Qto_SpaceBaseQuantities".to_string(), quantities);

        let mut semantics = HashMap::new();
        semantics.insert(
            "brep-0".to_string(),
            IfcEntitySemantics {
                ifc_class: Some("IFCSPACE".to_string()),
                property_sets,
                quantity_sets,
                ..IfcEntitySemantics::default()
            },
        );

        let config = IfcExportConfig {
            semantics: Some(semantics),
            error_policy: IfcErrorPolicy::Strict,
            ..IfcExportConfig::default()
        };

        let (text, report) = export_brep_to_ifc_text(&brep, &config).expect("ifc export");
        assert!(text.contains("IFCPROPERTYSINGLEVALUE('gfaM2',$,IFCREAL(4500.25),$)"));
        assert!(text.contains("IFCPROPERTYSINGLEVALUE('program',$,IFCTEXT('office'),$)"));
        assert!(text.contains("IFCPROPERTYSINGLEVALUE('compliant',$,IFCBOOLEAN(.T.),$)"));
        assert!(text.contains("IFCPROPERTYSET("));
        assert!(text.contains("IFCELEMENTQUANTITY("));
        assert!(text.contains("IFCRELDEFINESBYPROPERTIES("));
        assert_eq!(report.property_sets_written, 1);
        assert_eq!(report.quantity_sets_written, 1);

        let (again, _) = export_brep_to_ifc_text(
            &brep,
            &IfcExportConfig {
                semantics: config.semantics.clone(),
                error_policy: IfcErrorPolicy::Strict,
                ..IfcExportConfig::default()
            },
        )
        .expect("ifc export");
        assert_eq!(text, again, "SPF output is deterministic across runs");
    }

    /// Legacy string-only property-set JSON still parses (untagged enum).
    #[test]
    fn legacy_string_property_sets_still_parse() {
        let json = r#"{
            "ifc_class": "IFCWALL",
            "property_sets": { "Custom": { "material": "brick", "rating": "A" } }
        }"#;
        let semantics: IfcEntitySemantics =
            serde_json::from_str(json).expect("legacy semantics parse");
        let set = &semantics.property_sets["Custom"];
        assert_eq!(set["material"], IfcPropertyValue::Text("brick".to_string()));

        let typed = r#"{ "property_sets": { "Yield": { "gfa": 12.5, "ok": true } } }"#;
        let semantics: IfcEntitySemantics =
            serde_json::from_str(typed).expect("typed semantics parse");
        let set = &semantics.property_sets["Yield"];
        assert_eq!(set["gfa"], IfcPropertyValue::Number(12.5));
        assert_eq!(set["ok"], IfcPropertyValue::Bool(true));
    }
}
