// benches/registry_benchmark.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use opengeometry::scenegraph::OGEntityRegistry;
use opengeometry::export::projection::CameraParameters;
use opengeometry::brep::{Brep, BrepBuilder};
use openmaths::Vector3;
use uuid::Uuid;
use serde_json;

fn create_cube_brep(size: f64) -> Brep {
    let h = size / 2.0;
    let vertices = vec![
        Vector3::new(-h, -h, -h),
        Vector3::new( h, -h, -h),
        Vector3::new( h,  h, -h),
        Vector3::new(-h,  h, -h),
        Vector3::new(-h, -h,  h),
        Vector3::new( h, -h,  h),
        Vector3::new( h,  h,  h),
        Vector3::new(-h,  h,  h),
    ];
    
    let mut builder = BrepBuilder::new(Uuid::new_v4());
    builder.add_vertices(&vertices);
    
    let faces = vec![
        vec![0, 1, 2, 3],
        vec![4, 5, 6, 7],
        vec![0, 1, 5, 4],
        vec![2, 3, 7, 6],
        vec![0, 3, 7, 4],
        vec![1, 2, 6, 5],
    ];
    
    for face in faces {
        builder.add_wire(&face, true).unwrap();
    }
    
    builder.build().unwrap()
}

fn benchmark_batch_projection(c: &mut Criterion) {
    let mut registry = OGEntityRegistry::new();
    
    // Регистрируем 100 сущностей
    for i in 0..100 {
        let brep = create_cube_brep(1.0 + (i % 5) as f64);
        let brep_json = serde_json::to_string(&brep).unwrap();
        registry.register_entity(
            format!("entity-{}", i),
            "wall".to_string(),
            brep_json,
        ).unwrap();
    }

    let camera = CameraParameters::default();
    let camera_json = serde_json::to_string(&camera).unwrap();
    
    // Создаем 10 видов
    let mut views = Vec::new();
    for i in 0..10 {
        views.push(format!(
            r#"{{"id":"view-{}","camera":{}}}"#,
            i, camera_json
        ));
    }
    let views_json = format!("[{}]", views.join(","));

    c.bench_function("batch_projection_100_entities_10_views", |b| {
        b.iter(|| {
            let result = registry.project_current_to_views(black_box(views_json.clone())).unwrap();
            black_box(result)
        })
    });
}

criterion_group!(benches, benchmark_batch_projection);
criterion_main!(benches);
