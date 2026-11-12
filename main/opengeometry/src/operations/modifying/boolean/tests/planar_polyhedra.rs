use super::support::{extrusion_span, volume};
use crate::brep::{Accuracy, BrepEnvelope, FaceRole, Frame3};
use crate::operations::modifying::boolean::dispatch::boolean_brep_with_handlers;
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};
use crate::primitives;
use crate::query::{classify_point, PointClassification};

const ACCURACY: Accuracy = Accuracy {
    geometric: 1.0e-9,
    intersection: 1.0e-10,
    tessellation: 0.01,
    exchange: 1.0e-5,
};

fn cube() -> BrepEnvelope {
    primitives::cuboid(
        "cube".into(),
        Frame3 {
            origin: [-1.0, -1.0, -1.0],
            ..Frame3::IDENTITY
        },
        [2.0, 2.0, 2.0],
        ACCURACY,
    )
    .unwrap()
}

fn bar_turned_about_x() -> BrepEnvelope {
    let half = std::f64::consts::FRAC_1_SQRT_2;
    let (y, z) = ([0.0, half, half], [0.0, -half, half]);
    primitives::cuboid(
        "turned-bar".into(),
        Frame3 {
            origin: [-1.5, 0.0, -std::f64::consts::SQRT_2],
            x: [1.0, 0.0, 0.0],
            y,
            z,
        },
        [3.0, 2.0, 2.0],
        ACCURACY,
    )
    .unwrap()
}

fn bar_in_cube() -> f64 {
    16.0 * (std::f64::consts::SQRT_2 - 1.0)
}

fn operated(a: &BrepEnvelope, b: &BrepEnvelope, operation: BooleanOp) -> (BooleanResult, f64) {
    let (result, handlers) = boolean_brep_with_handlers(a, b, operation, "planar".into()).unwrap();
    let expected = if operation == BooleanOp::Subtraction {
        "subtract_planar_polyhedra"
    } else {
        "boolean_planar_polyhedra"
    };
    assert_eq!(handlers, [expected], "{operation:?}");
    assert_eq!(result.report.operation, operation);
    result.brep.validate().unwrap();
    assert!(result
        .brep
        .topology
        .shells
        .iter()
        .all(|shell| shell.is_closed));
    let measured = volume(&result.brep);
    assert!(measured > 0.0, "{operation:?} winds inward: {measured}");
    (result, measured)
}

fn assert_identities(a: &BrepEnvelope, b: &BrepEnvelope, overlap: f64) -> [BooleanResult; 3] {
    let (va, vb) = (volume(a), volume(b));
    let (union, united) = operated(a, b, BooleanOp::Union);
    let (intersection, common) = operated(a, b, BooleanOp::Intersection);
    let (subtraction, rest) = operated(a, b, BooleanOp::Subtraction);
    for (measured, expected) in [
        (common, overlap),
        (united, va + vb - overlap),
        (rest, va - overlap),
    ] {
        assert!(
            (measured - expected).abs() <= 1.0e-9 * va.max(vb),
            "{measured} != {expected}"
        );
    }
    [union, intersection, subtraction]
}

#[test]
fn turned_bar_through_a_cube_follows_the_volume_identities() {
    let (a, b) = (cube(), bar_turned_about_x());
    let [union, intersection, subtraction] = assert_identities(&a, &b, bar_in_cube());
    assert_eq!(union.brep.solids.len(), 1);
    assert_eq!(intersection.brep.solids.len(), 1);
    assert_eq!(subtraction.brep.solids.len(), 4);
    let tip = [0.0, 0.0, 1.3];
    assert_eq!(
        classify_point(&union.brep, tip).unwrap(),
        PointClassification::Inside
    );
    assert_eq!(
        classify_point(&intersection.brep, tip).unwrap(),
        PointClassification::Outside
    );
    assert_eq!(
        classify_point(&subtraction.brep, [0.0, 0.95, 0.95]).unwrap(),
        PointClassification::Inside
    );
}

#[test]
fn union_and_intersection_keep_cutter_faces_unturned_and_mark_whole_faces_preserved() {
    let (a, b) = (cube(), bar_turned_about_x());
    let [union, intersection, subtraction] = assert_identities(&a, &b, bar_in_cube());
    for result in [&union, &intersection] {
        assert!(result
            .brep
            .topology
            .faces
            .iter()
            .all(|face| { !face.provenance.reversed && face.provenance.role != FaceRole::Cut }));
    }
    let roles = |result: &BooleanResult, body: &str| {
        result
            .brep
            .topology
            .faces
            .iter()
            .filter(|face| face.provenance.sources[0].body == body)
            .map(|face| face.provenance.role)
            .collect::<Vec<_>>()
    };
    let bar_roles = roles(&union, &b.id);
    assert_eq!(
        bar_roles
            .iter()
            .filter(|role| **role == FaceRole::Preserved)
            .count(),
        2
    );
    assert!(roles(&union, &a.id)
        .iter()
        .all(|role| *role == FaceRole::Split));
    assert!(roles(&intersection, &a.id)
        .iter()
        .chain(&roles(&intersection, &b.id))
        .all(|role| *role == FaceRole::Split));
    assert!(subtraction
        .brep
        .topology
        .faces
        .iter()
        .any(|face| { face.provenance.reversed && face.provenance.role == FaceRole::Cut }));
}

#[test]
fn pitched_bar_through_a_u_profile_intersects_in_two_solids() {
    let host = extrusion_span(
        "u-profile",
        0.0,
        1.0,
        vec![
            [0.0, 0.0],
            [3.0, 0.0],
            [3.0, 3.0],
            [2.0, 3.0],
            [2.0, 1.0],
            [1.0, 1.0],
            [1.0, 3.0],
            [0.0, 3.0],
        ],
        Vec::new(),
    );
    let pitch = 5.0_f64.to_radians();
    let (x, z) = (
        [pitch.cos(), 0.0, pitch.sin()],
        [-pitch.sin(), 0.0, pitch.cos()],
    );
    let center = [1.5, 2.25, 0.5];
    let bar = primitives::cuboid(
        "pitched-bar".into(),
        Frame3 {
            origin: std::array::from_fn(|i| {
                center[i] - 2.5 * x[i] - 0.25 * z[i] - 0.25 * [0.0, 1.0, 0.0][i]
            }),
            x,
            y: [0.0, 1.0, 0.0],
            z,
        },
        [5.0, 0.5, 0.5],
        ACCURACY,
    )
    .unwrap();
    let [union, intersection, subtraction] = assert_identities(&host, &bar, 0.5 / pitch.cos());
    assert_eq!(union.brep.solids.len(), 1);
    assert_eq!(intersection.brep.solids.len(), 2);
    assert_eq!(subtraction.brep.solids.len(), 1);
}

fn corner_cube() -> BrepEnvelope {
    primitives::cuboid("cube".into(), Frame3::IDENTITY, [2.0, 2.0, 2.0], ACCURACY).unwrap()
}

fn corner_tetrahedron(id: &str, apex: f64, faces: &[Vec<u32>]) -> BrepEnvelope {
    primitives::polyhedron(
        id.into(),
        &[
            [1.0, 1.0, 2.0],
            [3.0, 1.0, 2.0],
            [1.0, 3.0, 2.0],
            [1.0, 1.0, apex],
        ],
        faces,
        ACCURACY,
    )
    .unwrap()
}

#[test]
fn a_face_shared_in_the_same_direction_is_kept_once_by_union_and_intersection() {
    let host = corner_cube();
    let pit = corner_tetrahedron(
        "pit",
        0.5,
        &[vec![0, 1, 2], vec![0, 3, 1], vec![0, 2, 3], vec![1, 3, 2]],
    );
    let [union, intersection, subtraction] = assert_identities(&host, &pit, 0.75);
    let inside_shared = [1.2, 1.2, 2.0 - 1.0e-3];
    assert_eq!(
        classify_point(&union.brep, inside_shared).unwrap(),
        PointClassification::Inside
    );
    assert_eq!(
        classify_point(&intersection.brep, inside_shared).unwrap(),
        PointClassification::Inside
    );
    assert_eq!(
        classify_point(&subtraction.brep, inside_shared).unwrap(),
        PointClassification::Outside
    );
}

#[test]
fn union_drops_a_face_shared_back_to_back_from_both_bodies() {
    let host = corner_cube();
    let cap = corner_tetrahedron(
        "cap",
        3.5,
        &[vec![0, 2, 1], vec![0, 1, 3], vec![0, 3, 2], vec![1, 2, 3]],
    );
    let (union, united) = operated(&host, &cap, BooleanOp::Union);
    assert!((united - 9.0).abs() <= 1.0e-9 * 9.0, "{united}");
    assert_eq!(union.brep.solids.len(), 1);
    assert_eq!(
        classify_point(&union.brep, [1.2, 1.2, 2.0]).unwrap(),
        PointClassification::Inside
    );
}
