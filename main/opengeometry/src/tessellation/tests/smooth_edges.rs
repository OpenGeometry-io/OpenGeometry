use crate::brep::{fine_accuracy, BrepEnvelope, CurveGeometry, EdgeGeometry, Frame3};
use crate::math::Point3;
use crate::primitives::{arc_edged_extrusion, cuboid, cylinder, linear_extrusion, ProfileEdge};
use crate::tessellation::display::{display_buffers, display_buffers_with_smooth_angle};
use crate::tessellation::{tessellate_with_smooth_angle, Tessellation};
use std::collections::{BTreeSet, HashMap};

fn prism(sides: usize) -> BrepEnvelope {
    let outer = (0..sides)
        .map(|i| {
            let angle = std::f64::consts::TAU * i as f64 / sides as f64;
            [2.0 * angle.cos(), 2.0 * angle.sin()]
        })
        .collect();
    linear_extrusion(
        "prism".into(),
        Frame3::IDENTITY,
        outer,
        vec![],
        3.0,
        fine_accuracy(),
    )
    .unwrap()
}

fn tangent_profile() -> BrepEnvelope {
    let edges = vec![
        ProfileEdge::Line {
            from: [0.0, -1.0],
            to: [2.0, -1.0],
        },
        ProfileEdge::Arc {
            center: [2.0, 0.0],
            radius: 1.0,
            start_angle: -std::f64::consts::FRAC_PI_2,
            sweep_angle: std::f64::consts::PI,
        },
        ProfileEdge::Line {
            from: [2.0, 1.0],
            to: [0.0, 1.0],
        },
        ProfileEdge::Line {
            from: [0.0, 1.0],
            to: [0.0, -1.0],
        },
    ];
    arc_edged_extrusion(
        "tangent".into(),
        Frame3::IDENTITY,
        edges,
        2.0,
        fine_accuracy(),
    )
    .unwrap()
}

fn mesh(brep: &BrepEnvelope, angle: f64) -> Tessellation {
    tessellate_with_smooth_angle(brep, 0.01, 1_000_000, angle).unwrap()
}

fn outlined(mesh: &Tessellation) -> BTreeSet<u32> {
    mesh.outline_edge_ids.iter().copied().collect()
}

fn ends(brep: &BrepEnvelope, edge: u32) -> [Point3; 2] {
    let halfedge = &brep.topology.halfedges[brep.topology.edges[edge as usize].halfedge as usize];
    [halfedge.from, halfedge.to].map(|v| brep.topology.vertices[v as usize].position)
}

fn vertical_edges(brep: &BrepEnvelope) -> BTreeSet<u32> {
    (0..brep.topology.edges.len() as u32)
        .filter(|&edge| {
            let [a, b] = ends(brep, edge);
            (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9
        })
        .collect()
}

fn side_normals_agree(mesh: &Tessellation, keep: impl Fn(Point3) -> bool) -> bool {
    let mut seen = HashMap::<[u64; 3], [f32; 3]>::new();
    let mut agree = true;
    for (vertex, n) in mesh.normals.chunks_exact(3).enumerate() {
        let point = mesh.point(vertex as u32);
        if n[2].abs() > 1e-6 || !keep(point) {
            continue;
        }
        let p = point.map(|v| (v * 1e9).round().to_bits());
        let n = [n[0], n[1], n[2]];
        let first = *seen.entry(p).or_insert(n);
        agree &= (0..3).all(|k| (first[k] - n[k]).abs() < 1e-6);
    }
    agree
}

#[test]
fn polygon_prism_folds_under_the_threshold_are_smooth() {
    let body = prism(26);
    let vertical = vertical_edges(&body);
    assert_eq!(vertical.len(), 26);
    let smooth = mesh(&body, 20.0);
    assert!(outlined(&smooth).is_disjoint(&vertical));
    assert_eq!(outlined(&smooth).len(), 52);
    assert_eq!(smooth.smooth_edges.len(), body.topology.edges.len());
    for edge in &body.topology.edges {
        let flag = u8::from(vertical.contains(&edge.id));
        assert_eq!(smooth.smooth_edges[edge.id as usize], flag);
    }
    let sharp = mesh(&body, 0.0);
    assert!(vertical.is_subset(&outlined(&sharp)));
    assert!(sharp.smooth_edges.iter().all(|&flag| flag == 0));
}

#[test]
fn cuboid_outlines_every_edge_at_any_threshold() {
    let body = cuboid(
        "box".into(),
        Frame3::IDENTITY,
        [1.0, 2.0, 3.0],
        fine_accuracy(),
    )
    .unwrap();
    for angle in [0.0, 20.0, 89.0] {
        assert_eq!(outlined(&mesh(&body, angle)).len(), 12);
    }
}

#[test]
fn tangent_line_and_arc_edges_are_not_outlined() {
    let body = tangent_profile();
    let tangent: BTreeSet<u32> = vertical_edges(&body)
        .into_iter()
        .filter(|&id| (ends(&body, id)[0][0] - 2.0).abs() < 1e-9)
        .collect();
    assert_eq!(tangent.len(), 2);
    assert!(outlined(&mesh(&body, 20.0)).is_disjoint(&tangent));
    assert!(tangent.is_subset(&outlined(&mesh(&body, 0.0))));
}

#[test]
fn normals_are_continuous_across_smooth_edges() {
    assert!(side_normals_agree(&mesh(&prism(26), 20.0), |_| true));
    assert!(side_normals_agree(&mesh(&tangent_profile(), 20.0), |p| p
        [0]
        > 1.0));
    assert!(!side_normals_agree(&mesh(&prism(26), 0.0), |_| true));
}

#[test]
fn smooth_angle_outside_its_range_is_rejected() {
    let body = prism(6);
    for angle in [-1.0, 90.0, f64::NAN] {
        assert!(tessellate_with_smooth_angle(&body, 0.01, 1_000, angle).is_err());
    }
}

#[test]
fn display_buffers_carry_the_smooth_flags_at_the_default_angle() {
    let body = prism(26);
    let vertical = vertical_edges(&body);
    let buffers = display_buffers(&body, 0.0078125, 1_000_000).unwrap();
    assert!(buffers.edge_ids.iter().all(|id| !vertical.contains(id)));
    assert_eq!(buffers.smooth_edges.iter().filter(|&&f| f == 1).count(), 26);
    let sharp = display_buffers_with_smooth_angle(&body, 0.0078125, 1_000_000, 0.0).unwrap();
    assert!(sharp.edge_ids.iter().any(|id| vertical.contains(id)));
}

#[test]
fn polygon_prism_folds_over_the_threshold_stay_sharp() {
    let body = prism(15);
    let vertical = vertical_edges(&body);
    assert_eq!(vertical.len(), 15);
    let folded = mesh(&body, 20.0);
    assert!(vertical.is_subset(&outlined(&folded)));
    assert!(folded.smooth_edges.iter().all(|&flag| flag == 0));
}

#[test]
fn cylinder_cap_circles_stay_sharp_at_the_default_angle() {
    let body = cylinder("round".into(), Frame3::IDENTITY, 1.0, 2.0, fine_accuracy()).unwrap();
    let circles: BTreeSet<u32> = body
        .topology
        .edges
        .iter()
        .filter(|edge| match edge.geometry {
            EdgeGeometry::Curve { curve, .. } => matches!(
                body.geometry.curves[curve as usize],
                CurveGeometry::Circle { .. }
            ),
            _ => false,
        })
        .map(|edge| edge.id)
        .collect();
    assert_eq!(circles.len(), 2);
    let buffers = display_buffers(&body, 0.0078125, 1_000_000).unwrap();
    assert!(circles
        .iter()
        .all(|&id| buffers.smooth_edges[id as usize] == 0));
    assert!(circles.is_subset(&buffers.edge_ids.iter().copied().collect()));
}
