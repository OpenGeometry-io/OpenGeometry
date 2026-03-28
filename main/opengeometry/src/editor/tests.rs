use openmaths::Vector3;
use uuid::Uuid;

use super::*;
use crate::brep::BrepBuilder;
use crate::freeform::OGFreeformGeometry;
use crate::primitives::cuboid::OGCuboid;
use crate::primitives::line::OGLine;
use crate::primitives::polygon::OGPolygon;
use crate::primitives::polyline::OGPolyline;

fn parse_edit_result(payload: &str) -> FreeformEditResult {
    serde_json::from_str(payload).expect("edit result payload")
}

fn parse_face_info(payload: &str) -> FaceInfo {
    serde_json::from_str(payload).expect("face info payload")
}

fn parse_vertex_info(payload: &str) -> VertexInfo {
    serde_json::from_str(payload).expect("vertex info payload")
}

fn create_editor() -> OGFreeformEditor {
    OGFreeformEditor::new()
}

fn face_info(editor: &OGFreeformEditor, entity: &OGFreeformGeometry, face_id: u32) -> FaceInfo {
    parse_face_info(&editor.get_face_info(entity, face_id).expect("face info"))
}

fn vertex_info(
    editor: &OGFreeformEditor,
    entity: &OGFreeformGeometry,
    vertex_id: u32,
) -> VertexInfo {
    parse_vertex_info(
        &editor
            .get_vertex_info(entity, vertex_id)
            .expect("vertex info"),
    )
}

fn entity_caps(editor: &OGFreeformEditor, entity: &OGFreeformGeometry) -> EditCapabilities {
    serde_json::from_str(
        &editor
            .get_edit_capabilities(entity)
            .expect("entity capabilities payload"),
    )
    .expect("entity capabilities json")
}

fn face_caps(
    editor: &OGFreeformEditor,
    entity: &OGFreeformGeometry,
    face_id: u32,
) -> FeatureEditCapabilities {
    serde_json::from_str(
        &editor
            .get_face_edit_capabilities(entity, face_id)
            .expect("face capabilities payload"),
    )
    .expect("face capabilities json")
}

fn edge_caps(
    editor: &OGFreeformEditor,
    entity: &OGFreeformGeometry,
    edge_id: u32,
) -> FeatureEditCapabilities {
    serde_json::from_str(
        &editor
            .get_edge_edit_capabilities(entity, edge_id)
            .expect("edge capabilities payload"),
    )
    .expect("edge capabilities json")
}

fn extrude_face(
    editor: &OGFreeformEditor,
    entity: &mut OGFreeformGeometry,
    face_id: u32,
    distance: f64,
    options: Option<&str>,
) -> FreeformEditResult {
    parse_edit_result(
        &editor
            .extrude_face(entity, face_id, distance, options.map(str::to_string))
            .expect("extrude face payload"),
    )
}

fn move_vertex(
    editor: &OGFreeformEditor,
    entity: &mut OGFreeformGeometry,
    vertex_id: u32,
    translation: Vector3,
    options: Option<&str>,
) -> FreeformEditResult {
    parse_edit_result(
        &editor
            .move_vertex(entity, vertex_id, translation, options.map(str::to_string))
            .expect("move vertex payload"),
    )
}

fn split_edge(
    editor: &OGFreeformEditor,
    entity: &mut OGFreeformGeometry,
    edge_id: u32,
    t: f64,
    options: Option<&str>,
) -> FreeformEditResult {
    parse_edit_result(
        &editor
            .split_edge(entity, edge_id, t, options.map(str::to_string))
            .expect("split edge payload"),
    )
}

fn remove_vertex(
    editor: &OGFreeformEditor,
    entity: &mut OGFreeformGeometry,
    vertex_id: u32,
    options: Option<&str>,
) -> FreeformEditResult {
    parse_edit_result(
        &editor
            .remove_vertex(entity, vertex_id, options.map(str::to_string))
            .expect("remove vertex payload"),
    )
}

fn loop_cut(
    editor: &OGFreeformEditor,
    entity: &mut OGFreeformGeometry,
    edge_id: u32,
    t: f64,
    options: Option<&str>,
) -> FreeformEditResult {
    parse_edit_result(
        &editor
            .loop_cut(entity, edge_id, t, options.map(str::to_string))
            .expect("loop cut payload"),
    )
}

fn cut_face(
    editor: &OGFreeformEditor,
    entity: &mut OGFreeformGeometry,
    face_id: u32,
    start_edge_id: u32,
    start_t: f64,
    end_edge_id: u32,
    end_t: f64,
    options: Option<&str>,
) -> FreeformEditResult {
    parse_edit_result(
        &editor
            .cut_face(
                entity,
                face_id,
                start_edge_id,
                start_t,
                end_edge_id,
                end_t,
                options.map(str::to_string),
            )
            .expect("cut face payload"),
    )
}

fn assert_vec3_close(actual: Vector3, expected: Vector3) {
    assert!((actual.x - expected.x).abs() < 1.0e-9);
    assert!((actual.y - expected.y).abs() < 1.0e-9);
    assert!((actual.z - expected.z).abs() < 1.0e-9);
}

fn top_face_id(editor: &OGFreeformEditor, entity: &OGFreeformGeometry) -> u32 {
    let mut best_face = 0;
    let mut best_y = f64::NEG_INFINITY;

    for face in &entity.local_brep.faces {
        let info = face_info(editor, entity, face.id);
        if info.centroid.y > best_y {
            best_y = info.centroid.y;
            best_face = face.id;
        }
    }

    best_face
}

fn vertical_edge_id(entity: &OGFreeformGeometry) -> u32 {
    entity
        .local_brep
        .edges
        .iter()
        .find_map(|edge| {
            let (start_id, end_id) = entity.local_brep.get_edge_endpoints(edge.id)?;
            let start = entity.local_brep.vertices.get(start_id as usize)?.position;
            let end = entity.local_brep.vertices.get(end_id as usize)?.position;

            let dx = (end.x - start.x).abs();
            let dy = (end.y - start.y).abs();
            let dz = (end.z - start.z).abs();

            if dy > 0.2 && dx <= 1.0e-6 && dz <= 1.0e-6 {
                Some(edge.id)
            } else {
                None
            }
        })
        .expect("vertical edge")
}

fn right_side_face_id(editor: &OGFreeformEditor, entity: &OGFreeformGeometry) -> u32 {
    entity
        .local_brep
        .faces
        .iter()
        .find_map(|face| {
            let info = face_info(editor, entity, face.id);
            if info.normal.x > 0.5 && info.normal.y.abs() < 1.0e-6 {
                Some(face.id)
            } else {
                None
            }
        })
        .expect("right side face")
}

fn opposite_vertical_face_edges(
    editor: &OGFreeformEditor,
    entity: &OGFreeformGeometry,
    face_id: u32,
) -> (u32, u32) {
    let info = face_info(editor, entity, face_id);
    let mut vertical_edges = info
        .edge_ids
        .iter()
        .filter_map(|edge_id| {
            let edge = entity.local_brep.edges.get(*edge_id as usize)?;
            let (start_id, end_id) = entity.local_brep.get_edge_endpoints(edge.id)?;
            let start = entity.local_brep.vertices.get(start_id as usize)?.position;
            let end = entity.local_brep.vertices.get(end_id as usize)?.position;

            let dx = (end.x - start.x).abs();
            let dy = (end.y - start.y).abs();
            let dz = (end.z - start.z).abs();

            if dy > 0.2 && dx <= 1.0e-6 && dz <= 1.0e-6 {
                let avg_x = (start.x + end.x) * 0.5;
                Some((avg_x, edge.id))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    vertical_edges.sort_by(|left, right| left.0.total_cmp(&right.0));

    let left_edge = vertical_edges.first().expect("left vertical edge").1;
    let right_edge = vertical_edges.last().expect("right vertical edge").1;

    (left_edge, right_edge)
}

fn remap_entry_for(entries: &[TopologyRemapEntry], old_id: u32) -> &TopologyRemapEntry {
    entries
        .iter()
        .find(|entry| entry.old_id == old_id)
        .expect("remap entry")
}

fn create_freeform_polygon_entity(id: &str) -> OGFreeformGeometry {
    let mut polygon = OGPolygon::new(format!("{}-source", id));
    polygon
        .set_config(vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(2.0, 0.0, 0.0),
            Vector3::new(2.0, 0.0, 2.0),
            Vector3::new(0.0, 0.0, 2.0),
        ])
        .expect("polygon config");

    OGFreeformGeometry::new(id.to_string(), polygon.get_local_brep_serialized())
        .expect("freeform polygon")
}

fn create_freeform_line_entity(id: &str) -> OGFreeformGeometry {
    let mut line = OGLine::new(format!("{}-source", id));
    line.set_config(Vector3::new(-1.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0))
        .expect("line config");

    OGFreeformGeometry::new(id.to_string(), line.get_local_brep_serialized())
        .expect("freeform line")
}

fn create_freeform_polyline_entity(id: &str, points: Vec<Vector3>) -> OGFreeformGeometry {
    let mut polyline = OGPolyline::new(format!("{}-source", id));
    polyline.set_config(points).expect("polyline config");

    OGFreeformGeometry::new(id.to_string(), polyline.get_local_brep_serialized())
        .expect("freeform polyline")
}

fn create_mixed_face_and_wire_entity(id: &str) -> OGFreeformGeometry {
    let mut builder = BrepBuilder::new(Uuid::new_v4());
    let positions = vec![
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(2.0, 0.0, 0.0),
        Vector3::new(2.0, 0.0, 2.0),
        Vector3::new(0.0, 0.0, 2.0),
        Vector3::new(3.0, 0.0, 0.0),
        Vector3::new(4.0, 0.0, 0.0),
    ];
    builder.add_vertices(&positions);
    builder.add_face(&[0, 1, 2, 3], &[]).expect("face");
    builder.add_wire(&[4, 5], false).expect("wire");

    let brep = builder.build().expect("mixed topology brep");
    OGFreeformGeometry::new(
        id.to_string(),
        serde_json::to_string(&brep).expect("mixed topology json"),
    )
    .expect("freeform mixed topology")
}

#[test]
fn extrude_face_on_open_surface_creates_watertight_shell_and_created_ids() {
    let editor = create_editor();
    let mut entity = create_freeform_polygon_entity("entity-polygon-extrude");

    let result = extrude_face(&editor, &mut entity, 0, 1.0, None);

    assert!(result.validity.ok);
    assert!(result.topology_changed);

    let remap = result.topology_remap.expect("topology remap");
    let face_entry = remap_entry_for(&remap.faces, 0);
    assert_eq!(face_entry.new_ids, vec![0]);
    assert_eq!(face_entry.primary_id, Some(0));
    assert_eq!(face_entry.status, TopologyRemapStatus::Unchanged);

    assert_eq!(remap.created_ids.vertices.len(), 4);
    assert_eq!(remap.created_ids.faces.len(), 5);
    assert!(!remap.created_ids.edges.is_empty());

    assert_eq!(entity.local_brep.faces.len(), 6);
    assert_eq!(entity.local_brep.shells.len(), 1);
    assert!(entity.local_brep.shells[0].is_closed);
}

#[test]
fn extrude_face_on_closed_solid_falls_back_to_push_pull_without_topology_change() {
    let editor = create_editor();
    let mut cuboid = OGCuboid::new("cuboid-extrude-solid".to_string());
    cuboid
        .set_config(Vector3::new(0.0, 0.0, 0.0), 2.0, 2.0, 2.0)
        .expect("cuboid config");

    let mut entity = OGFreeformGeometry::new(
        "entity-cuboid-extrude-solid".to_string(),
        cuboid.get_local_brep_serialized(),
    )
    .expect("freeform entity");

    let face_id = top_face_id(&editor, &entity);
    let result = extrude_face(&editor, &mut entity, face_id, 0.5, None);

    assert!(result.validity.ok);
    assert!(!result.topology_changed);
    assert!(result
        .validity
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "solid_face_extrude"));
}

#[test]
fn split_edge_and_remove_vertex_emit_semantic_remap_statuses() {
    let editor = create_editor();
    let mut entity = create_freeform_polygon_entity("entity-topology-edits");

    let split = split_edge(&editor, &mut entity, 0, 0.5, None);

    assert!(split.validity.ok);
    assert!(split.topology_changed);

    let split_remap = split.topology_remap.expect("split remap");
    let split_entry = remap_entry_for(&split_remap.edges, 0);
    assert_eq!(split_entry.status, TopologyRemapStatus::Split);
    assert_eq!(split_entry.new_ids.len(), 2);

    let inserted_vertex_id = *split_remap
        .created_ids
        .vertices
        .first()
        .expect("inserted vertex id");

    let removed = remove_vertex(&editor, &mut entity, inserted_vertex_id, None);

    assert!(removed.validity.ok);
    assert!(removed.topology_changed);

    let removed_remap = removed.topology_remap.expect("remove remap");
    let removed_vertex_entry = remap_entry_for(&removed_remap.vertices, inserted_vertex_id);
    assert_eq!(removed_vertex_entry.status, TopologyRemapStatus::Deleted);
    assert!(removed_vertex_entry.new_ids.is_empty());
    assert!(removed_remap
        .edges
        .iter()
        .any(|entry| entry.status == TopologyRemapStatus::Merged));
}

#[test]
fn repeated_identical_sequences_are_deterministic() {
    let editor = create_editor();
    let mut first = create_freeform_polygon_entity("entity-determinism-a");
    let mut second = create_freeform_polygon_entity("entity-determinism-b");

    let first_split = split_edge(&editor, &mut first, 1, 0.3, None);
    let second_split = split_edge(&editor, &mut second, 1, 0.3, None);

    let first_inserted = first_split
        .topology_remap
        .as_ref()
        .expect("first remap")
        .created_ids
        .vertices[0];
    let second_inserted = second_split
        .topology_remap
        .as_ref()
        .expect("second remap")
        .created_ids
        .vertices[0];

    let first_remove = remove_vertex(&editor, &mut first, first_inserted, None);
    let second_remove = remove_vertex(&editor, &mut second, second_inserted, None);

    let first_json = serde_json::to_string(&first_remove.topology_remap).expect("first remap json");
    let second_json =
        serde_json::to_string(&second_remove.topology_remap).expect("second remap json");

    assert_eq!(first_json, second_json);

    let mut first_brep: serde_json::Value =
        serde_json::from_str(&first.get_local_brep_serialized()).expect("first brep json");
    let mut second_brep: serde_json::Value =
        serde_json::from_str(&second.get_local_brep_serialized()).expect("second brep json");

    first_brep["id"] = serde_json::Value::Null;
    second_brep["id"] = serde_json::Value::Null;

    assert_eq!(first_brep, second_brep);
}

#[test]
fn unsupported_topology_edit_returns_structured_error_diagnostic() {
    let editor = create_editor();
    let mut cuboid = OGCuboid::new("cuboid-unsupported".to_string());
    cuboid
        .set_config(Vector3::new(0.0, 0.0, 0.0), 2.0, 2.0, 2.0)
        .expect("cuboid config");

    let mut entity = OGFreeformGeometry::new(
        "entity-cuboid-unsupported".to_string(),
        cuboid.get_local_brep_serialized(),
    )
    .expect("freeform entity");

    let result = remove_vertex(&editor, &mut entity, 0, None);

    assert!(!result.validity.ok);
    assert!(result
        .validity
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "unsupported_topology"));
}

#[test]
fn constraints_project_move_vertex_translation_to_axis() {
    let editor = create_editor();
    let mut cuboid = OGCuboid::new("cuboid-constraints".to_string());
    cuboid
        .set_config(Vector3::new(0.0, 0.0, 0.0), 2.0, 2.0, 2.0)
        .expect("cuboid config");

    let mut entity = OGFreeformGeometry::new(
        "entity-cuboid-constraints".to_string(),
        cuboid.get_local_brep_serialized(),
    )
    .expect("freeform entity");

    let before = vertex_info(&editor, &entity, 0);

    let options = r#"{
      "constraintAxis": { "x": 0.0, "y": 1.0, "z": 0.0 },
      "preserveCoplanarity": true
    }"#;

    let result = move_vertex(
        &editor,
        &mut entity,
        0,
        Vector3::new(1.0, 0.2, 3.0),
        Some(options),
    );

    assert!(result.validity.ok);

    let after = vertex_info(&editor, &entity, 0);

    assert!((after.position.x - before.position.x).abs() < 1.0e-9);
    assert!((after.position.y - before.position.y - 0.2).abs() < 1.0e-9);
    assert!((after.position.z - before.position.z).abs() < 1.0e-9);

    assert!(result
        .validity
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "preserve_coplanarity_requested"));
}

#[test]
fn lean_defaults_omit_heavy_payloads_and_include_deltas() {
    let editor = create_editor();
    let mut entity = create_freeform_polygon_entity("entity-lean-defaults");

    let result = move_vertex(&editor, &mut entity, 0, Vector3::new(0.25, 0.0, 0.0), None);

    assert!(result.validity.ok);
    assert!(result.brep_serialized.is_none());
    assert!(result.local_brep_serialized.is_none());
    assert!(result.geometry_serialized.is_none());
    assert!(result.outline_geometry_serialized.is_none());

    assert!(result.topology_remap.is_some());
    assert!(result.changed_vertices.is_some());
    assert!(!result
        .changed_vertices
        .expect("changed vertices")
        .is_empty());
}

#[test]
fn capability_endpoints_report_entity_and_feature_flags() {
    let editor = create_editor();
    let entity = create_freeform_polygon_entity("entity-capabilities");

    let entity_caps = entity_caps(&editor, &entity);

    assert!(entity_caps.can_insert_vertex_on_edge);
    assert!(entity_caps.can_split_edge);
    assert!(entity_caps.can_remove_vertex);
    assert!(entity_caps.can_cut_face);

    let face_caps = face_caps(&editor, &entity, 0);

    assert!(face_caps.can_extrude_face);
    assert!(face_caps.can_move_face);
    assert!(face_caps.can_cut_face);
}

#[test]
fn freeform_line_capabilities_report_wire_backed_edge_split_support() {
    let editor = create_editor();
    let entity = create_freeform_line_entity("entity-line-capabilities");

    let entity_caps = entity_caps(&editor, &entity);
    assert!(entity_caps.can_insert_vertex_on_edge);
    assert!(entity_caps.can_split_edge);
    assert!(!entity_caps.can_cut_face);
    assert!(!entity_caps.can_remove_vertex);

    let edge_caps = edge_caps(&editor, &entity, 0);
    assert!(edge_caps.can_insert_vertex_on_edge);
    assert!(edge_caps.can_split_edge);
    assert!(!edge_caps.can_loop_cut);
}

#[test]
fn split_edge_on_freeform_line_preserves_open_wire_topology() {
    let editor = create_editor();
    let mut entity = create_freeform_line_entity("entity-line-split");

    let result = split_edge(&editor, &mut entity, 0, 0.5, None);

    assert!(result.validity.ok);
    assert!(result.topology_changed);
    assert_eq!(entity.local_brep.faces.len(), 0);
    assert_eq!(entity.local_brep.wires.len(), 1);
    assert!(!entity.local_brep.wires[0].is_closed);
    assert_eq!(entity.local_brep.edges.len(), 2);
    assert_eq!(entity.local_brep.vertices.len(), 3);
    assert_eq!(entity.local_brep.get_wire_vertex_indices(0), vec![0, 2, 1]);
    assert_vec3_close(
        entity.local_brep.vertices[2].position,
        Vector3::new(0.0, 0.0, 0.0),
    );

    let remap = result.topology_remap.expect("topology remap");
    let edge_entry = remap_entry_for(&remap.edges, 0);
    assert_eq!(edge_entry.status, TopologyRemapStatus::Split);
    assert_eq!(edge_entry.new_ids.len(), 2);
    assert_eq!(remap.created_ids.vertices, vec![2]);
}

#[test]
fn split_edge_on_open_freeform_polyline_inserts_vertex_in_wire_order() {
    let editor = create_editor();
    let mut entity = create_freeform_polyline_entity(
        "entity-open-polyline-split",
        vec![
            Vector3::new(-2.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(2.0, 0.0, 0.0),
        ],
    );

    let result = split_edge(&editor, &mut entity, 1, 0.25, None);

    assert!(result.validity.ok);
    assert!(result.topology_changed);
    assert_eq!(entity.local_brep.wires.len(), 1);
    assert!(!entity.local_brep.wires[0].is_closed);
    assert_eq!(
        entity.local_brep.get_wire_vertex_indices(0),
        vec![0, 1, 3, 2]
    );
    assert_vec3_close(
        entity.local_brep.vertices[3].position,
        Vector3::new(0.5, 0.0, 0.0),
    );
    assert_eq!(entity.local_brep.edges.len(), 3);
}

#[test]
fn split_edge_on_closed_freeform_polyline_preserves_closed_wire_cycle() {
    let editor = create_editor();
    let mut entity = create_freeform_polyline_entity(
        "entity-closed-polyline-split",
        vec![
            Vector3::new(-1.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 1.5),
            Vector3::new(-1.0, 0.0, 0.0),
        ],
    );

    let result = split_edge(&editor, &mut entity, 2, 0.5, None);

    assert!(result.validity.ok);
    assert!(result.topology_changed);
    assert_eq!(entity.local_brep.wires.len(), 1);
    assert!(entity.local_brep.wires[0].is_closed);
    assert_eq!(
        entity.local_brep.get_wire_vertex_indices(0),
        vec![0, 1, 2, 3]
    );
    assert_vec3_close(
        entity.local_brep.vertices[3].position,
        Vector3::new(-0.5, 0.0, 0.0),
    );
    assert_eq!(entity.local_brep.edges.len(), 4);
}

#[test]
fn split_edge_on_face_preserves_unrelated_wire_topology() {
    let editor = create_editor();
    let mut entity = create_mixed_face_and_wire_entity("entity-mixed-face-wire");

    let result = split_edge(&editor, &mut entity, 0, 0.5, None);

    assert!(result.validity.ok);
    assert!(result.topology_changed);
    assert_eq!(entity.local_brep.faces.len(), 1);
    assert_eq!(entity.local_brep.wires.len(), 1);
    assert_eq!(entity.local_brep.get_wire_vertex_indices(0), vec![4, 5]);
    assert_eq!(entity.local_brep.edges.len(), 6);
}

#[test]
fn split_edge_on_wire_preserves_unrelated_face_topology() {
    let editor = create_editor();
    let mut entity = create_mixed_face_and_wire_entity("entity-mixed-wire-face");

    let result = split_edge(&editor, &mut entity, 4, 0.5, None);

    assert!(result.validity.ok);
    assert!(result.topology_changed);
    assert_eq!(entity.local_brep.faces.len(), 1);
    assert_eq!(
        entity
            .local_brep
            .get_loop_vertex_indices(entity.local_brep.faces[0].outer_loop),
        vec![0, 1, 2, 3]
    );
    assert_eq!(entity.local_brep.wires.len(), 1);
    assert_eq!(entity.local_brep.get_wire_vertex_indices(0), vec![4, 6, 5]);
    assert_eq!(entity.local_brep.edges.len(), 6);
}

#[test]
fn split_edge_on_wire_backed_topology_preserves_invalid_parameter_diagnostics() {
    let editor = create_editor();
    let mut entity = create_freeform_line_entity("entity-line-invalid-parameter");

    let result = split_edge(&editor, &mut entity, 0, 0.0, None);

    assert!(!result.validity.ok);
    assert!(result
        .validity
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "invalid_parameter"));
}

#[test]
fn converted_cuboid_supports_edge_split_and_returns_created_vertex() {
    let editor = create_editor();
    let mut cuboid = OGCuboid::new("cuboid-edge-split".to_string());
    cuboid
        .set_config(Vector3::new(0.0, 0.0, 0.0), 2.0, 2.0, 2.0)
        .expect("cuboid config");

    let mut entity = OGFreeformGeometry::new(
        "entity-cuboid-edge-split".to_string(),
        cuboid.get_local_brep_serialized(),
    )
    .expect("freeform entity");

    let edge_caps = edge_caps(&editor, &entity, 0);
    assert!(edge_caps.can_split_edge);

    let result = split_edge(&editor, &mut entity, 0, 0.5, None);

    assert!(result.validity.ok);
    assert!(result.topology_changed);
    assert_eq!(
        result
            .topology_remap
            .as_ref()
            .expect("topology remap")
            .created_ids
            .vertices
            .len(),
        1
    );
}

#[test]
fn converted_cuboid_loop_cut_splits_side_ring_into_additional_faces() {
    let editor = create_editor();
    let mut cuboid = OGCuboid::new("cuboid-loop-cut".to_string());
    cuboid
        .set_config(Vector3::new(0.0, 0.0, 0.0), 2.0, 2.0, 2.0)
        .expect("cuboid config");

    let mut entity = OGFreeformGeometry::new(
        "entity-cuboid-loop-cut".to_string(),
        cuboid.get_local_brep_serialized(),
    )
    .expect("freeform entity");

    let edge_id = vertical_edge_id(&entity);
    let edge_caps = edge_caps(&editor, &entity, edge_id);
    assert!(edge_caps.can_loop_cut);

    let result = loop_cut(&editor, &mut entity, edge_id, 0.5, None);

    assert!(result.validity.ok);
    assert!(result.topology_changed);
    assert_eq!(entity.local_brep.faces.len(), 10);
    assert_eq!(entity.local_brep.vertices.len(), 12);
    assert_eq!(entity.local_brep.shells.len(), 1);
    assert!(entity.local_brep.shells[0].is_closed);

    let remap = result.topology_remap.expect("topology remap");
    assert_eq!(remap.created_ids.vertices.len(), 4);
    assert_eq!(remap.created_ids.faces.len(), 4);
    assert!(remap.created_ids.edges.len() >= 4);
    assert_eq!(
        remap
            .faces
            .iter()
            .filter(|entry| entry.status == TopologyRemapStatus::Split)
            .count(),
        4
    );
}

#[test]
fn converted_cuboid_cut_face_splits_only_the_selected_side_face() {
    let editor = create_editor();
    let mut cuboid = OGCuboid::new("cuboid-cut-face".to_string());
    cuboid
        .set_config(Vector3::new(0.0, 0.0, 0.0), 2.0, 2.0, 2.0)
        .expect("cuboid config");

    let mut entity = OGFreeformGeometry::new(
        "entity-cuboid-cut-face".to_string(),
        cuboid.get_local_brep_serialized(),
    )
    .expect("freeform entity");

    let face_id = right_side_face_id(&editor, &entity);
    let face_caps = face_caps(&editor, &entity, face_id);
    assert!(face_caps.can_cut_face);

    let (start_edge_id, end_edge_id) = opposite_vertical_face_edges(&editor, &entity, face_id);
    let result = cut_face(
        &editor,
        &mut entity,
        face_id,
        start_edge_id,
        0.5,
        end_edge_id,
        0.5,
        None,
    );

    assert!(result.validity.ok);
    assert!(result.topology_changed);
    assert_eq!(entity.local_brep.faces.len(), 7);
    assert_eq!(entity.local_brep.vertices.len(), 10);
    assert_eq!(entity.local_brep.shells.len(), 1);

    let remap = result.topology_remap.expect("topology remap");
    let face_entry = remap_entry_for(&remap.faces, face_id);
    assert_eq!(face_entry.status, TopologyRemapStatus::Split);
    assert_eq!(face_entry.new_ids.len(), 2);
    assert_eq!(remap.created_ids.faces.len(), 1);
    assert_eq!(remap.created_ids.vertices.len(), 2);
    assert_eq!(remap.created_ids.edges.len(), 1);

    let created_face_id = remap.created_ids.faces[0];
    let original_face_info = face_info(&editor, &entity, face_id);
    let created_face_info = face_info(&editor, &entity, created_face_id);
    assert_eq!(original_face_info.edge_ids.len(), 4);
    assert_eq!(created_face_info.edge_ids.len(), 4);
}

#[test]
fn topology_remap_builder_supports_split_merge_and_deleted_statuses() {
    let old_ids = vec![10, 11, 12, 13];
    let mut mapping = std::collections::HashMap::<u32, Vec<u32>>::new();
    mapping.insert(10, vec![100, 101]);
    mapping.insert(11, vec![200]);
    mapping.insert(12, vec![200]);
    mapping.insert(13, Vec::new());

    let entries = super::remap::build_domain_entries_from_mapping(&old_ids, &mapping);

    let split_entry = remap_entry_for(&entries, 10);
    assert_eq!(split_entry.status, TopologyRemapStatus::Split);
    assert_eq!(split_entry.primary_id, Some(100));
    assert_eq!(split_entry.new_ids, vec![100, 101]);

    let merged_left = remap_entry_for(&entries, 11);
    assert_eq!(merged_left.status, TopologyRemapStatus::Merged);
    assert_eq!(merged_left.primary_id, Some(200));
    assert_eq!(merged_left.new_ids, vec![200]);

    let merged_right = remap_entry_for(&entries, 12);
    assert_eq!(merged_right.status, TopologyRemapStatus::Merged);
    assert_eq!(merged_right.primary_id, Some(200));
    assert_eq!(merged_right.new_ids, vec![200]);

    let deleted_entry = remap_entry_for(&entries, 13);
    assert_eq!(deleted_entry.status, TopologyRemapStatus::Deleted);
    assert_eq!(deleted_entry.primary_id, None);
    assert!(deleted_entry.new_ids.is_empty());
}
