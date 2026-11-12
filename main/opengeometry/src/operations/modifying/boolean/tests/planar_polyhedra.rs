use super::support::{extrusion_span, volume};
use crate::brep::{Accuracy, BrepEnvelope, Face, FaceRole, Frame3, GeometryError};
use crate::math::Point3;
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

fn faces_on(result: &BooleanResult, height: f64, facing: f64) -> Vec<&Face> {
    result
        .brep
        .topology
        .faces
        .iter()
        .filter(|face| {
            let origin = result.brep.geometry.surfaces[face.surface as usize]
                .frame()
                .origin;
            let normal = result.brep.face_normal_at(face.id, origin).unwrap();
            (origin[2] - height).abs() < 1.0e-9 && normal[2] * facing > 1.0 - 1.0e-9
        })
        .collect()
}

#[test]
fn faces_sharing_a_plane_and_a_facing_merge_into_one_coincident_face() {
    let host = corner_cube();
    let tent = primitives::polyhedron(
        "tent".into(),
        &[
            [1.0, 1.0, 0.0],
            [3.0, 1.0, 0.0],
            [1.0, 3.0, 0.0],
            [1.0, 1.0, 1.5],
        ],
        &[vec![0, 2, 1], vec![0, 1, 3], vec![0, 3, 2], vec![1, 2, 3]],
        ACCURACY,
    )
    .unwrap();
    let [union, intersection, _] = assert_identities(&host, &tent, 0.75);
    for result in [&union, &intersection] {
        let ground = faces_on(result, 0.0, -1.0);
        assert_eq!(ground.len(), 1);
        assert_eq!(ground[0].provenance.role, FaceRole::Coincident);
        let bodies = ground[0]
            .provenance
            .sources
            .iter()
            .map(|source| source.body.as_str())
            .collect::<Vec<_>>();
        assert_eq!(bodies, [host.id.as_str(), tent.id.as_str()]);
    }
}

#[test]
fn bodies_touching_only_along_an_edge_unite_apart_and_intersect_empty() {
    let host = corner_cube();
    let half = std::f64::consts::FRAC_1_SQRT_2;
    let ridge = primitives::cuboid(
        "ridge".into(),
        Frame3 {
            origin: [-0.5, 2.0, 2.0],
            x: [1.0, 0.0, 0.0],
            y: [0.0, half, half],
            z: [0.0, -half, half],
        },
        [3.0, 0.5, 0.5],
        ACCURACY,
    )
    .unwrap();
    for (operation, solids, handler) in [
        (BooleanOp::Union, 2, "generic_non_intersecting_boolean"),
        (BooleanOp::Intersection, 0, "boolean_planar_polyhedra"),
    ] {
        let (result, handlers) =
            boolean_brep_with_handlers(&host, &ridge, operation, "edge".into()).unwrap();
        assert_eq!(handlers, [handler]);
        assert_eq!(result.brep.solids.len(), solids);
    }
}

#[test]
fn subtracting_a_body_standing_on_a_face_leaves_the_host_whole() {
    let host = corner_cube();
    let standing = corner_tetrahedron(
        "standing",
        3.5,
        &[vec![0, 2, 1], vec![0, 1, 3], vec![0, 3, 2], vec![1, 2, 3]],
    );
    let (result, measured) = operated(&host, &standing, BooleanOp::Subtraction);
    assert!((measured - 8.0).abs() <= 1.0e-9 * 8.0, "{measured}");
    assert_eq!(result.brep.solids.len(), 1);
}

const WORLD: Accuracy = Accuracy {
    geometric: 1.0e-8,
    intersection: 1.0e-9,
    tessellation: 0.01,
    exchange: 1.0e-6,
};

fn cube_with_top(id: &str, top: &[Vec<u32>]) -> BrepEnvelope {
    let mut faces = vec![
        vec![3, 2, 1, 0],
        vec![0, 1, 5, 4],
        vec![1, 2, 6, 5],
        vec![2, 3, 7, 6],
        vec![3, 0, 4, 7],
    ];
    faces.extend_from_slice(top);
    primitives::polyhedron(
        id.into(),
        &[
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 2.0, 0.0],
            [0.0, 2.0, 0.0],
            [0.0, 0.0, 2.0],
            [2.0, 0.0, 2.0],
            [2.0, 2.0, 2.0],
            [0.0, 2.0, 2.0],
        ],
        &faces,
        WORLD,
    )
    .unwrap()
}

fn spike(id: &str, apex: Point3, base: [Point3; 3]) -> BrepEnvelope {
    primitives::polyhedron(
        id.into(),
        &[apex, base[0], base[1], base[2]],
        &[vec![0, 2, 1], vec![0, 3, 2], vec![0, 1, 3], vec![1, 2, 3]],
        WORLD,
    )
    .unwrap()
}

#[test]
fn a_dip_within_the_tolerance_band_is_contact_for_union_and_intersection() {
    let cube = cube_with_top("cube", &[vec![4, 5, 6, 7]]);
    for depth in [5.0e-9, 1.0e-8, 2.0e-8] {
        let dip = spike(
            "dip",
            [1.0, 1.0, 2.0 - depth],
            [[0.5, 0.5, 3.0], [1.5, 0.5, 3.0], [1.0, 1.5, 3.0]],
        );
        let (union, handlers) =
            boolean_brep_with_handlers(&cube, &dip, BooleanOp::Union, "dip".into()).unwrap();
        assert_eq!(handlers, ["generic_non_intersecting_boolean"], "{depth}");
        assert_eq!(union.brep.solids.len(), 2);
        assert!((volume(&union.brep) - volume(&cube) - volume(&dip)).abs() <= 1.0e-9);
        let (intersection, handlers) =
            boolean_brep_with_handlers(&cube, &dip, BooleanOp::Intersection, "dip".into()).unwrap();
        assert_eq!(handlers, ["boolean_planar_polyhedra"], "{depth}");
        assert!(intersection.brep.solids.is_empty(), "{depth}");
        assert!(intersection.brep.topology.faces.is_empty(), "{depth}");
    }
}

#[test]
fn a_dip_deeper_than_the_tolerance_intersects_in_a_solid() {
    let cube = cube_with_top("cube", &[vec![4, 5, 6, 7]]);
    let dip = spike(
        "dip",
        [1.0, 1.0, 2.0 - 1.0e-4],
        [[0.5, 0.5, 3.0], [1.5, 0.5, 3.0], [1.0, 1.5, 3.0]],
    );
    let (intersection, handlers) =
        boolean_brep_with_handlers(&cube, &dip, BooleanOp::Intersection, "dip".into()).unwrap();
    assert_eq!(handlers, ["boolean_planar_polyhedra"]);
    assert_eq!(intersection.brep.solids.len(), 1);
    assert!(volume(&intersection.brep) > 0.0);
}

#[test]
fn coplanar_faces_of_one_body_stay_apart_where_the_other_body_is_absent() {
    let cube = cube_with_top("split-top", &[vec![4, 5, 6], vec![4, 6, 7]]);
    let side = spike(
        "side",
        [1.5, 1.0, 1.0],
        [[3.0, 0.5, 0.5], [3.0, 1.5, 0.5], [3.0, 1.0, 1.5]],
    );
    let (union, _) = operated(&cube, &side, BooleanOp::Union);
    let top = faces_on(&union, 2.0, 1.0);
    assert_eq!(top.len(), 2);
    for face in top {
        assert_eq!(face.provenance.sources.len(), 1);
        assert_ne!(face.provenance.role, FaceRole::Coincident);
    }
}

fn turned_slab(overlap: f64) -> BrepEnvelope {
    let turn = 30.0_f64.to_radians();
    let corner = |u: f64, v: f64| {
        [
            1.0 + 0.5 * (u * turn.cos() - v * turn.sin()),
            1.0 + 0.5 * (u * turn.sin() + v * turn.cos()),
        ]
    };
    let ring = [
        corner(-1.0, -1.0),
        corner(1.0, -1.0),
        corner(1.0, 1.0),
        corner(-1.0, 1.0),
    ];
    let mut points = ring.map(|[x, y]| [x, y, 2.0 - overlap]).to_vec();
    points.extend(ring.map(|[x, y]| [x, y, 3.0]));
    primitives::polyhedron(
        "slab".into(),
        &points,
        &[
            vec![3, 2, 1, 0],
            vec![4, 5, 6, 7],
            vec![0, 1, 5, 4],
            vec![1, 2, 6, 5],
            vec![2, 3, 7, 6],
            vec![3, 0, 4, 7],
        ],
        WORLD,
    )
    .unwrap()
}

#[test]
fn a_slab_overlapping_within_the_tolerance_band_never_breaks_topology() {
    let cube = cube_with_top("cube", &[vec![4, 5, 6, 7]]);
    for overlap in [1.1e-8, 1.5e-8, 2.0e-8] {
        for operation in [BooleanOp::Union, BooleanOp::Intersection] {
            let result =
                boolean_brep_with_handlers(&cube, &turned_slab(overlap), operation, "slab".into());
            assert!(
                !matches!(result, Err(GeometryError::InvalidTopology(_))),
                "{overlap} {operation:?}: {:?}",
                result.err()
            );
        }
    }
    for overlap in [2.5e-8, 1.0e-6] {
        let (intersection, _) = boolean_brep_with_handlers(
            &cube,
            &turned_slab(overlap),
            BooleanOp::Intersection,
            "slab".into(),
        )
        .unwrap();
        assert!(
            (volume(&intersection.brep) - overlap).abs() <= 1.0e-12,
            "{overlap}"
        );
    }
}
