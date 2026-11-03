use super::classification::PointClassification;
use super::classify::{classify_point, classify_point_validated};
use super::face_trim::{face_contains_uv, FaceTrim};
use crate::brep::{
    coarse_accuracy, unit, Accuracy, BrepEnvelope, Frame3, SurfaceGeometry, UVBox, UV,
};
use crate::math::{add, cross, dot, scale, Point3};
use crate::operations::modifying::boolean::{boolean_brep, BooleanOp};
use crate::primitives::{self, ProfileEdge};
use sha2::{Digest, Sha256};
use std::f64::consts::PI;

const STEP: f64 = 10.0 * Accuracy::STANDARD.geometric;
const SURFACE: [PointClassification; 3] = [
    PointClassification::Boundary,
    PointClassification::Outside,
    PointClassification::Inside,
];

struct Probe {
    at: Point3,
    out: Option<Point3>,
    inward: Option<Point3>,
}

impl Probe {
    fn along(at: Point3, direction: Point3) -> Self {
        Self {
            at,
            out: Some(scale(direction, STEP)),
            inward: Some(scale(direction, -STEP)),
        }
    }

    fn without_offsets(self) -> Self {
        self.without_out().without_inward()
    }

    fn without_out(self) -> Self {
        Self { out: None, ..self }
    }

    fn without_inward(self) -> Self {
        Self {
            inward: None,
            ..self
        }
    }
}

#[test]
fn i3_unknown_classification_is_never_treated_as_outside() {
    assert!(!PointClassification::Unknown.is_proven_outside());
    assert!(!PointClassification::Boundary.is_proven_outside());
    assert!(!PointClassification::Inside.is_proven_outside());
    assert!(PointClassification::Outside.is_proven_outside());
}
#[test]
fn analytic_ray_classification_covers_all_surface_families() {
    let bodies = [
        (
            primitives::cuboid(
                "box".into(),
                Frame3::IDENTITY,
                [2.0; 3],
                coarse_accuracy(1e-5),
            )
            .unwrap(),
            [1.0, 1.0, 1.0],
            [3.0, 3.0, 3.0],
        ),
        (
            primitives::sphere(
                "sphere".into(),
                Frame3::IDENTITY,
                1.0,
                coarse_accuracy(1e-5),
            )
            .unwrap(),
            [0.0; 3],
            [2.0, 0.0, 0.0],
        ),
        (
            primitives::cylinder(
                "cylinder".into(),
                Frame3::IDENTITY,
                1.0,
                2.0,
                coarse_accuracy(1e-5),
            )
            .unwrap(),
            [0.0, 0.0, 1.0],
            [2.0, 0.0, 1.0],
        ),
        (
            primitives::cone(
                "cone".into(),
                Frame3::IDENTITY,
                1.0,
                2.0,
                coarse_accuracy(1e-5),
            )
            .unwrap(),
            [0.0, 0.0, 1.0],
            [2.0, 0.0, 1.0],
        ),
        (
            primitives::torus(
                "torus".into(),
                Frame3::IDENTITY,
                3.0,
                1.0,
                coarse_accuracy(1e-5),
            )
            .unwrap(),
            [3.0, 0.0, 0.0],
            [0.0; 3],
        ),
    ];
    for (body, inside, outside) in bodies {
        assert_eq!(
            classify_point(&body, inside).unwrap(),
            PointClassification::Inside,
            "{}",
            body.id
        );
        assert_eq!(
            classify_point(&body, outside).unwrap(),
            PointClassification::Outside,
            "{}",
            body.id
        );
    }
}

fn turned(origin: Point3) -> Frame3 {
    let axis = unit([1.0, 2.0, 3.0]).unwrap();
    let (sin, cos) = 37.0_f64.to_radians().sin_cos();
    let rotate = |v: Point3| {
        add(
            add(scale(v, cos), scale(cross(axis, v), sin)),
            scale(axis, dot(axis, v) * (1.0 - cos)),
        )
    };
    Frame3 {
        origin,
        x: rotate([1.0, 0.0, 0.0]),
        y: rotate([0.0, 1.0, 0.0]),
        z: rotate([0.0, 0.0, 1.0]),
    }
}

fn placements() -> [Frame3; 3] {
    [
        Frame3::IDENTITY,
        turned([0.3, -0.2, 0.5]),
        turned([1.0e3, -1.0e3, 1.0e3]),
    ]
}

fn tally(body: &BrepEnvelope, points: impl Iterator<Item = Point3>) -> [usize; 5] {
    let mut counts = [0; 5];
    for point in points {
        counts[classify_point_validated(body, point).map_or(4, slot)] += 1;
    }
    counts
}

fn probe_mismatches(
    body: &BrepEnvelope,
    frame: Frame3,
    probes: &[Probe],
    sides: [PointClassification; 3],
    label: &str,
) -> Vec<String> {
    let groups: [(&str, fn(&Probe) -> Option<Point3>); 3] = [
        ("on the surface", |probe| Some(probe.at)),
        ("outward", |probe| probe.out.map(|out| add(probe.at, out))),
        ("inward", |probe| {
            probe.inward.map(|inward| add(probe.at, inward))
        }),
    ];
    groups
        .into_iter()
        .zip(sides)
        .filter_map(|((group, offset), side)| {
            let points = probes.iter().filter_map(offset).collect::<Vec<_>>();
            let actual = tally(body, points.iter().map(|point| frame.point(*point)));
            (actual != tally_of(side, points.len())).then(|| {
                format!(
                    "{label} {group} at {:?}: expected all {side:?}, got {actual:?} [Inside, Outside, Boundary, Unknown, Error]",
                    frame.origin
                )
            })
        })
        .collect()
}

fn assert_none(mismatches: Vec<String>) {
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}

fn tally_of(side: PointClassification, count: usize) -> [usize; 5] {
    let mut counts = [0; 5];
    counts[slot(side)] = count;
    counts
}

fn slot(classification: PointClassification) -> usize {
    [
        PointClassification::Inside,
        PointClassification::Outside,
        PointClassification::Boundary,
        PointClassification::Unknown,
    ]
    .iter()
    .position(|candidate| *candidate == classification)
    .unwrap()
}

fn turn(angle: f64) -> (f64, f64) {
    angle.to_radians().sin_cos()
}

fn unit_cube(frame: Frame3, x0: f64) -> BrepEnvelope {
    let points = (0..8)
        .map(|index| {
            frame.point([
                x0 + f64::from(index & 1),
                f64::from((index >> 1) & 1),
                f64::from(index >> 2),
            ])
        })
        .collect::<Vec<_>>();
    let faces = [
        [0, 2, 3, 1],
        [4, 5, 7, 6],
        [0, 1, 5, 4],
        [2, 6, 7, 3],
        [0, 4, 6, 2],
        [1, 3, 7, 5],
    ]
    .map(|ring| ring.to_vec());
    primitives::polyhedron("cube".into(), &points, &faces, Accuracy::STANDARD).unwrap()
}

#[test]
fn every_point_of_a_face_two_rotated_cubes_share_is_boundary_of_both() {
    let probes = (1..40)
        .flat_map(|i| (1..40).map(move |j| [1.0, f64::from(i) / 40.0, f64::from(j) / 40.0]))
        .map(|at| Probe::along(at, [1.0, 0.0, 0.0]))
        .collect::<Vec<_>>();
    assert_eq!(probes.len(), 1521);
    let sides = [SURFACE[0], SURFACE[2], SURFACE[1]];
    assert_none(
        placements()
            .into_iter()
            .flat_map(|frame| {
                let mut found =
                    probe_mismatches(&unit_cube(frame, 0.0), frame, &probes, SURFACE, "cube A");
                found.extend(probe_mismatches(
                    &unit_cube(frame, 1.0),
                    frame,
                    &probes,
                    sides,
                    "cube B",
                ));
                found
            })
            .collect(),
    );
}

#[test]
fn cylinder_side_seam_rims_and_cap_centres_are_boundary() {
    let (radius, height) = (0.1, 0.7);
    let ring = |angle: f64, z: f64, rise: f64| {
        let (sin, cos) = turn(angle);
        Probe::along([radius * cos, radius * sin, z], [cos, sin, rise])
    };
    let mut probes = (0..360)
        .map(|step| ring(f64::from(step), height / 2.0, 0.0))
        .collect::<Vec<_>>();
    probes.extend((1..7).map(|step| ring(0.0, f64::from(step) / 10.0, 0.0)));
    probes.extend((0..36).map(|step| ring(f64::from(step) * 10.0, height, 1.0).without_offsets()));
    probes.extend((0..36).map(|step| ring(f64::from(step) * 10.0, 0.0, -1.0).without_offsets()));
    probes.push(Probe::along([0.0, 0.0, height], [0.0, 0.0, 1.0]));
    probes.push(Probe::along([0.0, 0.0, 0.0], [0.0, 0.0, -1.0]));
    assert_none(
        placements()
            .into_iter()
            .flat_map(|frame| {
                let body = primitives::cylinder(
                    "cylinder".into(),
                    frame,
                    radius,
                    height,
                    Accuracy::STANDARD,
                )
                .unwrap();
                probe_mismatches(&body, frame, &probes, SURFACE, "cylinder")
            })
            .collect(),
    );
}

#[test]
fn sphere_surface_points_and_both_poles_are_boundary() {
    let golden = std::f64::consts::PI * (3.0 - 5.0_f64.sqrt());
    let mut probes = (0..500)
        .map(|index| {
            let z = 1.0 - 2.0 * (f64::from(index) + 0.5) / 500.0;
            let (sin, cos) = (golden * f64::from(index)).sin_cos();
            let radial = (1.0 - z * z).sqrt();
            let at = [radial * cos, radial * sin, z];
            Probe::along(at, at)
        })
        .collect::<Vec<_>>();
    probes.push(Probe::along([0.0, 0.0, 1.0], [0.0, 0.0, 1.0]).without_inward());
    probes.push(Probe::along([0.0, 0.0, -1.0], [0.0, 0.0, -1.0]).without_out());
    assert_none(
        placements()
            .into_iter()
            .flat_map(|frame| {
                let body =
                    primitives::sphere("sphere".into(), frame, 1.0, Accuracy::STANDARD).unwrap();
                probe_mismatches(&body, frame, &probes, SURFACE, "sphere")
            })
            .collect(),
    );
}

#[test]
fn cone_side_points_and_apex_are_boundary() {
    let slant = 5.0_f64.sqrt();
    let mut probes = (0..20)
        .flat_map(|level| (0..18).map(move |step| (level, step)))
        .map(|(level, step)| {
            let z = 0.1 + 1.8 * f64::from(level) / 19.0;
            let (sin, cos) = turn(f64::from(step) * 20.0);
            let radius = 1.0 - z / 2.0;
            Probe::along(
                [radius * cos, radius * sin, z],
                [2.0 * cos / slant, 2.0 * sin / slant, 1.0 / slant],
            )
        })
        .collect::<Vec<_>>();
    probes.push(Probe::along([0.0, 0.0, 2.0], [0.0, 0.0, 1.0]).without_inward());
    assert_none(
        placements()
            .into_iter()
            .flat_map(|frame| {
                let body =
                    primitives::cone("cone".into(), frame, 1.0, 2.0, Accuracy::STANDARD).unwrap();
                probe_mismatches(&body, frame, &probes, SURFACE, "cone")
            })
            .collect(),
    );
}

#[test]
fn torus_surface_points_are_boundary() {
    let probes = (0..24)
        .flat_map(|around| (0..15).map(move |tube| (around, tube)))
        .map(|(around, tube)| {
            let (sin_u, cos_u) = turn(f64::from(around) * 15.0);
            let (sin_v, cos_v) = turn(f64::from(tube) * 24.0);
            let reach = 2.0 + 0.5 * cos_v;
            Probe::along(
                [reach * cos_u, reach * sin_u, 0.5 * sin_v],
                [cos_v * cos_u, cos_v * sin_u, sin_v],
            )
        })
        .collect::<Vec<_>>();
    assert_none(
        placements()
            .into_iter()
            .flat_map(|frame| {
                let body =
                    primitives::torus("torus".into(), frame, 2.0, 0.5, Accuracy::STANDARD).unwrap();
                probe_mismatches(&body, frame, &probes, SURFACE, "torus")
            })
            .collect(),
    );
}

#[test]
fn cuboid_corners_edges_and_face_points_are_boundary() {
    let size = [2.0, 1.0, 3.0];
    let side = |bit: bool| if bit { 1.0 } else { -1.0 };
    let mut probes = (0..8)
        .map(|index| {
            let bits: [bool; 3] = std::array::from_fn(|axis| index & (1 << axis) != 0);
            Probe::along(
                std::array::from_fn(|axis| if bits[axis] { size[axis] } else { 0.0 }),
                bits.map(side),
            )
        })
        .collect::<Vec<_>>();
    for axis in 0..3 {
        for corner in 0..4 {
            let bits = [corner & 1 != 0, corner & 2 != 0];
            let others = [(axis + 1) % 3, (axis + 2) % 3];
            for step in 1..10 {
                let mut at = [0.0; 3];
                let mut direction = [0.0; 3];
                at[axis] = size[axis] * f64::from(step) / 10.0;
                for (other, bit) in others.into_iter().zip(bits) {
                    at[other] = if bit { size[other] } else { 0.0 };
                    direction[other] = side(bit);
                }
                probes.push(Probe::along(at, direction));
            }
        }
    }
    probes.extend((1..10).flat_map(|i| {
        (1..10).map(move |j| {
            Probe::along(
                [
                    size[0] * f64::from(i) / 10.0,
                    size[1] * f64::from(j) / 10.0,
                    size[2],
                ],
                [0.0, 0.0, 1.0],
            )
        })
    }));
    assert_eq!(probes.len(), 8 + 108 + 81);
    assert_none(
        placements()
            .into_iter()
            .flat_map(|frame| {
                let body =
                    primitives::cuboid("cuboid".into(), frame, size, Accuracy::STANDARD).unwrap();
                probe_mismatches(&body, frame, &probes, SURFACE, "cuboid")
            })
            .collect(),
    );
}

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
