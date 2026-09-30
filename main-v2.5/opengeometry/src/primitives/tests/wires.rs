use crate::brep::fine_accuracy;
use crate::brep::{BrepEnvelope, CurveGeometry, Frame3};
use crate::math::{norm, sub};
use crate::primitives::circle::arc_wire;
use crate::tessellation::tessellate;

#[test]
fn analytic_arc_wires_retain_signed_endpoints_and_single_edge_authority() {
    let curve = CurveGeometry::Circle {
        frame: Frame3::IDENTITY,
        radius: 2.0,
    };
    for sweep in [1.4, -1.4, std::f64::consts::TAU, -std::f64::consts::TAU] {
        let brep = arc_wire("arc".into(), curve.clone(), 0.7, sweep, fine_accuracy()).unwrap();
        let h = &brep.topology.halfedges[0];
        assert_eq!(h.geometry_use.sense.multiplier(), sweep.signum());
        assert!(
            norm(sub(
                brep.topology.vertices[h.from as usize].position,
                curve.elementary_point(0.7).unwrap()
            )) < 1e-10
        );
        assert_eq!(brep.topology.edges.len(), 1);
        assert!(brep.topology.faces.is_empty());
        assert!(h.geometry_use.pcurve.is_none());
        assert_eq!(
            brep.topology.wires[0].is_closed,
            sweep.abs() == std::f64::consts::TAU
        );
        let coarse = tessellate(&brep, 0.05, 100_000).unwrap();
        let fine = tessellate(&brep, 0.005, 100_000).unwrap();
        assert!(fine.outline_edge_ids.len() > coarse.outline_edge_ids.len());
        assert!(fine.outline_edge_ids.iter().all(|id| *id == 0));
        assert!(fine.indices.is_empty());
        BrepEnvelope::from_json(&brep.to_json().unwrap()).unwrap();
    }
}
#[test]
fn elliptical_arc_sampling_meets_deflection_and_rejects_invalid_spans() {
    let curve = CurveGeometry::Ellipse {
        frame: Frame3::IDENTITY,
        major_radius: 3.0,
        minor_radius: 0.5,
    };
    let brep = arc_wire(
        "ellipse".into(),
        curve.clone(),
        0.0,
        std::f64::consts::TAU,
        fine_accuracy(),
    )
    .unwrap();
    let mesh = tessellate(&brep, 0.01, 100_000).unwrap();
    let n = mesh.outline_edge_ids.len();
    for (i, pair) in mesh.outline_positions.chunks_exact(6).enumerate() {
        let midpoint = curve
            .elementary_point(std::f64::consts::TAU * (i as f64 + 0.5) / n as f64)
            .unwrap();
        let chord = std::array::from_fn(|j| (pair[j] + pair[j + 3]) / 2.0);
        assert!(norm(sub(midpoint, chord)) < 0.01);
    }
    assert!(arc_wire("bad".into(), curve.clone(), 0.0, 0.0, fine_accuracy()).is_err());
    assert!(arc_wire("bad".into(), curve, 0.0, 7.0, fine_accuracy()).is_err());
}
