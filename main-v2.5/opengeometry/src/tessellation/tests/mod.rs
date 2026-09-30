mod display;
mod support;

use super::cache::TessellationCache;
use super::tessellate::tessellate;
use crate::brep::{
    bounds_chord_deviation, circular_face, fine_accuracy, Accuracy, BrepEnvelope, Curve,
    CurveGeometry, EdgeGeometry, Frame3, GeometryError, GeometryStore, Loop, Orientation,
    PcurveGeometry,
};
use crate::math::{add, cross, norm, scale, sub, Interval, Point3};
use crate::primitives;
use std::collections::HashMap;
use std::sync::Arc;
use support::bounded_curve_count;

fn shapes() -> Vec<BrepEnvelope> {
    vec![
        primitives::cylinder("c".into(), Frame3::IDENTITY, 1.0, 2.0, fine_accuracy()).unwrap(),
        primitives::cone("k".into(), Frame3::IDENTITY, 1.0, 2.0, fine_accuracy()).unwrap(),
        primitives::frustum("f".into(), Frame3::IDENTITY, 2.0, 1.0, 3.0, fine_accuracy()).unwrap(),
        primitives::sphere("s".into(), Frame3::IDENTITY, 1.0, fine_accuracy()).unwrap(),
        primitives::torus("t".into(), Frame3::IDENTITY, 3.0, 1.0, fine_accuracy()).unwrap(),
    ]
}
#[test]
fn conservative_curve_subdivision_respects_requested_error() {
    let mut store = GeometryStore::new();
    store.curves.push(CurveGeometry::Circle {
        frame: Frame3::from_axis([1.0, 2.0, 3.0], [1.0, 1.0, 1.0], [1.0, -1.0, 0.0]).unwrap(),
        radius: 2.0,
    });
    let curve = store.curve(0).unwrap();
    let range = Interval::new(0.0, std::f64::consts::TAU).unwrap();
    let loose = bounded_curve_count(&curve, range, 0.2, 1_000_000).unwrap();
    let fine = bounded_curve_count(&curve, range, 0.02, 1_000_000).unwrap();
    assert!(fine > loose);
    for index in 0..fine {
        let interval = Interval::new(
            range.lo + range.width() * index as f64 / fine as f64,
            range.lo + range.width() * (index + 1) as f64 / fine as f64,
        )
        .unwrap();
        assert!(
            bounds_chord_deviation(
                curve.enclose(interval).unwrap(),
                curve.point_at(interval.lo).unwrap(),
                curve.point_at(interval.hi).unwrap(),
            ) <= 0.02
        );
    }
}
#[test]
fn lod_changes_mesh_not_topology_and_retains_face_ids() {
    for b in shapes() {
        let original = b.to_json().unwrap();
        let coarse = tessellate(&b, 0.1, 1_000_000).unwrap();
        let fine = tessellate(&b, 0.02, 1_000_000).unwrap();
        assert!(fine.indices.len() > coarse.indices.len());
        assert_eq!(fine.indices.len() / 3, fine.triangle_face_ids.len());
        assert!(fine
            .triangle_face_ids
            .iter()
            .all(|&f| (f as usize) < b.topology.faces.len()));
        assert_eq!(b.to_json().unwrap(), original);
        for n in fine.normals.chunks_exact(3) {
            assert!((norm([n[0] as f64, n[1] as f64, n[2] as f64]) - 1.0).abs() < 1e-6);
        }
    }
}
#[test]
fn shared_boundaries_make_a_closed_geometric_mesh() {
    for b in shapes() {
        let mesh = tessellate(&b, 0.05, 1_000_000).unwrap();
        let key = |p: Point3| p.map(f64::to_bits);
        let mut edges = HashMap::<([u64; 3], [u64; 3]), usize>::new();
        for t in mesh.indices.chunks_exact(3) {
            for (a, c) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                let mut pair = [key(mesh.point(a)), key(mesh.point(c))];
                pair.sort();
                *edges.entry((pair[0], pair[1])).or_default() += 1;
            }
        }
        assert!(
            edges.values().all(|&count| count == 2),
            "{} has unmatched geometric mesh edges",
            b.id
        );
    }
}
#[test]
fn spherical_triangle_samples_obey_deflection() {
    let b = primitives::sphere("s".into(), Frame3::IDENTITY, 1.0, fine_accuracy()).unwrap();
    let deflection = 0.05;
    let mesh = tessellate(&b, deflection, 1_000_000).unwrap();
    for ids in mesh.indices.chunks_exact(3) {
        let p = ids.iter().map(|&id| mesh.point(id)).fold([0.0; 3], add);
        assert!(1.0 - norm(scale(p, 1.0 / 3.0)) <= deflection);
    }
}
#[test]
fn periodic_chart_lifts_keep_shared_positions_and_hide_seams() {
    let b =
        primitives::cylinder("lift".into(), Frame3::IDENTITY, 1.0, 2.0, fine_accuracy()).unwrap();
    let original = tessellate(&b, 0.05, 1_000_000).unwrap();
    let mut lifted = b;
    for h in &mut lifted.topology.halfedges {
        if h.face == Some(0) {
            h.geometry_use.periodic_lift = [1, 0];
        }
    }
    let tau = std::f64::consts::TAU;
    lifted.topology.faces[0].trim.uv_bounds[0] = Interval::new(tau, 2.0 * tau).unwrap();
    let mesh = tessellate(&lifted, 0.05, 1_000_000).unwrap();
    assert_eq!(original.positions, mesh.positions);
    assert_eq!(original.outline_positions, mesh.outline_positions);
    assert_eq!(original.outline_edge_ids, mesh.outline_edge_ids);
}
#[test]
fn planar_tessellation_preserves_a_circular_hole() {
    let mut b = circular_face();
    b.geometry.curves[0] = CurveGeometry::Circle {
        frame: Frame3::IDENTITY,
        radius: 2.0,
    };
    b.geometry.pcurves[0] = PcurveGeometry::Conic2 {
        origin: [0.0; 2],
        axis_a: [2.0, 0.0],
        axis_b: [0.0, 2.0],
    };
    b.topology.vertices[0].position = [2.0, 0.0, 0.0];
    b.topology.faces[0].trim.uv_bounds = [Interval::new(-2.0, 2.0).unwrap(); 2];
    b.topology.faces[0].trim.holes.push(1);
    b.geometry.curves.push(CurveGeometry::Circle {
        frame: Frame3::IDENTITY,
        radius: 1.0,
    });
    b.geometry.pcurves.push(PcurveGeometry::Conic2 {
        origin: [0.0; 2],
        axis_a: [1.0, 0.0],
        axis_b: [0.0, 1.0],
    });
    let mut vertex = b.topology.vertices[0].clone();
    vertex.id = 1;
    vertex.position = [1.0, 0.0, 0.0];
    vertex.outgoing_halfedge = Some(1);
    b.topology.vertices.push(vertex);
    let mut edge = b.topology.edges[0].clone();
    edge.id = 1;
    edge.halfedge = 1;
    edge.geometry = EdgeGeometry::Curve {
        curve: 1,
        range: Interval::new(0.0, std::f64::consts::TAU).unwrap(),
    };
    b.topology.edges.push(edge);
    let mut h = b.topology.halfedges[0].clone();
    h.id = 1;
    h.from = 1;
    h.to = 1;
    h.next = Some(1);
    h.prev = Some(1);
    h.edge = 1;
    h.loop_ref = Some(1);
    h.geometry_use.pcurve = Some(1);
    h.geometry_use.sense = Orientation::Reverse;
    b.topology.halfedges.push(h);
    b.topology.loops.push(Loop {
        id: 1,
        start_halfedge: 1,
        face_ref: 0,
        is_hole: true,
    });
    let deflection = 0.02;
    let mesh = tessellate(&b, deflection, 1_000_000).unwrap();
    let mut area = 0.0;
    for ids in mesh.indices.chunks_exact(3) {
        let [a, c, d] = [mesh.point(ids[0]), mesh.point(ids[1]), mesh.point(ids[2])];
        let center = scale(add(add(a, c), d), 1.0 / 3.0);
        assert!(center[0].hypot(center[1]) >= 1.0 - deflection);
        area += norm(cross(sub(c, a), sub(d, a))) * 0.5;
    }
    assert!((area - 3.0 * std::f64::consts::PI).abs() < 6.0 * std::f64::consts::PI * deflection);
}
#[test]
fn precision_limits_do_not_claim_unachievable_deflection() {
    let near_cylinder = primitives::frustum(
        "near".into(),
        Frame3::IDENTITY,
        1.0,
        1.0 + 2.0_f64.powi(-48),
        1.0,
        fine_accuracy(),
    )
    .unwrap();
    assert!(
        matches!(tessellate(&near_cylinder, 0.01, 1_000_000), Err(GeometryError::LimitExceeded(message)) if message.contains("coordinate precision"))
    );
    let frame = Frame3 {
        origin: [1e12, 0.0, 0.0],
        ..Frame3::IDENTITY
    };
    let b = primitives::sphere(
        "far".into(),
        frame,
        1.0,
        Accuracy {
            geometric: 0.001,
            intersection: 0.0001,
            ..fine_accuracy()
        },
    )
    .unwrap();
    assert!(
        matches!(tessellate(&b, 0.01, 1_000_000), Err(GeometryError::LimitExceeded(message)) if message.contains("coordinate precision"))
    );
    let mut cache = TessellationCache::new(16_000_000);
    let coarse = cache.tessellate(&b, 0.1, 1_000_000).unwrap();
    assert!(cache.tessellate(&b, 0.01, 1_000_000).is_err());
    assert!(Arc::ptr_eq(
        &coarse,
        &cache.tessellate(&b, 0.1, 1_000_000).unwrap()
    ));
}
#[test]
fn extreme_scales_return_errors_instead_of_invalid_or_empty_meshes() {
    for (radius, geometric, deflection) in [(1e300, 1e290, 1e299), (1e-200, 1e-210, 1e-201)] {
        let b = primitives::sphere(
            "extreme".into(),
            Frame3::IDENTITY,
            radius,
            Accuracy {
                geometric,
                intersection: geometric / 4.0,
                ..fine_accuracy()
            },
        )
        .unwrap();
        assert!(matches!(
            tessellate(&b, deflection, 1_000_000),
            Err(GeometryError::UnresolvedTessellation(_))
        ));
    }
}
#[test]
fn cache_tracks_geometry_even_when_revision_is_not_incremented() {
    let mut b =
        primitives::cylinder("c".into(), Frame3::IDENTITY, 1.0, 2.0, fine_accuracy()).unwrap();
    let mut cache = TessellationCache::new(16_000_000);
    let a = cache.tessellate(&b, 0.1, 1_000_000).unwrap();
    let again = cache.tessellate(&b, 0.1, 1_000_000).unwrap();
    assert!(Arc::ptr_eq(&a, &again));
    b.topology.faces[0].key = "changed".into();
    let changed = cache.tessellate(&b, 0.1, 1_000_000).unwrap();
    assert!(!Arc::ptr_eq(&a, &changed));
    for d in [0.08, 0.06, 0.04, 0.02] {
        cache.tessellate(&b, d, 1_000_000).unwrap();
    }
    assert_eq!(cache.len(), 3);
    assert!(tessellate(&b, 0.001, 1).is_err());
}
