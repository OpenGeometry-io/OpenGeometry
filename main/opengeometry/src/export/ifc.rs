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
const IFC_ALLOWED_CLASSES: [&str; 12] = [
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

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct IfcEntitySemantics {
    pub ifc_class: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub object_type: Option<String>,
    pub tag: Option<String>,
    #[serde(default)]
    pub property_sets: HashMap<String, HashMap<String, String>>,
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
        }
    }
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

    let length_unit = writer.add_entity("IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.)");
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

        let mesh_point_list = writer.add_entity(format!(
            "IFCCARTESIANPOINTLIST3D({})",
            format_ifc_coord_list(&mesh.points)
        ));

        let mesh_faceset = writer.add_entity(format!(
            "IFCTRIANGULATEDFACESET({},$,.T.,{},$)",
            Part21Writer::reference(mesh_point_list),
            format_ifc_face_index_list(&mesh.faces)
        ));

        let shape_representation = writer.add_entity(format!(
            "IFCSHAPEREPRESENTATION({},'Body','Tessellation',({}))",
            Part21Writer::reference(geom_context),
            Part21Writer::reference(mesh_faceset)
        ));

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

        let element_expr = format!(
            "{}('{}',$,'{}',{},'{}',{},{},'{}',.NOTDEFINED.)",
            class_name,
            ifc_guid(&format!("element-{}", entity.entity_id)),
            sanitize_string_literal(&name),
            if description.is_empty() {
                "$".to_string()
            } else {
                format!("'{}'", sanitize_string_literal(&description))
            },
            sanitize_string_literal(&object_type),
            Part21Writer::reference(placement),
            Part21Writer::reference(definition_shape),
            sanitize_string_literal(&tag)
        );

        let element_id = writer.add_entity(element_expr);
        element_ids.push(element_id);

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
        report.exported_triangles += mesh.faces.len();
        report.exported_faces += mesh.faces.len();
    }

    if element_ids.is_empty() {
        return Err(IfcExportError::MeshGeneration(
            "No elements were exported from the provided BREP inputs".to_string(),
        ));
    }

    writer.add_entity(format!(
        "IFCRELCONTAINEDINSPATIALSTRUCTURE('{}',$,'ContainedInStorey',$,({}),{})",
        ifc_guid("rel-contained-storey"),
        join_refs(&element_ids),
        Part21Writer::reference(storey)
    ));

    let text = writer.build().map_err(IfcExportError::Serialization)?;
    Ok((text, report))
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
    for (set_name, properties) in &semantics.property_sets {
        if properties.is_empty() {
            continue;
        }

        let mut property_ids = Vec::new();
        for (property_name, property_value) in properties {
            let property = writer.add_entity(format!(
                "IFCPROPERTYSINGLEVALUE('{}',$,IFCTEXT('{}'),$)",
                sanitize_string_literal(property_name),
                sanitize_string_literal(property_value)
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
    for (set_name, quantities) in &semantics.quantity_sets {
        if quantities.is_empty() {
            continue;
        }

        let mut quantity_ids = Vec::new();
        for (quantity_name, quantity_value) in quantities {
            let quantity = writer.add_entity(format!(
                "IFCQUANTITYLENGTH('{}',$,$,{},$)",
                sanitize_string_literal(quantity_name),
                format_real(*quantity_value)
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
}
