use super::{export_bodies, export_step, StepBodyInput};
use crate::brep::{
    coarse_accuracy, BrepEnvelope, Curve, CurveGeometry, EdgeGeometry, Frame3, GeometryError,
    IntersectionDefinition, IntersectionSide, PcurveGeometry, Surface, TraceAnchor,
};
use crate::math::Interval;
use crate::operations::modifying::boolean::{boolean_spheres, BooleanOp};
use crate::primitives;

fn frame() -> Frame3 {
    Frame3::from_axis([1.0, 2.0, 3.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]).unwrap()
}

fn numerical_edge_box() -> BrepEnvelope {
    let mut brep = primitives::cuboid(
        "numerical-edge".into(),
        Frame3::IDENTITY,
        [1.0, 2.0, 3.0],
        coarse_accuracy(1e-6),
    )
    .unwrap();
    let edge = brep.topology.edges[0].clone();
    let EdgeGeometry::Curve { curve, range } = edge.geometry else {
        panic!("cuboid edge must be regular")
    };
    let use_a = edge.halfedge;
    let use_b = edge.twin_halfedge.unwrap();
    let face_a = brep.topology.halfedges[use_a as usize].face.unwrap();
    let face_b = brep.topology.halfedges[use_b as usize].face.unwrap();
    let surface_ids = [
        brep.topology.faces[face_a as usize].surface,
        brep.topology.faces[face_b as usize].surface,
    ];
    let evaluator = brep.geometry.curve(curve).unwrap();
    let points = [
        evaluator.point_at(range.lo).unwrap(),
        evaluator.point_at(range.hi).unwrap(),
    ];
    let uv_a = points.map(|point| {
        brep.geometry.surfaces[surface_ids[0] as usize]
            .project(point, None)
            .unwrap()
    });
    let uv_b = points.map(|point| {
        brep.geometry.surfaces[surface_ids[1] as usize]
            .project(point, None)
            .unwrap()
    });
    let padding = 1e-10;
    let tubes = std::array::from_fn(|axis| {
        let values = if axis < 2 {
            [uv_a[0][axis], uv_a[1][axis]]
        } else {
            [uv_b[0][axis - 2], uv_b[1][axis - 2]]
        };
        Interval::new(
            values[0].min(values[1]) - padding,
            values[0].max(values[1]) + padding,
        )
        .unwrap()
    });
    let definition = brep.geometry.intersections.len() as u32;
    brep.geometry.intersections.push(IntersectionDefinition {
        surfaces: surface_ids,
        anchors: vec![
            TraceAnchor {
                parameter: range.lo,
                point: points[0],
                uv_a: uv_a[0],
                uv_b: uv_b[0],
            },
            TraceAnchor {
                parameter: range.hi,
                point: points[1],
                uv_a: uv_a[1],
                uv_b: uv_b[1],
            },
        ],
        uv_tubes: vec![tubes],
        residual_tolerance: coarse_accuracy(1e-6).intersection,
    });
    brep.geometry.curves[curve as usize] = CurveGeometry::Intersection { definition };
    let pcurve_a = brep.geometry.pcurves.len() as u32;
    brep.geometry
        .pcurves
        .push(PcurveGeometry::IntersectionSide {
            definition,
            side: IntersectionSide::A,
        });
    let pcurve_b = brep.geometry.pcurves.len() as u32;
    brep.geometry
        .pcurves
        .push(PcurveGeometry::IntersectionSide {
            definition,
            side: IntersectionSide::B,
        });
    brep.topology.halfedges[use_a as usize].geometry_use.pcurve = Some(pcurve_a);
    brep.topology.halfedges[use_b as usize].geometry_use.pcurve = Some(pcurve_b);
    brep.validate().unwrap();
    brep
}
#[test]
fn exports_all_families_shared_edges_seams_and_explicit_poles() {
    let bodies = [
        primitives::cylinder("c#999's".into(), frame(), 1.0, 2.0, coarse_accuracy(1e-6)).unwrap(),
        primitives::sphere("s".into(), frame(), 1.0, coarse_accuracy(1e-6)).unwrap(),
        primitives::cone("c".into(), frame(), 1.0, 2.0, coarse_accuracy(1e-6)).unwrap(),
        primitives::frustum("f".into(), frame(), 1.0, 0.4, 2.0, coarse_accuracy(1e-6)).unwrap(),
        primitives::torus("t".into(), frame(), 2.0, 0.5, coarse_accuracy(1e-6)).unwrap(),
        primitives::cylinder_with_circular_hole(
            "w".into(),
            frame(),
            frame(),
            0.3,
            1.5,
            2.0,
            coarse_accuracy(1e-6),
        )
        .unwrap(),
    ];
    for brep in bodies {
        let original = brep.to_json().unwrap();
        let (metres, report) = export_step(&brep, "metre").unwrap();
        assert_eq!(report.faces, brep.topology.faces.len());
        assert_eq!(metres.matches("=EDGE_CURVE(").count(), report.edges);
        assert!(!metres.contains("POLY_LOOP"));
        assert!(metres.contains("PCURVE("));
        if brep.topology.edges.iter().any(|e| e.chart_seam) {
            assert!(metres.contains("SEAM_CURVE("));
        }
        let (mm, _) = export_step(&brep, "millimetre").unwrap();
        assert!(mm.contains("SI_UNIT(.MILLI.,.METRE.)"));
        assert_ne!(metres, mm);
        assert_eq!(original, brep.to_json().unwrap());
    }
}

#[test]
fn numerical_intersection_edges_export_as_bounded_cubic_curves() {
    let brep = numerical_edge_box();
    let (step, report) = export_step(&brep, "metre").unwrap();
    assert!(step.contains("B_SPLINE_CURVE_WITH_KNOTS"));
    assert!(report.exchange_error_bound <= brep.accuracy.exchange);
}
#[test]
fn preserves_reversed_cut_faces_and_cavity_orientation() {
    let a = primitives::sphere("host".into(), frame(), 2.0, coarse_accuracy(1e-6)).unwrap();
    let b = primitives::sphere("cut".into(), frame(), 0.5, coarse_accuracy(1e-6)).unwrap();
    let cavity = boolean_spheres(&a, &b, BooleanOp::Subtraction, "cavity".into())
        .unwrap()
        .brep;
    let (text, report) = export_step(&cavity, "metre").unwrap();
    assert_eq!(report.cavity_shells, 1);
    assert!(text.contains("BREP_WITH_VOIDS("));
    assert!(text
        .lines()
        .any(|line| line.contains("=ADVANCED_FACE(") && line.ends_with(",.F.);")));
    let mut shifted = frame();
    shifted.origin[0] += 1.7;
    let cutter = primitives::sphere("cutter".into(), shifted, 1.0, coarse_accuracy(1e-6)).unwrap();
    let cut = boolean_spheres(&a, &cutter, BooleanOp::Subtraction, "cut".into())
        .unwrap()
        .brep;
    assert!(export_step(&cut, "metre").is_ok());
    let mut invalid = cut;
    invalid.topology.halfedges[0].next = None;
    assert!(matches!(
        export_step(&invalid, "metre"),
        Err(GeometryError::InvalidTopology(_))
    ));
}

#[test]
fn rejects_unrepresentable_exchange_and_preserves_large_revisions() {
    let mut brep =
        primitives::sphere("budget".into(), frame(), 1.0, coarse_accuracy(1e-6)).unwrap();
    brep.revision = u64::MAX;
    let (_, report) = export_step(&brep, "metre").unwrap();
    assert_eq!(report.revision, u64::MAX.to_string());
    brep.accuracy.exchange = 1e-10;
    assert!(matches!(
        export_step(&brep, "metre"),
        Err(GeometryError::LimitExceeded(_))
    ));
    brep.accuracy.exchange = 1e-6;
    brep.id = "x".repeat(4097);
    assert!(matches!(
        export_step(&brep, "metre"),
        Err(GeometryError::LimitExceeded(_))
    ));
    assert!(matches!(
        export_step(&brep, "inch"),
        Err(GeometryError::InvalidGeometry(_))
    ));
}

#[test]
fn two_million_entity_export_is_limit_exceeded() {
    let brep = primitives::cuboid(
        "cap".into(),
        Frame3::IDENTITY,
        [1.0, 2.0, 3.0],
        coarse_accuracy(1e-6),
    )
    .unwrap();
    let inputs = (0..7_000)
        .map(|_| StepBodyInput {
            og_id: "cap",
            shape_id: "cap",
            shape_revision: 0,
            brep: &brep,
        })
        .collect::<Vec<_>>();
    let failure = export_bodies(
        &inputs,
        "metre",
        "Y",
        "cap",
        "1970-01-01T00:00:00",
        Vec::new(),
    )
    .unwrap_err();
    assert!(
        matches!(&failure.error, GeometryError::LimitExceeded(message) if message.contains("2,000,000"))
    );
}
