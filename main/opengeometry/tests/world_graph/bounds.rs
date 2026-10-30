use super::support::{box_primitive, near_within};
use opengeometry::math::Point3;
use opengeometry::primitives::{ProfileSegment, SegmentArc};
use opengeometry::world_graph::{
    CreateOptions, CreatingOperation, Primitive, Transform, WorldGraph,
};
use opengeometry_test_support::world_graph::{graph, named};

#[test]
fn local_bounds_ignore_placement_and_parents() {
    let mut world = graph();
    world.create_system_assembly(named("parent")).unwrap();
    world
        .transform(
            "parent",
            Transform::Translate {
                offset: [10.0, 0.0, 0.0],
            },
        )
        .unwrap();
    world
        .create_primitive(
            box_primitive(),
            CreateOptions {
                parent: Some("parent".into()),
                ..named("child")
            },
        )
        .unwrap();
    world
        .transform(
            "child",
            Transform::Translate {
                offset: [1.0, 2.0, 3.0],
            },
        )
        .unwrap();
    near_within(
        &world.local_bounds("child").unwrap().unwrap(),
        &[-1.0, 0.0, -1.0, 1.0, 2.0, 1.0],
        1e-7,
    );
    near_within(
        &world.bounds("child").unwrap().unwrap(),
        &[10.0, 2.0, 2.0, 12.0, 4.0, 4.0],
        1e-7,
    );
    assert_eq!(world.local_bounds("parent").unwrap(), None);
}

fn points_box(points: &[Point3]) -> [f64; 6] {
    let mut bounds = [
        f64::INFINITY,
        f64::INFINITY,
        f64::INFINITY,
        -f64::INFINITY,
        -f64::INFINITY,
        -f64::INFINITY,
    ];
    for point in points {
        for axis in 0..3 {
            bounds[axis] = bounds[axis].min(point[axis]);
            bounds[axis + 3] = bounds[axis + 3].max(point[axis]);
        }
    }
    bounds
}

fn assert_bounds(world: &WorldGraph, og_id: &str, expected: [f64; 6]) {
    let tolerance = 2.0 * world.accuracy().geometric;
    for bounds in [world.bounds(og_id), world.local_bounds(og_id)] {
        near_within(&bounds.unwrap().unwrap(), &expected, tolerance);
    }
}

fn assert_polyhedron_bounds(points: Vec<Point3>, faces: Vec<Vec<u32>>) {
    let mut world = graph();
    let expected = points_box(&points);
    world
        .create_primitive(Primitive::Polyhedron { points, faces }, named("body"))
        .unwrap();
    assert_bounds(&world, "body", expected);
}

#[test]
fn a_hip_roof_is_bounded_by_its_points() {
    let eaves = [[-0.45, -0.45], [12.45, -0.45], [12.45, 8.45], [-0.45, 8.45]];
    let ridge = [[4.0, 4.0], [8.0, 4.0]];
    let layer = |corners: &[[f64; 2]], y: f64| {
        corners
            .iter()
            .map(|&[x, z]| [x, y, z])
            .collect::<Vec<Point3>>()
    };
    let points = [
        layer(&eaves, 7.379),
        layer(&ridge, 9.214),
        layer(&eaves, 7.179),
        layer(&ridge, 9.014),
    ]
    .concat();
    let faces = [
        &[4, 5, 1, 0][..],
        &[5, 2, 1],
        &[5, 4, 3, 2],
        &[4, 0, 3],
        &[7, 11, 10, 6],
        &[8, 11, 7],
        &[9, 10, 11, 8],
        &[6, 10, 9],
        &[0, 1, 7, 6],
        &[1, 2, 8, 7],
        &[2, 3, 9, 8],
        &[3, 0, 6, 9],
    ];
    assert_polyhedron_bounds(points, faces.iter().map(|ring| ring.to_vec()).collect());
}

#[test]
fn a_tetrahedron_with_a_sloped_face_is_bounded_by_its_points() {
    let points = vec![
        [0.0, 0.0, 0.0],
        [4.0, 0.0, 0.0],
        [0.0, 0.0, 4.0],
        [0.0, 3.0, 0.0],
    ];
    let faces = vec![vec![0, 1, 2], vec![0, 3, 1], vec![0, 2, 3], vec![1, 3, 2]];
    assert_polyhedron_bounds(points, faces);
}

#[test]
fn a_box_turned_about_the_vertical_is_bounded_by_its_corners() {
    let (sin, cos) = 0.5_f64.sin_cos();
    let points = (0..8)
        .map(|index| {
            let x = if index & 1 == 0 { -1.0 } else { 1.0 };
            let y = if index & 2 == 0 { 0.0 } else { 2.0 };
            let z = if index & 4 == 0 { -0.5 } else { 0.5 };
            [x * cos + z * sin, y, z * cos - x * sin]
        })
        .collect();
    let faces = [
        [0, 4, 6, 2],
        [1, 3, 7, 5],
        [0, 1, 5, 4],
        [2, 6, 7, 3],
        [0, 2, 3, 1],
        [4, 5, 7, 6],
    ];
    assert_polyhedron_bounds(points, faces.iter().map(|ring| ring.to_vec()).collect());
}

#[test]
fn a_quarter_curved_wall_is_bounded_by_its_arcs() {
    let mut world = graph();
    let arc = |to: [f64; 2], counter_clockwise: bool| ProfileSegment {
        to,
        arc: Some(SegmentArc {
            center: [0.0, 0.0],
            counter_clockwise,
        }),
    };
    let profile = Primitive::Profile {
        start: [2.15, 0.0],
        segments: vec![
            arc([0.0, 2.15], true),
            ProfileSegment {
                to: [0.0, 1.85],
                arc: None,
            },
            arc([1.85, 0.0], false),
            ProfileSegment {
                to: [2.15, 0.0],
                arc: None,
            },
        ],
        closed: true,
    };
    world.create_primitive(profile, named("profile")).unwrap();
    world
        .create_operation(
            CreatingOperation::Extrude {
                profile: "profile".into(),
                holes: Vec::new(),
                distance: 3.0,
            },
            named("wall"),
        )
        .unwrap();
    assert_bounds(&world, "wall", [0.0, 0.0, -2.15, 2.15, 3.0, 0.0]);
}

#[test]
fn curved_primitives_keep_their_exact_boxes() {
    let cases = [
        (box_primitive(), [-1.0, 0.0, -1.0, 1.0, 2.0, 1.0]),
        (
            Primitive::Cylinder {
                radius: 0.3,
                height: 0.6,
            },
            [-0.3, 0.0, -0.3, 0.3, 0.6, 0.3],
        ),
        (
            Primitive::Sphere { radius: 1.0 },
            [-1.0, -1.0, -1.0, 1.0, 1.0, 1.0],
        ),
        (
            Primitive::Cone {
                radius_bottom: 1.0,
                radius_top: 0.5,
                height: 2.0,
            },
            [-1.0, 0.0, -1.0, 1.0, 2.0, 1.0],
        ),
        (
            Primitive::Torus {
                major_radius: 2.0,
                minor_radius: 0.5,
            },
            [-2.5, 0.0, -2.5, 2.5, 1.0, 2.5],
        ),
    ];
    for (primitive, expected) in cases {
        let mut world = graph();
        world.create_primitive(primitive, named("body")).unwrap();
        assert_bounds(&world, "body", expected);
    }
}
