use super::build;
use crate::brep::{
    coarse_accuracy, BrepEnvelope, CurveGeometry, Edge, EdgeGeometry, PcurveGeometry,
    SurfaceGeometry, GROUND,
};
use crate::math::Point3;
use crate::operations::creating::ProfileLoop;

const L_PATH: [Point3; 3] = [[0.0, 0.0, 0.0], [0.0, 3.0, 0.0], [3.0, 3.0, 0.0]];
const SPATIAL_PATH: [Point3; 4] = [
    [0.0, 0.0, 0.0],
    [0.0, 3.0, 0.0],
    [3.0, 3.0, 0.0],
    [3.0, 3.0, 3.0],
];

fn circular_sweep(radius: f64, path: &[Point3]) -> BrepEnvelope {
    build(
        "solid".into(),
        ProfileLoop::Circle {
            frame: GROUND,
            radius,
        },
        path.to_vec(),
        coarse_accuracy(1e-6),
    )
    .unwrap()
}

fn edge_uses<'a>(
    body: &'a BrepEnvelope,
    edge: &Edge,
) -> Vec<(&'a SurfaceGeometry, &'a PcurveGeometry)> {
    [Some(edge.halfedge), edge.twin_halfedge]
        .into_iter()
        .flatten()
        .map(|id| {
            let halfedge = &body.topology.halfedges[id as usize];
            let face = &body.topology.faces[halfedge.face.unwrap() as usize];
            (
                &body.geometry.surfaces[face.surface as usize],
                &body.geometry.pcurves[halfedge.geometry_use.pcurve.unwrap() as usize],
            )
        })
        .collect()
}

fn is_line(body: &BrepEnvelope, edge: &Edge) -> bool {
    matches!(
        edge.geometry,
        EdgeGeometry::Curve { curve, .. }
            if matches!(body.geometry.curves[curve as usize], CurveGeometry::Line { .. })
    )
}

fn edge_class_counts(body: &BrepEnvelope) -> [usize; 3] {
    let mut counts = [0; 3];
    for edge in &body.topology.edges {
        let uses = edge_uses(body, edge);
        assert_eq!(uses.len(), 2);
        let on_plane = |surface: &SurfaceGeometry| matches!(surface, SurfaceGeometry::Plane { .. });
        if is_line(body, edge) {
            assert!(uses.iter().all(|(surface, pcurve)| {
                matches!(surface, SurfaceGeometry::Cylinder { .. })
                    && matches!(pcurve, PcurveGeometry::Line2 { .. })
            }));
            counts[1] += 1;
        } else if uses.iter().any(|(surface, _)| on_plane(surface)) {
            for (surface, pcurve) in &uses {
                if on_plane(surface) {
                    assert!(matches!(pcurve, PcurveGeometry::Conic2 { .. }));
                } else {
                    assert!(matches!(surface, SurfaceGeometry::Cylinder { .. }));
                    assert!(matches!(
                        pcurve,
                        PcurveGeometry::Line2 { direction, .. } if *direction == [1.0, 0.0]
                    ));
                }
            }
            counts[0] += 1;
        } else {
            assert!(uses
                .iter()
                .all(|(_, pcurve)| matches!(pcurve, PcurveGeometry::ProjectedCurve { .. })));
            counts[2] += 1;
        }
    }
    counts
}

fn checked_chart_seams(body: &BrepEnvelope) -> usize {
    let surface = |halfedge: u32| {
        let face = body.topology.halfedges[halfedge as usize].face.unwrap();
        body.topology.faces[face as usize].surface
    };
    let mut checked = 0;
    for edge in body.topology.edges.iter().filter(|edge| edge.chart_seam) {
        let Some(twin) = edge.twin_halfedge else {
            continue;
        };
        assert_eq!(surface(edge.halfedge), surface(twin));
        checked += 1;
    }
    checked
}

#[test]
fn circular_sweep_pcurve_kinds_per_edge_class() {
    assert_eq!(edge_class_counts(&circular_sweep(0.5, &L_PATH)), [4, 4, 2]);
    assert_eq!(
        edge_class_counts(&circular_sweep(0.4, &SPATIAL_PATH)),
        [4, 6, 4]
    );
}

#[test]
fn no_chart_seam_edge_spans_two_surfaces() {
    let straight = circular_sweep(0.5, &[[0.0, 0.0, 0.0], [0.0, 3.0, 0.0]]);
    assert!(checked_chart_seams(&straight) > 0);
    for body in [
        circular_sweep(0.5, &L_PATH),
        circular_sweep(0.4, &SPATIAL_PATH),
    ] {
        checked_chart_seams(&body);
        assert!(body
            .topology
            .edges
            .iter()
            .filter(|edge| is_line(&body, edge))
            .all(|edge| !edge.chart_seam));
    }
}
