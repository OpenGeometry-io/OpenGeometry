use opengeometry::brep::{BrepEnvelope, Frame3, Surface};
use opengeometry::operations::modifying::boolean::subtract_planar_cutters;
use opengeometry::tessellation::{
    display::{bucket_floor, deflection_bucket, display_buffers, static_bucket},
    tessellate, SnapshotStore,
};
use opengeometry::world_graph::{CreateOptions, Primitive, StepOptions, Transform};
use opengeometry_test_support::world_graph::{graph, named};

const PLAN_VIEW_BUCKET: f64 = 0.0078125;

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

fn cylinder_brep(radius: f64, height: f64) -> BrepEnvelope {
    let mut graph = graph();
    graph
        .create_primitive(Primitive::Cylinder { radius, height }, named("leg"))
        .unwrap();
    graph.brep("leg").unwrap().as_ref().clone()
}

fn circle_segments(brep: &BrepEnvelope, bucket: f64) -> Vec<usize> {
    let buffers = display_buffers(brep, bucket, 2_000_000).unwrap();
    let value: serde_json::Value = serde_json::from_str(&brep.to_json().unwrap()).unwrap();
    let curves = value["geometry"]["curves"].as_array().unwrap();
    value["topology"]["edges"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|edge| {
            edge["geometry"]["kind"] == "Curve"
                && curves[edge["geometry"]["curve"].as_u64().unwrap() as usize]["kind"] == "Circle"
        })
        .map(|edge| {
            let id = edge["id"].as_u64().unwrap() as u32;
            buffers
                .edge_ids
                .iter()
                .filter(|other| **other == id)
                .count()
        })
        .collect()
}

#[test]
fn a_thin_cylinder_keeps_at_least_twelve_cap_segments_when_the_bucket_coarsens() {
    let brep = cylinder_brep(0.011, 0.45);
    let bucket = static_bucket(&brep).unwrap();
    assert_eq!(circle_segments(&brep, bucket), [18, 18]);
    assert_eq!(circle_segments(&brep, 64.0 * bucket), [12, 12]);
    assert_eq!(circle_segments(&brep, PLAN_VIEW_BUCKET), [12, 12]);
}

#[test]
fn full_turn_cylinder_edges_have_even_segment_counts_at_rest_and_in_plan_view() {
    for (radius, at_rest, plan_view) in [
        (0.011, 18, 12),
        (0.015, 22, 12),
        (0.02, 24, 12),
        (0.1, 38, 14),
        (0.19, 52, 18),
    ] {
        let brep = cylinder_brep(radius, 0.42);
        let bucket = static_bucket(&brep).unwrap();
        assert_eq!(circle_segments(&brep, bucket), [at_rest; 2], "{radius}");
        assert_eq!(
            circle_segments(&brep, PLAN_VIEW_BUCKET),
            [plan_view; 2],
            "{radius}"
        );
    }
}

fn distinct(pairs: impl Iterator<Item = [f64; 2]>) -> usize {
    pairs
        .map(|pair| pair.map(|value| (value * 1e6).round() as i64))
        .collect::<std::collections::BTreeSet<_>>()
        .len()
}

fn grid_points(brep: &BrepEnvelope, bucket: f64) -> Vec<[f64; 3]> {
    tessellate(brep, 0.75 * bucket, 2_000_000)
        .unwrap()
        .positions
        .chunks_exact(3)
        .map(|point| [point[0], point[1], point[2]])
        .collect()
}

fn around(point: &[f64; 3]) -> [f64; 2] {
    let distance = point[0].hypot(point[1]);
    [point[0] / distance, point[1] / distance]
}

fn torus_grid(bucket: Option<f64>) -> [usize; 2] {
    let (major, minor) = (0.1, 0.02);
    let brep = opengeometry::primitives::torus(
        "ring".into(),
        Frame3::IDENTITY,
        major,
        minor,
        graph().accuracy(),
    )
    .unwrap();
    let bucket = bucket.unwrap_or_else(|| static_bucket(&brep).unwrap());
    let points = grid_points(&brep, bucket);
    let across = |point: &[f64; 3]| [(point[0].hypot(point[1]) - major) / minor, point[2] / minor];
    [
        distinct(points.iter().map(around)),
        distinct(points.iter().map(across)),
    ]
}

#[test]
fn torus_grid_counts_are_even_and_at_least_twelve_in_both_directions() {
    assert_eq!(torus_grid(None), [82, 34]);
    assert_eq!(torus_grid(Some(0.125)), [12, 12]);
}

fn worst_deviation(brep: &BrepEnvelope, deflection: f64) -> f64 {
    let mesh = tessellate(brep, deflection, 2_000_000).unwrap();
    let corner = |index: u32| -> [f64; 3] {
        std::array::from_fn(|axis| mesh.positions[index as usize * 3 + axis])
    };
    let mut worst = 0.0_f64;
    for (triangle, ids) in mesh.indices.chunks_exact(3).enumerate() {
        let face = &brep.topology.faces[mesh.triangle_face_ids[triangle] as usize];
        let surface = brep.geometry.surface(face.surface).unwrap();
        let corners = [corner(ids[0]), corner(ids[1]), corner(ids[2])];
        for i in 0..=8_usize {
            for j in 0..=8 - i {
                let weights = [i, j, 8 - i - j];
                if weights.contains(&8) {
                    continue;
                }
                let point: [f64; 3] = std::array::from_fn(|axis| {
                    (0..3)
                        .map(|k| weights[k] as f64 * corners[k][axis])
                        .sum::<f64>()
                        / 8.0
                });
                let nearest = surface
                    .point_at(surface.project(point, None).unwrap())
                    .unwrap();
                let gap: [f64; 3] = std::array::from_fn(|axis| point[axis] - nearest[axis]);
                worst = worst.max(gap[0].hypot(gap[1]).hypot(gap[2]));
            }
        }
    }
    worst
}

#[test]
fn a_sphere_and_a_torus_at_the_static_bucket_stay_within_half_the_deflection_in_few_triangles() {
    let accuracy = graph().accuracy();
    let sphere =
        opengeometry::primitives::sphere("ball".into(), Frame3::IDENTITY, 0.5, accuracy).unwrap();
    let torus =
        opengeometry::primitives::torus("ring".into(), Frame3::IDENTITY, 1.0, 0.25, accuracy)
            .unwrap();
    for (brep, ceiling) in [(sphere, 8_000), (torus, 9_000)] {
        let bucket = static_bucket(&brep).unwrap();
        let triangles = display_buffers(&brep, bucket, 2_000_000).unwrap().triangles;
        let deflection = 0.75 * bucket;
        let worst = worst_deviation(&brep, deflection);
        assert!(
            worst <= 0.5 * deflection,
            "{worst} above half of {deflection}"
        );
        assert!(triangles <= ceiling, "{triangles} above {ceiling}");
    }
}

#[test]
fn a_small_sphere_keeps_twelve_longitudes_and_six_latitude_bands() {
    let radius = 0.01;
    let brep = opengeometry::primitives::sphere(
        "ball".into(),
        Frame3::IDENTITY,
        radius,
        graph().accuracy(),
    )
    .unwrap();
    let points = grid_points(&brep, 0.0625);
    let latitudes = distinct(points.iter().map(|point| [point[2] / radius, 0.0]));
    let longitudes = distinct(
        points
            .iter()
            .filter(|point| point[0].hypot(point[1]) > radius * 1e-3)
            .map(around),
    );
    assert_eq!([longitudes, latitudes - 1], [12, 6]);
}

fn wall_with_round_hole() -> BrepEnvelope {
    let accuracy = graph().accuracy();
    let outer = vec![[0.0, 0.0], [6.0, 0.0], [6.0, 0.3], [0.0, 0.3]];
    let wall = opengeometry::primitives::linear_extrusion(
        "wall".into(),
        Frame3::IDENTITY,
        outer,
        Vec::new(),
        3.0,
        accuracy,
    )
    .unwrap();
    let round = |name: &str, x: f64| {
        let ground = Frame3 {
            origin: [x, 0.0, 1.5],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 0.0, -1.0],
            z: [0.0, 1.0, 0.0],
        };
        opengeometry::primitives::cylinder(name.into(), ground, 0.4, 0.3, accuracy).unwrap()
    };
    let rounds = [round("round-a", 1.5), round("round-b", 4.5)];
    subtract_planar_cutters(&wall, &rounds, "holed".into())
        .unwrap()
        .brep
}

#[test]
fn a_padded_half_cylinder_hole_face_has_one_column_per_boundary_arc_segment() {
    let brep = wall_with_round_hole();
    let mesh = tessellate(&brep, 0.05, 2_000_000).unwrap();
    let value: serde_json::Value = serde_json::from_str(&brep.to_json().unwrap()).unwrap();
    let surfaces = value["geometry"]["surfaces"].as_array().unwrap();
    let curves = value["geometry"]["curves"].as_array().unwrap();
    let edges = value["topology"]["edges"].as_array().unwrap();
    let mut checked = 0;
    for face in value["topology"]["faces"].as_array().unwrap() {
        let surface = &surfaces[face["surface"].as_u64().unwrap() as usize];
        if surface["kind"] != "Cylinder" {
            continue;
        }
        let id = face["id"].as_u64().unwrap() as u32;
        let width = face["trim"]["uv_bounds"][0]["hi"].as_f64().unwrap()
            - face["trim"]["uv_bounds"][0]["lo"].as_f64().unwrap();
        assert!(width > std::f64::consts::PI, "{width}");
        let origin = surface["frame"]["origin"].as_array().unwrap();
        let centre = [0, 2].map(|axis| origin[axis].as_f64().unwrap());
        let columns = distinct(
            mesh.indices
                .chunks_exact(3)
                .zip(&mesh.triangle_face_ids)
                .filter(|(_, owner)| **owner == id)
                .flat_map(|(triangle, _)| triangle.iter())
                .map(|vertex| {
                    let point = &mesh.positions[3 * *vertex as usize..];
                    around(&[point[0] - centre[0], point[2] - centre[1], 0.0])
                }),
        );
        let arcs = value["topology"]["halfedges"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|halfedge| halfedge["face"].as_u64() == Some(id as u64))
            .map(|halfedge| &edges[halfedge["edge"].as_u64().unwrap() as usize])
            .filter(|edge| {
                edge["geometry"]["kind"] == "Curve"
                    && curves[edge["geometry"]["curve"].as_u64().unwrap() as usize]["kind"]
                        == "Circle"
            })
            .map(|edge| {
                let edge = edge["id"].as_u64().unwrap() as u32;
                mesh.outline_edge_ids
                    .iter()
                    .filter(|id| **id == edge)
                    .count()
            })
            .collect::<Vec<_>>();
        assert_eq!(arcs.len(), 2, "face {id}");
        assert_eq!([columns - 1; 2], [arcs[0], arcs[1]], "face {id}");
        checked += 1;
    }
    assert_eq!(checked, 4);
}
