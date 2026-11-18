use super::polyhedron::{gable_roof, hip_roof, polyhedron, HIP_FACES};
use super::polyhedron_boolean_checks::Pair;
use opengeometry::math::Point3;
use opengeometry::operations::modifying::boolean::BooleanOp;
use opengeometry::world_graph::Primitive;

const GROUND: Point3 = [2.0, 0.0, 1.0];

const DOWN: Point3 = [0.0, -1.0, 0.0];

const SUNK_VALLEYS: [[Point3; 2]; 4] = [
    [[1.625, 0.0, 0.0], [1.875, 1.0, 1.0]],
    [[1.875, 1.0, 1.0], [1.625, 0.0, 2.0]],
    [[2.375, 0.0, 0.0], [2.125, 1.0, 1.0]],
    [[2.125, 1.0, 1.0], [2.375, 0.0, 2.0]],
];

const GROUND_VALLEYS: [[Point3; 2]; 4] = [
    [[1.5, 0.0, 0.0], [1.75, 1.0, 1.0]],
    [[1.75, 1.0, 1.0], [1.5, 0.0, 2.0]],
    [[2.5, 0.0, 0.0], [2.25, 1.0, 1.0]],
    [[2.25, 1.0, 1.0], [2.5, 0.0, 2.0]],
];

fn cross_gable(base: f64, ridge: f64) -> Primitive {
    let points = [
        [1.5, base, -1.0],
        [2.5, base, -1.0],
        [2.5, base, 3.0],
        [1.5, base, 3.0],
        [2.0, ridge, -1.0],
        [2.0, ridge, 3.0],
    ];
    let faces: [&[u32]; 5] = [
        &[0, 1, 2, 3],
        &[1, 0, 4],
        &[3, 2, 5],
        &[0, 3, 5, 4],
        &[2, 1, 4, 5],
    ];
    polyhedron(&points, &faces)
}

fn poking_hip() -> Primitive {
    let points = [
        [1.0, -0.5, 0.5],
        [3.0, -0.5, 0.5],
        [3.0, -0.5, 1.5],
        [1.0, -0.5, 1.5],
        [1.5, 2.0, 1.0],
        [2.5, 2.0, 1.0],
    ];
    polyhedron(&points, &HIP_FACES)
}

pub(super) fn triangular_prism(section: [[f64; 2]; 3], ends: [f64; 2]) -> Primitive {
    let points = ends
        .iter()
        .flat_map(|x| section.map(|[y, z]| [*x, y, z]))
        .collect::<Vec<Point3>>();
    let faces: [&[u32]; 5] = [
        &[0, 2, 1],
        &[3, 4, 5],
        &[0, 1, 4, 3],
        &[1, 2, 5, 4],
        &[2, 0, 3, 5],
    ];
    polyhedron(&points, &faces)
}

pub(super) fn tetrahedron(points: [Point3; 4]) -> Primitive {
    let faces: [&[u32]; 4] = [&[0, 2, 1], &[0, 1, 3], &[0, 3, 2], &[1, 2, 3]];
    polyhedron(&points, &faces)
}

#[test]
fn cross_gable_sunk_below_the_eaves_unions_and_intersects_along_its_valleys() {
    let pair = Pair::new(gable_roof(), cross_gable(-0.5, 1.5));
    let union = pair.planar(BooleanOp::Union, 1);
    let intersection = pair.planar(BooleanOp::Intersection, 1);
    let difference = pair.built(BooleanOp::Subtraction, 2);
    union.assert_volume(89.0 / 12.0);
    intersection.assert_volume(7.0 / 12.0);
    difference.assert_volume(41.0 / 12.0);
    pair.assert_identities(&union, &intersection, &difference);
    for built in [&union, &intersection] {
        pair.assert_both_contribute(built);
        for valley in SUNK_VALLEYS {
            pair.assert_edge_between_inputs(built, valley);
        }
    }
}

#[test]
fn crossing_gable_subtraction_keeps_subtract_planar_polyhedra() {
    let pair = Pair::new(gable_roof(), cross_gable(-0.5, 1.5));
    let difference = pair.built(BooleanOp::Subtraction, 2);
    assert_eq!(difference.handlers(), ["subtract_planar_polyhedra"]);
    difference.assert_volume(41.0 / 12.0);
}

#[test]
fn cross_gable_on_the_same_ground_unions_into_one_merged_bottom_face() {
    let pair = Pair::new(gable_roof(), cross_gable(0.0, 2.0));
    let union = pair.planar(BooleanOp::Union, 1);
    let intersection = pair.planar(BooleanOp::Intersection, 1);
    let difference = pair.built(BooleanOp::Subtraction, 2);
    union.assert_volume(43.0 / 6.0);
    intersection.assert_volume(5.0 / 6.0);
    difference.assert_volume(19.0 / 6.0);
    pair.assert_identities(&union, &intersection, &difference);
    pair.assert_one_coincident_face(&union, GROUND, DOWN);
    assert_eq!(intersection.faces_on_plane(GROUND, DOWN).len(), 1);
    for built in [&union, &intersection] {
        pair.assert_both_contribute(built);
        for valley in GROUND_VALLEYS {
            pair.assert_edge_between_inputs(built, valley);
        }
    }
}

#[test]
fn hip_poking_through_the_gable_unions_and_intersects() {
    let pair = Pair::new(gable_roof(), poking_hip());
    let union = pair.planar(BooleanOp::Union, 1);
    let intersection = pair.planar(BooleanOp::Intersection, 1);
    let difference = pair.built(BooleanOp::Subtraction, 1);
    union.assert_volume(3109.0 / 600.0);
    intersection.assert_volume(541.0 / 600.0);
    difference.assert_volume(1859.0 / 600.0);
    pair.assert_identities(&union, &intersection, &difference);
    pair.assert_both_contribute(&union);
    pair.assert_both_contribute(&intersection);
}

#[test]
fn hip_and_gable_on_shared_eaves_union_to_the_gable_and_intersect_to_the_hip() {
    let pair = Pair::new(gable_roof(), hip_roof());
    let union = pair.planar(BooleanOp::Union, 1);
    let intersection = pair.planar(BooleanOp::Intersection, 1);
    let difference = pair.built(BooleanOp::Subtraction, 2);
    union.assert_volume(4.0);
    intersection.assert_volume(10.0 / 3.0);
    difference.assert_volume(2.0 / 3.0);
    pair.assert_identities(&union, &intersection, &difference);
    pair.assert_one_coincident_face(&union, GROUND, DOWN);
    assert_eq!(intersection.faces_on_plane(GROUND, DOWN).len(), 1);
    for built in [&union, &intersection] {
        for corner in [
            [0.0, 0.0, 0.0],
            [4.0, 0.0, 0.0],
            [4.0, 0.0, 2.0],
            [0.0, 0.0, 2.0],
        ] {
            built.assert_vertex(corner);
        }
    }
}

#[test]
fn prism_with_an_edge_on_a_gable_slope_unions_and_intersects() {
    let pair = Pair::new(
        gable_roof(),
        triangular_prism([[0.5, 0.5], [1.0, 0.5], [0.5, 1.0]], [1.0, 3.0]),
    );
    let union = pair.planar(BooleanOp::Union, 1);
    let intersection = pair.planar(BooleanOp::Intersection, 1);
    let difference = pair.built(BooleanOp::Subtraction, 1);
    union.assert_volume(33.0 / 8.0);
    intersection.assert_volume(1.0 / 8.0);
    difference.assert_volume(31.0 / 8.0);
    pair.assert_identities(&union, &intersection, &difference);
    for built in [&union, &intersection] {
        pair.assert_both_contribute(built);
        pair.assert_edge_between_inputs(built, [[1.0, 0.5, 0.5], [3.0, 0.5, 0.5]]);
    }
}

#[test]
fn tetrahedron_sharing_the_ridge_end_unions_and_intersects() {
    let tetrahedron = tetrahedron([
        [4.0, 1.0, 1.0],
        [3.0, 2.0, 0.5],
        [2.0, 0.5, 1.0],
        [3.0, 0.25, 2.5],
    ]);
    let pair = Pair::new(gable_roof(), tetrahedron);
    let union = pair.planar(BooleanOp::Union, 1);
    let intersection = pair.planar(BooleanOp::Intersection, 1);
    let difference = pair.built(BooleanOp::Subtraction, 1);
    union.assert_volume(6051.0 / 1360.0);
    intersection.assert_volume(377.0 / 4080.0);
    difference.assert_volume(15943.0 / 4080.0);
    pair.assert_identities(&union, &intersection, &difference);
    for built in [&union, &intersection] {
        pair.assert_both_contribute(built);
        built.assert_vertex([4.0, 1.0, 1.0]);
    }
}
