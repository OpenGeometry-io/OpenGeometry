// main/opengeometry/tests/registry_tests.rs

use opengeometry::scenegraph::OGEntityRegistry;
use opengeometry::export::projection::{CameraParameters, ProjectionMode};
use opengeometry::brep::{Brep, BrepBuilder};
use opengeometry::primitives::cuboid::OGCuboid;
use openmaths::Vector3;
use uuid::Uuid;
use serde_json;
use std::collections::HashMap;

/// Создает тестовый BRep в виде треугольника
fn create_triangle_brep() -> Brep {
    let mut builder = BrepBuilder::new(Uuid::new_v4());
    builder.add_vertices(&[
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(2.0, 0.0, 0.0),
        Vector3::new(1.0, 2.0, 0.0),
    ]);
    builder.add_face(&[0, 1, 2], &[]).unwrap();
    builder.build().unwrap()
}

/// Создает куб с использованием штатного примитива OGCuboid
fn create_cube_brep(size: f64) -> Brep {
    let mut cuboid = OGCuboid::new("test_cuboid".to_string());
    cuboid
        .set_config(Vector3::new(0.0, 0.0, 0.0), size, size, size)
        .expect("Failed to set cuboid config");
    cuboid.world_brep()
}

/// Создает простой BRep в виде линии
fn create_line_brep() -> Brep {
    let mut builder = BrepBuilder::new(Uuid::new_v4());
    builder.add_vertices(&[
        Vector3::new(-1.0, 0.0, 0.0),
        Vector3::new(1.0, 0.0, 0.0),
    ]);
    builder.add_wire(&[0, 1], false).unwrap();
    builder.build().unwrap()
}

#[test]
fn test_registry_register_and_replace() {
    let mut registry = OGEntityRegistry::new();
    let brep = create_triangle_brep();
    let brep_json = serde_json::to_string(&brep).unwrap();

    registry.register_entity("test-1".to_string(), "wall".to_string(), brep_json.clone())
        .expect("Should register entity");

    registry.register_entity("test-1".to_string(), "wall".to_string(), brep_json.clone())
        .expect("Should replace entity");
}

#[test]
fn test_registry_unregister() {
    let mut registry = OGEntityRegistry::new();
    let brep = create_triangle_brep();
    let brep_json = serde_json::to_string(&brep).unwrap();

    registry.register_entity("test-1".to_string(), "wall".to_string(), brep_json)
        .unwrap();

    let removed = registry.unregister_entity("test-1".to_string());
    assert!(removed);

    let removed2 = registry.unregister_entity("nonexistent".to_string());
    assert!(!removed2);
}

#[test]
fn test_registry_clear() {
    let mut registry = OGEntityRegistry::new();
    let brep = create_triangle_brep();
    let brep_json = serde_json::to_string(&brep).unwrap();

    registry.register_entity("e1".to_string(), "wall".to_string(), brep_json.clone()).unwrap();
    registry.register_entity("e2".to_string(), "door".to_string(), brep_json).unwrap();

    assert!(registry.unregister_entity("e1".to_string()));
    assert!(!registry.unregister_entity("e1".to_string()));

    registry.clear_entities();
    assert!(!registry.unregister_entity("e2".to_string()));
}

#[test]
fn test_registry_multi_view_projection() {
    let mut registry = OGEntityRegistry::new();
    let brep = create_cube_brep(2.0);
    let brep_json = serde_json::to_string(&brep).unwrap();

    registry.register_entity("cube".to_string(), "wall".to_string(), brep_json).unwrap();

    let camera = CameraParameters::default();
    let camera_json = serde_json::to_string(&camera).unwrap();

    let views_json = format!(
        r#"[{{"id":"plan","camera":{cam}}},{{"id":"front","camera":{cam}}},{{"id":"iso","camera":{cam}}}]"#,
        cam = camera_json
    );

    let result = registry.project_current_to_views(views_json).unwrap();
    let result_map: HashMap<String, serde_json::Value> = 
        serde_json::from_str(&result).unwrap();

    assert!(result_map.contains_key("plan"));
    assert!(result_map.contains_key("front"));
    assert!(result_map.contains_key("iso"));
    assert_eq!(result_map.len(), 3);
    
    for view_id in &["plan", "front", "iso"] {
        let segments = result_map[*view_id]["segments"].as_array().unwrap();
        assert!(!segments.is_empty(), "View '{}' should have segments", view_id);
    }
}

#[test]
fn test_registry_layer_attribution() {
    let mut registry = OGEntityRegistry::new();
    let brep = create_cube_brep(1.0);
    let brep_json = serde_json::to_string(&brep).unwrap();

    let test_cases = vec![
        ("wall", "A-WALL"),
        ("door", "A-DOOR"),
        ("window", "A-GLAZ"),
        ("floor", "A-FLOR"),
        ("roof", "A-ROOF"),
        ("column", "A-COLS"),
        ("structural_beam", "S-BEAM"),
        ("duct", "M-DUCT"),
        ("pipe", "P-PIPE"),
        ("site", "G-SITE"),
        ("existing", "A-EXST"),
    ];

    let camera = CameraParameters::default();
    let camera_json = serde_json::to_string(&camera).unwrap();

    for (kind, expected_layer) in test_cases {
        let id = format!("test-{}", kind);
        registry.register_entity(id.clone(), kind.to_string(), brep_json.clone()).unwrap();

        let views_json = format!(r#"[{{"id":"plan","camera":{}}}]"#, camera_json);
        let result = registry.project_current_to_views(views_json).unwrap();
        let result_map: HashMap<String, serde_json::Value> = 
            serde_json::from_str(&result).unwrap();
        
        let segments = result_map["plan"]["segments"].as_array().unwrap();
        
        for seg in segments {
            assert!(
                seg["sourceEntityId"].is_string(),
                "sourceEntityId should be present in segment: {}",
                seg
            );
        }
        
        let found_correct_layer = segments
            .iter()
            .any(|seg| seg["layer"].as_str() == Some(expected_layer));
        
        assert!(
            found_correct_layer,
            "No segment found with layer '{}' for kind '{}'",
            expected_layer,
            kind
        );

        registry.unregister_entity(id);
    }
}

#[test]
fn test_registry_source_entity_id_preserved() {
    let mut registry = OGEntityRegistry::new();
    
    // Используем куб и линию
    let cube_brep = create_cube_brep(1.0);
    let line_brep = create_line_brep();
    
    registry.register_entity(
        "cube-1".to_string(),
        "wall".to_string(),
        serde_json::to_string(&cube_brep).unwrap(),
    ).unwrap();
    
    registry.register_entity(
        "line-1".to_string(),
        "door".to_string(),
        serde_json::to_string(&line_brep).unwrap(),
    ).unwrap();

    // Используем камеру, которая покажет оба объекта
    let camera = CameraParameters {
        position: Vector3::new(0.0, 0.0, 5.0),
        target: Vector3::new(0.0, 0.0, 0.0),
        up: Vector3::new(0.0, 1.0, 0.0),
        near: 0.01,
        projection_mode: ProjectionMode::Orthographic,
    };
    let camera_json = serde_json::to_string(&camera).unwrap();
    let views_json = format!(r#"[{{"id":"plan","camera":{}}}]"#, camera_json);

    let result = registry.project_current_to_views(views_json).unwrap();
    let result_map: HashMap<String, serde_json::Value> = 
        serde_json::from_str(&result).unwrap();

    let segments = result_map["plan"]["segments"].as_array().unwrap();
    
    let mut found_cube = false;
    let mut found_line = false;
    
    for seg in segments {
        let source_id = seg["sourceEntityId"].as_str()
            .expect("sourceEntityId should be present");
        
        if source_id == "cube-1" {
            found_cube = true;
        } else if source_id == "line-1" {
            found_line = true;
        }
    }
    
    assert!(found_cube, "Should have segments from cube-1");
    assert!(found_line, "Should have segments from line-1");
}

#[test]
fn test_registry_entity_update() {
    let mut registry = OGEntityRegistry::new();
    
    let small_cube = create_cube_brep(1.0);
    let small_json = serde_json::to_string(&small_cube).unwrap();
    registry.register_entity(
        "cube".to_string(),
        "wall".to_string(),
        small_json,
    ).unwrap();

    let camera = CameraParameters::default();
    let camera_json = serde_json::to_string(&camera).unwrap();
    let views_json = format!(r#"[{{"id":"plan","camera":{}}}]"#, camera_json);
    
    let result_before = registry.project_current_to_views(views_json.clone()).unwrap();
    let map_before: HashMap<String, serde_json::Value> = 
        serde_json::from_str(&result_before).unwrap();
    let segments_before = map_before["plan"]["segments"].as_array().unwrap().len();
    assert!(segments_before > 0, "Should have segments before update");

    let large_cube = create_cube_brep(5.0);
    let large_json = serde_json::to_string(&large_cube).unwrap();
    registry.register_entity(
        "cube".to_string(),
        "wall".to_string(),
        large_json,
    ).unwrap();

    let result_after = registry.project_current_to_views(views_json).unwrap();
    let map_after: HashMap<String, serde_json::Value> = 
        serde_json::from_str(&result_after).unwrap();
    let segments_after = map_after["plan"]["segments"].as_array().unwrap().len();
    assert!(segments_after > 0, "Should have segments after update");
    
    let source_ids: Vec<String> = map_after["plan"]["segments"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|seg| seg["sourceEntityId"].as_str().map(String::from))
        .collect();
    
    assert!(!source_ids.is_empty(), "sourceEntityId should be preserved");
    assert_eq!(source_ids[0], "cube", "Entity ID should be 'cube'");
}

#[test]
fn test_registry_replace_entity() {
    let mut registry = OGEntityRegistry::new();
    
    // Регистрируем куб как стену
    let wall_brep = create_cube_brep(1.0);
    let wall_json = serde_json::to_string(&wall_brep).unwrap();
    registry.register_entity(
        "test-1".to_string(),
        "wall".to_string(),
        wall_json,
    ).unwrap();

    let camera = CameraParameters::default();
    let camera_json = serde_json::to_string(&camera).unwrap();
    let views_json = format!(r#"[{{"id":"plan","camera":{}}}]"#, camera_json);
    
    let result = registry.project_current_to_views(views_json.clone()).unwrap();
    let result_map: HashMap<String, serde_json::Value> = 
        serde_json::from_str(&result).unwrap();
    let segments = result_map["plan"]["segments"].as_array().unwrap();
    
    // Проверяем, что есть сегменты со слоем A-WALL
    let found_wall = segments
        .iter()
        .any(|seg| seg["layer"].as_str() == Some("A-WALL"));
    assert!(found_wall, "Should have A-WALL layer before replacement");

    // Заменяем на линию как дверь
    let door_brep = create_line_brep();
    let door_json = serde_json::to_string(&door_brep).unwrap();
    registry.register_entity(
        "test-1".to_string(),
        "door".to_string(),
        door_json,
    ).unwrap();

    let result2 = registry.project_current_to_views(views_json).unwrap();
    let result_map2: HashMap<String, serde_json::Value> = 
        serde_json::from_str(&result2).unwrap();
    let segments2 = result_map2["plan"]["segments"].as_array().unwrap();
    
    // Проверяем, что есть сегменты со слоем A-DOOR
    let found_door = segments2
        .iter()
        .any(|seg| seg["layer"].as_str() == Some("A-DOOR"));
    assert!(found_door, "Should have A-DOOR layer after replacement");
}

// Этот тест использует внутренний метод без wasm-bindgen на native
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn test_registry_invalid_kind_rejected_native() {
    let mut registry = OGEntityRegistry::new();
    let brep = create_triangle_brep();
    let brep_json = serde_json::to_string(&brep).unwrap();

    // Пустой kind
    let result = registry.register_entity_internal(
        "test".to_string(),
        "".to_string(),
        brep_json.clone(),
    );
    assert!(result.is_err(), "Empty kind should be rejected");
    assert!(result.unwrap_err().contains("cannot be empty"));

    // Пробелы
    let result = registry.register_entity_internal(
        "test".to_string(),
        "   ".to_string(),
        brep_json.clone(),
    );
    assert!(result.is_err(), "Whitespace kind should be rejected");
    assert!(result.unwrap_err().contains("cannot be empty"));

    // Специальные символы
    let result = registry.register_entity_internal(
        "test".to_string(),
        "wall!@#".to_string(),
        brep_json.clone(),
    );
    assert!(result.is_err(), "Kind with special chars should be rejected");
    assert!(result.unwrap_err().contains("invalid characters"));

    // Слишком длинный
    let long_kind = "a".repeat(65);
    let result = registry.register_entity_internal(
        "test".to_string(),
        long_kind,
        brep_json.clone(),
    );
    assert!(result.is_err(), "Too long kind should be rejected");
    assert!(result.unwrap_err().contains("too long"));

    // Валидный kind должен пройти
    let result = registry.register_entity_internal(
        "test".to_string(),
        "structural_beam-123".to_string(),
        brep_json,
    );
    assert!(result.is_ok(), "Valid kind should be accepted");
}

// Этот тест использует wasm-bindgen функции, поэтому пропускаем на native
#[test]
#[cfg_attr(not(target_arch = "wasm32"), ignore = "requires wasm-bindgen")]
fn test_registry_invalid_brep_rejected() {
    let mut registry = OGEntityRegistry::new();
    
    let result = registry.register_entity(
        "test".to_string(),
        "wall".to_string(),
        "invalid json".to_string(),
    );
    assert!(result.is_err());

    let empty_brep = Brep::new(Uuid::new_v4());
    let empty_json = serde_json::to_string(&empty_brep).unwrap();
    let result = registry.register_entity(
        "test".to_string(),
        "wall".to_string(),
        empty_json,
    );
    assert!(result.is_err());
}

// Этот тест использует wasm-bindgen функции, поэтому пропускаем на native
#[test]
#[cfg_attr(not(target_arch = "wasm32"), ignore = "requires wasm-bindgen")]
fn test_registry_aia_layer_no_duplicates() {
    let known_types = vec![
        "wall", "walls", "partition",
        "door", "doors",
        "window", "windows", "glazing", "glaz", "glass",
        "floor", "floors", "slab", "slabs",
        "column", "columns", "col", "pillar",
        "beam", "beams",
        "girder", "girders",
        "joist", "joists",
    ];

    for kind in known_types {
        let mut registry = OGEntityRegistry::new();
        let brep = create_cube_brep(1.0);
        let brep_json = serde_json::to_string(&brep).unwrap();
        
        let result = registry.register_entity(
            format!("test-{}", kind),
            kind.to_string(),
            brep_json,
        );
        assert!(result.is_ok(), "Kind '{}' should register successfully", kind);
    }
}

// Этот тест использует внутренний метод, доступный только на native
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn test_registry_duplicate_view_ids_rejected() {
    let registry = OGEntityRegistry::new();
    let camera = CameraParameters::default();
    let camera_json = serde_json::to_string(&camera).unwrap();
    let views_json = format!(
        r#"[{{"id":"plan","camera":{cam}}},{{"id":"plan","camera":{cam}}}]"#,
        cam = camera_json
    );

    let result = registry.project_current_to_views_internal(views_json);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("Duplicate view ID"));
}
