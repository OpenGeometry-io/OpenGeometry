use super::super::face_trim::{face_contains_uv, FaceTrim};
use crate::brep::{Accuracy, BrepEnvelope, Frame3, Surface, SurfaceGeometry, UVBox, UV};
use crate::math::norm;
use crate::operations::modifying::boolean::{boolean_brep, BooleanOp};
use crate::primitives::{self, ProfileEdge};
use sha2::{Digest, Sha256};
use std::f64::consts::PI;

fn crossing_grid(bounds: UVBox) -> Vec<UV> {
    let steps = 24;
    let at = move |axis: usize, index: usize| {
        let interval = bounds[axis];
        interval.lo + interval.width() * (1.5 * index as f64 / steps as f64 - 0.25)
    };
    (0..=steps)
        .flat_map(|u| (0..=steps).map(move |v| [at(0, u), at(1, v)]))
        .collect()
}

fn tube() -> BrepEnvelope {
    let rod = Frame3 {
        origin: [0.0, 0.0, -0.5],
        ..Frame3::IDENTITY
    };
    let body = primitives::cylinder(
        "body".into(),
        Frame3::IDENTITY,
        0.3,
        1.0,
        Accuracy::STANDARD,
    );
    let bore = primitives::cylinder("bore".into(), rod, 0.1, 2.0, Accuracy::STANDARD);
    boolean_brep(
        &body.unwrap(),
        &bore.unwrap(),
        BooleanOp::Subtraction,
        "tube".into(),
    )
    .unwrap()
    .brep
}

fn holed_d() -> BrepEnvelope {
    let half_turn = |start_angle| ProfileEdge::Arc {
        center: [0.2, 0.2],
        radius: 0.1,
        start_angle,
        sweep_angle: PI,
    };
    let outer = vec![
        ProfileEdge::Line {
            from: [0.0, 0.0],
            to: [0.4, 0.0],
        },
        ProfileEdge::Arc {
            center: [0.4, 0.2],
            radius: 0.2,
            start_angle: -PI / 2.0,
            sweep_angle: PI,
        },
        ProfileEdge::Line {
            from: [0.4, 0.4],
            to: [0.0, 0.4],
        },
        ProfileEdge::Line {
            from: [0.0, 0.4],
            to: [0.0, 0.0],
        },
    ];
    let hole = vec![half_turn(0.0), half_turn(PI)];
    primitives::arc_edged_extrusion_with_holes(
        "holed".into(),
        Frame3::IDENTITY,
        outer,
        vec![hole],
        0.3,
        Accuracy::STANDARD,
    )
    .unwrap()
}

fn probed_bodies() -> [BrepEnvelope; 2] {
    [holed_d(), tube()]
}

fn stored_compared() -> bool {
    std::env::var("CI").as_deref() != Ok("true")
}

fn answer_record(answers: &[Option<bool>]) -> ([usize; 3], String) {
    let codes = answers
        .iter()
        .map(|answer| answer.map_or(0, |inside| 1 + u8::from(inside)))
        .collect::<Vec<_>>();
    let counts = [0, 1, 2].map(|code| codes.iter().filter(|value| **value == code).count());
    (counts, format!("{:x}", Sha256::digest(&codes)))
}

const RECORDED: [([usize; 3], &str); 2] = [
    (
        [417, 2863, 1720],
        "22153794c07f0549c60da52fa4442fef30cb55dfcee3bd6a3168f36f62ed78de",
    ),
    (
        [138, 1298, 1064],
        "1b5f23003c8b925162d8f2aed4ad0206f30c7f4956a727dfc9e779bb71c98d2d",
    ),
];

#[test]
fn one_face_trim_reused_for_every_probe_gives_the_recorded_answers_on_a_holed_d_and_a_tube() {
    let bodies = probed_bodies();
    for (body, (counts, digest)) in bodies.iter().zip(RECORDED) {
        let mut answers = Vec::new();
        for face in &body.topology.faces {
            let trim = FaceTrim::new(body, face).unwrap();
            for uv in crossing_grid(face.trim.uv_bounds) {
                let reused = trim.contains(body, face, uv).unwrap();
                let fresh = face_contains_uv(body, face, uv).unwrap();
                assert_eq!(reused, fresh, "{} face {} at {uv:?}", body.id, face.id);
                answers.push(reused);
            }
        }
        let (measured, sequence) = answer_record(&answers);
        if stored_compared() {
            assert_eq!(measured, counts, "{}", body.id);
            assert_eq!(sequence, digest, "{}", body.id);
        }
    }
    let faces = || {
        bodies
            .iter()
            .flat_map(|body| body.topology.faces.iter().map(move |face| (body, face)))
    };
    assert!(faces().any(|(_, face)| !face.trim.holes.is_empty()));
    assert!(faces().any(|(body, face)| {
        matches!(
            body.geometry.surfaces[face.surface as usize],
            SurfaceGeometry::Cylinder { .. }
        ) && face.trim.holes.len() == 1
    }));
}

fn edge_and_corner_probes(bounds: UVBox, depth: f64) -> [UV; 5] {
    let [u, v] = bounds;
    let (mid_u, mid_v) = (u.midpoint(), v.midpoint());
    [
        [mid_u, v.lo + depth],
        [mid_u, v.hi - depth],
        [u.lo + depth, mid_v],
        [u.hi - depth, mid_v],
        [u.lo + 0.7 * depth, v.lo + 0.7 * depth],
    ]
}

#[test]
fn a_probe_half_a_tolerance_inside_a_trim_edge_or_corner_is_boundary_and_three_tolerances_inside_is_inside(
) {
    let body = primitives::cuboid(
        "c".into(),
        Frame3::IDENTITY,
        [1.0, 1.0, 1.0],
        Accuracy::STANDARD,
    )
    .unwrap();
    let face = body
        .topology
        .faces
        .iter()
        .find(|face| {
            matches!(
                body.geometry.surfaces[face.surface as usize],
                SurfaceGeometry::Plane { .. }
            )
        })
        .unwrap();
    let bounds = face.trim.uv_bounds;
    let surface = body.geometry.surface(face.surface).unwrap();
    let jet = surface
        .derivatives([bounds[0].midpoint(), bounds[1].midpoint()])
        .unwrap();
    let tolerance = body.accuracy.geometric / norm(jet.du).min(norm(jet.dv));
    for (depth, expected) in [(0.5, None), (3.0, Some(true))] {
        for uv in edge_and_corner_probes(bounds, depth * tolerance) {
            let answer = face_contains_uv(&body, face, uv).unwrap();
            assert_eq!(answer, expected, "{depth} tolerances at {uv:?}");
        }
    }
}
