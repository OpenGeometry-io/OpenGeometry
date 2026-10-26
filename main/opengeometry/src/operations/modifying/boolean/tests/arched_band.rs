use super::support::volume_at_deflection;
use crate::brep::{
    coarse_accuracy, Accuracy, BrepEnvelope, Face, FaceRole, Frame3, GeometryError, SurfaceGeometry,
};
use crate::exchange::export_step;
use crate::operations::modifying::boolean::dispatch::boolean_brep_outcome_with_handlers;
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};
use crate::primitives::{self, ProfileEdge};
use crate::query::{classify_point, PointClassification};

const OUTER: f64 = 3.0;
const INNER: f64 = 2.7;
const HEIGHT: f64 = 3.0;
const HALF_WIDTH: f64 = 0.45;
const SILL: f64 = 0.2;
const SPRING: f64 = 2.2;

fn accuracy() -> Accuracy {
    coarse_accuracy(1e-5)
}

fn ring() -> BrepEnvelope {
    primitives::annular_cylinder(
        "ring".into(),
        Frame3::IDENTITY,
        INNER,
        OUTER,
        HEIGHT,
        accuracy(),
    )
    .unwrap()
}

fn radial_frame(angle: f64, radius: f64, height: f64) -> Frame3 {
    let (sin, cos) = angle.sin_cos();
    Frame3::from_axis(
        [radius * cos, radius * sin, height],
        [cos, sin, 0.0],
        [-sin, cos, 0.0],
    )
    .unwrap()
}

fn arched_profile(sill: f64, spring: f64) -> Vec<ProfileEdge> {
    vec![
        ProfileEdge::Line {
            from: [-HALF_WIDTH, sill],
            to: [HALF_WIDTH, sill],
        },
        ProfileEdge::Line {
            from: [HALF_WIDTH, sill],
            to: [HALF_WIDTH, spring],
        },
        ProfileEdge::Arc {
            center: [0.0, spring],
            radius: HALF_WIDTH,
            start_angle: 0.0,
            sweep_angle: std::f64::consts::PI,
        },
        ProfileEdge::Line {
            from: [-HALF_WIDTH, spring],
            to: [-HALF_WIDTH, sill],
        },
    ]
}

fn arched_cutter(angle: f64, start: f64, length: f64, spring: f64) -> BrepEnvelope {
    primitives::arc_edged_extrusion(
        "arch".into(),
        radial_frame(angle, start, 0.0),
        arched_profile(SILL, spring),
        length,
        accuracy(),
    )
    .unwrap()
}

fn through_arch(angle: f64) -> BrepEnvelope {
    arched_cutter(angle, 2.55, 0.6, SPRING)
}

fn cut_volume(sill: f64) -> f64 {
    let steps = 20_000;
    let width = 2.0 * HALF_WIDTH / steps as f64;
    (0..steps)
        .map(|step| {
            let u = -HALF_WIDTH + (step as f64 + 0.5) * width;
            let depth = (OUTER * OUTER - u * u).sqrt() - (INNER * INNER - u * u).sqrt();
            let height = SPRING - sill + (HALF_WIDTH * HALF_WIDTH - u * u).sqrt();
            depth * height * width
        })
        .sum()
}

fn measured(brep: &BrepEnvelope) -> f64 {
    volume_at_deflection(brep, 0.002)
}

fn subtracted(host: &BrepEnvelope, cutter: &BrepEnvelope, id: &str) -> BooleanResult {
    let (result, handlers) =
        boolean_brep_outcome_with_handlers(host, cutter, BooleanOp::Subtraction, id.into());
    let result = result.unwrap_or_else(|error| panic!("{id}: {error}"));
    assert_eq!(
        handlers.last().map(String::as_str),
        Some("generic_two_sided_cutter_band_subtraction"),
        "{id}: {handlers:?}"
    );
    result.brep.validate().unwrap();
    export_step(&result.brep, "metre").unwrap_or_else(|error| panic!("{id} STEP: {error}"));
    result
}

fn point_at(angle: f64, radius: f64, height: f64) -> [f64; 3] {
    [radius * angle.cos(), radius * angle.sin(), height]
}

fn assert_opening_at(brep: &BrepEnvelope, angle: f64) {
    let mid = (OUTER + INNER) / 2.0;
    assert_eq!(
        classify_point(brep, point_at(angle, mid, 1.2)).unwrap(),
        PointClassification::Outside
    );
    assert_eq!(
        classify_point(brep, point_at(angle + 0.3, mid, 1.2)).unwrap(),
        PointClassification::Inside
    );
}

#[test]
fn ring_minus_one_arched_cutter_on_the_seam_is_a_valid_analytic_body() {
    let result = subtracted(&ring(), &through_arch(0.0), "arched-seam");
    let faces = &result.brep.topology.faces;
    assert_eq!(faces.len(), 8);
    let cut = faces
        .iter()
        .filter(|face| face.provenance.role == FaceRole::Cut)
        .collect::<Vec<_>>();
    assert_eq!(cut.len(), 4);
    assert!(cut
        .iter()
        .all(|face| face.key.ends_with(":through-cut") && face.provenance.reversed));
    assert!(faces
        .iter()
        .filter(|face| face.provenance.role == FaceRole::Split)
        .all(|face| face.trim.holes.len() == 1));
    let expected = measured(&ring()) - cut_volume(SILL);
    let measured = measured(&result.brep);
    assert!(
        (measured - expected).abs() <= 1e-3 * expected,
        "{measured} versus {expected}"
    );
    assert_opening_at(&result.brep, 0.0);
}

#[test]
fn ring_minus_one_arched_cutter_off_the_seam_is_valid() {
    let result = subtracted(&ring(), &through_arch(0.7), "arched-off-seam");
    assert_opening_at(&result.brep, 0.7);
    let expected = measured(&ring()) - cut_volume(SILL);
    let measured = measured(&result.brep);
    assert!(
        (measured - expected).abs() <= 1e-3 * expected,
        "{measured} versus {expected}"
    );
}

#[test]
fn arched_cutter_after_a_rectangular_cut_is_valid() {
    let profile = vec![[-0.45, 0.0], [0.45, 0.0], [0.45, 2.1], [-0.45, 2.1]];
    let rectangle = primitives::linear_extrusion(
        "rectangle".into(),
        radial_frame(0.0, 2.55, 0.0),
        profile,
        Vec::new(),
        0.6,
        accuracy(),
    )
    .unwrap();
    let (first, _) = boolean_brep_outcome_with_handlers(
        &ring(),
        &rectangle,
        BooleanOp::Subtraction,
        "rect".into(),
    );
    let first = first.unwrap().brep;
    let result = subtracted(&first, &through_arch(0.7), "arched-after-rect");
    assert_opening_at(&result.brep, 0.7);
}

fn round_cutter() -> BrepEnvelope {
    primitives::cylinder(
        "round".into(),
        radial_frame(0.7, 2.55, 1.5),
        0.3,
        0.6,
        accuracy(),
    )
    .unwrap()
}

#[test]
fn ring_minus_a_round_cutter_off_the_seam_is_valid() {
    let result = subtracted(&ring(), &round_cutter(), "round-off-seam");
    assert_eq!(
        classify_point(&result.brep, point_at(0.7, 2.85, 1.5)).unwrap(),
        PointClassification::Outside
    );
}

#[test]
fn arched_cutter_after_a_round_cut_is_valid() {
    let (first, _) = boolean_brep_outcome_with_handlers(
        &ring(),
        &round_cutter(),
        BooleanOp::Subtraction,
        "round".into(),
    );
    let first = first.unwrap().brep;
    let result = subtracted(&first, &through_arch(0.0), "arched-after-round");
    assert_opening_at(&result.brep, 0.0);
    let expected = measured(&first) - cut_volume(SILL);
    let measured = measured(&result.brep);
    assert!(
        (measured - expected).abs() <= 1e-3 * expected,
        "{measured} versus {expected}"
    );
}

#[test]
fn straight_wall_minus_one_arched_cutter_is_valid() {
    let wall = primitives::cuboid(
        "wall".into(),
        Frame3 {
            origin: [2.7, -2.0, 0.0],
            ..Frame3::IDENTITY
        },
        [0.3, 4.0, 3.0],
        accuracy(),
    )
    .unwrap();
    let result = subtracted(&wall, &through_arch(0.0), "arched-straight");
    let measured = measured(&result.brep);
    let profile_area =
        2.0 * HALF_WIDTH * (SPRING - SILL) + std::f64::consts::FRAC_PI_2 * HALF_WIDTH * HALF_WIDTH;
    let expected = 0.3 * 4.0 * 3.0 - 0.3 * profile_area;
    assert!(
        (measured - expected).abs() <= 1e-3 * expected,
        "{measured} versus {expected}"
    );
}

#[test]
fn arched_cutter_reaching_the_top_cap_is_a_coverage_gap() {
    for spring in [2.8, HEIGHT - HALF_WIDTH] {
        let (result, _) = boolean_brep_outcome_with_handlers(
            &ring(),
            &arched_cutter(0.0, 2.55, 0.6, spring),
            BooleanOp::Subtraction,
            "arched-top".into(),
        );
        assert!(
            matches!(result, Err(GeometryError::CoverageGap { .. })),
            "spring {spring}: {:?}",
            result.err()
        );
    }
}

#[test]
fn arched_cutter_ending_inside_the_wall_is_a_coverage_gap() {
    let (result, _) = boolean_brep_outcome_with_handlers(
        &ring(),
        &arched_cutter(0.0, 2.85, 0.4, SPRING),
        BooleanOp::Subtraction,
        "arched-blind".into(),
    );
    assert!(
        matches!(result, Err(GeometryError::CoverageGap { .. })),
        "{:?}",
        result.err()
    );
}

fn coverage_gap(host: &BrepEnvelope, cutter: &BrepEnvelope) -> bool {
    let (result, _) =
        boolean_brep_outcome_with_handlers(host, cutter, BooleanOp::Subtraction, "gap".into());
    matches!(result, Err(GeometryError::CoverageGap { .. }))
}

#[test]
fn arched_cutter_around_an_existing_window_is_refused() {
    let (first, _) = boolean_brep_outcome_with_handlers(
        &ring(),
        &round_cutter(),
        BooleanOp::Subtraction,
        "window".into(),
    );
    let (result, _) = boolean_brep_outcome_with_handlers(
        &first.unwrap().brep,
        &through_arch(0.7),
        BooleanOp::Subtraction,
        "swallowed".into(),
    );
    assert!(result.is_err());
}

fn door(angle: f64, sill: f64) -> BrepEnvelope {
    primitives::arc_edged_extrusion(
        "door".into(),
        radial_frame(angle, 2.55, 0.0),
        arched_profile(sill, SPRING),
        0.6,
        accuracy(),
    )
    .unwrap()
}

fn assert_notch_at(brep: &BrepEnvelope, angle: f64) {
    let mid = (OUTER + INNER) / 2.0;
    for height in [0.05, 1.2] {
        assert_eq!(
            classify_point(brep, point_at(angle, mid, height)).unwrap(),
            PointClassification::Outside,
            "height {height}"
        );
    }
    assert_eq!(
        classify_point(brep, point_at(angle + 0.3, mid, 0.05)).unwrap(),
        PointClassification::Inside
    );
}

fn assert_notch_volume(brep: &BrepEnvelope) {
    let expected = measured(&ring()) - cut_volume(0.0);
    let measured = measured(brep);
    assert!(
        (measured - expected).abs() <= 1e-3 * expected,
        "{measured} versus {expected}"
    );
}

fn split_faces(brep: &BrepEnvelope) -> Vec<&Face> {
    brep.topology
        .faces
        .iter()
        .filter(|face| face.provenance.role == FaceRole::Split)
        .collect()
}

fn assert_notch_shape(brep: &BrepEnvelope) {
    let split = split_faces(brep);
    assert_eq!(split.len(), 3);
    assert!(split.iter().all(|face| face.trim.holes.is_empty()));
}

#[test]
fn door_with_its_sill_below_the_floor_cuts_a_notch() {
    let result = subtracted(&ring(), &door(0.7, -0.1), "door-notch");
    let faces = &result.brep.topology.faces;
    assert_notch_shape(&result.brep);
    assert_eq!(
        faces
            .iter()
            .filter(|face| face.provenance.role == FaceRole::Cut)
            .count(),
        3
    );
    assert_notch_at(&result.brep, 0.7);
    assert_notch_volume(&result.brep);
}

#[test]
fn door_with_its_sill_within_four_geometric_of_the_floor_cuts_the_same_notch() {
    let reach = 4.0 * accuracy().geometric;
    for fraction in [0.0, 0.5, -0.5, 0.95, -0.95] {
        let result = subtracted(&ring(), &door(0.7, fraction * reach), "door-touching");
        assert_notch_shape(&result.brep);
        assert_notch_at(&result.brep, 0.7);
        assert_notch_volume(&result.brep);
    }
}

#[test]
fn door_with_its_sill_beyond_four_geometric_above_the_floor_cuts_a_hole() {
    let sill = 1.5 * 4.0 * accuracy().geometric;
    let result = subtracted(&ring(), &door(0.7, sill), "door-sliver");
    let split = split_faces(&result.brep);
    assert_eq!(split.len(), 2);
    assert!(split.iter().all(|face| face.trim.holes.len() == 1));
}

#[test]
fn door_with_its_sill_above_the_floor_still_cuts_a_hole() {
    let result = subtracted(&ring(), &door(0.7, SILL), "door-hole");
    assert!(result
        .brep
        .topology
        .faces
        .iter()
        .filter(|face| face.provenance.role == FaceRole::Split)
        .all(|face| face.trim.holes.len() == 1));
    let mid = (OUTER + INNER) / 2.0;
    assert_eq!(
        classify_point(&result.brep, point_at(0.7, mid, 0.05)).unwrap(),
        PointClassification::Inside
    );
}

#[test]
fn door_on_the_seam_keeps_the_off_seam_notch_volume() {
    let volumes = [0.0, 0.7].map(|angle| {
        let result = subtracted(&ring(), &door(angle, -0.1), "door-seam");
        assert_notch_at(&result.brep, angle);
        measured(&result.brep)
    });
    assert!(
        (volumes[0] - volumes[1]).abs() <= 1e-3 * volumes[1],
        "{} versus {}",
        volumes[0],
        volumes[1]
    );
}

fn straight_wall() -> BrepEnvelope {
    primitives::cuboid(
        "wall".into(),
        Frame3 {
            origin: [2.7, -2.0, 0.0],
            ..Frame3::IDENTITY
        },
        [0.3, 4.0, 3.0],
        accuracy(),
    )
    .unwrap()
}

fn wall_door(offset: f64) -> BrepEnvelope {
    primitives::arc_edged_extrusion(
        "door".into(),
        Frame3::from_axis([2.55, offset, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap(),
        arched_profile(-0.1, SPRING),
        0.6,
        accuracy(),
    )
    .unwrap()
}

fn door_area() -> f64 {
    2.0 * HALF_WIDTH * SPRING + std::f64::consts::FRAC_PI_2 * HALF_WIDTH * HALF_WIDTH
}

#[test]
fn straight_wall_minus_a_door_from_the_floor_splits_the_floor_face() {
    let result = subtracted(&straight_wall(), &door(0.0, -0.1), "straight-door");
    assert_eq!(result.brep.topology.faces.len(), 10);
    let expected = 0.3 * 4.0 * 3.0 - 0.3 * door_area();
    let measured = measured(&result.brep);
    assert!(
        (measured - expected).abs() <= 1e-3 * expected,
        "{measured} versus {expected}"
    );
    for (point, side) in [
        ([2.85, 0.0, 0.05], PointClassification::Outside),
        ([2.85, 1.0, 0.05], PointClassification::Inside),
    ] {
        assert_eq!(classify_point(&result.brep, point).unwrap(), side);
    }
}

#[test]
fn straight_wall_minus_doors_from_the_floor_on_both_sides_of_a_door_is_valid() {
    let mut wall = straight_wall();
    for (index, offset) in [0.0, 1.3, -1.3].into_iter().enumerate() {
        wall = subtracted(&wall, &wall_door(offset), &format!("doors-{index}")).brep;
        let floor = wall
            .topology
            .faces
            .iter()
            .filter(|face| {
                face.provenance.role == FaceRole::Split
                    && matches!(
                        wall.geometry.surfaces[face.surface as usize],
                        SurfaceGeometry::Plane { frame } if frame.z[2].abs() > 0.5
                    )
            })
            .count();
        assert_eq!(floor, 2, "door {index}");
    }
    let expected = 0.3 * 4.0 * 3.0 - 3.0 * 0.3 * door_area();
    let measured = measured(&wall);
    assert!(
        (measured - expected).abs() <= 1e-3 * expected,
        "{measured} versus {expected}"
    );
}

#[test]
fn door_through_the_floor_and_the_roof_is_a_coverage_gap() {
    let cutter = primitives::arc_edged_extrusion(
        "tall-door".into(),
        radial_frame(0.7, 2.55, 0.0),
        arched_profile(-0.1, 2.8),
        0.6,
        accuracy(),
    )
    .unwrap();
    assert!(coverage_gap(&ring(), &cutter));
}

#[test]
fn ring_minus_an_arched_slot_is_valid() {
    let low = 0.2 + HALF_WIDTH;
    let profile = vec![
        ProfileEdge::Arc {
            center: [0.0, low],
            radius: HALF_WIDTH,
            start_angle: std::f64::consts::PI,
            sweep_angle: std::f64::consts::PI,
        },
        ProfileEdge::Line {
            from: [HALF_WIDTH, low],
            to: [HALF_WIDTH, SPRING],
        },
        ProfileEdge::Arc {
            center: [0.0, SPRING],
            radius: HALF_WIDTH,
            start_angle: 0.0,
            sweep_angle: std::f64::consts::PI,
        },
        ProfileEdge::Line {
            from: [-HALF_WIDTH, SPRING],
            to: [-HALF_WIDTH, low],
        },
    ];
    let slot = primitives::arc_edged_extrusion(
        "slot".into(),
        radial_frame(0.7, 2.55, 0.0),
        profile,
        0.6,
        accuracy(),
    )
    .unwrap();
    let result = subtracted(&ring(), &slot, "slot");
    assert_opening_at(&result.brep, 0.7);
}
