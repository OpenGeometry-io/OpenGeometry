use super::{export_bodies, export_step, StepBodyInput};
use crate::brep::{
    coarse_accuracy, wrapped_sector, BrepEnvelope, Curve, CurveGeometry, EdgeGeometry, Frame3,
    GeometryError, IntersectionDefinition, IntersectionSide, PcurveGeometry, Surface, TraceAnchor,
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

#[test]
fn a_wrapped_side_exports_as_b_splines_within_the_exchange_budget() {
    let brep = wrapped_sector();
    let (step, report) = export_step(&brep, "metre").unwrap();
    assert_eq!(report.faces, 6);
    assert_eq!(step.matches("B_SPLINE_SURFACE_WITH_KNOTS").count(), 1);
    assert_eq!(step.matches("B_SPLINE_CURVE_WITH_KNOTS").count(), 2);
    assert_eq!(step.matches("=PCURVE(").count(), 2 * report.edges);
    assert!(report.exchange_error_bound <= brep.accuracy.exchange);
}

type Entities = std::collections::HashMap<u32, (String, String)>;

fn entities(step: &str) -> Entities {
    step.lines()
        .filter_map(|line| {
            let (id, rest) = line.strip_prefix('#')?.split_once('=')?;
            let (name, args) = rest.split_once('(')?;
            Some((id.parse().ok()?, (name.to_string(), args.to_string())))
        })
        .collect()
}

fn refs(args: &str) -> Vec<u32> {
    args.split('#')
        .skip(1)
        .map(|s| s.split(|c: char| !c.is_ascii_digit()).next().unwrap())
        .map(|s| s.parse().unwrap())
        .collect()
}

fn flag(args: &str) -> f64 {
    if args.trim_end_matches([')', ';']).ends_with(".T.") {
        1.0
    } else {
        -1.0
    }
}

fn axis(entities: &Entities, placement: u32) -> [f64; 3] {
    let direction = &entities[&refs(&entities[&placement].1)[1]].1;
    let inner = direction
        .split_once("(")
        .unwrap()
        .1
        .split(')')
        .next()
        .unwrap();
    let v: Vec<f64> = inner.split(',').map(|s| s.parse().unwrap()).collect();
    [v[0], v[1], v[2]]
}

fn circular_bound_turns(step: &str) -> Vec<(String, f64)> {
    let e = entities(step);
    let mut turns = Vec::new();
    for (name, args) in e.values().filter(|(n, _)| n == "ADVANCED_FACE") {
        assert_eq!(name, "ADVANCED_FACE");
        let face_refs = refs(args);
        let (surface, bounds) = face_refs.split_last().unwrap();
        if e[surface].0 != "PLANE" {
            continue;
        }
        let normal = axis(&e, refs(&e[surface].1)[0]).map(|v| v * flag(args));
        for bound in bounds {
            let (kind, bound_args) = &e[bound];
            let edges = refs(&e[&refs(bound_args)[0]].1);
            let [oriented] = edges[..] else { continue };
            let oriented_args = &e[&oriented].1;
            let edge_args = &e[&refs(oriented_args)[0]].1;
            let mut curve = &e[&refs(edge_args)[2]];
            if curve.0 == "SURFACE_CURVE" || curve.0 == "SEAM_CURVE" {
                curve = &e[&refs(&curve.1)[0]];
            }
            if curve.0 != "CIRCLE" {
                continue;
            }
            let circle_axis = axis(&e, refs(&curve.1)[0]);
            let along: f64 = (0..3).map(|i| circle_axis[i] * normal[i]).sum();
            let sense = flag(bound_args) * flag(oriented_args) * flag(edge_args);
            turns.push((kind.clone(), along.signum() * sense));
        }
    }
    turns
}

#[test]
fn hole_bounds_run_clockwise_about_the_face_normal() {
    let bodies = [
        primitives::annular_cylinder("a".into(), frame(), 0.4, 1.0, 2.0, coarse_accuracy(1e-6)),
        primitives::cylinder_with_circular_hole(
            "w".into(),
            frame(),
            frame(),
            0.3,
            1.5,
            2.0,
            coarse_accuracy(1e-6),
        ),
    ];
    for brep in bodies {
        let (step, _) = export_step(&brep.unwrap(), "metre").unwrap();
        let turns = circular_bound_turns(&step);
        assert_eq!(turns.len(), 4, "{turns:?}");
        for (kind, turn) in turns {
            let expected = if kind == "FACE_OUTER_BOUND" {
                1.0
            } else {
                -1.0
            };
            assert_eq!(turn, expected, "{kind} runs the wrong way");
        }
    }
}
