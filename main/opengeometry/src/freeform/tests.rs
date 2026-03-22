use openmaths::Vector3;

use super::*;
use crate::primitives::arc::OGArc;
use crate::primitives::cuboid::OGCuboid;
use crate::primitives::curve::OGCurve;
use crate::primitives::cylinder::OGCylinder;
use crate::primitives::line::OGLine;
use crate::primitives::polygon::OGPolygon;
use crate::primitives::polyline::OGPolyline;
use crate::primitives::rectangle::OGRectangle;
use crate::primitives::sphere::OGSphere;
use crate::primitives::sweep::OGSweep;
use crate::primitives::wedge::OGWedge;

fn parse_edit_result(payload: &str) -> FreeformEditResult {
    serde_json::from_str(payload).expect("edit result payload")
}

fn parse_face_info(payload: &str) -> FaceInfo {
    serde_json::from_str(payload).expect("face info payload")
}

fn parse_vertex_info(payload: &str) -> VertexInfo {
    serde_json::from_str(payload).expect("vertex info payload")
}

fn top_face_id(entity: &OGFreeformGeometry) -> u32 {
    let mut best_face = 0;
    let mut best_y = f64::NEG_INFINITY;

    for face in &entity.local_brep.faces {
        let info = parse_face_info(&entity.get_face_info(face.id).expect("face info"));
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

fn right_side_face_id(entity: &OGFreeformGeometry) -> u32 {
    entity
        .local_brep
        .faces
        .iter()
        .find_map(|face| {
            let info = parse_face_info(&entity.get_face_info(face.id).ok()?);
            if info.normal.x > 0.5 && info.normal.y.abs() < 1.0e-6 {
                Some(face.id)
            } else {
                None
            }
        })
        .expect("right side face")
}

fn opposite_vertical_face_edges(entity: &OGFreeformGeometry, face_id: u32) -> (u32, u32) {
    let info = parse_face_info(&entity.get_face_info(face_id).expect("face info"));
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

#[test]
fn extrude_face_on_open_surface_creates_watertight_shell_and_created_ids() {
    let mut entity = create_freeform_polygon_entity("entity-polygon-extrude");

    let result = parse_edit_result(
        &entity
            .extrude_face(0, 1.0, None)
            .expect("extrude face should return payload"),
    );

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
    let mut cuboid = OGCuboid::new("cuboid-extrude-solid".to_string());
    cuboid
        .set_config(Vector3::new(0.0, 0.0, 0.0), 2.0, 2.0, 2.0)
        .expect("cuboid config");

    let mut entity = OGFreeformGeometry::new(
        "entity-cuboid-extrude-solid".to_string(),
        cuboid.get_local_brep_serialized(),
    )
    .expect("freeform entity");

    let face_id = top_face_id(&entity);

    let result = parse_edit_result(
        &entity
            .extrude_face(face_id, 0.5, None)
            .expect("extrude face payload"),
    );

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
    let mut entity = create_freeform_polygon_entity("entity-topology-edits");

    let split = parse_edit_result(&entity.split_edge(0, 0.5, None).expect("split edge payload"));

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

    let removed = parse_edit_result(
        &entity
            .remove_vertex(inserted_vertex_id, None)
            .expect("remove vertex payload"),
    );

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
    let mut first = create_freeform_polygon_entity("entity-determinism-a");
    let mut second = create_freeform_polygon_entity("entity-determinism-b");

    let first_split =
        parse_edit_result(&first.split_edge(1, 0.3, None).expect("first split payload"));
    let second_split = parse_edit_result(
        &second
            .split_edge(1, 0.3, None)
            .expect("second split payload"),
    );

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

    let first_remove = parse_edit_result(
        &first
            .remove_vertex(first_inserted, None)
            .expect("first remove payload"),
    );
    let second_remove = parse_edit_result(
        &second
            .remove_vertex(second_inserted, None)
            .expect("second remove payload"),
    );

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
    let mut cuboid = OGCuboid::new("cuboid-unsupported".to_string());
    cuboid
        .set_config(Vector3::new(0.0, 0.0, 0.0), 2.0, 2.0, 2.0)
        .expect("cuboid config");

    let mut entity = OGFreeformGeometry::new(
        "entity-cuboid-unsupported".to_string(),
        cuboid.get_local_brep_serialized(),
    )
    .expect("freeform entity");

    let result = parse_edit_result(
        &entity
            .remove_vertex(0, None)
            .expect("remove vertex payload"),
    );

    assert!(!result.validity.ok);
    assert!(result
        .validity
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "unsupported_topology"));
}

#[test]
fn constraints_project_move_vertex_translation_to_axis() {
    let mut cuboid = OGCuboid::new("cuboid-constraints".to_string());
    cuboid
        .set_config(Vector3::new(0.0, 0.0, 0.0), 2.0, 2.0, 2.0)
        .expect("cuboid config");

    let mut entity = OGFreeformGeometry::new(
        "entity-cuboid-constraints".to_string(),
        cuboid.get_local_brep_serialized(),
    )
    .expect("freeform entity");

    let before = parse_vertex_info(&entity.get_vertex_info(0).expect("vertex info before"));

    let options = r#"{
      "constraintAxis": { "x": 0.0, "y": 1.0, "z": 0.0 },
      "preserveCoplanarity": true
    }"#;

    let result = parse_edit_result(
        &entity
            .move_vertex(0, Vector3::new(1.0, 0.2, 3.0), Some(options.to_string()))
            .expect("move vertex payload"),
    );

    assert!(result.validity.ok);

    let after = parse_vertex_info(&entity.get_vertex_info(0).expect("vertex info after"));

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
    let mut entity = create_freeform_polygon_entity("entity-lean-defaults");

    let result = parse_edit_result(
        &entity
            .move_vertex(0, Vector3::new(0.25, 0.0, 0.0), None)
            .expect("move vertex payload"),
    );

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
    let entity = create_freeform_polygon_entity("entity-capabilities");

    let entity_caps: EditCapabilities = serde_json::from_str(
        &entity
            .get_edit_capabilities()
            .expect("entity capabilities payload"),
    )
    .expect("entity capabilities json");

    assert!(entity_caps.can_insert_vertex_on_edge);
    assert!(entity_caps.can_split_edge);
    assert!(entity_caps.can_remove_vertex);
    assert!(entity_caps.can_cut_face);

    let face_caps: FeatureEditCapabilities = serde_json::from_str(
        &entity
            .get_face_edit_capabilities(0)
            .expect("face capabilities payload"),
    )
    .expect("face capabilities json");

    assert!(face_caps.can_extrude_face);
    assert!(face_caps.can_move_face);
    assert!(face_caps.can_cut_face);
}

#[test]
fn converted_cuboid_supports_edge_split_and_returns_created_vertex() {
    let mut cuboid = OGCuboid::new("cuboid-edge-split".to_string());
    cuboid
        .set_config(Vector3::new(0.0, 0.0, 0.0), 2.0, 2.0, 2.0)
        .expect("cuboid config");

    let mut entity = OGFreeformGeometry::new(
        "entity-cuboid-edge-split".to_string(),
        cuboid.get_local_brep_serialized(),
    )
    .expect("freeform entity");

    let edge_caps: FeatureEditCapabilities = serde_json::from_str(
        &entity
            .get_edge_edit_capabilities(0)
            .expect("edge capabilities payload"),
    )
    .expect("edge capabilities json");
    assert!(edge_caps.can_split_edge);

    let result = parse_edit_result(&entity.split_edge(0, 0.5, None).expect("split edge payload"));

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
    let edge_caps: FeatureEditCapabilities = serde_json::from_str(
        &entity
            .get_edge_edit_capabilities(edge_id)
            .expect("edge capabilities payload"),
    )
    .expect("edge capabilities json");
    assert!(edge_caps.can_loop_cut);

    let result = parse_edit_result(
        &entity
            .loop_cut(edge_id, 0.5, None)
            .expect("loop cut payload"),
    );

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
    let mut cuboid = OGCuboid::new("cuboid-cut-face".to_string());
    cuboid
        .set_config(Vector3::new(0.0, 0.0, 0.0), 2.0, 2.0, 2.0)
        .expect("cuboid config");

    let mut entity = OGFreeformGeometry::new(
        "entity-cuboid-cut-face".to_string(),
        cuboid.get_local_brep_serialized(),
    )
    .expect("freeform entity");

    let face_id = right_side_face_id(&entity);
    let face_caps: FeatureEditCapabilities = serde_json::from_str(
        &entity
            .get_face_edit_capabilities(face_id)
            .expect("face capabilities payload"),
    )
    .expect("face capabilities json");
    assert!(face_caps.can_cut_face);

    let (start_edge_id, end_edge_id) = opposite_vertical_face_edges(&entity, face_id);
    let result = parse_edit_result(
        &entity
            .cut_face(face_id, start_edge_id, 0.5, end_edge_id, 0.5, None)
            .expect("cut face payload"),
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
    let original_face_info = parse_face_info(&entity.get_face_info(face_id).expect("face info"));
    let created_face_info = parse_face_info(
        &entity
            .get_face_info(created_face_id)
            .expect("created face info"),
    );
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

#[test]
fn freeform_entity_can_be_created_from_all_native_local_breps() {
    let mut line = OGLine::new("line-source".to_string());
    line.set_config(Vector3::new(-1.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0))
        .expect("line config");

    let mut polyline = OGPolyline::new("polyline-source".to_string());
    polyline
        .set_config(vec![
            Vector3::new(-1.0, 0.0, -1.0),
            Vector3::new(0.0, 0.0, 0.5),
            Vector3::new(1.0, 0.0, 0.0),
        ])
        .expect("polyline config");

    let mut arc = OGArc::new("arc-source".to_string());
    arc.set_config(
        Vector3::new(0.0, 0.0, 0.0),
        2.0,
        0.0,
        std::f64::consts::PI,
        24,
    )
    .expect("arc config");

    let mut curve = OGCurve::new("curve-source".to_string());
    curve
        .set_config(vec![
            Vector3::new(-1.0, 0.0, 0.0),
            Vector3::new(-0.2, 0.6, 0.2),
            Vector3::new(0.5, 0.3, 0.8),
            Vector3::new(1.0, 0.0, 0.0),
        ])
        .expect("curve config");

    let mut rectangle = OGRectangle::new("rectangle-source".to_string());
    rectangle
        .set_config(Vector3::new(0.0, 0.0, 0.0), 3.0, 2.0)
        .expect("rectangle config");

    let mut polygon = OGPolygon::new("polygon-source".to_string());
    polygon
        .set_config(vec![
            Vector3::new(-1.0, 0.0, -1.0),
            Vector3::new(1.0, 0.0, -1.0),
            Vector3::new(1.5, 0.0, 0.5),
            Vector3::new(-1.0, 0.0, 1.0),
        ])
        .expect("polygon config");

    let mut cuboid = OGCuboid::new("cuboid-source".to_string());
    cuboid
        .set_config(Vector3::new(0.0, 0.0, 0.0), 2.0, 3.0, 4.0)
        .expect("cuboid config");

    let mut cylinder = OGCylinder::new("cylinder-source".to_string());
    cylinder
        .set_config(
            Vector3::new(0.0, 0.0, 0.0),
            1.0,
            2.0,
            std::f64::consts::TAU,
            24,
        )
        .expect("cylinder config");

    let mut sphere = OGSphere::new("sphere-source".to_string());
    sphere
        .set_config(Vector3::new(0.0, 0.0, 0.0), 1.25, 16, 12)
        .expect("sphere config");

    let mut wedge = OGWedge::new("wedge-source".to_string());
    wedge
        .set_config(Vector3::new(0.0, 0.0, 0.0), 2.0, 1.5, 1.0)
        .expect("wedge config");

    let mut sweep = OGSweep::new("sweep-source".to_string());
    sweep
        .set_config(
            vec![
                Vector3::new(-1.0, 0.0, 0.0),
                Vector3::new(0.0, 0.8, 0.3),
                Vector3::new(1.0, 1.2, 0.0),
            ],
            vec![
                Vector3::new(-0.2, 0.0, -0.2),
                Vector3::new(0.2, 0.0, -0.2),
                Vector3::new(0.2, 0.0, 0.2),
                Vector3::new(-0.2, 0.0, 0.2),
            ],
        )
        .expect("sweep config");

    let sources = vec![
        ("line", line.get_local_brep_serialized()),
        ("polyline", polyline.get_local_brep_serialized()),
        ("arc", arc.get_local_brep_serialized()),
        ("curve", curve.get_local_brep_serialized()),
        ("rectangle", rectangle.get_local_brep_serialized()),
        ("polygon", polygon.get_local_brep_serialized()),
        ("cuboid", cuboid.get_local_brep_serialized()),
        ("cylinder", cylinder.get_local_brep_serialized()),
        ("sphere", sphere.get_local_brep_serialized()),
        ("wedge", wedge.get_local_brep_serialized()),
        ("sweep", sweep.get_local_brep_serialized()),
    ];

    for (label, local_brep) in sources {
        let entity = OGFreeformGeometry::new(format!("freeform-{}", label), local_brep)
            .unwrap_or_else(|_| panic!("{} should convert to freeform", label));

        assert!(!entity.get_local_brep_serialized().is_empty());
    }
}
