#![cfg(not(target_arch = "wasm32"))]
use opengeometry::brep::Accuracy;
use opengeometry::operations::CreatingOperation;
use opengeometry::query::classify_point;
use opengeometry::tessellation::tessellate;
use opengeometry::world_graph::{CreateOptions, Primitive, StepOptions, WorldGraph};
use sha2::{Digest, Sha256};

#[test]
fn noncoplanar_rectangular_sweep_orientation() {
    let mut world = WorldGraph::new(Accuracy {
        geometric: 1e-8,
        intersection: 1e-9,
        tessellation: 0.01,
        exchange: 1e-6,
    })
    .unwrap();
    world
        .create_primitive(
            Primitive::Rectangle {
                width: 1.0,
                breadth: 1.0,
            },
            CreateOptions {
                og_id: Some("profile".into()),
                ..CreateOptions::default()
            },
        )
        .unwrap();
    world
        .create_primitive(
            Primitive::Polyline {
                points: vec![
                    [0.0, 0.0, 0.0],
                    [0.0, 3.0, 0.0],
                    [3.0, 3.0, 0.0],
                    [3.0, 3.0, 3.0],
                ],
                closed: false,
            },
            CreateOptions {
                og_id: Some("path".into()),
                ..CreateOptions::default()
            },
        )
        .unwrap();
    world
        .create_operation(
            CreatingOperation::Sweep {
                profile: "profile".into(),
                path: "path".into(),
            },
            CreateOptions {
                og_id: Some("solid".into()),
                ..CreateOptions::default()
            },
        )
        .unwrap();
    let body = world.brep("solid").unwrap();
    let mesh = tessellate(&body, 0.01, 2_000_000).unwrap();
    let mut hash = Sha256::new();
    hash.update(body.to_json().unwrap().as_bytes());
    for value in mesh.positions {
        hash.update(value.to_bits().to_le_bytes());
    }
    for value in mesh.normals {
        hash.update(value.to_bits().to_le_bytes());
    }
    for value in mesh.indices {
        hash.update(value.to_le_bytes());
    }
    for value in mesh.triangle_face_ids {
        hash.update(value.to_le_bytes());
    }
    for point in [[0.0, 1.0, 0.0], [0.0, 1.0, 2.0], [3.0, 3.0, 3.0]] {
        hash.update(format!("{:?}", classify_point(&body, point).unwrap()).as_bytes());
    }
    let (step, report) = world
        .export_step(&["solid".into()], &StepOptions::default())
        .unwrap();
    hash.update(step.as_bytes());
    hash.update(serde_json::to_vec(&report).unwrap());
    let actual = format!("{:x}", hash.finalize());
    let target = if cfg!(target_os = "macos") {
        format!("{}-apple-darwin", std::env::consts::ARCH)
    } else if cfg!(target_os = "linux") {
        format!("{}-unknown-linux-gnu", std::env::consts::ARCH)
    } else {
        format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS)
    };
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/golden")
        .join(format!("{target}.sweep-3d.sha256"));
    if std::env::var("OG_GOLDEN_UPDATE").ok().as_deref() == Some("1") {
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, format!("{actual}\n")).unwrap();
    }
    assert_eq!(std::fs::read_to_string(&file).unwrap().trim(), actual);
}
