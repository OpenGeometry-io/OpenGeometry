use opengeometry::tessellation::{
    display::{bucket_floor, deflection_bucket, display_buffers, static_bucket},
    tessellate, SnapshotStore,
};
use opengeometry::world_graph::{CreateOptions, Primitive, StepOptions, Transform};
use opengeometry_test_support::world_graph::graph;

#[test]
fn i1_brep_remains_canonical_when_display_uses_snapshot_buffers() {
    let mut graph = graph();
    graph
        .create_primitive(
            Primitive::Cuboid {
                width: 1.0,
                height: 1.0,
                depth: 1.0,
            },
            CreateOptions {
                og_id: Some("cube".into()),
                ..CreateOptions::default()
            },
        )
        .unwrap();
    let shape_id = graph.node("cube").unwrap().shape.clone().unwrap();
    let brep_before = graph.brep("cube").unwrap().to_json().unwrap();
    let step_before = graph
        .export_step(&["cube".into()], &StepOptions::default())
        .unwrap()
        .0;
    let mut tessellator = SnapshotStore::new();
    let slot = tessellator
        .load(&graph.snapshot(&shape_id).unwrap())
        .unwrap();
    for bucket in [0.01, 0.02] {
        let direct = graph.buffers(&shape_id, bucket, 2_000_000).unwrap();
        let loaded = tessellator.buffers(slot, bucket, 2_000_000).unwrap();
        assert_eq!(direct, loaded);
        assert_eq!(direct.triangles, 12);
        assert_eq!(direct.face_ranges.len(), 6 * 3);
        assert_eq!(direct.edge_ids.len(), 12);
        assert!(direct.achieved_deflection <= bucket);
        assert!(direct.positions.iter().all(|value| value.abs() <= 1.0));
        let source = tessellate(&graph.brep("cube").unwrap(), 0.75 * bucket, 2_000_000).unwrap();
        for range in direct.face_ranges.chunks_exact(3) {
            let (face, first, count) = (range[0], range[1] as usize, range[2] as usize);
            assert!(source.triangle_face_ids[first..first + count]
                .iter()
                .all(|id| *id == face));
        }
        assert_eq!(direct.edge_ids, source.outline_edge_ids);
    }
    assert_eq!(graph.brep("cube").unwrap().to_json().unwrap(), brep_before);
    assert_eq!(
        graph
            .export_step(&["cube".into()], &StepOptions::default())
            .unwrap()
            .0,
        step_before
    );
    assert!(tessellator.release(slot));
    assert!(tessellator.buffers(slot, 0.01, 1000).is_err());
}

#[test]
fn distant_placement_does_not_enter_local_display_coordinates() {
    let mut graph = graph();
    graph
        .create_primitive(
            Primitive::Cuboid {
                width: 1.0,
                height: 1.0,
                depth: 1.0,
            },
            CreateOptions {
                og_id: Some("cube".into()),
                ..CreateOptions::default()
            },
        )
        .unwrap();
    graph
        .transform(
            "cube",
            Transform::Translate {
                offset: [500_000.0, 0.0, -5_000_000.0],
            },
        )
        .unwrap();
    let bounds = graph.bounds("cube").unwrap().unwrap();
    assert!(bounds[0] <= 499_999.5 && bounds[3] >= 500_000.5);
    assert!(bounds[2] <= -5_000_000.5 && bounds[5] >= -4_999_999.5);
    let shape_id = graph.node("cube").unwrap().shape.clone().unwrap();
    let buffers = graph.buffers(&shape_id, 0.01, 1000).unwrap();
    assert!(buffers.positions.iter().all(|value| value.abs() <= 1.0));
    assert_eq!(buffers.origin, [0.0, 0.5, 0.0]);
}

#[test]
fn i4_display_buckets_leave_brep_and_step_unchanged_and_enforce_limits() {
    let graph = graph();
    let brep = opengeometry::primitives::cuboid(
        "cube".into(),
        opengeometry::brep::Frame3::IDENTITY,
        [1.0; 3],
        graph.accuracy(),
    )
    .unwrap();
    let canonical = brep.to_json().unwrap();
    let step = opengeometry::exchange::export_step(&brep, "metre")
        .unwrap()
        .0;
    let floor = bucket_floor(&brep).unwrap();
    assert!(floor > 0.0 && floor.log2().fract() == 0.0);
    assert!(static_bucket(&brep).unwrap() >= floor);
    assert_eq!(deflection_bucket(0.03).unwrap(), 0.015625);
    assert!(display_buffers(&brep, floor / 2.0, 1000).is_err());
    assert!(display_buffers(&brep, 0.01, 0).is_err());
    assert!(display_buffers(&brep, 0.01, 2_000_001).is_err());
    let mut tessellator = SnapshotStore::new();
    assert!(tessellator.load(&vec![0; 64 * 1024 * 1024 + 1]).is_err());
    assert_eq!(brep.to_json().unwrap(), canonical);
    assert_eq!(
        opengeometry::exchange::export_step(&brep, "metre")
            .unwrap()
            .0,
        step
    );
}
