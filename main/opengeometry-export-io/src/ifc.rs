use std::collections::HashMap;
use std::fmt;

use opengeometry_export_schema::{
    ExportMaterial, ExportMesh, ExportSceneSnapshot, IfcEntitySemantics,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::part21::{sanitize_string_literal, Part21Writer};

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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IfcExportConfig {
    pub schema: IfcSchemaVersion,
    pub project_name: Option<String>,
    pub site_name: Option<String>,
    pub building_name: Option<String>,
    pub storey_name: Option<String>,
    pub scale: f64,
    pub error_policy: IfcErrorPolicy,
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
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct IfcExportReport {
    pub input_entities: usize,
    pub input_triangles: usize,
    pub exported_elements: usize,
    pub skipped_entities: usize,
    pub skipped_triangles: usize,
    pub semantics_applied: usize,
    pub proxy_fallbacks: usize,
    pub property_sets_written: usize,
    pub quantity_sets_written: usize,
    pub materials_written: usize,
    pub material_assignments_written: usize,
    pub styled_items_written: usize,
    pub missing_material_refs: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum IfcExportError {
    EmptyInput,
    InvalidMesh(String),
    InvalidSemantics(String),
    InvalidMaterial(String),
    Serialization(String),
    Io(String),
}

impl fmt::Display for IfcExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IfcExportError::EmptyInput => write!(f, "No input entities provided for IFC export"),
            IfcExportError::InvalidMesh(msg) => write!(f, "Invalid mesh data: {}", msg),
            IfcExportError::InvalidSemantics(msg) => write!(f, "Invalid IFC semantics: {}", msg),
            IfcExportError::InvalidMaterial(msg) => write!(f, "Invalid IFC material data: {}", msg),
            IfcExportError::Serialization(msg) => write!(f, "IFC serialization failed: {}", msg),
            IfcExportError::Io(msg) => write!(f, "IFC I/O failed: {}", msg),
        }
    }
}

impl std::error::Error for IfcExportError {}

#[derive(Clone, Default)]
struct TessellatedMesh {
    points: Vec<[f64; 3]>,
    triangles: Vec<[usize; 3]>,
}

#[derive(Clone, Copy)]
struct MaterialBindings {
    material_id: usize,
    style_assignment_id: Option<usize>,
}

pub fn export_snapshot_to_ifc_text(
    snapshot: &ExportSceneSnapshot,
    config: &IfcExportConfig,
) -> Result<(String, IfcExportReport), IfcExportError> {
    let scale = validate_config(config)?;
    if snapshot.entities.is_empty() {
        return Err(IfcExportError::EmptyInput);
    }

    let mut report = IfcExportReport {
        input_entities: snapshot.entities.len(),
        ..IfcExportReport::default()
    };

    let project_name = config
        .project_name
        .clone()
        .or_else(|| {
            if snapshot.scene.name.is_empty() {
                None
            } else {
                Some(snapshot.scene.name.clone())
            }
        })
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

    let material_map: HashMap<&str, &ExportMaterial> = snapshot
        .materials
        .iter()
        .map(|material| (material.id.as_str(), material))
        .collect();
    let mut material_cache: HashMap<String, MaterialBindings> = HashMap::new();

    let mut element_ids = Vec::new();

    for entity in &snapshot.entities {
        let mesh = sanitize_mesh(
            &entity.id,
            &entity.mesh,
            scale,
            config.error_policy,
            &mut report,
        )?;

        if mesh.points.is_empty() || mesh.triangles.is_empty() {
            if config.error_policy == IfcErrorPolicy::Strict {
                return Err(IfcExportError::InvalidMesh(format!(
                    "Entity '{}' generated no exportable mesh",
                    entity.id
                )));
            }
            report.skipped_entities += 1;
            continue;
        }

        let class_name =
            resolve_ifc_class(&entity.id, entity.semantics.as_ref(), config, &mut report)?;

        let mesh_point_list = writer.add_entity(format!(
            "IFCCARTESIANPOINTLIST3D({})",
            format_ifc_coord_list(&mesh.points)
        ));

        let mesh_faceset = writer.add_entity(format!(
            "IFCTRIANGULATEDFACESET({},$,.T.,{},$)",
            Part21Writer::reference(mesh_point_list),
            format_ifc_face_index_list(&mesh.triangles)
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

        let default_name = format!("{}-{}", entity.kind, entity.id);
        let name = entity
            .semantics
            .as_ref()
            .and_then(|sem| sem.name.clone())
            .unwrap_or(default_name);
        let description = entity
            .semantics
            .as_ref()
            .and_then(|sem| sem.description.clone())
            .unwrap_or_default();
        let object_type = entity
            .semantics
            .as_ref()
            .and_then(|sem| sem.object_type.clone())
            .unwrap_or_else(|| entity.kind.clone());
        let tag = entity
            .semantics
            .as_ref()
            .and_then(|sem| sem.tag.clone())
            .unwrap_or_else(|| entity.id.clone());

        let element_expr = format!(
            "{}('{}',$,'{}',{},'{}',{},{},'{}',.NOTDEFINED.)",
            class_name,
            ifc_guid(&format!("element-{}", entity.id)),
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

        if let Some(semantics) = &entity.semantics {
            write_property_sets(&mut writer, element_id, &entity.id, semantics, &mut report);
            write_quantity_sets(&mut writer, element_id, &entity.id, semantics, &mut report);
        }

        if let Some(material_key) = entity.material_id.as_deref() {
            let material = material_map.get(material_key).copied();
            if let Some(material) = material {
                let bindings = ensure_material_bindings(
                    &mut writer,
                    material,
                    &mut material_cache,
                    &mut report,
                );

                writer.add_entity(format!(
                    "IFCRELASSOCIATESMATERIAL('{}',$,$,$,({}),{})",
                    ifc_guid(&format!("rel-mat-{}", entity.id)),
                    Part21Writer::reference(element_id),
                    Part21Writer::reference(bindings.material_id)
                ));
                report.material_assignments_written += 1;

                if let Some(style_assignment_id) = bindings.style_assignment_id {
                    writer.add_entity(format!(
                        "IFCSTYLEDITEM({},({}),$)",
                        Part21Writer::reference(mesh_faceset),
                        Part21Writer::reference(style_assignment_id)
                    ));
                    report.styled_items_written += 1;
                }
            } else if config.error_policy == IfcErrorPolicy::Strict {
                return Err(IfcExportError::InvalidMaterial(format!(
                    "Entity '{}' references missing material '{}'",
                    entity.id, material_key
                )));
            } else {
                report.missing_material_refs += 1;
            }
        }

        report.exported_elements += 1;
    }

    if element_ids.is_empty() {
        return Err(IfcExportError::InvalidMesh(
            "No elements were exported from the provided snapshot".to_string(),
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

#[cfg(not(target_arch = "wasm32"))]
pub fn export_snapshot_to_ifc_file(
    snapshot: &ExportSceneSnapshot,
    file_path: &str,
    config: &IfcExportConfig,
) -> Result<IfcExportReport, IfcExportError> {
    let (text, report) = export_snapshot_to_ifc_text(snapshot, config)?;
    std::fs::write(file_path, text).map_err(|err| IfcExportError::Io(err.to_string()))?;
    Ok(report)
}

fn ensure_material_bindings(
    writer: &mut Part21Writer,
    material: &ExportMaterial,
    cache: &mut HashMap<String, MaterialBindings>,
    report: &mut IfcExportReport,
) -> MaterialBindings {
    if let Some(existing) = cache.get(&material.id) {
        return *existing;
    }

    let material_entity = writer.add_entity(format!(
        "IFCMATERIAL('{}',{},{} )",
        sanitize_string_literal(if material.name.is_empty() {
            "Material"
        } else {
            &material.name
        }),
        format_optional_string(material.description.as_deref()),
        format_optional_string(material.category.as_deref())
    ));

    report.materials_written += 1;

    let style_assignment_id = material.color.as_ref().map(|color| {
        let rgb = writer.add_entity(format!(
            "IFCCOLOURRGB($,{},{},{})",
            format_real(clamp01(color.red)),
            format_real(clamp01(color.green)),
            format_real(clamp01(color.blue))
        ));

        let rendering = writer.add_entity(format!(
            "IFCSURFACESTYLERENDERING({},$,$,$,$,$,$,$,.NOTDEFINED.)",
            Part21Writer::reference(rgb)
        ));

        let style = writer.add_entity(format!(
            "IFCSURFACESTYLE('{}',.BOTH.,({}))",
            sanitize_string_literal(if material.name.is_empty() {
                "MaterialStyle"
            } else {
                &material.name
            }),
            Part21Writer::reference(rendering)
        ));

        writer.add_entity(format!(
            "IFCPRESENTATIONSTYLEASSIGNMENT(({}))",
            Part21Writer::reference(style)
        ))
    });

    let bindings = MaterialBindings {
        material_id: material_entity,
        style_assignment_id,
    };

    cache.insert(material.id.clone(), bindings);
    bindings
}

fn format_optional_string(value: Option<&str>) -> String {
    match value {
        Some(raw) if !raw.trim().is_empty() => {
            format!("'{}'", sanitize_string_literal(raw.trim()))
        }
        _ => "$".to_string(),
    }
}

fn validate_config(config: &IfcExportConfig) -> Result<f64, IfcExportError> {
    if !config.scale.is_finite() || config.scale <= 0.0 {
        return Err(IfcExportError::InvalidMesh(
            "IFC scale must be a finite positive value".to_string(),
        ));
    }
    Ok(config.scale)
}

fn sanitize_mesh(
    entity_id: &str,
    mesh: &ExportMesh,
    scale: f64,
    policy: IfcErrorPolicy,
    report: &mut IfcExportReport,
) -> Result<TessellatedMesh, IfcExportError> {
    let mut points = Vec::with_capacity(mesh.points.len());
    for (index, point) in mesh.points.iter().enumerate() {
        let scaled = [point[0] * scale, point[1] * scale, point[2] * scale];
        if !scaled[0].is_finite() || !scaled[1].is_finite() || !scaled[2].is_finite() {
            if policy == IfcErrorPolicy::Strict {
                return Err(IfcExportError::InvalidMesh(format!(
                    "Entity '{}' point {} has non-finite coordinates",
                    entity_id, index
                )));
            }
            report.skipped_entities += 1;
            return Ok(TessellatedMesh::default());
        }
        points.push(scaled);
    }

    let mut triangles = Vec::new();
    for triangle in &mesh.triangles {
        report.input_triangles += 1;

        if triangle[0] >= points.len() || triangle[1] >= points.len() || triangle[2] >= points.len()
        {
            if policy == IfcErrorPolicy::Strict {
                return Err(IfcExportError::InvalidMesh(format!(
                    "Entity '{}' has out-of-range triangle indices",
                    entity_id
                )));
            }
            report.skipped_triangles += 1;
            continue;
        }

        let a = points[triangle[0]];
        let b = points[triangle[1]];
        let c = points[triangle[2]];

        if is_degenerate_triangle(a, b, c) {
            if policy == IfcErrorPolicy::Strict {
                return Err(IfcExportError::InvalidMesh(format!(
                    "Entity '{}' contains degenerate triangle",
                    entity_id
                )));
            }
            report.skipped_triangles += 1;
            continue;
        }

        triangles.push([triangle[0] + 1, triangle[1] + 1, triangle[2] + 1]);
    }

    Ok(TessellatedMesh { points, triangles })
}

fn resolve_ifc_class<'a>(
    entity_id: &str,
    semantics: Option<&'a IfcEntitySemantics>,
    config: &IfcExportConfig,
    report: &mut IfcExportReport,
) -> Result<&'a str, IfcExportError> {
    let Some(semantics) = semantics else {
        return Ok(IFC_CLASS_PROXY);
    };

    let Some(raw_class) = semantics.ifc_class.as_deref() else {
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

fn format_ifc_coord_list(points: &[[f64; 3]]) -> String {
    let coords = points
        .iter()
        .map(|point| {
            format!(
                "({},{},{})",
                format_real(point[0]),
                format_real(point[1]),
                format_real(point[2])
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

fn is_degenerate_triangle(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> bool {
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];

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

fn clamp01(value: f64) -> f64 {
    if value.is_nan() {
        return 0.0;
    }
    value.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use opengeometry_export_schema::{
        ExportColor, ExportEntity, ExportFeatureTree, ExportMaterial, ExportMesh, ExportScene,
        ExportSceneSnapshot, IfcEntitySemantics,
    };

    fn sample_snapshot() -> ExportSceneSnapshot {
        let mut semantics = IfcEntitySemantics::default();
        semantics.ifc_class = Some("IFCWALL".to_string());
        semantics.name = Some("Wall A".to_string());

        ExportSceneSnapshot {
            scene: ExportScene {
                id: "scene-1".to_string(),
                name: "Sample Scene".to_string(),
            },
            entities: vec![ExportEntity {
                id: "wall-1".to_string(),
                kind: "OGCuboid".to_string(),
                mesh: ExportMesh {
                    points: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                    triangles: vec![[0, 1, 2]],
                },
                semantics: Some(semantics),
                material_id: Some("mat-1".to_string()),
                brep_json: None,
            }],
            feature_tree: ExportFeatureTree::default(),
            materials: vec![ExportMaterial {
                id: "mat-1".to_string(),
                name: "Paint".to_string(),
                description: None,
                category: Some("Finish".to_string()),
                color: Some(ExportColor {
                    red: 0.8,
                    green: 0.2,
                    blue: 0.2,
                    alpha: 1.0,
                }),
            }],
            ..ExportSceneSnapshot::default()
        }
    }

    #[test]
    fn exports_ifc_snapshot_with_materials() {
        let snapshot = sample_snapshot();
        let (text, report) = export_snapshot_to_ifc_text(&snapshot, &IfcExportConfig::default())
            .expect("ifc export");

        assert!(text.starts_with("ISO-10303-21;"));
        assert!(text.contains("IFCWALL("));
        assert!(text.contains("IFCMATERIAL("));
        assert!(text.contains("IFCRELASSOCIATESMATERIAL("));
        assert!(text.contains("IFCSTYLEDITEM("));
        assert_eq!(report.exported_elements, 1);
        assert_eq!(report.materials_written, 1);
        assert_eq!(report.material_assignments_written, 1);
    }
}
