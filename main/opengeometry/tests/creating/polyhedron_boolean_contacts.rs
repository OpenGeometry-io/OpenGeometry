use super::polyhedron::{gable_roof, l_roof, polyhedron};
use super::polyhedron_boolean_checks::Pair;
use super::polyhedron_booleans::{tetrahedron, triangular_prism};
use opengeometry::math::Point3;
use opengeometry::operations::modifying::boolean::BooleanOp;
use opengeometry::world_graph::{Primitive, Transform};

fn cuboid(size: f64) -> Primitive {
    Primitive::Cuboid {
        width: size,
        height: size,
        depth: size,
    }
}

fn turned(degrees: f64, pivot: Point3) -> Transform {
    Transform::Rotate {
        axis: [0.0, 1.0, 0.0],
        degrees,
        pivot: Some(pivot),
    }
}

fn hexahedron(origin: Point3, [a, b, c]: [Point3; 3]) -> Primitive {
    let points = (0..8)
        .map(|corner| {
            let steps = [corner & 1, (corner >> 1) & 1, (corner >> 2) & 1].map(f64::from);
            std::array::from_fn(|i| origin[i] + steps[0] * a[i] + steps[1] * b[i] + steps[2] * c[i])
        })
        .collect::<Vec<Point3>>();
    let faces: [&[u32]; 6] = [
        &[0, 4, 6, 2],
        &[1, 3, 7, 5],
        &[0, 1, 5, 4],
        &[2, 6, 7, 3],
        &[0, 2, 3, 1],
        &[4, 5, 7, 6],
    ];
    polyhedron(&points, &faces)
}

#[test]
fn turned_unit_cuboid_unions_and_intersects_with_a_cuboid() {
    let pair = Pair::placed([
        (cuboid(2.0), &[]),
        (
            cuboid(1.0),
            &[
                Transform::Translate {
                    offset: [1.5, 0.5, 0.0],
                },
                turned(30.0, [1.5, 0.5, 0.0]),
            ],
        ),
    ]);
    let clipped = (2.0 - 3f64.sqrt()) / (4.0 * 3f64.sqrt());
    let union = pair.planar(BooleanOp::Union, 1);
    let intersection = pair.planar(BooleanOp::Intersection, 1);
    let difference = pair.built(BooleanOp::Subtraction, 1);
    union.assert_volume(9.0 - clipped);
    intersection.assert_volume(clipped);
    difference.assert_volume(8.0 - clipped);
    pair.assert_identities(&union, &intersection, &difference);
    pair.assert_both_contribute(&union);
    pair.assert_both_contribute(&intersection);
}

#[test]
fn bar_across_both_arms_of_an_l_roof_intersects_in_two_solids() {
    let bar = hexahedron(
        [-0.75, -1.0, 5.25],
        [[6.0, 0.0, -6.0], [0.0, 3.0, 0.0], [0.5, 0.0, 0.5]],
    );
    let pair = Pair::new(l_roof(), bar);
    let union = pair.planar(BooleanOp::Union, 1);
    let intersection = pair.planar(BooleanOp::Intersection, 2);
    let difference = pair.built(BooleanOp::Subtraction, 3);
    union.assert_volume(547.0 / 24.0);
    intersection.assert_volume(13.0 / 24.0);
    difference.assert_volume(115.0 / 24.0);
    pair.assert_identities(&union, &intersection, &difference);
    pair.assert_both_contribute(&intersection);
}

#[test]
fn l_roof_and_a_box_through_one_arm_union_into_one_solid() {
    let post = hexahedron(
        [0.5, -0.5, 0.5],
        [[1.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 3.0]],
    );
    let pair = Pair::new(l_roof(), post);
    let union = pair.planar(BooleanOp::Union, 1);
    let intersection = pair.planar(BooleanOp::Intersection, 1);
    let difference = pair.built(BooleanOp::Subtraction, 1);
    union.assert_volume(73.0 / 8.0);
    intersection.assert_volume(53.0 / 24.0);
    difference.assert_volume(25.0 / 8.0);
    pair.assert_identities(&union, &intersection, &difference);
    pair.assert_both_contribute(&union);
    pair.assert_both_contribute(&intersection);
}

#[test]
fn turned_box_inside_a_turned_box_unions_to_the_outer_and_intersects_to_the_inner() {
    let pair = Pair::placed([
        (cuboid(2.0), &[turned(30.0, [0.0, 1.0, 0.0])]),
        (
            cuboid(1.0),
            &[
                Transform::Translate {
                    offset: [0.0, 0.5, 0.0],
                },
                turned(45.0, [0.0, 1.0, 0.0]),
            ],
        ),
    ]);
    let union = pair.built(BooleanOp::Union, 1);
    let intersection = pair.built(BooleanOp::Intersection, 1);
    let difference = pair.built(BooleanOp::Subtraction, 1);
    union.assert_volume(8.0);
    intersection.assert_volume(1.0);
    difference.assert_volume(7.0);
    pair.assert_identities(&union, &intersection, &difference);
}

#[test]
fn wedge_touching_the_ridge_along_an_edge_unions_apart_and_intersects_empty() {
    let pair = Pair::new(
        gable_roof(),
        triangular_prism([[1.0, 1.0], [2.0, 0.5], [2.0, 1.5]], [1.0, 3.0]),
    );
    pair.assert_touching_kept_apart();
}

#[test]
fn tetrahedron_touching_the_ridge_end_at_a_corner_unions_apart_and_intersects_empty() {
    let tetrahedron = tetrahedron([
        [4.0, 1.0, 1.0],
        [5.0, 1.0, 2.0],
        [5.0, 1.0, 0.0],
        [5.0, 2.0, 1.0],
    ]);
    let pair = Pair::new(gable_roof(), tetrahedron);
    pair.assert_touching_kept_apart();
}

#[test]
fn box_standing_on_a_turned_cuboid_subtracts_to_the_unchanged_host() {
    let pair = Pair::placed([
        (cuboid(1.0), &[turned(30.0, [0.0, 0.5, 0.0])]),
        (
            cuboid(0.2),
            &[Transform::Translate {
                offset: [0.0, 1.0, 0.0],
            }],
        ),
    ]);
    pair.built(BooleanOp::Subtraction, 1).assert_volume(1.0);
}

fn eave_slab(plate: bool, shift: f64) -> Primitive {
    let (ridge, eave, sole) = (5.3006226146291295, 2.8498962308951454, 2.6498962308951453);
    let inner = if plate { 5.100622614629129 } else { sole };
    let points = [
        [-0.5, ridge, 3.0],
        [8.5, ridge, 3.0],
        [8.5, eave, -0.5],
        [-0.5, eave, -0.5],
        [-0.5, sole, -0.5],
        [8.5, sole, -0.5],
        [8.5, inner, 3.0],
        [-0.5, inner, 3.0],
        [-0.5, eave, 6.5],
        [8.5, eave, 6.5],
        [8.5, sole, 6.5],
        [-0.5, sole, 6.5],
    ]
    .map(|[x, y, z]| [x, y, z + shift]);
    let faces: [&[u32]; 10] = [
        &[0, 1, 2, 3],
        &[4, 5, 6, 7],
        &[1, 0, 8, 9],
        &[10, 11, 7, 6],
        &[2, 1, 6, 5],
        &[3, 2, 5, 4],
        &[0, 3, 4, 7],
        &[8, 0, 7, 11],
        &[9, 8, 11, 10],
        &[1, 9, 10, 6],
    ];
    polyhedron(&points, &faces)
}

#[test]
fn mass_roofs_touching_along_an_eave_intersect_empty() {
    Pair::new(eave_slab(false, 0.0), eave_slab(false, 7.0)).assert_empty_intersection();
}

#[test]
fn plate_roofs_touching_along_an_eave_intersect_empty() {
    Pair::new(eave_slab(true, 0.0), eave_slab(true, 7.0)).assert_empty_intersection();
}

fn gable_on_a_wall_top(length: f64, span: f64, ridge: f64) -> Primitive {
    let points = [
        [0.0, ridge, span / 2.0],
        [length, ridge, span / 2.0],
        [length, 3.2, 0.0],
        [0.0, 3.2, 0.0],
        [0.0, 3.0, 0.0],
        [length, 3.0, 0.0],
        [length, 3.0, span / 2.0],
        [0.0, 3.0, span / 2.0],
        [0.0, 3.2, span],
        [length, 3.2, span],
        [length, 3.0, span],
        [0.0, 3.0, span],
    ];
    let faces: [&[u32]; 10] = [
        &[0, 1, 2, 3],
        &[4, 5, 6, 7],
        &[1, 0, 8, 9],
        &[10, 11, 7, 6],
        &[2, 1, 6, 5],
        &[3, 2, 5, 4],
        &[0, 3, 4, 7],
        &[8, 0, 7, 11],
        &[9, 8, 11, 10],
        &[1, 9, 10, 6],
    ];
    polyhedron(&points, &faces)
}

#[test]
fn roof_wholly_inside_a_roof_on_the_same_wall_top_subtracts_to_nothing() {
    let (main_ridge, wing_ridge) = (5.3006226146291295, 3.9002075382097097);
    let main = 8.0 * (6.0 * 0.2 + 3.0 * (main_ridge - 3.2));
    let wing = 3.0 * (2.0 * 0.2 + (wing_ridge - 3.2));
    let wing_place = [
        turned(90.0, [0.0, 0.0, 0.0]),
        Transform::Translate {
            offset: [3.0, 0.0, 4.5],
        },
    ];
    let main_roof = || gable_on_a_wall_top(8.0, 6.0, main_ridge);
    let wing_roof = || gable_on_a_wall_top(3.0, 2.0, wing_ridge);
    let pair = Pair::placed([(wing_roof(), &wing_place), (main_roof(), &[])]);
    pair.assert_empty(BooleanOp::Subtraction, "subtract_planar_polyhedra");
    pair.built(BooleanOp::Union, 1).assert_volume(main);
    pair.built(BooleanOp::Intersection, 1).assert_volume(wing);
    Pair::placed([(main_roof(), &[]), (wing_roof(), &wing_place)])
        .built(BooleanOp::Subtraction, 1)
        .assert_volume(main - wing);
}

#[test]
fn box_on_the_floor_inside_an_l_roof_subtracts_to_nothing() {
    let block = || {
        hexahedron(
            [0.8, 0.0, 0.8],
            [[0.4, 0.0, 0.0], [0.0, 0.5, 0.0], [0.0, 0.0, 0.4]],
        )
    };
    let pair = Pair::new(block(), l_roof());
    pair.assert_empty(BooleanOp::Subtraction, "subtract_planar_polyhedra");
    pair.built(BooleanOp::Union, 1).assert_volume(16.0 / 3.0);
    pair.built(BooleanOp::Intersection, 1).assert_volume(0.08);
    Pair::new(l_roof(), block())
        .built(BooleanOp::Subtraction, 1)
        .assert_volume(16.0 / 3.0 - 0.08);
}

fn turned_points(primitive: Primitive, (axis, degrees): (Point3, f64)) -> Primitive {
    let Primitive::Polyhedron { points, faces } = primitive else {
        panic!("not a polyhedron");
    };
    let length = axis.iter().map(|value| value * value).sum::<f64>().sqrt();
    let axis = axis.map(|value| value / length);
    let (sin, cos) = degrees.to_radians().sin_cos();
    let points = points
        .into_iter()
        .map(|point: Point3| {
            let along = (0..3).map(|i| axis[i] * point[i]).sum::<f64>();
            let across = [
                axis[1] * point[2] - axis[2] * point[1],
                axis[2] * point[0] - axis[0] * point[2],
                axis[0] * point[1] - axis[1] * point[0],
            ];
            std::array::from_fn(|i| {
                point[i] * cos + across[i] * sin + axis[i] * along * (1.0 - cos)
            })
        })
        .collect();
    Primitive::Polyhedron { points, faces }
}

#[test]
fn box_strictly_inside_a_gable_roof_with_turned_points_subtracts_to_nothing() {
    for turn in [
        ([1.0, 2.0, 3.0], 37.0),
        (
            [-0.5252771006644055, 0.9494761331925581, 0.9087649454590425],
            207.2570567402658,
        ),
    ] {
        let block = || {
            turned_points(
                hexahedron(
                    [1.0, 0.1, 0.8],
                    [[1.0, 0.0, 0.0], [0.0, 0.2, 0.0], [0.0, 0.0, 0.4]],
                ),
                turn,
            )
        };
        let roof = || turned_points(gable_roof(), turn);
        let pair = Pair::new(block(), roof());
        pair.assert_empty(BooleanOp::Subtraction, "generic_non_intersecting_boolean");
        pair.built(BooleanOp::Union, 1).assert_volume(4.0);
        pair.built(BooleanOp::Intersection, 1).assert_volume(0.08);
        Pair::new(roof(), block())
            .built(BooleanOp::Subtraction, 1)
            .assert_volume(3.92);
    }
}

fn scaled(primitive: Primitive, factor: f64) -> Primitive {
    let Primitive::Polyhedron { points, faces } = primitive else {
        panic!("not a polyhedron");
    };
    let points = points
        .into_iter()
        .map(|point: Point3| point.map(|value| value * factor))
        .collect();
    Primitive::Polyhedron { points, faces }
}

#[test]
fn box_just_above_the_floor_inside_a_large_gable_roof_subtracts_to_nothing() {
    let block = || {
        hexahedron(
            [150.0, 2.0e-4, 80.0],
            [[100.0, 0.0, 0.0], [0.0, 30.0, 0.0], [0.0, 0.0, 40.0]],
        )
    };
    let roof = || scaled(gable_roof(), 100.0);
    let pair = Pair::new(block(), roof());
    pair.assert_empty(BooleanOp::Subtraction, "generic_non_intersecting_boolean");
    pair.built(BooleanOp::Union, 1).assert_volume(4.0e6);
    pair.built(BooleanOp::Intersection, 1).assert_volume(1.2e5);
    Pair::new(roof(), block())
        .built(BooleanOp::Subtraction, 1)
        .assert_volume(3.88e6);
}
