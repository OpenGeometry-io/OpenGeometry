use opengeometry::brep::BrepEnvelope;
use opengeometry::primitives::{ProfileSegment, SegmentArc};
use opengeometry::tessellation::display::{deflection_bucket, display_buffers, static_bucket};
use opengeometry::world_graph::{
    CreatingOperation, EditScope, ModifyingOperation, Primitive, Transform, WorldGraph,
};
use opengeometry_test_support::world_graph::{graph, named};
use std::collections::HashMap;

const WELD: f64 = 1e-8;
const NEEDLE: f64 = 1e-6;

#[derive(Debug, Default, PartialEq)]
struct Faults {
    zero_area: usize,
    needles: usize,
    collapsed: usize,
    edges_not_shared_by_two: usize,
}

fn d_profile() -> Primitive {
    let line = |to: [f64; 2]| ProfileSegment { to, arc: None };
    Primitive::Profile {
        start: [0.2, 0.25],
        segments: vec![
            line([-0.2, 0.25]),
            line([-0.2, -0.05]),
            ProfileSegment {
                to: [0.2, -0.05],
                arc: Some(SegmentArc {
                    center: [0.0, -0.05],
                    counter_clockwise: true,
                }),
            },
            line([0.2, 0.25]),
        ],
        closed: true,
    }
}

fn d_sweep() -> WorldGraph {
    let mut world = graph();
    world
        .create_primitive(d_profile(), named("profile"))
        .unwrap();
    world
        .create_primitive(
            Primitive::Polyline {
                points: vec![[0.0, 0.0, 0.0], [0.0, 0.4, 0.0]],
                closed: false,
            },
            named("path"),
        )
        .unwrap();
    world
        .create_operation(
            CreatingOperation::Sweep {
                profile: "profile".into(),
                path: "path".into(),
                caps: None,
            },
            named("solid"),
        )
        .unwrap();
    world
}

fn welded(points: &[[f64; 3]]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..points.len()).collect();
    order.sort_by(|&a, &b| points[a][0].total_cmp(&points[b][0]));
    let mut ids: Vec<usize> = (0..points.len()).collect();
    for (rank, &a) in order.iter().enumerate() {
        for &b in &order[rank + 1..] {
            if points[b][0] - points[a][0] > WELD {
                break;
            }
            if distance(points[a], points[b]) <= WELD {
                ids[b] = ids[a].min(ids[b]);
            }
        }
    }
    ids
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn cross_length(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> f64 {
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let w = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    distance(
        [
            u[1] * w[2] - u[2] * w[1],
            u[2] * w[0] - u[0] * w[2],
            u[0] * w[1] - u[1] * w[0],
        ],
        [0.0; 3],
    )
}

fn faults(body: &BrepEnvelope, bucket: f64) -> Faults {
    let buffers = display_buffers(body, bucket, 2_000_000).unwrap();
    let points: Vec<[f64; 3]> = buffers
        .positions
        .chunks_exact(3)
        .map(|p| std::array::from_fn(|axis| f64::from(p[axis]) + buffers.origin[axis]))
        .collect();
    let ids = welded(&points);
    let mut faults = Faults::default();
    let mut uses = HashMap::<[usize; 2], usize>::new();
    for triangle in buffers.indices.chunks_exact(3) {
        let [a, b, c] = [0, 1, 2].map(|k| triangle[k] as usize);
        if cross_length(points[a], points[b], points[c]) < 1e-12 {
            faults.zero_area += 1;
        }
        let shortest = distance(points[a], points[b])
            .min(distance(points[b], points[c]))
            .min(distance(points[c], points[a]));
        if shortest < NEEDLE {
            faults.needles += 1;
        }
        let [a, b, c] = [ids[a], ids[b], ids[c]];
        if a == b || b == c || c == a {
            faults.collapsed += 1;
            continue;
        }
        for [from, to] in [[a, b], [b, c], [c, a]] {
            *uses.entry([from.min(to), from.max(to)]).or_default() += 1;
        }
    }
    faults.edges_not_shared_by_two = uses.values().filter(|&&count| count != 2).count();
    faults
}

fn assert_clean_walls(body: &BrepEnvelope) {
    for bucket in [
        static_bucket(body).unwrap(),
        deflection_bucket(0.002).unwrap(),
    ] {
        assert_eq!(faults(body, bucket), Faults::default(), "bucket {bucket}");
    }
}

#[test]
fn extruded_arc_wall_has_no_needles_and_every_edge_shared_by_two_triangles() {
    let mut world = graph();
    world
        .create_primitive(d_profile(), named("profile"))
        .unwrap();
    world
        .create_operation(
            CreatingOperation::Extrude {
                profile: "profile".into(),
                holes: Vec::new(),
                distance: 0.4,
            },
            named("solid"),
        )
        .unwrap();
    assert_clean_walls(&world.brep("solid").unwrap());
}

#[test]
fn swept_arc_wall_has_no_needles_and_every_edge_shared_by_two_triangles() {
    assert_clean_walls(&d_sweep().brep("solid").unwrap());
}

#[test]
fn shell_of_swept_arc_wall_has_no_needles_and_every_edge_shared_by_two_triangles() {
    let mut world = d_sweep();
    world.shell("solid", 0.02, EditScope::Node).unwrap();
    assert_clean_walls(&world.brep("solid").unwrap());
}

fn s_slab() -> WorldGraph {
    let theta = (5.0_f64 / 6.0).acos();
    let c1 = [4.0, 6.0];
    let angle = -std::f64::consts::FRAC_PI_2 + theta;
    let p1 = [c1[0] + 6.0 * angle.cos(), c1[1] + 6.0 * angle.sin()];
    let c2 = [p1[0] + 6.0 * theta.sin(), p1[1] - 6.0 * theta.cos()];
    let p2 = [c2[0], c2[1] + 6.0];
    let end = [p2[0] + 4.0, p2[1]];
    let line = |to: [f64; 2]| ProfileSegment { to, arc: None };
    let arc = |to: [f64; 2], center: [f64; 2], counter_clockwise: bool| ProfileSegment {
        to,
        arc: Some(SegmentArc {
            center,
            counter_clockwise,
        }),
    };
    let mut world = graph();
    world
        .create_primitive(
            Primitive::Profile {
                start: [0.0, 0.0],
                segments: vec![
                    line([4.0, 0.0]),
                    arc(p1, c1, true),
                    arc(p2, c2, false),
                    line(end),
                    line([end[0], 15.0]),
                    line([0.0, 15.0]),
                    line([0.0, 0.0]),
                ],
                closed: true,
            },
            named("profile"),
        )
        .unwrap();
    world
        .create_operation(
            CreatingOperation::Extrude {
                profile: "profile".into(),
                holes: Vec::new(),
                distance: 0.25,
            },
            named("slab"),
        )
        .unwrap();
    world
}

fn plate_with_thirty_holes() -> WorldGraph {
    let mut world = graph();
    world
        .create_primitive(
            Primitive::Cuboid {
                width: 6.0,
                height: 0.05,
                depth: 3.6,
            },
            named("plate"),
        )
        .unwrap();
    let mut tools = Vec::new();
    for i in 0..6 {
        for j in 0..5 {
            let og_id = format!("hole-{i}-{j}");
            world
                .create_primitive(
                    Primitive::Cylinder {
                        radius: 0.05,
                        height: 0.25,
                    },
                    named(&og_id),
                )
                .unwrap();
            world
                .transform(
                    &og_id,
                    Transform::Translate {
                        offset: [
                            -0.75 + 0.3 * f64::from(i),
                            -0.1,
                            -0.48 + 0.24 * f64::from(j),
                        ],
                    },
                )
                .unwrap();
            tools.push(og_id);
        }
    }
    world
        .operate(
            "plate",
            ModifyingOperation::Subtract,
            &tools,
            EditScope::Node,
        )
        .unwrap();
    world
}

fn display_buckets(body: &BrepEnvelope) -> [f64; 3] {
    [
        static_bucket(body).unwrap(),
        deflection_bucket(0.002).unwrap(),
        deflection_bucket(0.004).unwrap(),
    ]
}

#[test]
fn s_slab_cap_has_no_zero_area_triangle_at_the_arc_arc_tangent() {
    let body = s_slab().brep("slab").unwrap();
    for bucket in display_buckets(&body) {
        assert_eq!(faults(&body, bucket), Faults::default(), "bucket {bucket}");
    }
}

#[test]
fn plate_cut_by_thirty_holes_has_no_zero_area_triangle() {
    let body = plate_with_thirty_holes().brep("plate").unwrap();
    for bucket in display_buckets(&body) {
        assert_eq!(faults(&body, bucket).zero_area, 0, "bucket {bucket}");
    }
}
