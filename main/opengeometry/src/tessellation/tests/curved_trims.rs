use crate::brep::{fine_accuracy, BrepEnvelope, Frame3, PcurveGeometry, SurfaceGeometry};
use crate::math::cross;
use crate::operations::modifying::boolean::{boolean_brep, BooleanOp};
use crate::primitives::{self, ProfileEdge};
use crate::tessellation::tessellate;
use std::f64::consts::FRAC_PI_2;

const WIDTH: f64 = 100.0;
const OUTER: f64 = 3.0;

fn quarter_tube() -> BrepEnvelope {
    let inner = 2.0;
    primitives::arc_edged_extrusion(
        "quarter-tube".into(),
        Frame3::IDENTITY,
        vec![
            ProfileEdge::Line {
                from: [inner, 0.0],
                to: [OUTER, 0.0],
            },
            ProfileEdge::Arc {
                center: [0.0, 0.0],
                radius: OUTER,
                start_angle: 0.0,
                sweep_angle: FRAC_PI_2,
            },
            ProfileEdge::Line {
                from: [0.0, OUTER],
                to: [0.0, inner],
            },
            ProfileEdge::Arc {
                center: [0.0, 0.0],
                radius: inner,
                start_angle: FRAC_PI_2,
                sweep_angle: -FRAC_PI_2,
            },
        ],
        WIDTH,
        fine_accuracy(),
    )
    .unwrap()
}

fn holed_quarter_tube() -> BrepEnvelope {
    let angle = 0.7_f64;
    let axis = [angle.cos(), angle.sin(), 0.0];
    let x = [0.0, 0.0, 1.0];
    let hole = primitives::cylinder(
        "hole".into(),
        Frame3 {
            origin: [0.0, 0.0, WIDTH / 2.0],
            x,
            y: cross(axis, x),
            z: axis,
        },
        0.4,
        10.0,
        fine_accuracy(),
    )
    .unwrap();
    let cut = boolean_brep(
        &quarter_tube(),
        &hole,
        BooleanOp::Subtraction,
        "holed".into(),
    )
    .unwrap()
    .brep;
    let holed = cut
        .topology
        .faces
        .iter()
        .filter(|face| {
            !face.trim.holes.is_empty()
                && matches!(
                    cut.geometry.surfaces[face.surface as usize],
                    SurfaceGeometry::Cylinder { radius, .. } if radius >= 2.0
                )
        })
        .count();
    assert_eq!(holed, 2, "the hole goes through both cylinder walls");
    cut
}

#[test]
fn a_small_hole_in_a_curved_face_tessellates_at_a_coarse_deflection() {
    let body = holed_quarter_tube();
    for deflection in [2.0, 5.0] {
        let mesh = tessellate(&body, deflection, 1_000_000).unwrap();
        for face in &body.topology.faces {
            assert!(
                mesh.triangle_face_ids.contains(&face.id),
                "face {} has no triangles at deflection {deflection}",
                face.key
            );
        }
    }
}

#[test]
fn a_trim_run_off_its_corner_vertex_by_rounding_tessellates() {
    let body = holed_quarter_tube();
    let face = body
        .topology
        .faces
        .iter()
        .find(|face| {
            !face.trim.holes.is_empty()
                && matches!(
                    body.geometry.surfaces[face.surface as usize],
                    SurfaceGeometry::Cylinder { radius, .. } if radius == OUTER
                )
        })
        .unwrap();
    let start = body.topology.loops[face.trim.outer as usize].start_halfedge;
    for steps in [1.0, 2.0, 4.0] {
        let mut noisy = body.clone();
        let mut current = start;
        loop {
            let halfedge = &body.topology.halfedges[current as usize];
            let pcurve = halfedge.geometry_use.pcurve.unwrap() as usize;
            if let PcurveGeometry::Line2 { origin, direction } = &mut noisy.geometry.pcurves[pcurve]
            {
                if direction[1] == 0.0 && origin[1] == WIDTH {
                    origin[1] = WIDTH + steps * WIDTH * f64::EPSILON;
                }
            }
            current = halfedge.next.unwrap();
            if current == start {
                break;
            }
        }
        assert_ne!(noisy.geometry.pcurves, body.geometry.pcurves);
        tessellate(&noisy, 0.01, 1_000_000).unwrap();
    }
}
